//! The practice extension of the facade: the maps (srl-practice) and the practice instance's game
//! (`pipeline.rs` with `Instance::Practice`). The Practice page reads `PracticeMaps::snapshot`
//! once, then applies `Event::Practice`, and asks for work through the trait's methods. Like
//! `Backend`: methods return at once, the work runs on the backend's runtime, idle sends nothing.
//!
//! What the practice instance runs, decided here once:
//! - **Game and mods**: Minecraft, Fabric and Java from srl-practice's [`PRACTICE`], the mods from
//!   its explicit id list through srl-mods' legal checks (`resolve_ids`), into
//!   `instances/practice` (never a mode's folder).
//! - **Tools, memory, account**: RSG's ([`TOOLS_LIKE`]). Practice is 1.16.1 like RSG, so RSG's
//!   tool rules fit (Ninjabrain Bot allowed, waywall's GLFW check for 1.16.1), and the player's
//!   RSG choices (which tools start, its memory) are the ones they already made for that game.
//!   The account is the modes' too: `account::launch_account` (offline unless a Microsoft
//!   account is active, `account::play_as`), signed in the same way before the game starts.
//! - **Two games**: the practice game runs next to a mode's game the way two modes do (each
//!   instance one game at a time, `InstanceGuard`); it never opens the game screen, the Practice
//!   row says where to find the map instead.
//!
//! Two implementations: `RealPractice` for `RealBackend`, `FakePractice` (simulated, for
//! screenshots and UI work, `SRL_FAKE=1` and the `practice-*` screen ids).
use std::{
    collections::HashMap,
    future::Future,
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::Result;
use srl_core::{Instance, Mode, Stage, Unit};
use srl_mods::{ModMeta, ModSet, Os};
use srl_practice::{
    CarryOver, InstalledMap, MapEntry, MapStatus, PRACTICE, PracticeError, Progress, ResetPlan,
    Step, UpdateChoice, UpdatePlan, WorldState, WorldView,
};
use tokio::sync::watch;

use super::{
    Event, Events, FailDetail, FailKind, Failure, Launched, ModeState, Settings,
    account::{self, Accounts, RealAccounts},
    pipeline::{self, Core, Local, Startup},
    real::{self, log, wait_cancel},
};

/// The mode whose tool rules and settings (tools that start, memory) the practice game uses.
pub const TOOLS_LIKE: Mode = Mode::Rsg;

/// What srl-game installs for the practice instance.
pub fn target() -> Result<srl_game::Target> {
    srl_game::Target::practice(
        PRACTICE.minecraft_version,
        PRACTICE.java_major,
        PRACTICE.min_fabric_loader,
    )
}

/// The practice instance's mods on this OS: [`PRACTICE`]'s ids (plus the accessibility ones when
/// the player turned them on), with srl-mods' legal checks.
pub fn mod_set(meta: &ModMeta, accessibility: bool) -> Result<ModSet> {
    let os = Os::current();
    let ids = PRACTICE.mod_ids(accessibility, os == Os::MacOs);
    srl_mods::resolve_ids(
        meta,
        Instance::Practice,
        PRACTICE.minecraft_version,
        &ids,
        os,
    )
}

/// What a map's row is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobKind {
    Install,
    Update,
    Reset,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapJob {
    pub kind: JobKind,
    /// `None` until srl-practice reports (waiting for another map's change, reading files).
    pub progress: Option<Progress>,
}

impl MapJob {
    /// Cancel stops a download only: once extracting starts, srl-practice finishes on its own in
    /// a few seconds and a cancel would change nothing.
    pub fn cancellable(&self) -> bool {
        self.progress.is_some_and(|p| p.step == Step::Downloading)
    }
}

/// What the row says about a failed change: one line per kind (srl-practice's "For the UI").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapFailKind {
    /// No connection, a timeout, a server error.
    Network,
    /// The author moved or removed the file.
    SourceGone,
    /// The download isn't the version that was checked; nothing was installed.
    HashMismatch,
    /// Not a world the launcher can install (a broken, unsafe or empty zip).
    NotAWorld,
    /// The game has the world open.
    WorldInUse,
    /// Anything else (the disk); the detail says what.
    Other,
}

pub fn fail_kind(e: &PracticeError) -> MapFailKind {
    match e {
        PracticeError::Network(_) => MapFailKind::Network,
        PracticeError::SourceGone { .. } => MapFailKind::SourceGone,
        PracticeError::HashMismatch { .. } => MapFailKind::HashMismatch,
        PracticeError::BadArchive(_) | PracticeError::UnsafeArchive(_) | PracticeError::NoWorld => {
            MapFailKind::NotAWorld
        }
        PracticeError::WorldInUse { .. } => MapFailKind::WorldInUse,
        _ => MapFailKind::Other,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapFailure {
    pub kind: MapFailKind,
    /// What was being done.
    pub job: JobKind,
    /// The error in English, for `Other` and the launcher log.
    pub detail: String,
}

impl MapFailure {
    fn of(job: JobKind, e: &PracticeError) -> Self {
        Self {
            kind: fail_kind(e),
            job,
            detail: e.to_string(),
        }
    }
}

/// One catalogue entry as the page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct MapView {
    pub id: String,
    pub status: MapStatus,
    pub job: Option<MapJob>,
    /// The last change failed, and why. Cleared by the next one.
    pub failure: Option<MapFailure>,
}

/// The practice instance's game. `state` is the modes' `ModeState`: NotInstalled until the
/// instance was installed once, Installing (with progress) while Play installs and starts it,
/// Running, then Ready or Failed.
#[derive(Clone, Debug, PartialEq)]
pub struct PracticeGame {
    pub state: ModeState,
    /// The map whose Play started it: its row says where to find it in the game.
    pub map: Option<String>,
    /// What the running game was started with.
    pub launched: Option<Launched>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PracticeSnapshot {
    /// The statuses have been read from disk once. Until then the rows show no action.
    pub loaded: bool,
    /// Catalogue order.
    pub maps: Vec<MapView>,
    pub game: PracticeGame,
}

impl PracticeSnapshot {
    pub fn map(&self, id: &str) -> Option<&MapView> {
        self.maps.iter().find(|m| m.id == id)
    }

    /// The practice game is starting or running: Reset, Update and Remove wait (srl-practice's
    /// rule for callers: the game may have the world open).
    pub fn game_busy(&self) -> bool {
        matches!(
            self.game.state,
            ModeState::Installing(_) | ModeState::Running
        )
    }
}

/// A confirmation the page shows before a change that can lose progress.
#[derive(Clone, Debug, PartialEq)]
pub enum Question {
    Reset {
        id: String,
        plan: ResetPlan,
    },
    /// `UpdatePlan::Ask`: a world has been played.
    Update {
        id: String,
        from: String,
        to: String,
        played: Vec<WorldView>,
        carry_over: Vec<CarryOver>,
    },
    Remove {
        id: String,
        /// The worlds Remove deletes.
        worlds: Vec<WorldView>,
        /// Worlds handed over to the player earlier, which stay.
        kept: Vec<String>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum PracticeEvent {
    /// Every map, read again from disk (`reload`).
    Maps(Vec<MapView>),
    Map(MapView),
    Game(PracticeGame),
    Ask(Question),
}

/// The practice side of the facade.
pub trait PracticeMaps {
    /// Cheap: no I/O.
    fn snapshot(&self) -> PracticeSnapshot;
    /// Reads every map's state again, off the UI thread (the page opening: the player may have
    /// played or deleted a world since).
    fn reload(&self);
    /// Downloads, checks and installs the map: `job` with progress, then its new status, or a
    /// failure.
    fn install(&self, id: &str);
    /// Stops a download (`MapJob::cancellable`); the map goes back to what it was.
    fn cancel(&self, id: &str);
    /// Update to the catalogue's version: at once when no world was played, else
    /// `Question::Update`.
    fn ask_update(&self, id: &str);
    fn update(&self, id: &str, choice: UpdateChoice);
    /// Sends `Question::Reset` with what Reset would do.
    fn ask_reset(&self, id: &str);
    fn reset(&self, id: &str, keep_data: bool);
    /// Sends `Question::Remove` with the worlds Remove deletes.
    fn ask_remove(&self, id: &str);
    fn remove(&self, id: &str);
    /// Installs the practice instance if needed and starts it (`PracticeGame`). `id` is the map
    /// the player wants to open.
    fn play(&self, id: &str);
    /// Stops the install, or asks the running game to quit (it saves first).
    fn stop(&self);
}

/// How often a running change sends its progress to the UI.
const PROGRESS_EVERY: Duration = Duration::from_millis(50);

/// The snapshot and the channel, shared by both implementations: every change goes through here,
/// so an event is sent only when something changed, and in the order the changes were made.
struct Shared {
    snap: Mutex<PracticeSnapshot>,
    events: Events,
}

impl Shared {
    fn send(&self, ev: PracticeEvent) {
        let _ = self.events.unbounded_send(Event::Practice(ev));
    }

    fn view(&self, id: &str) -> Option<MapView> {
        self.snap.lock().unwrap().map(id).cloned()
    }

    fn update_map(&self, id: &str, f: impl FnOnce(&mut MapView)) {
        let mut snap = self.snap.lock().unwrap();
        let Some(view) = snap.maps.iter_mut().find(|m| m.id == id) else {
            return;
        };
        let before = view.clone();
        f(view);
        if *view != before {
            self.send(PracticeEvent::Map(view.clone()));
        }
    }

    fn game(&self) -> PracticeGame {
        self.snap.lock().unwrap().game.clone()
    }

    fn update_game(&self, f: impl FnOnce(&mut PracticeGame)) {
        let mut snap = self.snap.lock().unwrap();
        let before = snap.game.clone();
        f(&mut snap.game);
        if snap.game != before {
            self.send(PracticeEvent::Game(snap.game.clone()));
        }
    }

    /// Every map's status from disk; maps with a change under way keep theirs until it ends.
    fn set_statuses(&self, statuses: Vec<(String, MapStatus)>) {
        let mut snap = self.snap.lock().unwrap();
        for (id, status) in statuses {
            if let Some(v) = snap.maps.iter_mut().find(|m| m.id == id)
                && v.job.is_none()
            {
                v.status = status;
            }
        }
        snap.loaded = true;
        self.send(PracticeEvent::Maps(snap.maps.clone()));
    }

    fn game_busy(&self) -> bool {
        self.snap.lock().unwrap().game_busy()
    }
}

/// Throttles progress: the first report, a new step and a finished step pass at once, the rest
/// at most every `PROGRESS_EVERY`.
fn progress_sink(shared: Arc<Shared>, id: String) -> srl_practice::ProgressFn {
    let last: Mutex<Option<(Instant, Step)>> = Mutex::new(None);
    Arc::new(move |p: Progress| {
        let mut last = last.lock().unwrap();
        let due = match *last {
            None => true,
            Some((at, step)) => {
                step != p.step || p.done >= p.total || at.elapsed() >= PROGRESS_EVERY
            }
        };
        if due {
            *last = Some((Instant::now(), p.step));
            shared.update_map(&id, |v| {
                if let Some(job) = &mut v.job {
                    job.progress = Some(p);
                }
            });
        }
    })
}

/// Settings and the Tools page, from `RealBackend`.
pub struct Host {
    /// The newest settings (Play reads them when it starts, like a mode's Play).
    pub settings: Box<dyn Fn() -> Settings + Send + Sync>,
    /// Play may have installed or updated a tool: the Tools page reads them again.
    pub tools_changed: Box<dyn Fn() + Send + Sync>,
    /// The Microsoft accounts: who the practice game plays as, its sign-in, and the login
    /// dialog when the sign-in has to be done again.
    pub accounts: RealAccounts,
}

/// `RealBackend`'s practice: srl-practice built once for the data folder, with the core's
/// downloader, and the core for the game.
pub struct RealPractice {
    inner: Arc<Inner>,
    rt: tokio::runtime::Handle,
}

struct Inner {
    core: Arc<Core>,
    maps: srl_practice::Practice,
    shared: Arc<Shared>,
    /// Map changes under way, and the switch that cancels a download.
    jobs: Mutex<HashMap<String, watch::Sender<bool>>>,
    /// The practice game's Play while it installs or runs, and the switch that stops it.
    game: Mutex<Option<watch::Sender<bool>>>,
    host: Host,
}

/// Every catalogue entry before its status is read: what the catalogue alone says.
fn initial_views(catalogue: &srl_practice::Catalogue) -> Vec<MapView> {
    catalogue
        .maps
        .iter()
        .map(|m| MapView {
            id: m.id.clone(),
            status: if m.is_installable() {
                MapStatus::NotInstalled
            } else {
                MapStatus::LinkOnly
            },
            job: None,
            failure: None,
        })
        .collect()
}

impl RealPractice {
    /// Builds srl-practice for the core's data folder and reads the maps' states in the
    /// background (`PracticeSnapshot::loaded`). The game's state comes from its install record
    /// (a few stats, as for the modes).
    pub fn new(
        core: Arc<Core>,
        events: Events,
        rt: tokio::runtime::Handle,
        host: Host,
    ) -> Result<Self> {
        let maps = srl_practice::Practice::new(core.paths().clone(), core.downloader().clone())?;
        let state = match core.local_state(Instance::Practice) {
            Local::NotInstalled => ModeState::NotInstalled,
            Local::Ready(_) | Local::Damaged(_) => ModeState::Ready,
        };
        let shared = Arc::new(Shared {
            snap: Mutex::new(PracticeSnapshot {
                loaded: false,
                maps: initial_views(maps.catalogue()),
                game: PracticeGame {
                    state,
                    map: None,
                    launched: None,
                },
            }),
            events,
        });
        let this = Self {
            inner: Arc::new(Inner {
                core,
                maps,
                shared,
                jobs: Mutex::new(HashMap::new()),
                game: Mutex::new(None),
                host,
            }),
            rt,
        };
        this.reload();
        Ok(this)
    }

    /// Runs one map change: marks the row, reports progress, reads the map's status again when
    /// it ends (also after a failure: a failed change leaves the map as it was, and the row
    /// shows what is really there).
    fn run<F, Fut>(&self, id: &str, kind: JobKind, work: F)
    where
        F: FnOnce(srl_practice::Practice, srl_practice::ProgressFn) -> Fut + Send + 'static,
        Fut: Future<Output = Result<String, PracticeError>> + Send + 'static,
    {
        let inner = &self.inner;
        if kind != JobKind::Install && self.blocked_by_game(id, kind) {
            return;
        }
        let (cancel, mut cancelled) = watch::channel(false);
        {
            let mut jobs = inner.jobs.lock().unwrap();
            if jobs.contains_key(id) {
                return;
            }
            jobs.insert(id.to_string(), cancel);
        }
        inner.shared.update_map(id, |v| {
            v.job = Some(MapJob {
                kind,
                progress: None,
            });
            v.failure = None;
        });
        log(format_args!("practice: {kind:?} {id}"));
        let (inner, id) = (inner.clone(), id.to_string());
        self.rt.spawn(async move {
            let progress = progress_sink(inner.shared.clone(), id.clone());
            let work = work(inner.maps.clone(), progress);
            let result = tokio::select! {
                r = work => Some(r),
                () = wait_cancel(&mut cancelled) => None,
            };
            inner.jobs.lock().unwrap().remove(&id);
            let failure = match result {
                None => {
                    log(format_args!("practice: {kind:?} {id} cancelled"));
                    None
                }
                Some(Ok(done)) => {
                    log(format_args!("practice: {kind:?} {id}: {done}"));
                    None
                }
                // A world was played while the update downloaded: ask, never decide.
                Some(Err(PracticeError::NeedsDecision(plan))) => {
                    ask_update(&inner.shared, &id, *plan);
                    None
                }
                Some(Err(e)) => {
                    log(format_args!("practice: {kind:?} {id} failed: {e:#}"));
                    Some(MapFailure::of(kind, &e))
                }
            };
            let status = inner.maps.status(&id).await;
            inner.shared.update_map(&id, |v| {
                v.job = None;
                v.failure = failure;
                if let Ok(s) = status {
                    v.status = s;
                }
            });
        });
    }

    /// Reset, Update and Remove wait while the practice game runs (this session's, or one an
    /// earlier session left running): the row says so with the `WorldInUse` line.
    fn blocked_by_game(&self, id: &str, kind: JobKind) -> bool {
        let earlier = self.inner.core.earlier_game(Instance::Practice);
        if !self.inner.shared.game_busy() && earlier.is_none() {
            return false;
        }
        log(format_args!(
            "practice: {kind:?} {id} refused: the practice game runs"
        ));
        self.inner.shared.update_map(id, |v| {
            v.failure = Some(MapFailure {
                kind: MapFailKind::WorldInUse,
                job: kind,
                detail: "the practice game runs".into(),
            })
        });
        true
    }

    /// Runs a read (a plan) for the map; a failure shows on its row.
    fn plan<Fut>(&self, id: &str, kind: JobKind, work: impl FnOnce(Arc<Inner>) -> Fut)
    where
        Fut: Future<Output = Result<(), PracticeError>> + Send + 'static,
    {
        let (shared, id) = (self.inner.shared.clone(), id.to_string());
        let fut = work(self.inner.clone());
        self.rt.spawn(async move {
            if let Err(e) = fut.await {
                log(format_args!("practice: {kind:?} {id}: {e:#}"));
                shared.update_map(&id, |v| v.failure = Some(MapFailure::of(kind, &e)));
            }
        });
    }
}

fn ask_update(shared: &Shared, id: &str, plan: UpdatePlan) {
    if let UpdatePlan::Ask {
        from,
        to,
        played,
        carry_over,
    } = plan
    {
        shared.send(PracticeEvent::Ask(Question::Update {
            id: id.to_string(),
            from,
            to,
            played,
            carry_over,
        }));
    }
}

impl PracticeMaps for RealPractice {
    fn snapshot(&self) -> PracticeSnapshot {
        self.inner.shared.snap.lock().unwrap().clone()
    }

    fn reload(&self) {
        let inner = self.inner.clone();
        self.rt.spawn(async move {
            match inner.maps.statuses().await {
                Ok(s) => inner.shared.set_statuses(s),
                Err(e) => log(format_args!("practice: reading the maps: {e:#}")),
            }
        });
    }

    fn install(&self, id: &str) {
        let key = id.to_string();
        self.run(id, JobKind::Install, move |maps, progress| async move {
            let done = maps.install(&key, Some(progress)).await?;
            Ok(format!(
                "v{} in {}",
                done.version,
                done.worlds
                    .iter()
                    .map(|w| w.folder.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        });
    }

    fn cancel(&self, id: &str) {
        let downloading = self
            .inner
            .shared
            .view(id)
            .and_then(|v| v.job)
            .is_some_and(|j| j.cancellable());
        if downloading && let Some(c) = self.inner.jobs.lock().unwrap().get(id) {
            c.send_replace(true);
        }
    }

    fn ask_update(&self, id: &str) {
        if self.blocked_by_game(id, JobKind::Update) {
            return;
        }
        let key = id.to_string();
        let this = RealPractice {
            inner: self.inner.clone(),
            rt: self.rt.clone(),
        };
        self.plan(id, JobKind::Update, move |inner| async move {
            match inner.maps.plan_update(&key).await? {
                UpdatePlan::UpToDate => {
                    let status = inner.maps.status(&key).await?;
                    inner.shared.update_map(&key, |v| v.status = status);
                }
                // Nothing played: replacing the worlds loses nothing.
                UpdatePlan::Replace { .. } => {
                    let id = key.clone();
                    this.run(&key, JobKind::Update, move |maps, progress| async move {
                        let done = maps.update(&id, None, Some(progress)).await?;
                        Ok(format!("v{}", done.version))
                    });
                }
                plan @ UpdatePlan::Ask { .. } => ask_update(&inner.shared, &key, plan),
            }
            Ok(())
        });
    }

    fn update(&self, id: &str, choice: UpdateChoice) {
        let key = id.to_string();
        self.run(id, JobKind::Update, move |maps, progress| async move {
            let done = maps.update(&key, Some(choice), Some(progress)).await?;
            Ok(format!(
                "v{} ({choice:?}), handed over: {:?}",
                done.version, done.handed_over
            ))
        });
    }

    fn ask_reset(&self, id: &str) {
        if self.blocked_by_game(id, JobKind::Reset) {
            return;
        }
        let key = id.to_string();
        self.plan(id, JobKind::Reset, move |inner| async move {
            let plan = inner.maps.plan_reset(&key).await?;
            inner
                .shared
                .send(PracticeEvent::Ask(Question::Reset { id: key, plan }));
            Ok(())
        });
    }

    fn reset(&self, id: &str, keep_data: bool) {
        let key = id.to_string();
        self.run(id, JobKind::Reset, move |maps, progress| async move {
            let done = maps.reset(&key, keep_data, Some(progress)).await?;
            Ok(format!(
                "v{}, kept: {:?}",
                done.version,
                done.carried.iter().map(|c| &c.what).collect::<Vec<_>>()
            ))
        });
    }

    fn ask_remove(&self, id: &str) {
        if self.blocked_by_game(id, JobKind::Remove) {
            return;
        }
        let key = id.to_string();
        self.plan(id, JobKind::Remove, move |inner| async move {
            let status = inner.maps.status(&key).await?;
            inner.shared.update_map(&key, |v| v.status = status.clone());
            if let MapStatus::Installed(m) = status {
                inner.shared.send(PracticeEvent::Ask(Question::Remove {
                    id: key,
                    worlds: m.worlds,
                    kept: m.handed_over,
                }));
            }
            Ok(())
        });
    }

    fn remove(&self, id: &str) {
        let key = id.to_string();
        self.run(id, JobKind::Remove, move |maps, _| async move {
            let done = maps.uninstall(&key).await?;
            Ok(format!("removed {:?}, kept {:?}", done.removed, done.kept))
        });
    }

    fn play(&self, id: &str) {
        let (cancel, cancelled) = watch::channel(false);
        {
            let mut game = self.inner.game.lock().unwrap();
            if game.is_some() {
                return;
            }
            *game = Some(cancel);
        }
        let before = self.inner.shared.game().state;
        log(format_args!("practice: play ({id})"));
        self.inner.shared.update_game(|g| {
            g.state = ModeState::Installing(real::stage_only(Stage::Metadata));
            g.map = Some(id.to_string());
            g.launched = None;
        });
        self.rt.spawn(run_game(
            self.inner.clone(),
            before,
            cancelled,
            Instant::now(),
        ));
    }

    fn stop(&self) {
        if let Some(c) = self.inner.game.lock().unwrap().as_ref() {
            log(format_args!("practice: stop"));
            c.send_replace(true);
        }
    }
}

impl Inner {
    /// Ends a Play: the instance is free again, then the game shows `state`, and the maps are
    /// read again (the worlds the player opened are Played now).
    async fn finish_game(&self, state: ModeState) {
        *self.game.lock().unwrap() = None;
        self.shared.update_game(|g| {
            g.state = state;
            g.launched = None;
        });
        match self.maps.statuses().await {
            Ok(s) => self.shared.set_statuses(s),
            Err(e) => log(format_args!("practice: reading the maps: {e:#}")),
        }
    }
}

/// One practice Play, from the click to the game's exit: `pipeline` as for a mode
/// (`real::play`), with `Instance::Practice`.
async fn run_game(
    inner: Arc<Inner>,
    before: ModeState,
    mut cancelled: watch::Receiver<bool>,
    clicked: Instant,
) {
    let settings = (inner.host.settings)();
    let (opts, tools) = (settings.mod_rules(), settings.tool_selection(TOOLS_LIKE));
    let sink: srl_core::ProgressFn = {
        let shared = inner.shared.clone();
        let mux = Mutex::new(real::Mux::default());
        Arc::new(move |p: srl_core::Progress| {
            let shown = mux.lock().unwrap().update(p, Instant::now());
            if let Some(shown) = shown {
                shared.update_game(|g| g.state = ModeState::Installing(shown));
            }
        })
    };
    let prepared = tokio::select! {
        r = inner.core.prepare_instance(Instance::Practice, &opts, &tools, Some(sink)) => r,
        () = wait_cancel(&mut cancelled) => {
            log(format_args!("practice: install cancelled"));
            inner.finish_game(before).await;
            return;
        }
    };
    let was_installed = match &before {
        ModeState::Failed(f) => f.installed,
        ModeState::NotInstalled => false,
        _ => true,
    };
    let prepared = match prepared {
        Ok(p) => p,
        Err(e) => {
            log(format_args!("practice: failed: {e:#}"));
            let failure = Failure {
                kind: pipeline::install_failure_kind(&e),
                detail: FailDetail::Core(format!("{e:#}")),
                installed: was_installed,
                log: None,
            };
            inner.finish_game(ModeState::Failed(failure)).await;
            return;
        }
    };
    let r = &prepared.report;
    log(format_args!(
        "practice: installed in {:.2}s: {} (Java {}), mods {} ({} added, {} updated, {} removed)",
        clicked.elapsed().as_secs_f64(),
        prepared.installed.version_id,
        prepared.installed.java.version,
        prepared
            .mods
            .mods
            .iter()
            .map(|m| m.modid.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        r.added.len(),
        r.updated.len(),
        r.removed.len(),
    ));
    for note in &prepared.notes {
        log(format_args!("practice: note: {note}"));
    }
    (inner.host.tools_changed)();

    // Who plays: the same choice and sign-in as a mode's Play (`real::play`).
    let settings = (inner.host.settings)();
    let accounts = &inner.host.accounts;
    let signed_in = tokio::select! {
        r = account::launch_account(
            accounts.auth(),
            pipeline::offline_account(&settings),
            || inner.shared.update_game(|g| {
                g.state = ModeState::Installing(real::stage_only(Stage::SigningIn));
            }),
        ) => r,
        () = wait_cancel(&mut cancelled) => {
            log(format_args!("practice: cancelled while signing in"));
            inner.finish_game(ModeState::Ready).await;
            return;
        }
    };
    let signed_in = match signed_in {
        Ok(a) => a,
        Err(problem) => {
            log(format_args!(
                "practice: Microsoft sign-in failed: {problem}"
            ));
            let relogin = problem.relogin;
            let failure = Failure {
                kind: FailKind::SignIn,
                detail: FailDetail::Account(problem),
                installed: true,
                log: None,
            };
            inner.finish_game(ModeState::Failed(failure)).await;
            if relogin {
                // The saved sign-in is gone: the login dialog opens; Play again after it.
                accounts.login();
            }
            return;
        }
    };
    log(format_args!(
        "practice: plays as {} ({})",
        signed_in.account.name(),
        if signed_in.account.is_microsoft() {
            "Microsoft"
        } else {
            "offline"
        }
    ));

    inner
        .shared
        .update_game(|g| g.state = ModeState::Installing(real::stage_only(Stage::Launching)));
    let mut game = match inner
        .core
        .launch(&prepared, &settings, signed_in.account)
        .await
    {
        Ok(g) => g,
        Err(e) => {
            log(format_args!("practice: failed to start: {e:#}"));
            let failure = Failure {
                kind: FailKind::Launch,
                detail: FailDetail::Core(format!("{e:#}")),
                installed: true,
                log: None,
            };
            inner.finish_game(ModeState::Failed(failure)).await;
            return;
        }
    };
    game.account_in_use = signed_in.in_use;
    game.launched.valid_until = signed_in.valid_until;
    let log_path = game.process.log_path().to_path_buf();
    log(format_args!(
        "practice: game arguments: {}",
        game.game_args.join(" ")
    ));
    log(format_args!(
        "practice: game process started, pid {}, {:.2}s after Play; log {}; tools: {:?}",
        game.process.pid(),
        clicked.elapsed().as_secs_f64(),
        log_path.display(),
        game.launched
            .tools
            .iter()
            .map(|t| t.info().name)
            .collect::<Vec<_>>()
    ));
    for note in &game.launched.tool_notes {
        log(format_args!("practice: tools: {note}"));
    }
    inner.shared.update_game(|g| {
        g.state = ModeState::Running;
        g.launched = Some(game.launched.clone());
    });

    // Stop: ask the game to quit the way its close button does, so it saves.
    let stopper = {
        let process = game.process.clone();
        let mut cancelled = cancelled.clone();
        tokio::spawn(async move {
            wait_cancel(&mut cancelled).await;
            let report = process.stop_detailed(real::STOP_GRACE).await;
            log(format_args!(
                "practice: stopped: exit {:?} (escalated: {:?})",
                report.exit.code, report.escalated
            ));
        })
    };
    let startup = game.startup(pipeline::STARTUP_LIMIT).await;
    let mut failure = None;
    match &startup {
        Startup::Ready(after) => log(format_args!(
            "practice: title screen loading {:.1}s after the process started",
            after.as_secs_f64()
        )),
        Startup::Crashed(reason) => {
            log(format_args!("practice: Fabric stopped the start: {reason}"));
            game.process.kill().await;
            failure = Some(FailDetail::Stopped(reason.clone()));
        }
        Startup::Exited(_) | Startup::TimedOut => {}
    }
    let exit = game.wait().await;
    let stopped = *cancelled.borrow();
    if stopped {
        let _ = stopper.await;
    } else {
        stopper.abort();
    }
    log(format_args!(
        "practice: game exited (code {:?}){}",
        exit.code,
        if stopped { " after Stop" } else { "" }
    ));
    let launch_failure = |detail: FailDetail| {
        log(format_args!(
            "practice: failed to start: {}",
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
        (Some(detail), _) => launch_failure(detail),
        (None, Startup::Exited(exit)) if !stopped && !exit.success => {
            launch_failure(pipeline::startup_failure(*exit, &log_path))
        }
        _ => ModeState::Ready,
    };
    inner.finish_game(end).await;
}

// ---------------------------------------------------------------------------------------------
// The fake
// ---------------------------------------------------------------------------------------------

/// The states the fake starts in (`SRL_SCREEN=practice-*`, README "Screen ids").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PracticePreset {
    /// Nothing installed (the first state everyone sees), or with `SRL_DEMO` the prototype's
    /// install states.
    Default,
    /// A few maps installed: BT Practice fresh, Portal Practice v2 played, Zero Practice played
    /// with an update out, The MCSR Practice Map and End Portal Fill fresh.
    Installed,
    /// `Installed`, with The MCSR Practice Map downloading (40 %).
    Installing,
    /// `Installed`, with the update question for Zero Practice open.
    Update,
    /// `Installed`, with the reset confirmation for Zero Practice open.
    Reset,
    /// `Installed`, with the remove confirmation for BT Practice open.
    Remove,
    /// `Installed`, with the practice game installing for BT Practice.
    Starting,
    /// `Installed`, with the practice game running for BT Practice.
    Running,
    /// `Installed`, with failed changes: Crafting Practice v2 offline, Portal Practice v2 open in
    /// the game.
    Error,
}

/// A made-up older version for the fake's update state (the catalogue holds only the newest).
const FAKE_OLD_ZERO: &str = "1.2.1";

fn worlds_of(entry: &MapEntry, state: WorldState) -> Vec<WorldView> {
    let Some(dl) = &entry.download else {
        return vec![];
    };
    dl.worlds
        .iter()
        .map(|folder| {
            let folder = if folder.is_empty() {
                entry.name.clone()
            } else {
                folder.clone()
            };
            WorldView {
                name: folder.clone(),
                folder,
                state,
                data_version: dl.data_version,
            }
        })
        .collect()
}

fn installed(entry: &MapEntry, state: WorldState, old: Option<&str>) -> MapStatus {
    let latest = entry.version().unwrap_or_default().to_string();
    let version = old.map_or(latest.clone(), str::to_string);
    let mut worlds = worlds_of(entry, state);
    // An older version's world names its own version.
    for w in &mut worlds {
        w.name = w.name.replace(&latest, &version);
        w.folder = w.folder.replace(&latest, &version);
    }
    MapStatus::Installed(InstalledMap {
        stored_version: version.clone(),
        version,
        stored: true,
        update: old.map(|_| latest),
        worlds,
        handed_over: vec![],
    })
}

/// The fake's played Zero Practice worlds, of the made-up older version.
fn old_zero_worlds(e: &MapEntry) -> Vec<WorldView> {
    match installed(e, WorldState::Played, Some(FAKE_OLD_ZERO)) {
        MapStatus::Installed(m) => m.worlds,
        _ => vec![],
    }
}

const MCSR: &str = "the-mcsr-practice-map-dibedy";
const BT: &str = "bt-practice-mescht";
const ZERO: &str = "zero-practice-mescht";
const PORTAL2: &str = "portal-practice-v2-semperzz";
const CRAFTING: &str = "crafting-practice-v2-semperzz";
const EPF: &str = "end-portal-fill-v2-cylorun";

/// The fake's starting snapshot, and the question to open with.
fn fake_snapshot(
    catalogue: &srl_practice::Catalogue,
    preset: PracticePreset,
    demo: Option<&HashMap<String, String>>,
) -> (PracticeSnapshot, Option<Question>) {
    let entry = |id: &str| catalogue.get(id).expect("in the catalogue");
    let mut maps = initial_views(catalogue);
    let mut set = |id: &str, f: &dyn Fn(&mut MapView)| {
        if let Some(v) = maps.iter_mut().find(|m| m.id == id) {
            f(v)
        }
    };
    if preset == PracticePreset::Default {
        // The prototype's states, for design comparison.
        for (id, status) in demo.into_iter().flatten() {
            let e = entry(id);
            match status.as_str() {
                "installed" => set(id, &|v| v.status = installed(e, WorldState::Fresh, None)),
                "update" => {
                    // One version back of the catalogue's, made up for the demo.
                    set(id, &|v| {
                        v.status = installed(e, WorldState::Played, Some("3.14.0"))
                    })
                }
                _ => {}
            }
        }
    } else {
        set(BT, &|v| {
            v.status = installed(entry(BT), WorldState::Fresh, None)
        });
        set(PORTAL2, &|v| {
            v.status = installed(entry(PORTAL2), WorldState::Played, None)
        });
        set(ZERO, &|v| {
            v.status = installed(entry(ZERO), WorldState::Played, Some(FAKE_OLD_ZERO))
        });
        set(MCSR, &|v| {
            v.status = installed(entry(MCSR), WorldState::Fresh, None)
        });
        set(EPF, &|v| {
            v.status = installed(entry(EPF), WorldState::Fresh, None)
        });
    }
    let mut game = PracticeGame {
        state: if preset == PracticePreset::Default {
            ModeState::NotInstalled
        } else {
            ModeState::Ready
        },
        map: None,
        launched: None,
    };
    let mut question = None;
    match preset {
        PracticePreset::Installing => {
            let size = entry(MCSR).download.as_ref().map_or(0, |d| d.size);
            set(MCSR, &|v| {
                v.status = MapStatus::NotInstalled;
                v.job = Some(MapJob {
                    kind: JobKind::Install,
                    progress: Some(Progress {
                        step: Step::Downloading,
                        done: size * 2 / 5,
                        total: size,
                    }),
                });
            });
        }
        PracticePreset::Update => {
            let e = entry(ZERO);
            question = Some(Question::Update {
                id: ZERO.into(),
                from: FAKE_OLD_ZERO.into(),
                to: e.version().unwrap_or_default().into(),
                played: old_zero_worlds(e),
                carry_over: e.carry_over.clone(),
            });
        }
        PracticePreset::Reset => {
            let e = entry(ZERO);
            question = Some(Question::Reset {
                id: ZERO.into(),
                plan: ResetPlan {
                    worlds: old_zero_worlds(e),
                    version: FAKE_OLD_ZERO.into(),
                    needs_download: false,
                    carry_over: e.carry_over.clone(),
                },
            });
        }
        PracticePreset::Remove => {
            question = Some(Question::Remove {
                id: BT.into(),
                worlds: worlds_of(entry(BT), WorldState::Fresh),
                kept: vec![],
            });
        }
        PracticePreset::Starting => {
            game.map = Some(BT.into());
            game.state = ModeState::Installing(srl_core::Progress {
                stage: Stage::Mods,
                done: 3,
                total: 9,
                unit: Unit::Files,
            });
        }
        PracticePreset::Running => {
            game.map = Some(BT.into());
            game.state = ModeState::Running;
        }
        PracticePreset::Error => {
            set(CRAFTING, &|v| {
                v.failure = Some(MapFailure {
                    kind: MapFailKind::Network,
                    job: JobKind::Install,
                    detail: "download failed: error sending request (simulated)".into(),
                })
            });
            set(PORTAL2, &|v| {
                v.failure = Some(MapFailure {
                    kind: MapFailKind::WorldInUse,
                    job: JobKind::Reset,
                    detail: "the game has saves/Portal Practice v2 open (simulated)".into(),
                })
            });
        }
        PracticePreset::Default | PracticePreset::Installed => {}
    }
    (
        PracticeSnapshot {
            loaded: true,
            maps,
            game,
        },
        question,
    )
}

pub struct FakePractice {
    shared: Arc<Shared>,
    cmds: mpsc::Sender<Cmd>,
}

enum Cmd {
    Job(String, JobKind, Option<UpdateChoice>),
    Cancel(String),
    Ask(String, JobKind),
    Play(String),
    Stop,
}

struct FakeJob {
    id: String,
    kind: JobKind,
    /// The player's answer, for an update.
    choice: Option<UpdateChoice>,
    start: Instant,
    before: MapView,
}

/// Starts a simulated change, unless the map is busy or (except Install) the game runs.
fn start_fake_job(
    shared: &Shared,
    jobs: &mut Vec<FakeJob>,
    id: String,
    kind: JobKind,
    choice: Option<UpdateChoice>,
) {
    let busy = jobs.iter().any(|j| j.id == id);
    let game = kind != JobKind::Install && shared.game_busy();
    if let Some(before) = shared.view(&id).filter(|_| !busy && !game) {
        shared.update_map(&id, |v| {
            v.job = Some(MapJob {
                kind,
                progress: None,
            });
            v.failure = None;
        });
        jobs.push(FakeJob {
            id,
            kind,
            choice,
            start: Instant::now(),
            before,
        });
    }
}

/// What a finished simulated change leaves: the catalogue's version, fresh; an update the player
/// put off (`StoreOnly`) changes only the stored copy, and `KeepBoth` hands the played worlds
/// over.
fn fake_done(entry: &MapEntry, job: &FakeJob) -> MapStatus {
    let mut status = installed(entry, WorldState::Fresh, None);
    let (MapStatus::Installed(new), MapStatus::Installed(old)) = (&mut status, &job.before.status)
    else {
        return status;
    };
    match job.choice {
        Some(UpdateChoice::StoreOnly) => {
            new.version = old.version.clone();
            new.worlds = old.worlds.clone();
        }
        Some(UpdateChoice::KeepBoth) => {
            new.handed_over = old.worlds.iter().map(|w| w.folder.clone()).collect();
            for w in &mut new.worlds {
                w.name = format!("{} (2)", w.name);
                w.folder = format!("{} (2)", w.folder);
            }
        }
        _ => {}
    }
    status
}

/// How long the fake's steps take.
const FAKE_DOWNLOAD: Duration = Duration::from_secs(3);
const FAKE_EXTRACT: Duration = Duration::from_millis(1200);
const FAKE_START: Duration = Duration::from_secs(2);
const TICK: Duration = Duration::from_millis(50);

impl FakePractice {
    /// `demo`: the prototype's install state per map id (`SRL_DEMO`).
    pub fn new(
        events: Events,
        preset: PracticePreset,
        demo: Option<&HashMap<String, String>>,
    ) -> Self {
        let catalogue = srl_practice::Catalogue::builtin().expect("the built-in catalogue");
        let (snap, question) = fake_snapshot(&catalogue, preset, demo);
        let shared = Arc::new(Shared {
            snap: Mutex::new(snap),
            events,
        });
        // The preset's question arrives like a real one: as an event.
        if let Some(q) = question {
            shared.send(PracticeEvent::Ask(q));
        }
        let (cmds, rx) = mpsc::channel();
        let worker = shared.clone();
        thread::Builder::new()
            .name("fake-practice".into())
            .spawn(move || run_fake(worker, catalogue, rx))
            .expect("spawn the fake practice thread");
        Self { shared, cmds }
    }
}

impl PracticeMaps for FakePractice {
    fn snapshot(&self) -> PracticeSnapshot {
        self.shared.snap.lock().unwrap().clone()
    }
    fn reload(&self) {}
    fn install(&self, id: &str) {
        let _ = self.cmds.send(Cmd::Job(id.into(), JobKind::Install, None));
    }
    fn cancel(&self, id: &str) {
        let _ = self.cmds.send(Cmd::Cancel(id.into()));
    }
    fn ask_update(&self, id: &str) {
        let _ = self.cmds.send(Cmd::Ask(id.into(), JobKind::Update));
    }
    fn update(&self, id: &str, choice: UpdateChoice) {
        let _ = self
            .cmds
            .send(Cmd::Job(id.into(), JobKind::Update, Some(choice)));
    }
    fn ask_reset(&self, id: &str) {
        let _ = self.cmds.send(Cmd::Ask(id.into(), JobKind::Reset));
    }
    fn reset(&self, id: &str, _keep_data: bool) {
        let _ = self.cmds.send(Cmd::Job(id.into(), JobKind::Reset, None));
    }
    fn ask_remove(&self, id: &str) {
        let _ = self.cmds.send(Cmd::Ask(id.into(), JobKind::Remove));
    }
    fn remove(&self, id: &str) {
        let _ = self.cmds.send(Cmd::Job(id.into(), JobKind::Remove, None));
    }
    fn play(&self, id: &str) {
        let _ = self.cmds.send(Cmd::Play(id.into()));
    }
    fn stop(&self) {
        let _ = self.cmds.send(Cmd::Stop);
    }
}

fn run_fake(shared: Arc<Shared>, catalogue: srl_practice::Catalogue, rx: mpsc::Receiver<Cmd>) {
    let mut jobs: Vec<FakeJob> = Vec::new();
    // The practice game's start, while it runs.
    let mut starting: Option<Instant> = None;
    loop {
        // Idle blocks without a timeout: nothing to simulate costs nothing.
        let cmd = if jobs.is_empty() && starting.is_none() {
            rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
        } else {
            rx.recv_timeout(TICK)
        };
        match cmd {
            Ok(Cmd::Job(id, kind, choice)) => start_fake_job(&shared, &mut jobs, id, kind, choice),
            Ok(Cmd::Cancel(id)) => {
                let cancellable = shared
                    .view(&id)
                    .and_then(|v| v.job)
                    .is_some_and(|j| j.cancellable());
                if cancellable {
                    if let Some(i) = jobs.iter().position(|j| j.id == id) {
                        let mut before = jobs.remove(i).before;
                        before.job = None;
                        shared.update_map(&id, |v| *v = before);
                    } else {
                        // A preset download with nothing behind it.
                        shared.update_map(&id, |v| v.job = None);
                    }
                }
            }
            Ok(Cmd::Ask(id, kind)) => {
                let (Some(view), Some(entry)) = (shared.view(&id), catalogue.get(&id)) else {
                    continue;
                };
                let MapStatus::Installed(m) = view.status.clone() else {
                    continue;
                };
                let question = match kind {
                    JobKind::Reset => Question::Reset {
                        id,
                        plan: ResetPlan {
                            worlds: m.worlds,
                            version: m.stored_version,
                            needs_download: false,
                            carry_over: entry.carry_over.clone(),
                        },
                    },
                    JobKind::Update if m.any_played() => Question::Update {
                        id,
                        from: m.version,
                        to: entry.version().unwrap_or_default().into(),
                        played: m
                            .worlds
                            .into_iter()
                            .filter(|w| w.state == WorldState::Played)
                            .collect(),
                        carry_over: entry
                            .carry_over
                            .iter()
                            .filter(|c| !c.same_version_only)
                            .cloned()
                            .collect(),
                    },
                    // Nothing played: the update runs at once, as the real one does.
                    JobKind::Update => {
                        start_fake_job(&shared, &mut jobs, id, kind, None);
                        continue;
                    }
                    JobKind::Remove | JobKind::Install => Question::Remove {
                        id,
                        worlds: m.worlds,
                        kept: m.handed_over,
                    },
                };
                shared.send(PracticeEvent::Ask(question));
            }
            Ok(Cmd::Play(id)) => {
                if !shared.game_busy() {
                    starting = Some(Instant::now());
                    shared.update_game(|g| {
                        g.map = Some(id);
                        g.state = ModeState::Installing(real::stage_only(Stage::Metadata));
                    });
                }
            }
            Ok(Cmd::Stop) => {
                starting = None;
                shared.update_game(|g| {
                    g.state = ModeState::Ready;
                    g.launched = None;
                });
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let now = Instant::now();
        if let Some(at) = starting {
            let f = now.saturating_duration_since(at).as_secs_f32() / FAKE_START.as_secs_f32();
            if f >= 1.0 {
                starting = None;
                shared.update_game(|g| {
                    g.state = ModeState::Running;
                    g.launched = Some(Launched {
                        player: "Player".into(),
                        memory_mb: 2048,
                        tools: vec![],
                        tool_notes: vec![],
                        microsoft: false,
                        valid_until: None,
                    });
                });
            } else {
                shared.update_game(|g| {
                    g.state = ModeState::Installing(srl_core::Progress {
                        stage: if f < 0.8 {
                            Stage::Mods
                        } else {
                            Stage::Launching
                        },
                        done: (9. * f / 0.8).min(9.) as u64,
                        total: 9,
                        unit: Unit::Files,
                    })
                });
            }
        }
        jobs.retain(|job| {
            let Some(entry) = catalogue.get(&job.id) else {
                return false;
            };
            let size = entry.download.as_ref().map_or(0, |d| d.size);
            let t = now.saturating_duration_since(job.start);
            let download = match job.kind {
                JobKind::Install | JobKind::Update => FAKE_DOWNLOAD,
                JobKind::Reset => Duration::ZERO,
                JobKind::Remove => {
                    shared.update_map(&job.id, |v| {
                        v.job = None;
                        v.status = MapStatus::NotInstalled;
                    });
                    return false;
                }
            };
            let progress = if t < download {
                Progress {
                    step: Step::Downloading,
                    done: (size as f64 * t.as_secs_f64() / download.as_secs_f64()) as u64,
                    total: size,
                }
            } else if t < download + FAKE_EXTRACT {
                let files = 400;
                Progress {
                    step: Step::Extracting,
                    done: files * (t - download).as_millis() as u64
                        / FAKE_EXTRACT.as_millis() as u64,
                    total: files,
                }
            } else {
                let status = fake_done(entry, job);
                shared.update_map(&job.id, |v| {
                    v.job = None;
                    v.status = status;
                });
                return false;
            };
            shared.update_map(&job.id, |v| {
                if let Some(j) = &mut v.job {
                    j.progress = Some(progress);
                }
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
    fn errors_get_one_line_each() {
        let cases = [
            (PracticeError::Network("x".into()), MapFailKind::Network),
            (
                PracticeError::SourceGone { url: "u".into() },
                MapFailKind::SourceGone,
            ),
            (
                PracticeError::HashMismatch {
                    expected: "a".into(),
                    got: "b".into(),
                },
                MapFailKind::HashMismatch,
            ),
            (
                PracticeError::BadArchive("x".into()),
                MapFailKind::NotAWorld,
            ),
            (
                PracticeError::UnsafeArchive("x".into()),
                MapFailKind::NotAWorld,
            ),
            (PracticeError::NoWorld, MapFailKind::NotAWorld),
            (
                PracticeError::WorldInUse { folder: "w".into() },
                MapFailKind::WorldInUse,
            ),
            (
                PracticeError::Other(anyhow::anyhow!("disk full")),
                MapFailKind::Other,
            ),
        ];
        for (e, kind) in cases {
            let f = MapFailure::of(JobKind::Install, &e);
            assert_eq!(f.kind, kind, "{e}");
            assert_eq!(f.detail, e.to_string());
        }
    }

    #[test]
    fn only_a_download_can_be_cancelled() {
        let job = |step| MapJob {
            kind: JobKind::Install,
            progress: Some(Progress {
                step,
                done: 1,
                total: 2,
            }),
        };
        assert!(job(Step::Downloading).cancellable());
        assert!(!job(Step::Extracting).cancellable());
        assert!(!job(Step::Verifying).cancellable());
        assert!(
            !MapJob {
                kind: JobKind::Install,
                progress: None
            }
            .cancellable()
        );
    }

    /// The practice game shares RSG's tool rules; the modes keep their own.
    #[test]
    fn tools_follow_rsg_for_practice() {
        let s = Settings::default();
        assert_eq!(s.rules_mode(Instance::Practice), Mode::Rsg);
        for m in Mode::ALL {
            assert_eq!(s.rules_mode(m.into()), m);
        }
        let t = target().unwrap();
        assert_eq!(t.instance, Instance::Practice);
        assert_eq!(t.profile.minecraft_version, PRACTICE.minecraft_version);
        assert_eq!(t.profile.java_major, PRACTICE.java_major);
    }

    /// The real snapshot from an empty data folder: every catalogue entry, installable or a link,
    /// nothing installed, the game not installed.
    #[test]
    fn real_starts_from_the_catalogue() {
        let root = std::env::temp_dir().join(format!("srl-app-practice-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let core = Arc::new(Core::new(srl_core::Paths::new(&root)).unwrap());
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let paths = srl_core::Paths::new(&root);
        // A build without Microsoft login (no client ID): the practice game plays offline. The
        // file keystore, so no test can reach the desktop's keyring.
        let auth = srl_auth::Auth::with_keystore(&paths, None, srl_auth::KeystoreChoice::FileOnly);
        let host = Host {
            settings: Box::new(Settings::default),
            tools_changed: Box::new(|| {}),
            accounts: RealAccounts::new(auth, &paths, tx.clone(), rt.handle().clone()),
        };
        let p = RealPractice::new(core, tx, rt.handle().clone(), host).unwrap();
        let snap = p.snapshot();
        assert_eq!(snap.game.state, ModeState::NotInstalled);
        let catalogue = srl_practice::Catalogue::builtin().unwrap();
        assert_eq!(snap.maps.len(), catalogue.maps.len());
        // The background read arrives as one event and marks the snapshot loaded.
        let ev = futures::executor::block_on(rx.next()).unwrap();
        let Event::Practice(PracticeEvent::Maps(maps)) = ev else {
            panic!("{ev:?}")
        };
        for (v, m) in maps.iter().zip(&catalogue.maps) {
            assert_eq!(v.id, m.id);
            let link = matches!(v.status, MapStatus::LinkOnly);
            assert_eq!(link, !m.is_installable(), "{}", m.id);
            assert!(link || v.status == MapStatus::NotInstalled, "{}", m.id);
        }
        assert!(p.snapshot().loaded);
        // Reset, Update and Remove ask first: with nothing installed the reset plan fails on the
        // row instead.
        p.ask_reset(BT);
        let ev = futures::executor::block_on(rx.next()).unwrap();
        let Event::Practice(PracticeEvent::Map(v)) = ev else {
            panic!("{ev:?}")
        };
        assert_eq!(v.failure.unwrap().job, JobKind::Reset);
        drop(rt);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fake_presets_cover_every_state() {
        let catalogue = srl_practice::Catalogue::builtin().unwrap();
        let (snap, q) = fake_snapshot(&catalogue, PracticePreset::Default, None);
        assert!(q.is_none() && snap.game.state == ModeState::NotInstalled);
        assert!(
            snap.maps
                .iter()
                .all(|m| matches!(m.status, MapStatus::NotInstalled | MapStatus::LinkOnly))
        );
        let (snap, _) = fake_snapshot(&catalogue, PracticePreset::Installing, None);
        assert!(snap.map(MCSR).unwrap().job.unwrap().cancellable());
        let (_, q) = fake_snapshot(&catalogue, PracticePreset::Update, None);
        assert!(
            matches!(q, Some(Question::Update { ref carry_over, .. }) if !carry_over.is_empty())
        );
        let (_, q) = fake_snapshot(&catalogue, PracticePreset::Reset, None);
        assert!(matches!(q, Some(Question::Reset { .. })));
        let (_, q) = fake_snapshot(&catalogue, PracticePreset::Remove, None);
        assert!(matches!(q, Some(Question::Remove { .. })));
        let (snap, _) = fake_snapshot(&catalogue, PracticePreset::Running, None);
        assert!(snap.game_busy() && snap.game.map.as_deref() == Some(BT));
        let (snap, _) = fake_snapshot(&catalogue, PracticePreset::Error, None);
        assert_eq!(
            snap.map(CRAFTING).unwrap().failure.as_ref().unwrap().kind,
            MapFailKind::Network
        );
        let MapStatus::Installed(z) = &snap.map(ZERO).unwrap().status else {
            panic!()
        };
        assert!(z.update.is_some() && z.any_played());
    }

    #[test]
    fn a_fake_install_reports_progress_then_installs() {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let p = FakePractice::new(tx, PracticePreset::Default, None);
        p.install(BT);
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut steps = Vec::new();
        futures::executor::block_on(async {
            loop {
                assert!(Instant::now() < deadline, "no install");
                let Some(Event::Practice(PracticeEvent::Map(v))) = rx.next().await else {
                    continue;
                };
                if let Some(MapJob {
                    progress: Some(pr), ..
                }) = v.job
                    && steps.last() != Some(&pr.step)
                {
                    steps.push(pr.step);
                }
                if matches!(v.status, MapStatus::Installed(_)) {
                    break;
                }
            }
        });
        assert_eq!(steps, [Step::Downloading, Step::Extracting]);
    }
}
