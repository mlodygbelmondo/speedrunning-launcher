//! The tools extension of the facade: Ninjabrain Bot, Toolscreen (Windows) and waywall (Linux).
//! The Tools page reads `Tools::snapshot` once, then applies `Event::Tool`, and asks for installs
//! through `Tools::install`. Which modes start a tool with the game is a setting
//! (`Settings::starts_with`), so it is saved with the others and read at launch.
//!
//! The tools themselves, their pins and their rules are srl-tools'; this file only puts them in
//! the shape the page needs. Two implementations:
//! - `RealTools`, what `RealBackend` runs: the state from the files on disk (`srl_tools::Tools::
//!   status`, no network), installs and updates through `pipeline::Core::install_tool`, which
//!   checks every hash;
//! - `FakeTools`, simulated states and installs on a thread of its own, for screenshots and UI work
//!   (`SRL_FAKE=1`, the `tools-*` screen ids).
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

use srl_core::{Mode, Progress, ProgressFn, Stage, Unit};
use srl_game::rules::Os;
pub use srl_tools::ToolId;
use srl_tools::{Source, Unavailable};
use tokio::sync::watch;

use super::{Event, Events, FailKind, pipeline};

/// The tool's source without the scheme ("github.com/Ninjabrain1/Ninjabrain-Bot").
pub fn source(tool: ToolId) -> &'static str {
    let home = tool.info().homepage;
    home.strip_prefix("https://").unwrap_or(home)
}

/// Whether the tool runs on the OS this build is for.
pub fn runs_here(tool: ToolId) -> bool {
    tool.info().runs_on(srl_tools::current_os())
}

/// Why `mode` can't start `tool` (its rules ban it, or the launcher can't set it up for that
/// mode yet), on a system the tool runs on. `None`: the player may turn it on.
pub fn mode_rule(tool: ToolId, mode: Mode) -> Option<Unavailable> {
    let os = tool.info().os.first().copied().unwrap_or(Os::Linux);
    srl_tools::availability(tool, mode, os).err()
}

/// A failed install or update.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolFailure {
    /// The network could not be reached, as opposed to any other reason.
    pub offline: bool,
    /// What went wrong, in English (the error chain). The UI shows it under its own line.
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ToolState {
    /// Not made for this OS (Toolscreen off Windows, waywall off Linux).
    Unsupported,
    NotInstalled,
    /// `install` is downloading or unpacking it.
    Installing(Progress),
    Installed,
    /// Installed, and a newer version the rules allow is out (`ToolSnapshot::latest`).
    UpdateAvailable,
    /// The last install or update stopped. `install` tries again.
    Failed(ToolFailure),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolSnapshot {
    pub id: ToolId,
    pub state: ToolState,
    /// The installed version, when installed.
    pub installed: Option<String>,
    /// The version an install or update gets: the newest one the rules allow. `None` until
    /// known.
    pub latest: Option<String>,
    /// Bytes an install or update downloads, when known.
    pub download: Option<u64>,
    /// The player installs it with the system's package manager and the launcher only finds it
    /// (waywall): no Install button.
    pub system: bool,
    /// Where a tool of the system was found (`/usr/bin/waywall`).
    pub path: Option<PathBuf>,
}

/// The tools side of the facade. Like `Backend`: methods return at once, changes arrive as
/// `Event::Tool` on the backend's channel, and idle sends nothing.
pub trait Tools {
    /// Every tool, in `ToolId::ALL` order. Cheap: no I/O.
    fn snapshot(&self) -> Vec<ToolSnapshot>;
    /// Installs the tool, or updates it to `latest`: `Installing` with progress, then
    /// `Installed` or `Failed`. Does nothing for a tool that doesn't run here or is installing.
    fn install(&self, tool: ToolId);
    /// Stops an install; the tool goes back to what it was.
    fn cancel(&self, tool: ToolId);
    /// Reads the tools again from disk and PATH, off the UI thread (the Tools page opening: a
    /// waywall installed while the launcher is open shows up). Sends an event only on a change.
    fn reload(&self) {}
}

/// The states the fake starts in (`SRL_SCREEN=tools-*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolsPreset {
    /// Ninjabrain Bot installed and up to date, the screen tool of this OS not installed (waywall:
    /// not found), the other OS's tool not available.
    Default,
    /// A download installing, 45 % in: Toolscreen on Windows, else Ninjabrain Bot.
    Installing,
    /// Ninjabrain Bot 1.5.1 installed, 1.5.2 out.
    Update,
    /// That download failed offline.
    Failed,
}

/// How often an install sends its progress to the UI.
const PROGRESS_EVERY: Duration = Duration::from_millis(50);

/// `RealBackend`'s tools: srl-tools on the backend's runtime. The state comes from the files on
/// disk (no network); an install or update downloads the pinned version and checks its hashes
/// (`Core::install_tool`, one tool install at a time, shared with Play's `Stage::Tools`).
pub struct RealTools {
    core: Arc<pipeline::Core>,
    shared: Arc<Mutex<Vec<ToolSnapshot>>>,
    /// Installs under way, and the switch that cancels each.
    jobs: Arc<Mutex<HashMap<ToolId, watch::Sender<bool>>>>,
    events: Events,
    rt: tokio::runtime::Handle,
}

/// A tool's snapshot from srl-tools, local files only.
pub fn local_snapshot(tools: &srl_tools::Tools, id: ToolId) -> ToolSnapshot {
    use srl_tools::ToolStatus;
    let offer = tools.offer(id);
    let system = id.info().source == Source::System;
    let mut snap = ToolSnapshot {
        id,
        state: ToolState::NotInstalled,
        installed: None,
        latest: offer.map(|o| o.version.to_string()),
        download: offer.map(|o| o.download),
        system,
        path: None,
    };
    match tools.status(id) {
        ToolStatus::NotAvailable => {
            snap.state = ToolState::Unsupported;
            snap.latest = None;
            snap.download = None;
        }
        ToolStatus::NotInstalled => {}
        ToolStatus::Installed(t) => {
            snap.state = ToolState::Installed;
            snap.installed = t.version;
            snap.download = None;
            if !t.managed {
                snap.path = Some(t.path);
            }
        }
        ToolStatus::UpdateAvailable { installed, latest } => {
            snap.state = ToolState::UpdateAvailable;
            snap.installed = Some(installed);
            snap.latest = Some(latest);
        }
    }
    snap
}

impl RealTools {
    pub fn new(core: Arc<pipeline::Core>, events: Events, rt: tokio::runtime::Handle) -> Self {
        let snaps = ToolId::ALL
            .into_iter()
            .map(|id| local_snapshot(core.tools(), id))
            .collect();
        Self {
            core,
            shared: Arc::new(Mutex::new(snaps)),
            jobs: Arc::default(),
            events,
            rt,
        }
    }

    /// Reads every tool again from disk (after a Play installed some), except those installing.
    pub fn refresh(&self) {
        refresh(&self.core, &self.shared, &self.jobs, &self.events);
    }
}

/// [`RealTools::refresh`]. A failed install keeps saying so until the tool is there after all:
/// the files on disk alone would turn it back into "not installed" and hide why.
fn refresh(
    core: &pipeline::Core,
    shared: &Mutex<Vec<ToolSnapshot>>,
    jobs: &Mutex<HashMap<ToolId, watch::Sender<bool>>>,
    events: &Events,
) {
    let busy: Vec<ToolId> = jobs.lock().unwrap().keys().copied().collect();
    for id in ToolId::ALL {
        if busy.contains(&id) {
            continue;
        }
        let snap = local_snapshot(core.tools(), id);
        let failed = shared
            .lock()
            .unwrap()
            .iter()
            .any(|s| s.id == id && matches!(s.state, ToolState::Failed(_)));
        if failed && snap.state != ToolState::Installed {
            continue;
        }
        set_tool(shared, events, snap);
    }
}

/// Stores a tool's snapshot and tells the UI, if it changed.
fn set_tool(shared: &Mutex<Vec<ToolSnapshot>>, events: &Events, snap: ToolSnapshot) {
    let mut all = shared.lock().unwrap();
    if let Some(slot) = all.iter_mut().find(|s| s.id == snap.id)
        && *slot != snap
    {
        *slot = snap.clone();
        let _ = events.unbounded_send(Event::Tool(snap));
    }
}

impl Tools for RealTools {
    fn snapshot(&self) -> Vec<ToolSnapshot> {
        self.shared.lock().unwrap().clone()
    }

    fn reload(&self) {
        let (core, shared, jobs, events) = (
            self.core.clone(),
            self.shared.clone(),
            self.jobs.clone(),
            self.events.clone(),
        );
        // A PATH scan and a few stats: cheap, but still never on the UI thread.
        self.rt
            .spawn_blocking(move || refresh(&core, &shared, &jobs, &events));
    }

    fn install(&self, tool: ToolId) {
        let before = {
            let all = self.shared.lock().unwrap();
            all.iter().find(|s| s.id == tool).cloned()
        };
        let Some(before) = before else { return };
        if before.system
            || matches!(
                before.state,
                ToolState::Unsupported | ToolState::Installing(_) | ToolState::Installed
            )
        {
            return;
        }
        let (cancel, mut cancelled) = watch::channel(false);
        {
            let mut jobs = self.jobs.lock().unwrap();
            if jobs.contains_key(&tool) {
                return;
            }
            jobs.insert(tool, cancel);
        }
        let total = before.download.unwrap_or(0);
        set_tool(
            &self.shared,
            &self.events,
            ToolSnapshot {
                state: ToolState::Installing(Progress {
                    stage: Stage::Tools,
                    done: 0,
                    total,
                    unit: Unit::Bytes,
                }),
                ..before.clone()
            },
        );
        super::real::log(format_args!("tools: install {}", tool.id()));
        let (core, shared, jobs, events) = (
            self.core.clone(),
            self.shared.clone(),
            self.jobs.clone(),
            self.events.clone(),
        );
        self.rt.spawn(async move {
            let progress: ProgressFn = {
                let (shared, events, before) = (shared.clone(), events.clone(), before.clone());
                let last = Mutex::new(None::<Instant>);
                Arc::new(move |p: Progress| {
                    let mut last = last.lock().unwrap();
                    let due =
                        p.done >= p.total || last.is_none_or(|at| at.elapsed() >= PROGRESS_EVERY);
                    if due {
                        *last = Some(Instant::now());
                        set_tool(
                            &shared,
                            &events,
                            ToolSnapshot {
                                state: ToolState::Installing(p),
                                ..before.clone()
                            },
                        );
                    }
                })
            };
            let result = tokio::select! {
                r = core.install_tool(tool, Some(progress)) => Some(r),
                () = super::real::wait_cancel(&mut cancelled) => None,
            };
            jobs.lock().unwrap().remove(&tool);
            let snap = match result {
                None => {
                    super::real::log(format_args!("tools: {} install cancelled", tool.id()));
                    local_snapshot(core.tools(), tool)
                }
                Some(Ok(())) => {
                    let snap = local_snapshot(core.tools(), tool);
                    super::real::log(format_args!(
                        "tools: {} installed: {:?}",
                        tool.id(),
                        snap.installed
                    ));
                    snap
                }
                Some(Err(e)) => {
                    super::real::log(format_args!("tools: {} failed: {e:#}", tool.id()));
                    ToolSnapshot {
                        state: ToolState::Failed(ToolFailure {
                            offline: pipeline::install_failure_kind(&e) == FailKind::Offline,
                            detail: format!("{e:#}"),
                        }),
                        ..before
                    }
                }
            };
            set_tool(&shared, &events, snap);
        });
    }

    fn cancel(&self, tool: ToolId) {
        if let Some(c) = self.jobs.lock().unwrap().get(&tool) {
            c.send_replace(true);
        }
    }
}

const MIB: u64 = 1024 * 1024;
/// How long a simulated install takes, and how often it reports.
const FAKE_INSTALL: Duration = Duration::from_secs(3);
const TICK: Duration = Duration::from_millis(50);

/// The downloadable tool the install presets show: Toolscreen on Windows, else Ninjabrain Bot
/// (waywall comes from the system's package manager, so it never installs).
fn preset_tool() -> ToolId {
    if cfg!(windows) {
        ToolId::Toolscreen
    } else {
        ToolId::NinjabrainBot
    }
}

/// The fake's data: the versions and sizes srl-tools pins (Ninjabrain Bot 1.5.2, Toolscreen 1.4.7
/// for x64).
fn fake_tools(preset: ToolsPreset) -> Vec<ToolSnapshot> {
    ToolId::ALL
        .into_iter()
        .map(|id| {
            let (latest, size) = match id {
                ToolId::NinjabrainBot => (Some("1.5.2"), Some(7_618_461)),
                ToolId::Toolscreen => (Some("1.4.7"), Some(9_363_156)),
                ToolId::Waywall => (None, None),
            };
            let mut snap = ToolSnapshot {
                id,
                state: ToolState::NotInstalled,
                installed: None,
                latest: latest.map(str::to_string),
                download: size,
                system: id.info().source == Source::System,
                path: None,
            };
            let size = size.unwrap_or(MIB);
            if !runs_here(id) {
                snap.state = ToolState::Unsupported;
                snap.latest = None;
                snap.download = None;
            } else if id == preset_tool() && preset == ToolsPreset::Installing {
                snap.state = ToolState::Installing(Progress {
                    stage: Stage::Tools,
                    done: size * 45 / 100,
                    total: size,
                    unit: Unit::Bytes,
                });
            } else if id == preset_tool() && preset == ToolsPreset::Failed {
                snap.state = ToolState::Failed(ToolFailure {
                    offline: true,
                    detail: format!(
                        "downloading {}: error sending request for url ({}) (simulated)",
                        id.info().name,
                        id.info().homepage
                    ),
                });
            } else if id == ToolId::NinjabrainBot {
                if preset == ToolsPreset::Update {
                    snap.state = ToolState::UpdateAvailable;
                    snap.installed = Some("1.5.1".into());
                } else {
                    snap.state = ToolState::Installed;
                    snap.installed = latest.map(str::to_string);
                    snap.download = None;
                }
            }
            snap
        })
        .collect()
}

pub struct FakeTools {
    shared: Arc<Mutex<Vec<ToolSnapshot>>>,
    cmds: mpsc::Sender<Cmd>,
}

enum Cmd {
    Install(ToolId),
    Cancel(ToolId),
}

struct Job {
    tool: ToolId,
    start: Instant,
    before: ToolSnapshot,
}

impl FakeTools {
    pub fn new(events: Events, preset: ToolsPreset) -> Self {
        let shared = Arc::new(Mutex::new(fake_tools(preset)));
        let (cmds, rx) = mpsc::channel();
        let worker = shared.clone();
        thread::Builder::new()
            .name("fake-tools".into())
            .spawn(move || run_fake(worker, events, rx))
            .expect("spawn the fake tools thread");
        Self { shared, cmds }
    }
}

impl Tools for FakeTools {
    fn snapshot(&self) -> Vec<ToolSnapshot> {
        self.shared.lock().unwrap().clone()
    }
    fn install(&self, tool: ToolId) {
        let _ = self.cmds.send(Cmd::Install(tool));
    }
    fn cancel(&self, tool: ToolId) {
        let _ = self.cmds.send(Cmd::Cancel(tool));
    }
}

fn run_fake(shared: Arc<Mutex<Vec<ToolSnapshot>>>, events: Events, rx: mpsc::Receiver<Cmd>) {
    let mut jobs: Vec<Job> = Vec::new();
    let set = |snap: ToolSnapshot| set_tool(&shared, &events, snap);
    let get = |tool: ToolId| {
        shared
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.id == tool)
            .cloned()
            .unwrap()
    };
    loop {
        // Idle blocks without a timeout: nothing to simulate costs nothing.
        let cmd = if jobs.is_empty() {
            rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
        } else {
            rx.recv_timeout(TICK)
        };
        match cmd {
            Ok(Cmd::Install(tool)) => {
                let before = get(tool);
                let busy = jobs.iter().any(|j| j.tool == tool);
                let preset_install = matches!(before.state, ToolState::Installing(_));
                if !busy
                    && !preset_install
                    && !before.system
                    && !matches!(before.state, ToolState::Unsupported | ToolState::Installed)
                {
                    jobs.push(Job {
                        tool,
                        start: Instant::now(),
                        before,
                    });
                }
            }
            Ok(Cmd::Cancel(tool)) => {
                if let Some(i) = jobs.iter().position(|j| j.tool == tool) {
                    set(jobs.remove(i).before);
                } else if let ToolState::Installing(_) = get(tool).state {
                    // A preset install with nothing behind it.
                    set(ToolSnapshot {
                        state: ToolState::NotInstalled,
                        ..get(tool)
                    });
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let now = Instant::now();
        jobs.retain(|job| {
            let f =
                now.saturating_duration_since(job.start).as_secs_f32() / FAKE_INSTALL.as_secs_f32();
            let total = job.before.download.unwrap_or(MIB);
            if f >= 1.0 {
                set(ToolSnapshot {
                    state: ToolState::Installed,
                    installed: job.before.latest.clone(),
                    download: None,
                    ..job.before.clone()
                });
                return false;
            }
            set(ToolSnapshot {
                state: ToolState::Installing(Progress {
                    stage: Stage::Tools,
                    done: (total as f32 * f) as u64,
                    total,
                    unit: Unit::Bytes,
                }),
                ..job.before.clone()
            });
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    #[test]
    fn presets_cover_every_state() {
        let state = |p, id: ToolId| {
            fake_tools(p)
                .into_iter()
                .find(|s| s.id == id)
                .unwrap()
                .state
        };
        let other = if cfg!(windows) {
            ToolId::Waywall
        } else {
            ToolId::Toolscreen
        };
        assert_eq!(state(ToolsPreset::Default, other), ToolState::Unsupported);
        assert_eq!(
            state(ToolsPreset::Default, ToolId::NinjabrainBot),
            ToolState::Installed
        );
        assert_eq!(
            state(ToolsPreset::Update, ToolId::NinjabrainBot),
            ToolState::UpdateAvailable
        );
        assert!(matches!(
            state(ToolsPreset::Installing, preset_tool()),
            ToolState::Installing(_)
        ));
        assert!(matches!(
            state(ToolsPreset::Failed, preset_tool()),
            ToolState::Failed(ToolFailure { offline: true, .. })
        ));
        // waywall is the system's: never an install of its own.
        let waywall = fake_tools(ToolsPreset::Default)
            .into_iter()
            .find(|s| s.id == ToolId::Waywall)
            .unwrap();
        assert!(waywall.system && waywall.latest.is_none() && waywall.download.is_none());
    }

    #[test]
    fn a_fake_install_reports_progress_then_installs() {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let tools = FakeTools::new(tx, ToolsPreset::Failed);
        tools.install(preset_tool());
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut progressed = false;
        futures::executor::block_on(async {
            loop {
                assert!(Instant::now() < deadline, "no install");
                let Some(Event::Tool(s)) = rx.next().await else {
                    continue;
                };
                match s.state {
                    ToolState::Installing(p) => progressed |= p.done > 0,
                    ToolState::Installed => {
                        assert_eq!(s.installed.as_deref(), s.latest.as_deref());
                        break;
                    }
                    other => panic!("unexpected {other:?}"),
                }
            }
        });
        assert!(progressed);
    }

    /// The real tools from an empty data folder: what the page shows comes from srl-tools'
    /// pins, with nothing invented.
    #[test]
    fn real_snapshots_come_from_the_pins() {
        let root = std::env::temp_dir().join(format!("srl-app-tools-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = srl_core::Paths::new(&root);
        let tools = srl_tools::Tools::new(paths, srl_core::Downloader::new().unwrap());
        let nb = local_snapshot(&tools, ToolId::NinjabrainBot);
        assert_eq!(nb.state, ToolState::NotInstalled);
        assert_eq!(nb.latest.as_deref(), Some(srl_tools::ninjabrain::VERSION));
        assert_eq!(nb.download, Some(srl_tools::ninjabrain::SIZE));
        assert!(!nb.system);
        let ts = local_snapshot(&tools, ToolId::Toolscreen);
        assert_eq!(ts.state == ToolState::Unsupported, !cfg!(windows));
        let ww = local_snapshot(&tools, ToolId::Waywall);
        assert!(ww.system && ww.latest.is_none());
        assert_eq!(
            ww.state == ToolState::Unsupported,
            !cfg!(target_os = "linux")
        );
        assert_eq!(
            mode_rule(ToolId::NinjabrainBot, Mode::Draftout),
            Some(Unavailable::Banned)
        );
        assert_eq!(
            mode_rule(ToolId::Toolscreen, Mode::Draftout),
            Some(Unavailable::NotSupported)
        );
        assert_eq!(mode_rule(ToolId::Waywall, Mode::Draftout), None);
        assert_eq!(source(ToolId::Waywall), "github.com/tesselslate/waywall");
        let _ = std::fs::remove_dir_all(&root);
    }
}
