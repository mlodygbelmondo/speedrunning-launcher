//! A stand-in backend: it simulates installs (every stage, with bytes or file counts) and a running
//! game on its own thread, so the UI can be built, clicked through and shot before `srl-game` and
//! `srl-mods` are wired in. It touches no network and no game files; the only file it writes is
//! the settings file, in the same place the real backend will keep it.
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

use srl_core::{Mode, Progress, Stage, Unit};

use super::{
    Backend, Event, Events, FailDetail, FailKind, Failure, Launched, ModeInfo, ModeSnapshot,
    ModeState, Settings, Snapshot,
    account::{AccountProblem, AccountSnapshot, Accounts, AccountsPreset, FakeAccounts},
    importer::{FakeImport, ImportPreset, Importer},
    index,
    practice::{FakePractice, PracticeMaps, PracticePreset},
    stats::StatsService,
    stats_sample::{FakeStats, StatsPreset},
    tools::{FakeTools, ToolId, Tools, ToolsPreset},
    update::{FakeUpdates, UpdatePreset, Updates},
};

/// How often a running install reports progress: 20 Hz is smooth for a bar and cheap to draw.
const TICK: Duration = Duration::from_millis(50);
const MIB: u64 = 1024 * 1024;

pub struct FakeOptions {
    /// The starting state of every mode. `None`: nothing installed.
    pub modes: Option<[ModeSnapshot; 5]>,
    /// Settings to start with instead of the settings file (the demo uses its own player).
    pub settings: Option<Settings>,
    /// Where settings are loaded from and saved to. `None` keeps them in memory.
    pub settings_file: Option<PathBuf>,
    /// The first install fails halfway through the assets (`SRL_FAKE_FAIL=1`), to try Retry.
    pub fail_first: bool,
    /// Screenshot states (`SRL_SCREEN`): this mode starts in this state and stays in it, with no
    /// simulated work behind it, until the user acts.
    pub preset: Option<(Mode, ModeState)>,
    /// The tools' starting states (`SRL_SCREEN=tools-*`).
    pub tools: ToolsPreset,
    /// What the stats answer (`SRL_SCREEN=stats-*`, `SRL_DEMO`); the preset also sets the Ranked
    /// and Draftout names it needs.
    pub stats: StatsPreset,
    /// The Microsoft accounts' starting state (`SRL_SCREEN=account*`, `login*`, `msa-*`).
    pub accounts: AccountsPreset,
    /// A running game started with a Microsoft sign-in that couldn't be renewed
    /// (`msa-warning`): the game screen's "valid until".
    pub renew_warning: bool,
    /// The practice maps' and game's starting states (`SRL_SCREEN=practice-*`).
    pub practice: PracticePreset,
    /// The prototype's install state per map id (`SRL_DEMO`), for `PracticePreset::Default`.
    pub practice_demo: Option<std::collections::HashMap<String, String>>,
    /// What Settings shows as the data folder. The fake opens nothing there.
    pub data_dir: PathBuf,
    /// The import's starting state (`SRL_SCREEN=import*`).
    pub import: ImportPreset,
    /// The launcher update's starting state (`SRL_SCREEN=update-*`).
    pub update: UpdatePreset,
}

pub struct FakeBackend {
    shared: Arc<Mutex<Snapshot>>,
    cmds: mpsc::Sender<Cmd>,
    tools: FakeTools,
    stats: FakeStats,
    accounts: FakeAccounts,
    practice: FakePractice,
    import: FakeImport,
    updates: FakeUpdates,
}

enum Cmd {
    Play(Mode),
    Cancel(Mode),
    Settings(Settings),
    AddJar(Mode, PathBuf),
}

/// The mod lists the fake's Mods page is built from: srl-mods' test snapshot of the real
/// manifests (2026-10-26), built into the binary so the fake needs no network or cache.
fn fake_meta() -> &'static srl_mods::ModMeta {
    static META: std::sync::OnceLock<srl_mods::ModMeta> = std::sync::OnceLock::new();
    META.get_or_init(|| {
        macro_rules! fixture {
            ($f:literal) => {
                include_bytes!(concat!("../../../srl-mods/tests/fixtures/meta/", $f)).as_slice()
            };
        }
        srl_mods::ModMeta::from_files(
            fixture!("mods.json"),
            fixture!("extra.json"),
            fixture!("legal-builds.csv"),
            fixture!("conditional-mods.json"),
            [
                ("mcsrranked", fixture!("modrinth-mcsrranked.json")),
                ("fsg-mod", fixture!("modrinth-fsg-mod.json")),
                ("draftout", fixture!("modrinth-draftout.json")),
            ],
        )
        .expect("the built-in mod lists parse")
    })
}

/// The Mods page of `mode` for `settings`, as the real update check fills it in.
fn fake_mods(
    mode: Mode,
    settings: &Settings,
) -> (Vec<srl_mods::Available>, Vec<srl_mods::Concern>) {
    let meta = fake_meta();
    let rules = settings.mod_rules();
    let os = srl_mods::Os::current();
    let Ok(own) = srl_mods::resolve(meta, mode, os, &rules.opts) else {
        return (Vec::new(), Vec::new());
    };
    let set = srl_mods::apply(meta, own.clone(), &rules.choice(mode));
    (
        srl_mods::catalogue(meta, &own, &set, mode),
        srl_mods::concerns(meta, &set, mode),
    )
}

/// One simulated stage: what it counts, up to what total, and how long it takes.
#[derive(Clone, Copy)]
struct Step {
    stage: Stage,
    unit: Unit,
    total: u64,
    secs: f32,
}

struct Job {
    mode: Mode,
    steps: Vec<Step>,
    step: usize,
    step_start: Instant,
    /// The state to go back to on cancel.
    before: ModeState,
    /// Fail at this fraction of this step.
    fail_at: Option<(usize, f32)>,
}

impl FakeBackend {
    pub fn new(events: Events, options: FakeOptions) -> Self {
        let mut settings = options
            .settings
            .clone()
            .or_else(|| options.settings_file.as_deref().and_then(Settings::load))
            .unwrap_or_default();
        options.stats.apply_names(&mut settings.stats);
        let blank = ModeSnapshot {
            state: ModeState::NotInstalled,
            info: ModeInfo::default(),
        };
        let mut modes = options
            .modes
            .clone()
            .unwrap_or_else(|| std::array::from_fn(|_| blank.clone()));
        if let Some((mode, state)) = &options.preset {
            modes[index(*mode)].state = state.clone();
        }
        let installed = std::array::from_fn(|i| match &modes[i].state {
            ModeState::NotInstalled | ModeState::Installing(_) => false,
            ModeState::Failed(f) => f.installed,
            _ => true,
        });
        let accounts = FakeAccounts::new(events.clone(), options.accounts);
        let account_state = accounts.shared();
        for (i, m) in modes.iter_mut().enumerate() {
            if installed[i] && m.info.java.is_none() {
                m.info.java = Some(fake_java(Mode::ALL[i]).into());
            }
            if m.info.catalogue.is_none() {
                let (catalogue, concerns) = fake_mods(Mode::ALL[i], &settings);
                m.info.catalogue = Some(catalogue);
                m.info.concerns = concerns;
            }
            if m.state == ModeState::Running {
                let mut launched =
                    fake_launched(Mode::ALL[i], &settings, &account_state.lock().unwrap());
                if options.renew_warning {
                    launched.valid_until = Some(fake_valid_until());
                }
                m.info.launched = Some(launched);
            }
        }
        let shared = Arc::new(Mutex::new(Snapshot {
            modes,
            settings,
            data_dir: options.data_dir.clone(),
            data_error: None,
        }));
        let tools = FakeTools::new(events.clone(), options.tools);
        let stats = FakeStats::new(options.stats, events.clone());
        let import = FakeImport::new(events.clone(), options.import, options.data_dir.clone());
        let updates = FakeUpdates::new(events.clone(), options.update, &options.data_dir);
        let practice = FakePractice::new(
            events.clone(),
            options.practice,
            options.practice_demo.as_ref(),
        );
        let (cmds, rx) = mpsc::channel();
        let worker = Worker {
            shared: shared.clone(),
            accounts: account_state,
            events,
            jobs: Vec::new(),
            installed,
            fail_next: options.fail_first,
            settings_file: if options.settings.is_some() {
                None
            } else {
                options.settings_file
            },
        };
        thread::Builder::new()
            .name("fake-backend".into())
            .spawn(move || worker.run(rx))
            .expect("spawn the fake backend thread");
        Self {
            shared,
            cmds,
            tools,
            stats,
            accounts,
            practice,
            import,
            updates,
        }
    }
}

/// When a sign-in the fake couldn't renew stops working: today at 18:40 UTC.
fn fake_valid_until() -> chrono::DateTime<chrono::Utc> {
    let today = chrono::Utc::now().date_naive();
    today
        .and_hms_opt(18, 40, 0)
        .expect("a valid time")
        .and_utc()
}

/// Play before the first sign-in in a build that can log in (the `msa-sign-in-first` screen id).
pub fn fake_sign_in_first() -> Failure {
    Failure {
        kind: FailKind::SignIn,
        detail: FailDetail::Account(AccountProblem::sign_in_first()),
        installed: true,
        log: None,
    }
}

/// A Play the account stopped, as the real backend reports it (the `msa-relogin` and
/// `msa-keyring` screen ids).
pub fn fake_sign_in_failure(keyring: bool) -> Failure {
    let e = if keyring {
        srl_auth::AuthError::Keystore("the Secret Service is locked (simulated)".into())
    } else {
        srl_auth::AuthError::ReloginRequired("the refresh token was revoked (simulated)".into())
    };
    Failure {
        kind: FailKind::SignIn,
        detail: FailDetail::Account(AccountProblem::of(&e)),
        installed: true,
        log: None,
    }
}

/// What a fake game was started with: the active Microsoft account or the settings' name, the
/// memory, and of the tools the mode starts with, Ninjabrain Bot (the one tool the fake has
/// installed).
fn fake_launched(mode: Mode, settings: &Settings, accounts: &AccountSnapshot) -> Launched {
    let ninjabrain = settings.starts_with(ToolId::NinjabrainBot, mode);
    let microsoft = accounts.active_name().map(str::to_string);
    Launched {
        microsoft: microsoft.is_some(),
        valid_until: None,
        player: microsoft.unwrap_or_else(|| {
            settings
                .player_name
                .clone()
                .unwrap_or_else(|| super::pipeline::DEFAULT_PLAYER.into())
        }),
        memory_mb: settings.memory_mb(mode),
        tools: if ninjabrain {
            vec![ToolId::NinjabrainBot]
        } else {
            vec![]
        },
        tool_notes: vec![],
    }
}

/// The Java version a fake install reports: the runtimes Mojang ships for each mode today.
fn fake_java(mode: Mode) -> &'static str {
    if mode == Mode::Draftout {
        "25.0.1"
    } else {
        "21.0.7"
    }
}

impl Backend for FakeBackend {
    fn snapshot(&self) -> Snapshot {
        self.shared.lock().unwrap().clone()
    }
    fn play(&self, mode: Mode) {
        let _ = self.cmds.send(Cmd::Play(mode));
    }
    fn cancel(&self, mode: Mode) {
        let _ = self.cmds.send(Cmd::Cancel(mode));
    }
    fn set_settings(&self, settings: Settings) {
        self.shared.lock().unwrap().settings = settings.clone();
        let _ = self.cmds.send(Cmd::Settings(settings));
    }
    fn open_data_folder(&self) {
        // Never a real file manager from the fake: it runs in Xvfb for screenshots, and a file
        // manager reached over D-Bus would open on the desktop instead.
        let dir = self.shared.lock().unwrap().data_dir.clone();
        eprintln!("fake: would open {} in the file manager", dir.display());
    }
    fn open_containing_folder(&self, path: &Path) {
        eprintln!(
            "fake: would open the folder of {} in the file manager",
            path.display()
        );
    }
    fn tools(&self) -> &dyn Tools {
        &self.tools
    }
    fn stats(&self) -> &dyn StatsService {
        &self.stats
    }
    fn account(&self) -> &dyn Accounts {
        &self.accounts
    }
    fn practice(&self) -> &dyn PracticeMaps {
        &self.practice
    }
    fn import(&self) -> &dyn Importer {
        &self.import
    }
    fn updates(&self) -> &dyn Updates {
        &self.updates
    }
    fn add_jar(&self, mode: Mode, path: PathBuf) {
        let _ = self.cmds.send(Cmd::AddJar(mode, path));
    }
}

struct Worker {
    shared: Arc<Mutex<Snapshot>>,
    /// The fake accounts' state: who a fake game plays as.
    accounts: Arc<Mutex<AccountSnapshot>>,
    events: Events,
    jobs: Vec<Job>,
    /// Whether the game files of each mode are in place (so Retry after a failed launch skips
    /// the downloads).
    installed: [bool; 5],
    fail_next: bool,
    settings_file: Option<PathBuf>,
}

impl Worker {
    fn run(mut self, rx: mpsc::Receiver<Cmd>) {
        loop {
            // Idle blocks without a timeout, so a fake backend with nothing to do costs nothing.
            let cmd = if self.jobs.is_empty() {
                rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
            } else {
                rx.recv_timeout(TICK)
            };
            match cmd {
                Ok(Cmd::Play(mode)) => self.play(mode),
                Ok(Cmd::Cancel(mode)) => self.cancel(mode),
                Ok(Cmd::Settings(s)) => {
                    if let Some(path) = &self.settings_file
                        && let Err(e) = s.save(path)
                    {
                        eprintln!("could not save settings to {}: {e}", path.display());
                    }
                    self.refresh_mods(&s);
                }
                Ok(Cmd::AddJar(mode, path)) => {
                    let file_name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let answer = if file_name.to_ascii_lowercase().ends_with(".jar") {
                        Ok(srl_mods::CustomJar {
                            name: file_name.trim_end_matches(".jar").to_string(),
                            sha512: format!("{:0>128}", file_name.len()),
                            size: std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
                            file_name,
                        })
                    } else {
                        Err(format!("{} is not a .jar", path.display()))
                    };
                    let _ = self.events.unbounded_send(Event::JarAdded(mode, answer));
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            self.advance(Instant::now());
        }
    }

    fn state(&self, mode: Mode) -> ModeState {
        self.shared.lock().unwrap().modes[index(mode)].state.clone()
    }

    fn refresh_mods(&self, settings: &Settings) {
        for mode in Mode::ALL {
            let (catalogue, concerns) = fake_mods(mode, settings);
            self.update_info(mode, |info| {
                info.catalogue = Some(catalogue);
                info.concerns = concerns;
            });
        }
    }

    fn set_state(&self, mode: Mode, state: ModeState) {
        let mut shared = self.shared.lock().unwrap();
        let slot = &mut shared.modes[index(mode)].state;
        if *slot != state {
            *slot = state.clone();
            let _ = self.events.unbounded_send(Event::State(mode, state));
        }
    }

    fn update_info(&self, mode: Mode, f: impl FnOnce(&mut ModeInfo)) {
        let mut shared = self.shared.lock().unwrap();
        let info = &mut shared.modes[index(mode)].info;
        let before = info.clone();
        f(info);
        if *info != before {
            let _ = self.events.unbounded_send(Event::Info(mode, info.clone()));
        }
    }

    fn play(&mut self, mode: Mode) {
        let before = self.state(mode);
        if matches!(before, ModeState::Installing(_) | ModeState::Running) {
            return;
        }
        let step = |stage, unit, total, secs| Step {
            stage,
            unit,
            total,
            secs,
        };
        let check = step(Stage::Metadata, Unit::Files, 3, 0.5);
        let launch = step(Stage::Launching, Unit::Files, 0, 1.2);
        let mut steps = if !self.installed[index(mode)] {
            vec![
                step(Stage::Metadata, Unit::Files, 14, 0.8),
                step(Stage::Java, Unit::Bytes, 48 * MIB, 1.8),
                step(Stage::Game, Unit::Bytes, 31 * MIB, 1.4),
                step(Stage::Assets, Unit::Bytes, 352 * MIB, 3.0),
                step(Stage::Mods, Unit::Files, 23, 1.0),
                launch,
            ]
        } else if before == ModeState::UpdateAvailable {
            vec![check, step(Stage::Mods, Unit::Files, 1, 0.8), launch]
        } else {
            vec![check, launch]
        };
        // A Microsoft account signs in before the game starts.
        if self.accounts.lock().unwrap().active_name().is_some() {
            let at = steps.len() - 1;
            steps.insert(at, step(Stage::SigningIn, Unit::Files, 0, 0.8));
        }
        let fail_at = std::mem::take(&mut self.fail_next).then(|| {
            let at = steps
                .iter()
                .position(|s| s.stage == Stage::Assets)
                .unwrap_or(steps.len() - 1);
            (at, 0.4)
        });
        self.jobs.push(Job {
            mode,
            steps,
            step: 0,
            step_start: Instant::now(),
            before,
            fail_at,
        });
    }

    fn cancel(&mut self, mode: Mode) {
        if let Some(i) = self.jobs.iter().position(|j| j.mode == mode) {
            let job = self.jobs.remove(i);
            self.set_state(mode, job.before);
            return;
        }
        match self.state(mode) {
            // The game "exits".
            ModeState::Running => {
                self.update_info(mode, |info| info.launched = None);
                self.set_state(mode, ModeState::Ready)
            }
            // A preset install with nothing behind it.
            ModeState::Installing(_) => {
                let back = if self.installed[index(mode)] {
                    ModeState::Ready
                } else {
                    ModeState::NotInstalled
                };
                self.set_state(mode, back);
            }
            _ => {}
        }
    }

    fn advance(&mut self, now: Instant) {
        let mut finished = Vec::new();
        for (ji, job) in self.jobs.iter_mut().enumerate() {
            // Move past finished steps (a slow tick can skip a short one).
            loop {
                let s = job.steps[job.step];
                let f = now.saturating_duration_since(job.step_start).as_secs_f32() / s.secs;
                if let Some((at, frac)) = job.fail_at
                    && at == job.step
                    && f >= frac
                {
                    finished.push((ji, Err(fake_failure(s.stage))));
                    break;
                }
                if f < 1.0 {
                    let state = ModeState::Installing(Progress {
                        stage: s.stage,
                        done: (s.total as f32 * f) as u64,
                        total: s.total,
                        unit: s.unit,
                    });
                    let mut shared = self.shared.lock().unwrap();
                    shared.modes[index(job.mode)].state = state.clone();
                    let _ = self.events.unbounded_send(Event::State(job.mode, state));
                    break;
                }
                if job.step + 1 == job.steps.len() {
                    finished.push((ji, Ok(())));
                    break;
                }
                job.step_start += Duration::from_secs_f32(s.secs);
                job.step += 1;
            }
        }
        for (ji, result) in finished.into_iter().rev() {
            let job = self.jobs.remove(ji);
            let mode = job.mode;
            match result {
                Ok(()) => {
                    self.installed[index(mode)] = true;
                    let launched = fake_launched(
                        mode,
                        &self.shared.lock().unwrap().settings,
                        &self.accounts.lock().unwrap(),
                    );
                    self.update_info(mode, |info| {
                        info.download = None;
                        info.update = None;
                        info.java = Some(fake_java(mode).into());
                        info.launched = Some(launched);
                    });
                    self.set_state(mode, ModeState::Running);
                }
                Err(failure) => self.set_state(mode, ModeState::Failed(failure)),
            }
        }
    }
}

/// The simulated failure of a stage: an offline download, or a game that closes at once.
pub fn fake_failure(stage: Stage) -> Failure {
    match stage {
        Stage::Assets => Failure {
            kind: FailKind::Offline,
            detail: FailDetail::Core(
                "connection reset while downloading game assets (simulated)".into(),
            ),
            installed: false,
            log: None,
        },
        _ => Failure {
            kind: FailKind::Launch,
            detail: FailDetail::Exited {
                code: Some(1),
                said: None,
            },
            installed: true,
            log: None,
        },
    }
}

/// A game that failed to start, as the real backend reports it, for the `crash-*` and `stopped-*`
/// screen ids: the JVM refusing the memory setting, or Draftout refusing a mod (the reason from
/// a real Draftout 1.18.0 log). The log path is where the real game's log would be.
pub fn fake_launch_failure(mode: Mode, fabric: bool, data_dir: &Path) -> Failure {
    let detail = if fabric {
        FailDetail::Stopped(
            "Draftout blocked launch because disallowed mods were found in your mods folder. \
             Disallowed mods (1): planifolia Remove the disallowed mods and restart Minecraft."
                .into(),
        )
    } else {
        FailDetail::Exited {
            code: Some(1),
            said: Some(
                "Error occurred during initialization of VM / Could not reserve enough space \
                 for 6291456KB object heap"
                    .into(),
            ),
        }
    };
    Failure {
        kind: FailKind::Launch,
        detail,
        installed: true,
        log: Some(
            data_dir
                .join("logs")
                .join(format!("{}-latest.log", mode.id())),
        ),
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    fn options() -> FakeOptions {
        FakeOptions {
            modes: None,
            settings: Some(Settings::default()),
            settings_file: None,
            fail_first: false,
            preset: None,
            tools: ToolsPreset::Default,
            stats: StatsPreset::Nothing,
            accounts: AccountsPreset::Unavailable,
            renew_warning: false,
            practice: PracticePreset::Default,
            practice_demo: None,
            data_dir: std::env::temp_dir(),
            import: ImportPreset::Found,
            update: UpdatePreset::Default,
        }
    }

    /// Collects events until `done` says stop, or panics after `secs`.
    fn wait_for(
        rx: &mut futures::channel::mpsc::UnboundedReceiver<Event>,
        secs: u64,
        mut done: impl FnMut(&Event) -> bool,
    ) -> Vec<Event> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        let mut seen = Vec::new();
        futures::executor::block_on(async {
            while Instant::now() < deadline {
                let ev = rx.next().await.expect("backend alive");
                let stop = done(&ev);
                seen.push(ev);
                if stop {
                    return;
                }
            }
            panic!("timed out; saw {} events", seen.len());
        });
        seen
    }

    #[test]
    fn play_installs_every_stage_then_runs_and_stop_makes_it_ready() {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let b = FakeBackend::new(tx, options());
        b.play(Mode::Rsg);
        let seen = wait_for(&mut rx, 20, |e| {
            *e == Event::State(Mode::Rsg, ModeState::Running)
        });
        let stages: Vec<Stage> = seen
            .iter()
            .filter_map(|e| match e {
                Event::State(_, ModeState::Installing(p)) => Some(p.stage),
                _ => None,
            })
            .fold(Vec::new(), |mut v, s| {
                if v.last() != Some(&s) {
                    v.push(s);
                }
                v
            });
        assert_eq!(
            stages,
            [
                Stage::Metadata,
                Stage::Java,
                Stage::Game,
                Stage::Assets,
                Stage::Mods,
                Stage::Launching
            ]
        );
        assert_eq!(
            b.snapshot().modes[index(Mode::Rsg)].state,
            ModeState::Running
        );
        b.cancel(Mode::Rsg);
        wait_for(&mut rx, 5, |e| {
            *e == Event::State(Mode::Rsg, ModeState::Ready)
        });
    }

    #[test]
    fn a_failed_install_reports_and_cancel_restores() {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let b = FakeBackend::new(
            tx,
            FakeOptions {
                fail_first: true,
                ..options()
            },
        );
        b.play(Mode::Ssg);
        wait_for(&mut rx, 20, |e| {
            matches!(e, Event::State(Mode::Ssg, ModeState::Failed(_)))
        });
        // Retry, then cancel mid-install: back to Failed's predecessor state, i.e. what it was.
        b.play(Mode::Ssg);
        wait_for(&mut rx, 5, |e| {
            matches!(e, Event::State(Mode::Ssg, ModeState::Installing(_)))
        });
        b.cancel(Mode::Ssg);
        wait_for(&mut rx, 5, |e| {
            matches!(e, Event::State(Mode::Ssg, ModeState::Failed(_)))
        });
    }

    #[test]
    fn settings_round_trip_through_the_file() {
        let dir = std::env::temp_dir().join(format!("srl-app-fake-{}", std::process::id()));
        let file = dir.join("settings.json");
        let (tx, _rx) = futures::channel::mpsc::unbounded();
        let b = FakeBackend::new(
            tx,
            FakeOptions {
                settings: None,
                settings_file: Some(file.clone()),
                ..options()
            },
        );
        assert_eq!(b.snapshot().settings.player_name, None);
        let s = Settings {
            player_name: Some("Violet_Runner".into()),
            language: super::super::Language::Pl,
            max_memory_mb: 4096,
            last_mode: Some(Mode::Fsg),
            draftout_extras: true,
            ..Settings::default()
        };
        b.set_settings(s.clone());
        let deadline = Instant::now() + Duration::from_secs(5);
        while Settings::load(&file).as_ref() != Some(&s) {
            assert!(Instant::now() < deadline, "settings never written");
            thread::sleep(Duration::from_millis(20));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
