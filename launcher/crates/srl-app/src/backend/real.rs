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
//! - **Custom instances** go through the same Play, keyed by `Instance::Custom`, with their state
//!   in `Snapshot::customs`. Creating, duplicating and deleting them runs on the blocking pool
//!   (`instances.rs` for the files); the backend owns which exist and saves the settings itself.
//!
//! Everything it does is logged to stderr and `logs/launcher.log`, with seconds since start.
use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use srl_core::{CustomId, Instance, Mode, Paths, Progress, ProgressFn, Stage, Unit};
use tokio::sync::watch;

use super::{
    Backend, CustomInstance, Event, Events, FailDetail, FailKind, Failure, ModeInfo, ModeSnapshot,
    ModeState, Settings, Snapshot,
    account::{self, Accounts, RealAccounts},
    importer::{Importer, RealImport},
    info_event,
    instances::{self, CopyFrom, CopyPlan, CopyProgress, CopyWhat, Created, NewInstance},
    pipeline::{self, Core, Local, Startup},
    practice::{Host, PracticeMaps, RealPractice},
    state_event,
    stats::{RealStats, StatsService},
    tools::{RealTools, Tools},
    update::{RealUpdates, Updates},
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
    /// The launcher's own updates (srl-update).
    updates: RealUpdates,
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
    /// Instances (modes and custom ones) with a `play` under way (installing or running), and
    /// the switch that cancels it. A custom instance being deleted is in it too, so no Play can
    /// start while its directory moves.
    runs: Mutex<HashMap<Instance, watch::Sender<bool>>>,
    /// Instances a duplicate is copying from right now: they can't be deleted meanwhile.
    copying: Mutex<HashSet<Instance>>,
    /// Set by `cancel_copy`: the running duplicates stop before their next file.
    cancel_copy: std::sync::atomic::AtomicBool,
    /// One new custom instance number at a time (`instances::reserve`, then its definition).
    new_ids: Mutex<()>,
    /// The runtime, for work a blocking task starts (the update check after a create).
    rt: tokio::runtime::Handle,
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
        Self::open(Paths::from_env()?, events, true)
    }

    /// [`RealBackend::new`] on `paths`. Without `start_checks` (tests) nothing reaches the
    /// network on its own: no update check on start, no launcher update check.
    fn open(paths: Paths, events: Events, start_checks: bool) -> Result<Self> {
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
        let modes = std::array::from_fn(|i| local_snapshot(&core, Mode::ALL[i].into()));
        let customs: std::collections::BTreeMap<CustomId, ModeSnapshot> = settings
            .custom_instances
            .iter()
            .map(|c| (c.id, local_snapshot(&core, c.instance())))
            .collect();
        log(format_args!(
            "data in {}; modes: {}",
            core.paths().root().display(),
            Mode::ALL
                .iter()
                .zip(&modes)
                .map(|(m, s): (&Mode, &ModeSnapshot)| format!("{} {:?}", m.id(), s.state))
                .chain(customs.iter().map(|(id, s)| format!("{id} {:?}", s.state)))
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
                customs,
                settings,
                data_dir,
                data_error: data_error.clone(),
            }),
            events,
            runs: Mutex::new(HashMap::new()),
            copying: Mutex::new(HashSet::new()),
            cancel_copy: std::sync::atomic::AtomicBool::new(false),
            new_ids: Mutex::new(()),
            rt: rt.clone(),
            settings_file,
            settings_write: Mutex::new(()),
            check_lock: tokio::sync::Mutex::new(()),
        });
        let list = inner.snap.lock().unwrap().settings.custom_instances.clone();
        inner.stats_follow(&list);
        if data_error.is_none() && start_checks {
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
                    .any(|i| inner.busy(i) || inner.core.game_running(i))
            })
        };
        let import = RealImport::new(
            inner.core.paths().clone(),
            busy,
            inner.events.clone(),
            rt.clone(),
        );
        let updates = RealUpdates::new(
            inner.core.paths(),
            inner.core.downloader().clone(),
            inner.events.clone(),
            rt.clone(),
        );
        if data_error.is_none() && start_checks {
            let auto = inner.snap.lock().unwrap().settings.auto_update;
            updates.start(auto);
        }
        Ok(Self {
            inner,
            practice,
            import,
            updates,
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
        self.start(mode.into());
    }

    fn cancel(&self, mode: Mode) {
        self.stop(mode.into());
    }

    fn set_settings(&self, settings: Settings) {
        let mods_changed = {
            let mut snap = self.inner.snap.lock().unwrap();
            let mut settings = settings;
            // The custom instances that exist are the backend's to say (a create can be on its
            // way while the UI sends older settings); the UI's changes to them are taken.
            let fixed = snap.settings.adopt_customs(&mut settings);
            let changed = snap.settings.mod_rules() != settings.mod_rules();
            snap.settings = settings;
            if fixed {
                // Under the lock, so it can't overtake a newer list from a create or delete.
                log(format_args!(
                    "settings: kept the custom instances as they are, sent back to the UI"
                ));
                let list = snap.settings.custom_instances.clone();
                let _ = self
                    .inner
                    .events
                    .unbounded_send(Event::CustomInstances(list));
            }
            changed
        };
        // Another set of optional mods changes what the installed modes need: check again, from
        // the mod lists in memory, so Home says which modes Play will update.
        if mods_changed && self.inner.snap.lock().unwrap().data_error.is_none() {
            self.rt
                .spawn(check_updates(self.inner.clone(), pipeline::META_FRESH));
        }
        let inner = self.inner.clone();
        self.rt.spawn_blocking(move || inner.write_settings());
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

    fn updates(&self) -> &dyn Updates {
        &self.updates
    }

    fn add_jar(&self, mode: Mode, path: std::path::PathBuf) {
        let inner = self.inner.clone();
        self.rt.spawn_blocking(move || {
            let answer = inner.add_jar(mode.into(), &path);
            let _ = inner.events.unbounded_send(Event::JarAdded(mode, answer));
        });
    }

    fn play_custom(&self, id: CustomId) {
        self.start(Instance::Custom(id));
    }

    fn cancel_custom(&self, id: CustomId) {
        self.stop(Instance::Custom(id));
    }

    fn create_instance(&self, spec: NewInstance) {
        let inner = self.inner.clone();
        self.rt.spawn_blocking(move || {
            let answer = inner.create(spec).map_err(|e| format!("{e:#}"));
            inner.created(answer, false);
        });
    }

    fn duplicate_instance(&self, from: Instance, name: String, with_worlds: bool) {
        // Marked before the task starts, so a delete right after this call is refused.
        self.inner.copying.lock().unwrap().insert(from);
        self.inner
            .cancel_copy
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let inner = self.inner.clone();
        self.rt.spawn_blocking(move || {
            let answer = inner
                .duplicate(CopyFrom::Instance(from), &name, with_worlds)
                .map_err(|e| format!("{e:#}"));
            inner.copying.lock().unwrap().remove(&from);
            inner.created(answer, true);
        });
    }

    fn import_instance(
        &self,
        dir: std::path::PathBuf,
        base: Mode,
        name: String,
        with_worlds: bool,
    ) {
        // Never one of ours: that is Make a copy, and copying the data folder into itself
        // would not end.
        if dir.starts_with(self.inner.core.paths().root()) {
            self.inner
                .created(Err("that folder is this launcher's own".to_string()), true);
            return;
        }
        self.inner
            .cancel_copy
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let inner = self.inner.clone();
        self.rt.spawn_blocking(move || {
            let answer = inner
                .duplicate(CopyFrom::Folder(dir, base), &name, with_worlds)
                .map_err(|e| format!("{e:#}"));
            inner.created(answer, true);
        });
    }

    fn cancel_copy(&self) {
        self.inner
            .cancel_copy
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn delete_instance(&self, id: CustomId) {
        let instance = Instance::Custom(id);
        let refused = {
            let mut runs = self.inner.runs.lock().unwrap();
            if !self.inner.snap.lock().unwrap().customs.contains_key(&id) {
                Some(format!("there is no instance {id}"))
            } else if runs.contains_key(&instance) || self.inner.core.game_running(instance) {
                Some(format!("{id} is installing or running; stop it first"))
            } else if self.inner.copying.lock().unwrap().contains(&instance) {
                Some(format!("{id} is being copied; wait for the copy"))
            } else {
                // Taken like a Play, so none starts while the directory moves.
                runs.insert(instance, watch::channel(false).0);
                None
            }
        };
        if let Some(why) = refused {
            log(format_args!("delete refused: {why}"));
            let _ = self
                .inner
                .events
                .unbounded_send(Event::InstanceDeleted(Err(why)));
            return;
        }
        let inner = self.inner.clone();
        self.rt.spawn_blocking(move || {
            let answer = inner.delete(id).map_err(|e| format!("{e:#}"));
            inner.runs.lock().unwrap().remove(&instance);
            if let Err(e) = &answer {
                log(format_args!("{id}: not deleted: {e}"));
            }
            let _ = inner.events.unbounded_send(Event::InstanceDeleted(answer));
        });
    }

    fn add_instance_jar(&self, id: CustomId, path: std::path::PathBuf) {
        let inner = self.inner.clone();
        self.rt.spawn_blocking(move || {
            let answer = inner.add_jar(Instance::Custom(id), &path);
            let _ = inner
                .events
                .unbounded_send(Event::InstanceJarAdded(id, answer));
        });
    }

    fn open_game_folder(&self, instance: Instance) {
        // Opening creates the folder: never for a custom instance that is gone.
        if let Instance::Custom(id) = instance
            && !self.inner.snap.lock().unwrap().customs.contains_key(&id)
        {
            return;
        }
        let dir = self.inner.core.paths().game_dir(instance);
        log(format_args!(
            "opening {} in the file manager",
            dir.display()
        ));
        super::open_folder(&dir);
    }
}

impl RealBackend {
    /// Play for a mode or a custom instance: one per game directory, nothing for a custom
    /// instance that doesn't exist.
    fn start(&self, instance: Instance) {
        let clicked = Instant::now();
        let id = instance.id();
        if let Instance::Custom(c) = instance
            && !self.inner.snap.lock().unwrap().customs.contains_key(&c)
        {
            log(format_args!("{id}: play refused, no such instance"));
            return;
        }
        let (cancel, cancelled) = watch::channel(false);
        {
            let mut runs = self.inner.runs.lock().unwrap();
            // One Play per game directory: between one 1.16.1 mode's install and its game's
            // start, another's install would swap the shared mods/ and Atum settings under it.
            // The Play button waits too (`sibling_busy`).
            let dir = instance.dir_id();
            if let Some(other) = runs.keys().find(|i| i.dir_id() == dir) {
                if *other != instance {
                    log(format_args!("{id}: play refused, {} runs", other.id()));
                }
                return;
            }
            runs.insert(instance, cancel);
        }
        let before = self.inner.state(instance);
        log(format_args!("{id}: play"));
        self.inner
            .set_state(instance, ModeState::Installing(stage_only(Stage::Metadata)));
        self.rt.spawn(play(
            self.inner.clone(),
            instance,
            before,
            cancelled,
            clicked,
        ));
    }

    fn stop(&self, instance: Instance) {
        if let Some(cancel) = self.inner.runs.lock().unwrap().get(&instance) {
            log(format_args!("{}: cancel", instance.id()));
            cancel.send_replace(true);
        }
    }
}

/// An instance's state on start, from its install record and the files it names (no network).
fn local_snapshot(core: &Core, instance: Instance) -> ModeSnapshot {
    let (state, record) = match core.local_state(instance) {
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
}

impl Inner {
    fn state(&self, instance: Instance) -> ModeState {
        self.snap
            .lock()
            .unwrap()
            .instance(instance)
            .map_or(ModeState::NotInstalled, |s| s.state.clone())
    }

    fn busy(&self, instance: Instance) -> bool {
        self.runs.lock().unwrap().contains_key(&instance)
    }

    /// Sets the state and tells the UI, if it changed. The event is sent under the lock, so the
    /// UI sees the states in the order they were set.
    fn set_state(&self, instance: Instance, state: ModeState) {
        self.set_state_if(instance, |_| true, state);
    }

    fn set_state_if(
        &self,
        instance: Instance,
        when: impl FnOnce(&ModeState) -> bool,
        state: ModeState,
    ) {
        let mut snap = self.snap.lock().unwrap();
        let Some(slot) = snap.instance_mut(instance).map(|s| &mut s.state) else {
            return;
        };
        if *slot != state && when(slot) {
            *slot = state.clone();
            if let Some(ev) = state_event(instance, state) {
                let _ = self.events.unbounded_send(ev);
            }
        }
    }

    fn set_info(&self, instance: Instance, f: impl FnOnce(&mut ModeInfo)) {
        let mut snap = self.snap.lock().unwrap();
        let Some(info) = snap.instance_mut(instance).map(|s| &mut s.info) else {
            return;
        };
        let before = info.clone();
        f(info);
        if *info != before
            && let Some(ev) = info_event(instance, info.clone())
        {
            let _ = self.events.unbounded_send(ev);
        }
    }

    /// Ends a `play`: the instance is free again, then it shows `state`. In that order, so a
    /// Play pressed as soon as the UI shows the new state is not ignored.
    fn finish(&self, instance: Instance, state: ModeState) {
        self.runs.lock().unwrap().remove(&instance);
        self.set_state(instance, state);
    }

    /// Progress from the pipeline, throttled and with the parallel download stages combined.
    fn progress_sink(self: &Arc<Self>, instance: Instance) -> ProgressFn {
        let inner = self.clone();
        let mux = Mutex::new(Mux::default());
        Arc::new(move |p: Progress| {
            let shown = mux.lock().unwrap().update(p, Instant::now());
            if let Some(shown) = shown {
                inner.set_state(instance, ModeState::Installing(shown));
            }
        })
    }

    /// Writes the newest settings (blocking). Writes run one at a time and each takes the
    /// settings as they are when it runs, so the last one leaves the newest on disk.
    fn write_settings(&self) {
        let _one_at_a_time = self.settings_write.lock().unwrap();
        let latest = self.snap.lock().unwrap().settings.clone();
        if let Err(e) = latest.save(&self.settings_file) {
            log(format_args!(
                "could not save settings to {}: {e}",
                self.settings_file.display()
            ));
        }
    }

    /// Copies a jar the player picked into the store (blocking), for a mode's or a custom
    /// instance's Mods page.
    fn add_jar(
        &self,
        instance: Instance,
        path: &std::path::Path,
    ) -> Result<srl_mods::CustomJar, String> {
        let id = instance.id();
        let answer = self.core.add_custom_jar(path).map_err(|e| {
            log(format_args!("{id}: adding {}: {e:#}", path.display()));
            format!("{e:#}")
        });
        if let Ok(jar) = &answer {
            log(format_args!("{id}: added {} ({})", jar.file_name, jar.name));
        }
        answer
    }

    /// `create_instance` (blocking): reserves the directory, copies the settings asked for,
    /// then adds the definition.
    fn create(self: &Arc<Self>, spec: NewInstance) -> Result<Created> {
        let paths = self.core.paths();
        // Checked under the lock the definition is added under, so two creates with the same
        // name (a double click) can't both pass.
        let _one_at_a_time = self.new_ids.lock().unwrap();
        let name = self
            .snap
            .lock()
            .unwrap()
            .settings
            .validate_instance_name(&spec.name, None)
            .map_err(|p| anyhow::anyhow!("{p}"))?;
        let first = self.snap.lock().unwrap().settings.next_custom_id();
        let id = instances::reserve(paths, first)?;
        let dir = paths.game_dir(Instance::Custom(id));
        let mut skipped = Vec::new();
        if let Some(from) = spec.copy_settings_from {
            let src = paths.game_dir(from);
            skipped = CopyPlan::new(&src, CopyWhat::Settings).run(&src, &dir, |_, _| {});
            if let Err(e) = instances::fix_per_mode_files(paths, from, &dir) {
                instances::discard(paths, id);
                return Err(e);
            }
        }
        let def = CustomInstance {
            id,
            name,
            base: spec.base,
            memory_mb: None,
            mods: srl_mods::ModChoice::default(),
            jvm_args: Vec::new(),
            created_ms: now_ms(),
        };
        log(format_args!(
            "{id}: created \"{}\" on {}{}",
            def.name,
            def.base.id(),
            spec.copy_settings_from
                .map(|f| format!(", settings from {}", f.id()))
                .unwrap_or_default()
        ));
        Ok(self.add_custom(def, skipped))
    }

    /// `duplicate_instance` (blocking): the source's definition (a mode's memory and mods, a
    /// custom instance's whole definition), its directory copied with progress, then the new
    /// definition. A failed copy leaves nothing behind.
    fn duplicate(self: &Arc<Self>, from: CopyFrom, name: &str, worlds: bool) -> Result<Created> {
        let (name, base, memory_mb, mods, jvm_args) = {
            let snap = self.snap.lock().unwrap();
            let s = &snap.settings;
            let name = s
                .validate_instance_name(name, None)
                .map_err(|p| anyhow::anyhow!("{p}"))?;
            match &from {
                // Another launcher's instance: its files, and the base mode's mods and memory.
                CopyFrom::Folder(_, base) => (
                    name,
                    *base,
                    None,
                    srl_mods::ModChoice::default(),
                    Vec::new(),
                ),
                // The shared directory's copy plays like the mode it was duplicated from (Ranked
                // stays Ranked); the mode's own memory and mod changes come along.
                CopyFrom::Instance(Instance::Mode(m)) => (
                    name,
                    *m,
                    s.mode_memory_mb.get(m).copied(),
                    s.mod_choices.get(m).cloned().unwrap_or_default(),
                    Vec::new(),
                ),
                // Practice is a 1.16.1 game played like RSG; its fixed practice mods don't come
                // along (a custom instance starts from its base's set).
                CopyFrom::Instance(Instance::Practice) => (
                    name,
                    super::practice::TOOLS_LIKE,
                    None,
                    srl_mods::ModChoice::default(),
                    Vec::new(),
                ),
                CopyFrom::Instance(Instance::Custom(c)) => {
                    let def = s
                        .custom(*c)
                        .with_context(|| format!("there is no instance {c}"))?;
                    (
                        name,
                        def.base,
                        def.memory_mb,
                        def.mods.clone(),
                        def.jvm_args.clone(),
                    )
                }
            }
        };
        let paths = self.core.paths();
        let id = {
            let _one_at_a_time = self.new_ids.lock().unwrap();
            let first = self.snap.lock().unwrap().settings.next_custom_id();
            instances::reserve(paths, first)?
        };
        let src = match &from {
            CopyFrom::Instance(i) => paths.game_dir(*i),
            CopyFrom::Folder(dir, _) => dir.clone(),
        };
        let dir = paths.game_dir(Instance::Custom(id));
        let started = Instant::now();
        let plan = CopyPlan::new(&src, CopyWhat::Everything { worlds });
        let (files_total, bytes_total) = (plan.files(), plan.bytes());
        log(format_args!(
            "{id}: copying {} into \"{name}\" ({files_total} files, {} MB{})",
            src.display(),
            bytes_total / 1_000_000,
            if worlds { ", worlds too" } else { "" }
        ));
        let progress = |files, bytes| CopyProgress {
            id,
            name: name.clone(),
            files,
            files_total,
            bytes,
            bytes_total,
        };
        let _ = self
            .events
            .unbounded_send(Event::InstanceCopy(progress(0, 0)));
        let mut last = Instant::now();
        let copied = plan.run_until(
            &src,
            &dir,
            |files, bytes| {
                if files == files_total || last.elapsed() >= PROGRESS_EVERY {
                    last = Instant::now();
                    let _ = self
                        .events
                        .unbounded_send(Event::InstanceCopy(progress(files, bytes)));
                }
            },
            &self.cancel_copy,
        );
        let Some(skipped) = copied else {
            instances::discard(paths, id);
            log(format_args!("{id}: copy cancelled, removed"));
            anyhow::bail!(instances::COPY_CANCELLED);
        };
        if let CopyFrom::Instance(from) = from
            && let Err(e) = instances::fix_per_mode_files(paths, from, &dir)
        {
            instances::discard(paths, id);
            return Err(e);
        }
        for s in &skipped {
            log(format_args!("{id}: left out of the copy: {s}"));
        }
        log(format_args!(
            "{id}: copied in {:.1}s, {} left out",
            started.elapsed().as_secs_f64(),
            skipped.len()
        ));
        let def = CustomInstance {
            id,
            // Another instance may have taken the name while this one copied.
            name: self
                .snap
                .lock()
                .unwrap()
                .settings
                .unique_instance_name(&name),
            base,
            memory_mb,
            mods,
            jvm_args,
            created_ms: now_ms(),
        };
        Ok(self.add_custom(def, skipped))
    }

    /// Adds a new instance's definition and its snapshot (not installed), saves the settings,
    /// tells the UI the new list and state, and fills in its Mods page in the background.
    fn add_custom(self: &Arc<Self>, def: CustomInstance, skipped: Vec<String>) -> Created {
        let created = Created {
            id: def.id,
            name: def.name.clone(),
            skipped,
        };
        {
            let mut snap = self.snap.lock().unwrap();
            snap.settings.custom_instances.push(def);
            snap.customs.insert(
                created.id,
                ModeSnapshot {
                    state: ModeState::NotInstalled,
                    info: ModeInfo::default(),
                },
            );
            let list = snap.settings.custom_instances.clone();
            self.stats_follow(&list);
            let _ = self.events.unbounded_send(Event::CustomInstances(list));
            let _ = self
                .events
                .unbounded_send(Event::CustomState(created.id, ModeState::NotInstalled));
        }
        self.write_settings();
        self.rt
            .spawn(check_updates(self.clone(), pipeline::META_FRESH));
        created
    }

    /// Tells the UI how a create or duplicate ended.
    /// Statistics read the custom instances' worlds too, as runs of their base modes.
    fn stats_follow(&self, list: &[CustomInstance]) {
        let paths = self.core.paths();
        self.stats.set_instance_dirs(
            list.iter()
                .map(|c| (paths.game_dir(c.instance()), c.base))
                .collect(),
        );
    }

    fn created(&self, answer: Result<Created, String>, copy: bool) {
        if let Err(e) = &answer {
            log(format_args!("new instance failed: {e}"));
        }
        let _ = self.events.unbounded_send(if copy {
            Event::InstanceCopied(answer)
        } else {
            Event::InstanceCreated(answer)
        });
    }

    /// `delete_instance` (blocking, with the instance held in `runs`): the directory to the
    /// trash first, and only then the definition, so a directory that can't be moved leaves the
    /// instance as it was.
    fn delete(&self, id: CustomId) -> Result<CustomId> {
        let moved = instances::move_to_trash(self.core.paths(), id)?;
        log(format_args!(
            "{id}: deleted; its directory is {}",
            moved
                .as_ref()
                .map_or("gone already".to_string(), |p| format!(
                    "in {}",
                    p.display()
                ))
        ));
        {
            let mut snap = self.snap.lock().unwrap();
            snap.settings.custom_instances.retain(|c| c.id != id);
            snap.customs.remove(&id);
            let list = snap.settings.custom_instances.clone();
            self.stats_follow(&list);
            let _ = self.events.unbounded_send(Event::CustomInstances(list));
        }
        self.write_settings();
        Ok(id)
    }
}

/// Milliseconds since the Unix epoch, for `CustomInstance::created_ms`.
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// One Play, from the click to the game's exit.
async fn play(
    inner: Arc<Inner>,
    instance: Instance,
    before: ModeState,
    mut cancelled: watch::Receiver<bool>,
    clicked: Instant,
) {
    let id = instance.id().into_owned();
    let (rules, tools) = {
        let snap = inner.snap.lock().unwrap();
        (
            snap.settings.mod_rules(),
            snap.settings.tool_selection_of(instance),
        )
    };
    let prepared = tokio::select! {
        r = inner.core.prepare_instance(
            instance, &rules, &tools, Some(inner.progress_sink(instance)),
        ) => r,
        () = wait_cancel(&mut cancelled) => {
            log(format_args!("{id}: install cancelled"));
            inner.finish(instance, before);
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
            inner.finish(instance, ModeState::Failed(failure));
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
    inner.set_info(instance, |info| {
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
            || inner.set_state(instance, ModeState::Installing(stage_only(Stage::SigningIn))),
        ) => r,
        () = wait_cancel(&mut cancelled) => {
            log(format_args!("{id}: cancelled while signing in"));
            inner.finish(instance, ModeState::Ready);
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
            inner.finish(instance, ModeState::Failed(failure));
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

    inner.set_state(
        instance,
        ModeState::Installing(stage_only(Stage::Launching)),
    );
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
            inner.finish(instance, ModeState::Failed(failure));
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
    inner.set_info(instance, |info| info.launched = Some(game.launched.clone()));
    inner.set_state(instance, ModeState::Running);

    // Stop (cancel while running): ask the game to quit the way its window's close button does,
    // so it saves; SIGTERM, then a kill, only if it doesn't.
    let stopper = {
        let process = game.process.clone();
        let mut cancelled = cancelled.clone();
        let id = id.clone();
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
    inner.set_info(instance, |info| info.launched = None);
    inner.finish(instance, end);
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
    // Every mode, then every custom instance (against its base mode's set).
    let customs: Vec<Instance> = inner
        .snap
        .lock()
        .unwrap()
        .customs
        .keys()
        .map(|c| Instance::Custom(*c))
        .collect();
    for instance in Mode::ALL.into_iter().map(Instance::Mode).chain(customs) {
        if inner.busy(instance) {
            continue;
        }
        let id = instance.id();
        let pipeline::Sets { base, own, set } = match inner.core.sets(&meta, instance, &rules) {
            Ok(sets) => sets,
            Err(e) => {
                log(format_args!("update check: {id}: {e:#}"));
                continue;
            }
        };
        let catalogue = srl_mods::catalogue(&meta, &own, &set, base);
        let concerns = srl_mods::concerns(&meta, &set, base);
        let installed = matches!(
            inner.state(instance),
            ModeState::Ready | ModeState::UpdateAvailable
        );
        let plan = if installed {
            match inner.core.mods_plan(&set).await {
                Ok(p) => p,
                Err(e) => {
                    log(format_args!("update check: {id}: {e:#}"));
                    continue;
                }
            }
        } else {
            None
        };
        // A Play that started meanwhile installs the set itself.
        if inner.busy(instance) {
            continue;
        }
        let download = plan.as_ref().map(|p| inner.core.plan_download(p));
        inner.set_info(instance, |info| {
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
                    "update check: {id}: {} to add or update, {} to remove",
                    plan.add.len() + plan.update.len(),
                    plan.remove.len()
                ));
                inner.set_state_if(
                    instance,
                    |s| *s == ModeState::Ready,
                    ModeState::UpdateAvailable,
                );
            }
            None if installed => {
                inner.set_state_if(
                    instance,
                    |s| *s == ModeState::UpdateAvailable,
                    ModeState::Ready,
                );
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

    /// Waits for the first event `pick` takes, or panics after `secs`.
    fn next_event<T>(
        rx: &mut futures::channel::mpsc::UnboundedReceiver<Event>,
        secs: u64,
        mut pick: impl FnMut(Event) -> Option<T>,
    ) -> T {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            match rx.try_recv() {
                Ok(ev) => {
                    if let Some(t) = pick(ev) {
                        return t;
                    }
                }
                Err(futures::channel::mpsc::TryRecvError::Closed) => panic!("the backend is gone"),
                Err(futures::channel::mpsc::TryRecvError::Empty) => {
                    assert!(Instant::now() < deadline, "timed out");
                    thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }

    fn write(path: &std::path::Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// Create, duplicate, change and delete custom instances through the real backend, on a
    /// temp data folder and the built-in mod lists (no network, no game).
    #[test]
    fn custom_instances_from_create_to_trash() {
        let root = std::env::temp_dir().join(format!("srl-real-customs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = Paths::new(&root);
        // The shared directory as a player left it, last played as SSG.
        let shared = paths.instance(Mode::Rsg);
        let atum = srl_core::shared::PER_MODE_FILES[0];
        write(&shared.join(atum), "rsg");
        srl_core::shared::switch_to(&paths, Mode::Rsg).unwrap();
        srl_core::shared::switch_to(&paths, Mode::Ssg).unwrap();
        write(&shared.join(atum), "ssg seed");
        for f in [
            "options.txt",
            "config/sodium.json",
            "resourcepacks/p.zip",
            "saves/W/level.dat",
            "mods/sodium.jar",
            "logs/latest.log",
        ] {
            write(&shared.join(f), f);
        }
        let mut saved = Settings {
            player_name: Some("Violet_Runner".into()),
            ..Settings::default()
        };
        saved.set_memory_mb(Mode::Ssg, 4096);
        saved.save(&Settings::file(&paths)).unwrap();

        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let b = RealBackend::open(paths.clone(), tx, false).unwrap();
        b.rt.block_on(
            b.inner
                .core
                .seed_mod_meta(super::super::fake::fake_meta().clone()),
        );

        // Create with RSG's settings.
        b.create_instance(NewInstance {
            name: " Bastion grind ".into(),
            base: Mode::Rsg,
            copy_settings_from: Some(Instance::Mode(Mode::Rsg)),
        });
        let one = next_event(&mut rx, 10, |e| match e {
            Event::InstanceCreated(r) => Some(r.unwrap()),
            _ => None,
        });
        assert_eq!((one.id, one.name.as_str()), (CustomId(1), "Bastion grind"));
        let dir1 = paths.game_dir(Instance::Custom(one.id));
        assert!(dir1.join("options.txt").is_file() && dir1.join("resourcepacks/p.zip").is_file());
        assert!(!dir1.join("saves").exists() && !dir1.join("mods").exists());
        // RSG's own Atum settings, not SSG's that were in place.
        assert_eq!(std::fs::read_to_string(dir1.join(atum)).unwrap(), "rsg");
        let snap = b.snapshot();
        assert_eq!(snap.customs[&one.id].state, ModeState::NotInstalled);
        assert_eq!(snap.settings.custom(one.id).unwrap().base, Mode::Rsg);
        // The update check fills in its Mods page from the mod lists.
        let info = next_event(&mut rx, 10, |e| match e {
            Event::CustomInfo(id, info) if id == one.id && info.catalogue.is_some() => Some(info),
            _ => None,
        });
        assert!(info.mods.unwrap() > 0);

        // A taken name fails at once.
        b.create_instance(NewInstance {
            name: "BASTION GRIND".into(),
            base: Mode::Fsg,
            copy_settings_from: None,
        });
        let err = next_event(&mut rx, 10, |e| match e {
            Event::InstanceCreated(r) => Some(r.unwrap_err()),
            _ => None,
        });
        assert!(err.contains("another instance"), "{err}");

        // Duplicate SSG with its worlds: SSG's memory and Atum settings come along.
        b.duplicate_instance(Instance::Mode(Mode::Ssg), "Seeds".into(), true);
        let mut progress = 0;
        let two = next_event(&mut rx, 20, |e| match e {
            Event::InstanceCopy(p) => {
                assert_eq!(p.files_total, 5);
                progress += 1;
                None
            }
            Event::InstanceCopied(r) => Some(r.unwrap()),
            _ => None,
        });
        assert!(progress >= 2, "a start and an end");
        assert_eq!(two.id, CustomId(2));
        assert!(two.skipped.is_empty(), "{:?}", two.skipped);
        let dir2 = paths.game_dir(Instance::Custom(two.id));
        assert!(dir2.join("saves/W/level.dat").is_file());
        assert!(!dir2.join("mods").exists() && !dir2.join("logs").exists());
        assert!(!dir2.join(".srl-modes").exists());
        assert_eq!(
            std::fs::read_to_string(dir2.join(atum)).unwrap(),
            "ssg seed"
        );
        let def = b.snapshot().settings.custom(two.id).cloned().unwrap();
        assert_eq!((def.base, def.memory_mb), (Mode::Ssg, Some(4096)));

        // Older settings from the UI (no custom instances, a rename the other way): the
        // backend keeps its list and says so.
        let mut ui = saved.clone();
        ui.language = super::super::Language::Pl;
        b.set_settings(ui);
        let list = next_event(&mut rx, 10, |e| match e {
            Event::CustomInstances(list) => Some(list),
            _ => None,
        });
        assert_eq!(list.len(), 2);
        let mut ui = b.snapshot().settings;
        assert_eq!(ui.language, super::super::Language::Pl);
        ui.rename_custom(two.id, "Set seeds").unwrap();
        b.set_settings(ui);
        assert_eq!(
            b.snapshot().settings.custom(two.id).unwrap().name,
            "Set seeds"
        );

        // Delete: the directory goes to the trash, the definition and its state go.
        b.delete_instance(one.id);
        let deleted = next_event(&mut rx, 10, |e| match e {
            Event::InstanceDeleted(r) => Some(r),
            _ => None,
        });
        assert_eq!(deleted, Ok(one.id));
        assert!(!dir1.exists());
        let trashed: Vec<_> = std::fs::read_dir(paths.trash())
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(trashed.len(), 1);
        assert!(trashed[0].path().join("options.txt").is_file());
        let snap = b.snapshot();
        assert!(!snap.customs.contains_key(&one.id));
        assert!(snap.settings.custom(one.id).is_none());
        b.delete_instance(one.id);
        let again = next_event(&mut rx, 10, |e| match e {
            Event::InstanceDeleted(r) => Some(r),
            _ => None,
        });
        assert!(again.is_err());

        // The settings file has what the backend has, once its writes are done.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let on_disk = Settings::load(&Settings::file(&paths)).unwrap();
            if on_disk.custom_instances == b.snapshot().settings.custom_instances {
                assert_eq!(on_disk.custom_instances.len(), 1);
                break;
            }
            assert!(Instant::now() < deadline, "settings never written");
            thread::sleep(Duration::from_millis(20));
        }
        // A new instance never gets a number whose directory is still there.
        std::fs::create_dir_all(paths.game_dir(Instance::Custom(CustomId(3)))).unwrap();
        b.create_instance(NewInstance {
            name: "Next".into(),
            base: Mode::Draftout,
            copy_settings_from: None,
        });
        let next = next_event(&mut rx, 10, |e| match e {
            Event::InstanceCreated(r) => Some(r.unwrap()),
            _ => None,
        });
        assert_eq!(next.id, CustomId(4));
        drop(b);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// Another launcher's instance comes over as a custom instance: its files without its mods
    /// and logs, worlds only when asked; the launcher's own folders are refused.
    #[test]
    fn another_launchers_instance_is_imported() {
        let root = std::env::temp_dir().join(format!("srl-real-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = Paths::new(root.join("data"));
        let theirs = root.join("MultiMC/instances/RSG/.minecraft");
        for f in [
            "options.txt",
            "config/atum.json",
            "saves/W/level.dat",
            "mods/cheat.jar",
            "logs/latest.log",
        ] {
            write(&theirs.join(f), f);
        }
        Settings {
            player_name: Some("Violet_Runner".into()),
            ..Settings::default()
        }
        .save(&Settings::file(&paths))
        .unwrap();
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let b = RealBackend::open(paths.clone(), tx, false).unwrap();

        b.import_instance(theirs.clone(), Mode::Fsg, "RSG".into(), false);
        let made = next_event(&mut rx, 10, |e| match e {
            Event::InstanceCopied(r) => Some(r.unwrap()),
            _ => None,
        });
        let dir = paths.game_dir(Instance::Custom(made.id));
        assert!(dir.join("options.txt").exists());
        assert!(dir.join("config/atum.json").exists());
        assert!(!dir.join("saves").exists(), "no worlds unless asked");
        assert!(!dir.join("mods").exists(), "mods come from the launcher");
        assert!(!dir.join("logs").exists());
        assert_eq!(
            b.snapshot().settings.custom(made.id).unwrap().base,
            Mode::Fsg
        );

        b.import_instance(theirs.clone(), Mode::Fsg, "RSG 2".into(), true);
        let two = next_event(&mut rx, 10, |e| match e {
            Event::InstanceCopied(r) => Some(r.unwrap()),
            _ => None,
        });
        assert!(
            paths
                .game_dir(Instance::Custom(two.id))
                .join("saves/W/level.dat")
                .exists()
        );

        b.import_instance(
            paths.game_dir(Instance::Custom(two.id)),
            Mode::Fsg,
            "Own".into(),
            true,
        );
        let err = next_event(&mut rx, 10, |e| match e {
            Event::InstanceCopied(r) => Some(r.unwrap_err()),
            _ => None,
        });
        assert!(err.contains("own"), "{err}");
        drop(b);
        std::fs::remove_dir_all(&root).unwrap();
    }

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
