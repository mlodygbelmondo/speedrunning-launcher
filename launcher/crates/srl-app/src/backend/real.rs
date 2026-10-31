//! The backend the app runs: `pipeline.rs` over srl-game and srl-mods, on one tokio runtime that
//! lives on a background thread. The UI thread only locks the snapshot and sends work to the
//! runtime; it never waits for I/O.
//!
//! - **On start** every mode's state comes from `Core::local_state` (a few stats, no network), so
//!   an installed mode shows Ready at once. The update check then runs in the background: it
//!   fetches the mod lists, resolves every mode's set and compares it with `mods/`, which can turn
//!   Ready into UpdateAvailable and fills in the mod counts.
//! - **Play** runs the pipeline in a task per mode: Installing (with progress), then Running once
//!   the game process exists, then Ready when it exits, or Failed with the exit code and the log
//!   path if it died before its title screen.
//! - **The account:** srl-auth's `Auth` is built once here (`account::RealAccounts`). With an
//!   active Microsoft account Play signs in after the install ("Signing in", `Stage::SigningIn`)
//!   and holds the account in use while the game runs; otherwise the offline name plays
//!   (`account::play_as`).
//! - **Cancel** drops an install where it is (downloads go to `.part` files, so nothing broken is
//!   left behind) or asks the running game to quit.
//!
//! Everything it does is logged to stderr and `logs/launcher.log`, with seconds since start.
use std::{
    collections::HashMap,
    fmt::Display,
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

use anyhow::Result;
use srl_core::{Mode, Paths, Progress, ProgressFn, Stage, Unit};
use tokio::sync::watch;

use super::{
    Backend, Event, Events, FailDetail, FailKind, Failure, ModeInfo, ModeSnapshot, ModeState,
    Settings, Snapshot,
    account::{self, Accounts, RealAccounts},
    importer::{Importer, RealImport},
    index,
    pipeline::{self, Core, Local, Startup},
    practice::{Host, PracticeMaps, RealPractice},
    stats::{RealStats, StatsService},
    tools::{RealTools, Tools},
};

/// At most this often a running install sends progress to the UI: smooth for a bar, and a few
/// thousand small asset files don't turn into a few thousand frames.
const PROGRESS_EVERY: Duration = Duration::from_millis(50);

/// How long Stop waits for the game to quit before it kills it.
pub(super) const STOP_GRACE: Duration = Duration::from_secs(10);

/// The download stages `GameInstaller::install` runs at the same time.
const PARALLEL: [Stage; 3] = [Stage::Java, Stage::Game, Stage::Assets];

pub struct RealBackend {
    inner: Arc<Inner>,
    /// The maps and the practice game (srl-practice, built once here).
    practice: RealPractice,
    /// The settings import from other launchers (srl-import).
    import: RealImport,
    rt: tokio::runtime::Handle,
    /// Dropping it ends the runtime thread. Games keep running: they aren't tied to the launcher,
    /// and neither are the tools started with them (a Ninjabrain Bot next to a game that still
    /// runs stays open).
    _stop: tokio::sync::oneshot::Sender<()>,
}

struct Inner {
    core: Arc<Core>,
    /// The Tools page's side of srl-tools (the same `Tools` as the core's).
    tools: RealTools,
    /// srl-stats: the records scan and the Ranked and Draftout APIs.
    stats: RealStats,
    /// srl-auth, built once: Settings' accounts and Play's sign-in.
    accounts: RealAccounts,
    snap: Mutex<Snapshot>,
    events: Events,
    /// Modes with a `play` under way (installing or running), and the switch that cancels it.
    runs: Mutex<HashMap<Mode, watch::Sender<bool>>>,
    settings_file: PathBuf,
    /// Serialises settings writes, so the last change is also the last one on disk.
    settings_write: Mutex<()>,
    /// One update check at a time (`check_updates`).
    check_lock: tokio::sync::Mutex<()>,
}

impl RealBackend {
    /// Reads the settings and the local state of every mode, starts the runtime and the
    /// background update check. Fails only without a data directory or a runtime. A data
    /// directory that can't be written is not an error here: the snapshot carries it
    /// (`data_error`) so the window can say so, instead of a launcher that accepts a name it
    /// can't save and fails every Play with a raw path error.
    pub fn new(events: Events) -> Result<Self> {
        let paths = Paths::from_env()?;
        let data_error = writable(paths.root())
            .err()
            .map(|e| format!("{}: {e}", paths.root().display()));
        open_log(&paths);
        if let Some(e) = &data_error {
            log(format_args!("the data folder can't be written: {e}"));
        }
        let settings_file = Settings::file(&paths);
        let settings = Settings::load(&settings_file).unwrap_or_default();
        // Before any mode's state is read: earlier versions had a directory per 1.16.1 mode.
        match srl_core::shared::migrate(&paths) {
            Ok(m) if m.is_empty() => {}
            Ok(m) => log(format_args!("one directory for the 1.16.1 modes: {m:?}")),
            Err(e) => log(format_args!(
                "moving the 1.16.1 modes into one directory (tried again next start): {e:#}"
            )),
        }
        let core = Core::new(paths)?;
        let modes = std::array::from_fn(|i| {
            let local = core.local_state(Mode::ALL[i]);
            let (state, record) = match local {
                Local::NotInstalled => (ModeState::NotInstalled, None),
                Local::Ready(r) => (ModeState::Ready, Some(r)),
                // Play puts the missing jars back first.
                Local::Damaged(r) => (ModeState::UpdateAvailable, Some(r)),
            };
            ModeSnapshot {
                state,
                info: ModeInfo {
                    mods: record.as_ref().map(|r| r.mods.len() as u32),
                    disk: record.as_ref().map(|r| r.disk),
                    java: record.as_ref().map(|r| r.java_version.clone()),
                    ..ModeInfo::default()
                },
            }
        });
        log(format_args!(
            "data in {}; modes: {}",
            core.paths().root().display(),
            Mode::ALL
                .iter()
                .zip(&modes)
                .map(|(m, s): (&Mode, &ModeSnapshot)| format!("{} {:?}", m.id(), s.state))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        let data_dir = core.paths().root().to_path_buf();

        let (handle_tx, handle_rx) = std::sync::mpsc::channel();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        thread::Builder::new()
            .name("srl-backend".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(4)
                    .thread_name("srl-io")
                    .enable_all()
                    .build();
                let rt = match rt {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = handle_tx.send(Err(e));
                        return;
                    }
                };
                let _ = handle_tx.send(Ok(rt.handle().clone()));
                rt.block_on(async {
                    let _ = stop_rx.await;
                });
                // Installs in flight stop here; their `.part` files are cleaned up next time.
                rt.shutdown_timeout(Duration::from_secs(1));
            })?;
        let rt = handle_rx.recv()??;
        let auth = srl_auth::Auth::new(core.paths(), account::auth_config());
        let accounts = RealAccounts::new(auth, core.paths(), events.clone(), rt.clone());
        let core = Arc::new(core);
        let tools = RealTools::new(core.clone(), events.clone(), rt.clone());
        log(format_args!(
            "tools: {}",
            tools
                .snapshot()
                .iter()
                .map(|t| format!("{} {:?} {:?}", t.id.id(), t.state, t.installed))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        let stats = RealStats::new(
            srl_stats::Stats::new(core.paths().clone(), core.downloader()),
            events.clone(),
            rt.clone(),
        );
        let inner = Arc::new(Inner {
            core: core.clone(),
            tools,
            stats,
            accounts,
            snap: Mutex::new(Snapshot {
                modes,
                settings,
                data_dir,
                data_error: data_error.clone(),
            }),
            events,
            runs: Mutex::new(HashMap::new()),
            settings_file,
            settings_write: Mutex::new(()),
            check_lock: tokio::sync::Mutex::new(()),
        });
        if data_error.is_none() {
            rt.spawn(check_updates(inner.clone(), std::time::Duration::ZERO));
            // A Ninjabrain Bot an earlier session started and left behind (it closed or crashed
            // while a game ran): ours again, and stopped once that game is gone.
            let core = inner.core.clone();
            rt.spawn(async move {
                for (tool, did) in core.adopt_earlier_tools().await {
                    log(format_args!(
                        "{} from an earlier session: {did:?}",
                        tool.info().name
                    ));
                }
            });
        }
        let practice = {
            let (settings, tools) = (inner.clone(), inner.clone());
            RealPractice::new(
                core,
                inner.events.clone(),
                rt.clone(),
                Host {
                    settings: Box::new(move || settings.snap.lock().unwrap().settings.clone()),
                    tools_changed: Box::new(move || tools.tools.refresh()),
                    accounts: inner.accounts.clone(),
                },
            )?
        };
        // An instance whose game runs (this session's or an earlier one's) or installs is left
        // out of an import.
        let busy = {
            let inner = inner.clone();
            // Any mode of the shared instance counts for it.
            Arc::new(move |instance: srl_core::Instance| {
                instance
                    .siblings()
                    .into_iter()
                    .any(|i| i.mode().is_some_and(|m| inner.busy(m)) || inner.core.game_running(i))
            })
        };
        let import = RealImport::new(
            inner.core.paths().clone(),
            busy,
            inner.events.clone(),
            rt.clone(),
        );
        Ok(Self {
            inner,
            practice,
            import,
            rt,
            _stop: stop_tx,
        })
    }
}

impl Backend for RealBackend {
    fn snapshot(&self) -> Snapshot {
        self.inner.snap.lock().unwrap().clone()
    }

    fn play(&self, mode: Mode) {
        let clicked = Instant::now();
        let (cancel, cancelled) = watch::channel(false);
        {
            let mut runs = self.inner.runs.lock().unwrap();
            // One Play per game directory: between one 1.16.1 mode's install and its game's
            // start, another's install would swap the shared mods/ and Atum settings under it.
            // The Play button waits too (`sibling_busy`).
            let dir = srl_core::Instance::Mode(mode).dir_id();
            if let Some(other) = runs
                .keys()
                .find(|m| srl_core::Instance::Mode(**m).dir_id() == dir)
            {
                if *other != mode {
                    log(format_args!(
                        "{}: play refused, {} runs",
                        mode.id(),
                        other.id()
                    ));
                }
                return;
            }
            runs.insert(mode, cancel);
        }
        let before = self.inner.state(mode);
        log(format_args!("{}: play", mode.id()));
        self.inner
            .set_state(mode, ModeState::Installing(stage_only(Stage::Metadata)));
        self.rt
            .spawn(play(self.inner.clone(), mode, before, cancelled, clicked));
    }

    fn cancel(&self, mode: Mode) {
        if let Some(cancel) = self.inner.runs.lock().unwrap().get(&mode) {
            log(format_args!("{}: cancel", mode.id()));
            cancel.send_replace(true);
        }
    }

    fn set_settings(&self, settings: Settings) {
        let mods_changed = {
            let mut snap = self.inner.snap.lock().unwrap();
            let changed = snap.settings.mod_rules() != settings.mod_rules();
            snap.settings = settings;
            changed
        };
        // Another set of optional mods changes what the installed modes need: check again, from
        // the mod lists in memory, so Home says which modes Play will update.
        if mods_changed && self.inner.snap.lock().unwrap().data_error.is_none() {
            self.rt
                .spawn(check_updates(self.inner.clone(), pipeline::META_FRESH));
        }
        let inner = self.inner.clone();
        self.rt.spawn_blocking(move || {
            let _one_at_a_time = inner.settings_write.lock().unwrap();
            // The newest settings, not the ones this write was queued with: writes may run in
            // any order, and the last one must leave the newest on disk.
            let latest = inner.snap.lock().unwrap().settings.clone();
            if let Err(e) = latest.save(&inner.settings_file) {
                log(format_args!(
                    "could not save settings to {}: {e}",
                    inner.settings_file.display()
                ));
            }
        });
    }

    fn open_data_folder(&self) {
        let dir = self.inner.core.paths().root().to_path_buf();
        log(format_args!(
            "opening {} in the file manager",
            dir.display()
        ));
        super::open_folder(&dir);
    }

    fn open_containing_folder(&self, file: &std::path::Path) {
        let Some(dir) = file.parent() else { return };
        log(format_args!(
            "opening {} in the file manager",
            dir.display()
        ));
        super::open_folder(dir);
    }

    fn tools(&self) -> &dyn Tools {
        &self.inner.tools
    }

    fn stats(&self) -> &dyn StatsService {
        &self.inner.stats
    }

    fn account(&self) -> &dyn Accounts {
        &self.inner.accounts
    }

    fn practice(&self) -> &dyn PracticeMaps {
        &self.practice
    }

    fn import(&self) -> &dyn Importer {
        &self.import
    }

    fn add_jar(&self, mode: Mode, path: std::path::PathBuf) {
        let inner = self.inner.clone();
        self.rt.spawn_blocking(move || {
            let answer = inner.core.add_custom_jar(&path).map_err(|e| {
                log(format_args!(
                    "{}: adding {}: {e:#}",
                    mode.id(),
                    path.display()
                ));
                format!("{e:#}")
            });
            if let Ok(jar) = &answer {
                log(format_args!(
                    "{}: added {} ({})",
                    mode.id(),
                    jar.file_name,
                    jar.name
                ));
            }
            let _ = inner.events.unbounded_send(Event::JarAdded(mode, answer));
        });
    }
}

impl Inner {
    fn state(&self, mode: Mode) -> ModeState {
        self.snap.lock().unwrap().modes[index(mode)].state.clone()
    }

    fn busy(&self, mode: Mode) -> bool {
        self.runs.lock().unwrap().contains_key(&mode)
    }

    /// Sets the state and tells the UI, if it changed. The event is sent under the lock, so the
    /// UI sees the states in the order they were set.
    fn set_state(&self, mode: Mode, state: ModeState) {
        self.set_state_if(mode, |_| true, state);
    }

    fn set_state_if(&self, mode: Mode, when: impl FnOnce(&ModeState) -> bool, state: ModeState) {
        let mut snap = self.snap.lock().unwrap();
        let slot = &mut snap.modes[index(mode)].state;
        if *slot != state && when(slot) {
            *slot = state.clone();
            let _ = self.events.unbounded_send(Event::State(mode, state));
        }
    }

    fn set_info(&self, mode: Mode, f: impl FnOnce(&mut ModeInfo)) {
        let mut snap = self.snap.lock().unwrap();
        let info = &mut snap.modes[index(mode)].info;
        let before = info.clone();
        f(info);
        if *info != before {
            let _ = self.events.unbounded_send(Event::Info(mode, info.clone()));
        }
    }

    /// Ends a `play`: the mode is free again, then it shows `state`. In that order, so a Play
    /// pressed as soon as the UI shows the new state is not ignored.
    fn finish(&self, mode: Mode, state: ModeState) {
        self.runs.lock().unwrap().remove(&mode);
        self.set_state(mode, state);
    }

    /// Progress from the pipeline, throttled and with the parallel download stages combined.
    fn progress_sink(self: &Arc<Self>, mode: Mode) -> ProgressFn {
        let inner = self.clone();
        let mux = Mutex::new(Mux::default());
        Arc::new(move |p: Progress| {
            let shown = mux.lock().unwrap().update(p, Instant::now());
            if let Some(shown) = shown {
                inner.set_state(mode, ModeState::Installing(shown));
            }
        })
    }
}

/// One Play, from the click to the game's exit.
async fn play(
    inner: Arc<Inner>,
    mode: Mode,
    before: ModeState,
    mut cancelled: watch::Receiver<bool>,
    clicked: Instant,
) {
    let id = mode.id();
    let (rules, tools) = {
        let snap = inner.snap.lock().unwrap();
        (
            snap.settings.mod_rules(),
            snap.settings.tool_selection(mode),
        )
    };
    let prepared = tokio::select! {
        r = inner.core.prepare(mode, &rules, &tools, Some(inner.progress_sink(mode))) => r,
        () = wait_cancel(&mut cancelled) => {
            log(format_args!("{id}: install cancelled"));
            inner.finish(mode, before);
            return;
        }
    };
    // An install that fails leaves the files of an installed mode where they were.
    let was_installed = match &before {
        ModeState::Ready | ModeState::UpdateAvailable | ModeState::Running => true,
        ModeState::Failed(f) => f.installed,
        ModeState::NotInstalled | ModeState::Installing(_) => false,
    };
    let prepared = match prepared {
        Ok(p) => p,
        Err(e) => {
            log(format_args!("{id}: failed: {e:#}"));
            let failure = Failure {
                kind: pipeline::install_failure_kind(&e),
                detail: FailDetail::Core(format!("{e:#}")),
                installed: was_installed,
                log: None,
            };
            inner.finish(mode, ModeState::Failed(failure));
            return;
        }
    };
    let r = &prepared.report;
    log(format_args!(
        "{id}: installed in {:.2}s: {} (Java {}), {} mods ({} added, {} updated, {} removed)",
        clicked.elapsed().as_secs_f64(),
        prepared.installed.version_id,
        prepared.installed.java.version,
        prepared.mods.mods.len(),
        r.added.len(),
        r.updated.len(),
        r.removed.len(),
    ));
    for p in &r.set_aside {
        log(format_args!(
            "{id}: moved out of mods/ (not part of the set): {}",
            p.display()
        ));
    }
    for note in &prepared.notes {
        log(format_args!("{id}: note: {note}"));
    }
    // Play may have installed or updated a tool: the Tools page shows it.
    inner.tools.refresh();
    inner.set_info(mode, |info| {
        info.mods = Some(prepared.mods.mods.len() as u32);
        info.disk = Some(prepared.record.disk);
        info.download = None;
        info.update = None;
        info.java = Some(prepared.record.java_version.clone());
    });

    // Who plays: the offline name, or the active Microsoft account signed in now (usually
    // instant; seconds when the sign-in is renewed).
    let settings = inner.snap.lock().unwrap().settings.clone();
    let signed_in = tokio::select! {
        r = account::launch_account(
            inner.accounts.auth(),
            pipeline::offline_account(&settings),
            || inner.set_state(mode, ModeState::Installing(stage_only(Stage::SigningIn))),
        ) => r,
        () = wait_cancel(&mut cancelled) => {
            log(format_args!("{id}: cancelled while signing in"));
            inner.finish(mode, ModeState::Ready);
            return;
        }
    };
    let signed_in = match signed_in {
        Ok(a) => a,
        Err(problem) => {
            log(format_args!("{id}: Microsoft sign-in failed: {problem}"));
            let relogin = problem.relogin;
            let failure = Failure {
                kind: FailKind::SignIn,
                detail: FailDetail::Account(problem),
                installed: true,
                log: None,
            };
            inner.finish(mode, ModeState::Failed(failure));
            if relogin {
                // The saved sign-in is gone: the login dialog opens; Play again after it.
                inner.accounts.login();
            }
            return;
        }
    };
    let microsoft = signed_in.account.is_microsoft();
    log(format_args!(
        "{id}: plays as {} ({})",
        signed_in.account.name(),
        if microsoft { "Microsoft" } else { "offline" }
    ));
    if let Some(w) = &signed_in.warning {
        log(format_args!(
            "{id}: couldn't renew the Microsoft sign-in ({w}); starting with the saved one, \
             valid until {:?}",
            signed_in.valid_until
        ));
    }

    inner.set_state(mode, ModeState::Installing(stage_only(Stage::Launching)));
    let mut game = match inner
        .core
        .launch(&prepared, &settings, signed_in.account)
        .await
    {
        Ok(g) => g,
        Err(e) => {
            log(format_args!("{id}: failed to start: {e:#}"));
            let failure = Failure {
                kind: FailKind::Launch,
                detail: FailDetail::Core(format!("{e:#}")),
                installed: true,
                log: None,
            };
            inner.finish(mode, ModeState::Failed(failure));
            return;
        }
    };
    game.account_in_use = signed_in.in_use;
    game.launched.valid_until = signed_in.valid_until;
    let log_path = game.process.log_path().to_path_buf();
    log(format_args!(
        "{id}: game arguments: {}",
        game.game_args.join(" ")
    ));
    log(format_args!(
        "{id}: game process started, pid {}, {:.2}s after Play; log {}",
        game.process.pid(),
        clicked.elapsed().as_secs_f64(),
        log_path.display()
    ));
    log(format_args!(
        "{id}: tools with the game: {}",
        if game.launched.tools.is_empty() {
            "none".to_string()
        } else {
            game.launched
                .tools
                .iter()
                .map(|t| t.info().name)
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));
    for note in &game.launched.tool_notes {
        log(format_args!("{id}: tools: {note}"));
    }
    inner.set_info(mode, |info| info.launched = Some(game.launched.clone()));
    inner.set_state(mode, ModeState::Running);

    // Stop (cancel while running): ask the game to quit the way its window's close button does,
    // so it saves; SIGTERM, then a kill, only if it doesn't.
    let stopper = {
        let process = game.process.clone();
        let mut cancelled = cancelled.clone();
        tokio::spawn(async move {
            wait_cancel(&mut cancelled).await;
            let asked = Instant::now();
            let report = process.stop_detailed(STOP_GRACE).await;
            log(format_args!(
                "{id}: stopped in {:.1}s: exit {:?} (asked by {:?}, escalated: {:?})",
                asked.elapsed().as_secs_f64(),
                report.exit.code,
                report.request,
                report.escalated
            ));
        })
    };
    let startup = game.startup(pipeline::STARTUP_LIMIT).await;
    let mut failure = None;
    match &startup {
        Startup::Ready(after) => log(format_args!(
            "{id}: title screen loading {:.1}s after the process started",
            after.as_secs_f64()
        )),
        Startup::Crashed(reason) => {
            log(format_args!("{id}: Fabric stopped the start: {reason}"));
            // The game sits in Fabric's error window; the launcher shows the reason instead.
            game.process.kill().await;
            failure = Some(FailDetail::Stopped(reason.clone()));
        }
        Startup::Exited(_) => {}
        Startup::TimedOut => log(format_args!(
            "{id}: no title screen in the log after {}s; counting the game as started",
            pipeline::STARTUP_LIMIT.as_secs()
        )),
    }
    // The game's exit, then the tools started for it closing.
    let exit = game.wait().await;
    let stopped = *cancelled.borrow();
    if stopped {
        // It has logged how the stop went (or is about to).
        let _ = stopper.await;
    } else {
        stopper.abort();
    }
    log(format_args!(
        "{id}: game exited (code {:?}){}",
        exit.code,
        if stopped { " after Stop" } else { "" }
    ));
    let launch_failure = |detail: FailDetail| {
        log(format_args!(
            "{id}: failed to start: {}",
            pipeline::describe(&detail, &log_path)
        ));
        ModeState::Failed(Failure {
            kind: FailKind::Launch,
            detail,
            installed: true,
            log: Some(log_path.clone()),
        })
    };
    let end = match (failure, &startup) {
        (Some(message), _) => launch_failure(message),
        // Closing the window while it loads is a normal exit, not a failure.
        (None, Startup::Exited(exit)) if !stopped && !exit.success => {
            launch_failure(pipeline::startup_failure(*exit, &log_path))
        }
        _ => ModeState::Ready,
    };
    inner.set_info(mode, |info| info.launched = None);
    inner.finish(mode, end);
}

/// The background check: mod lists no older than `max_age` (on start: fresh ones), every mode's
/// set with the optional mods of the settings, and for installed modes whether `mods/` matches
/// it. It changes only modes nobody is playing.
async fn check_updates(inner: Arc<Inner>, max_age: Duration) {
    let started = Instant::now();
    // Checks queued one after the other (the toggles flipped quickly) run one at a time and read
    // the options once they run, so the last one, with the newest options, has the last word.
    let _one_at_a_time = inner.check_lock.lock().await;
    let rules = inner.snap.lock().unwrap().settings.mod_rules();
    let meta = match inner.core.mod_meta(max_age).await {
        Ok(m) => m,
        Err(e) => {
            log(format_args!("update check: no mod lists: {e:#}"));
            return;
        }
    };
    for note in &meta.notes {
        log(format_args!("update check: {note}"));
    }
    for mode in Mode::ALL {
        if inner.busy(mode) {
            continue;
        }
        let (own, set) = match inner.core.mode_set(&meta, mode, &rules.opts) {
            Ok(own) => {
                let set = srl_mods::apply(&meta, own.clone(), &rules.choice(mode));
                (own, set)
            }
            Err(e) => {
                log(format_args!("update check: {}: {e:#}", mode.id()));
                continue;
            }
        };
        let catalogue = srl_mods::catalogue(&meta, &own, &set, mode);
        let concerns = srl_mods::concerns(&meta, &set, mode);
        let installed = matches!(
            inner.state(mode),
            ModeState::Ready | ModeState::UpdateAvailable
        );
        let plan = if installed {
            match inner.core.mods_plan(&set).await {
                Ok(p) => p,
                Err(e) => {
                    log(format_args!("update check: {}: {e:#}", mode.id()));
                    continue;
                }
            }
        } else {
            None
        };
        // A Play that started meanwhile installs the set itself.
        if inner.busy(mode) {
            continue;
        }
        let download = plan.as_ref().map(|p| inner.core.plan_download(p));
        inner.set_info(mode, |info| {
            info.mods = Some(set.mods.len() as u32);
            info.catalogue = Some(catalogue);
            info.concerns = concerns;
            if installed {
                info.update = plan.as_ref().and_then(pipeline::describe_plan);
                info.download = download.filter(|b| *b > 0);
            }
        });
        match &plan {
            Some(plan) => {
                log(format_args!(
                    "update check: {}: {} to add or update, {} to remove",
                    mode.id(),
                    plan.add.len() + plan.update.len(),
                    plan.remove.len()
                ));
                inner.set_state_if(mode, |s| *s == ModeState::Ready, ModeState::UpdateAvailable);
            }
            None if installed => {
                inner.set_state_if(mode, |s| *s == ModeState::UpdateAvailable, ModeState::Ready);
            }
            None => {}
        }
    }
    log(format_args!(
        "update check done in {:.2}s",
        started.elapsed().as_secs_f64()
    ));
}

/// Creates the data folder if needed and writes and removes a file in it.
fn writable(root: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    let probe = root.join(".srl-write-test");
    std::fs::write(&probe, b"ok")?;
    std::fs::remove_file(&probe)
}

/// Resolves once the switch is on. If the switch is dropped it never resolves.
pub(super) async fn wait_cancel(rx: &mut watch::Receiver<bool>) {
    if rx.wait_for(|c| *c).await.is_err() {
        std::future::pending::<()>().await;
    }
}

/// A stage with nothing to count yet: the UI shows only its name.
pub(super) fn stage_only(stage: Stage) -> Progress {
    Progress {
        stage,
        done: 0,
        total: 0,
        unit: Unit::Files,
    }
}

/// Turns the pipeline's progress into what the UI shows. Java, Minecraft and the assets download
/// at the same time; the UI shows one bar, so while they run it shows their bytes added up,
/// named after the one with the most bytes still to go (what the wait is mostly made of).
/// Updates are throttled to `PROGRESS_EVERY`, except when the stage changes or a stage
/// completes.
#[derive(Default)]
pub(super) struct Mux {
    parallel: Vec<Progress>,
    last: Option<(Instant, Stage)>,
}

impl Mux {
    pub(super) fn update(&mut self, p: Progress, now: Instant) -> Option<Progress> {
        let shown = if PARALLEL.contains(&p.stage) {
            match self.parallel.iter_mut().find(|q| q.stage == p.stage) {
                Some(q) => *q = p,
                None => self.parallel.push(p),
            }
            combine(&self.parallel)
        } else {
            p
        };
        let due = match self.last {
            None => true,
            Some((at, stage)) => {
                stage != shown.stage
                    || shown.done >= shown.total
                    || now.saturating_duration_since(at) >= PROGRESS_EVERY
            }
        };
        if !due {
            return None;
        }
        self.last = Some((now, shown.stage));
        Some(shown)
    }
}

fn combine(stages: &[Progress]) -> Progress {
    let running = stages
        .iter()
        .filter(|p| p.done < p.total)
        .max_by_key(|p| p.total - p.done);
    match running {
        Some(r) if stages.iter().all(|p| p.unit == Unit::Bytes) => Progress {
            stage: r.stage,
            done: stages.iter().map(|p| p.done).sum(),
            total: stages.iter().map(|p| p.total).sum(),
            unit: Unit::Bytes,
        },
        Some(r) => *r,
        None => *stages.last().expect("called after a push"),
    }
}

/// The launcher's log: stderr and `logs/launcher.log` (the previous run's is kept as
/// `launcher-previous.log`), each line stamped with seconds since start.
static LOG: OnceLock<(Instant, Mutex<Option<std::fs::File>>)> = OnceLock::new();

fn open_log(paths: &Paths) {
    let dir = paths.logs();
    let path = dir.join("launcher.log");
    let file = std::fs::create_dir_all(&dir)
        .and_then(|()| {
            if path.exists() {
                let _ = std::fs::rename(&path, dir.join("launcher-previous.log"));
            }
            std::fs::File::create(&path)
        })
        .map_err(|e| eprintln!("no launcher log at {}: {e}", path.display()))
        .ok();
    let _ = LOG.set((Instant::now(), Mutex::new(file)));
}

pub fn log(msg: impl Display) {
    let (start, file) = LOG.get_or_init(|| (Instant::now(), Mutex::new(None)));
    let line = format!("[{:8.3}] {msg}", start.elapsed().as_secs_f64());
    eprintln!("{line}");
    if let Some(f) = file.lock().unwrap().as_mut() {
        let _ = writeln!(f, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(stage: Stage, done: u64, total: u64, unit: Unit) -> Progress {
        Progress {
            stage,
            done,
            total,
            unit,
        }
    }

    #[test]
    fn parallel_downloads_show_one_bar() {
        let mut mux = Mux::default();
        let t = Instant::now();
        let later = |ms| t + Duration::from_millis(ms);
        assert_eq!(
            mux.update(p(Stage::Java, 0, 100, Unit::Bytes), t),
            Some(p(Stage::Java, 0, 100, Unit::Bytes))
        );
        // Named after the stage with the most left; a new name is shown at once.
        assert_eq!(
            mux.update(p(Stage::Assets, 0, 300, Unit::Bytes), later(10)),
            Some(p(Stage::Assets, 0, 400, Unit::Bytes))
        );
        // Within the throttle window: nothing.
        assert_eq!(
            mux.update(p(Stage::Assets, 10, 300, Unit::Bytes), later(20)),
            None
        );
        assert_eq!(
            mux.update(p(Stage::Assets, 50, 300, Unit::Bytes), later(60)),
            Some(p(Stage::Assets, 50, 400, Unit::Bytes))
        );
        assert_eq!(
            mux.update(p(Stage::Java, 100, 100, Unit::Bytes), later(120)),
            Some(p(Stage::Assets, 150, 400, Unit::Bytes))
        );
        // A stage counted in files can't be added to bytes: the biggest one shows on its own.
        assert_eq!(
            mux.update(p(Stage::Game, 1, 3, Unit::Files), later(200)),
            Some(p(Stage::Assets, 50, 300, Unit::Bytes))
        );
        // A later stage passes straight through.
        assert_eq!(
            mux.update(p(Stage::Mods, 0, 0, Unit::Files), later(201)),
            Some(p(Stage::Mods, 0, 0, Unit::Files))
        );
    }

    #[test]
    fn nothing_to_download_still_moves_on() {
        let mut mux = Mux::default();
        let t = Instant::now();
        for s in PARALLEL {
            assert!(mux.update(p(s, 0, 0, Unit::Bytes), t).is_some());
        }
    }
}
