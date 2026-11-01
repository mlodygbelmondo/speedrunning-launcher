//! What Play does, as plain async functions over `srl-game` and `srl-mods`:
//!
//! 1. refresh the metadata (mcsr-meta and Modrinth here; Mojang, Fabric and Java lists inside
//!    `GameInstaller::install`);
//! 2. install the game: Java, Minecraft, Fabric Loader, libraries, natives, assets;
//! 3. install the mode's mods, never while that mode's game runs, then the tools the mode starts
//!    with (srl-tools, `Stage::Tools`);
//! 4. start the game with its account (the offline name, or the active Microsoft account signed in
//!    by `account::launch_account` first) and the settings, through the tools' launch hook
//!    (waywall wraps the command, Toolscreen is injected, Ninjabrain Bot starts next to it), and
//!    watch it reach the title screen (`Game::startup`). The tools started for it stop when it
//!    exits.
//!
//! `RealBackend` runs these on its tokio runtime for the window, and the headless CLI (`cli.rs`)
//! runs the very same functions, so CI exercises exactly the path the Play button takes.
//!
//! After a successful install the mode gets a small record (`meta/launcher/<mode>.json`) with what
//! was installed. On start the launcher only stats the files it names (`Core::local_state`), so it
//! can show Ready without any network; update checks run afterwards in the background.
//!
//! The practice instance goes through the same steps (`Core::prepare_instance` with
//! `Instance::Practice`, then `Core::launch`): its own game directory, log, pid file and record,
//! the game and mods `practice.rs` names, and the one account and tools logic of the modes.
//!
//! So does a custom instance (`Instance::Custom`): its base mode's game (`srl_game::profile`),
//! its base mode's legal set with the instance's own changes (`ModRules::choice_of`), its base
//! mode's tools, and its own directory, log, pid file, record, memory and JVM arguments. It
//! never touches the shared directory's per-mode Atum settings.
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use srl_core::{
    Downloader, Instance, Mode, OfflineAccount, Paths, Progress, ProgressFn, Stage, Unit,
};
use srl_game::{
    GameAccount, GameExit, GameInstaller, GameProcess, Installed, LaunchOptions, READY_MARKERS,
};
use srl_mods::{
    GameToken, InstallReport, InstanceGuard, ModMeta, ModSet, Os, Plan, ResolveOptions,
};
use srl_tools::{ToolId, ToolNote, ToolSelection, ToolSession, Tools};

use super::{FailDetail, FailKind, Launched, ModRules, Settings};

/// How old the mod lists may be before Play fetches them again. The update check on start
/// refreshes them, so a Play soon after start (or a second Play) needs no network for them. The
/// game's own lists use srl-game's default (an hour).
pub const META_FRESH: Duration = Duration::from_secs(10 * 60);

/// A game that logs none of the `READY_MARKERS` this long is taken as started, so a crash much
/// later isn't reported as a failure to start.
pub const STARTUP_LIMIT: Duration = Duration::from_secs(300);

/// Lines Fabric Loader logs when the game can't start: a mod threw during startup, or the mods
/// don't fit together. Fabric then shows its own error window and the process stays alive, so
/// waiting for an exit would wait until `STARTUP_LIMIT`.
const CRASH_MARKERS: &[&str] = &["A mod crashed on startup!", "Incompatible mods found!"];

/// The name the game gets when no player name is saved (the CLI on a fresh machine).
pub const DEFAULT_PLAYER: &str = "Player";

/// `SRL_DEV_GAME_HOME=<folder>`: test runs start every game (modes and practice) with
/// `-Duser.home=<folder>`, so SpeedRunIGT keeps its records (`~/speedrunigt`) out of the real home
/// folder. Only that one property: the launcher strips JVM options from the environment on purpose
/// (srl-game's `SCRUBBED_ENV`, speedrun rule A.8.19), and this must not become a way around it.
/// The modes have no JVM arguments setting (custom instances do, checked by
/// `validate_jvm_args`); this is for tests only.
pub const DEV_GAME_HOME_ENV: &str = "SRL_DEV_GAME_HOME";

/// The offline account a game uses when no Microsoft account plays (`account::play_as`, the one
/// place that choice is made, for modes and practice alike): the saved name, else
/// `DEFAULT_PLAYER`.
pub fn offline_account(settings: &Settings) -> OfflineAccount {
    let name = settings
        .player_name
        .clone()
        .filter(|n| OfflineAccount::is_valid_name(n))
        .unwrap_or_else(|| DEFAULT_PLAYER.to_string());
    OfflineAccount::new(&name)
}

/// Everything Play needs, shared by every mode. One per process.
pub struct Core {
    paths: Paths,
    downloader: Downloader,
    installer: GameInstaller,
    /// Keeps a mods install and a running game of the same mode apart (srl-mods).
    guard: InstanceGuard,
    /// One install at a time. Modes share libraries, assets, Java and mod jars by path, and two
    /// downloads of the same file at once would write the same `.part` file.
    install_lock: tokio::sync::Mutex<()>,
    /// The mod lists, and when they were last fetched. Behind an async lock so that a Play that
    /// starts while the update check is fetching waits for that fetch instead of starting another.
    mod_meta: tokio::sync::Mutex<Option<(Instant, Arc<ModMeta>)>>,
    /// Ninjabrain Bot, Toolscreen and waywall. One per process: it knows which tool processes it
    /// started.
    tools: Tools,
    /// One tool install at a time (the Tools page and Play's `Stage::Tools` write the same files).
    tool_lock: tokio::sync::Mutex<()>,
}

/// The outcome of the install steps: what the game will run with.
pub struct Prepared {
    pub installed: Installed,
    pub mods: ModSet,
    pub report: InstallReport,
    pub record: InstallRecord,
    /// Offline fallbacks and left-out mods, for the log.
    pub notes: Vec<String>,
    /// The tools the game starts with (from the settings), installed in `Stage::Tools`.
    pub tools: ToolSelection,
    /// Tools that could not be installed (they are left out of the launch, the game starts).
    pub tool_notes: Vec<ToolNote>,
    /// The mode whose rules the game follows: its tools' rules (a mode's own, practice's
    /// `practice::TOOLS_LIKE`, a custom instance's base).
    pub rules_mode: Mode,
}

/// [`Core::sets`]: an instance's base mode, that mode's own set and the set Play installs.
pub struct Sets {
    pub base: Mode,
    pub own: ModSet,
    pub set: ModSet,
}

/// What an install left on disk, so the next start can tell Ready from NotInstalled with a few
/// stats (`Core::local_state`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecord {
    pub minecraft_version: String,
    pub version_id: String,
    pub fabric_loader: String,
    pub java_version: String,
    pub java_executable: PathBuf,
    pub mods: Vec<RecordedJar>,
    /// Bytes on disk of everything the mode runs from, shared files included.
    pub disk: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedJar {
    pub file_name: String,
    pub size: u64,
}

/// What a mode looks like on disk, from `Core::local_state`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Local {
    NotInstalled,
    /// Everything the record names is in place.
    Ready(InstallRecord),
    /// Installed once, but a mod jar is missing or changed: Play repairs it.
    Damaged(InstallRecord),
}

impl Core {
    pub fn new(paths: Paths) -> Result<Self> {
        let downloader = Downloader::new()?;
        Ok(Self {
            installer: GameInstaller::new(paths.clone(), downloader.clone()),
            tools: Tools::new(paths.clone(), downloader.clone()),
            paths,
            downloader,
            guard: InstanceGuard::new(),
            install_lock: tokio::sync::Mutex::new(()),
            mod_meta: tokio::sync::Mutex::new(None),
            tool_lock: tokio::sync::Mutex::new(()),
        })
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn tools(&self) -> &Tools {
        &self.tools
    }

    /// The one downloader (its connection pool and retry rules), for srl-practice and the stats
    /// APIs: its User-Agent is what the MCSR Ranked API asks callers to send.
    pub fn downloader(&self) -> &Downloader {
        &self.downloader
    }

    /// Installs or updates one tool (the Tools page), with its hashes checked.
    pub async fn install_tool(&self, tool: ToolId, progress: Option<ProgressFn>) -> Result<()> {
        let _one_at_a_time = self.tool_lock.lock().await;
        self.tools.install(tool, progress).await
    }

    /// The mod lists, fetched again when the copy in memory is older than `max_age`
    /// (`Duration::ZERO` always fetches). Offline, srl-mods falls back to the cached files and
    /// says so in `ModMeta::notes`.
    pub async fn mod_meta(&self, max_age: Duration) -> Result<Arc<ModMeta>> {
        let mut slot = self.mod_meta.lock().await;
        if let Some((at, meta)) = slot.as_ref()
            && at.elapsed() < max_age
        {
            return Ok(meta.clone());
        }
        let meta = Arc::new(ModMeta::load(&self.paths, &self.downloader, true).await?);
        *slot = Some((Instant::now(), meta.clone()));
        Ok(meta)
    }

    /// Puts mod lists in memory as if just fetched, so tests resolve without the network.
    #[cfg(test)]
    pub async fn seed_mod_meta(&self, meta: ModMeta) {
        *self.mod_meta.lock().await = Some((Instant::now(), Arc::new(meta)));
    }

    /// The mode's own mod set for this OS, with the optional mods `opts` asks for, before the
    /// player's changes.
    pub fn mode_set(&self, meta: &ModMeta, mode: Mode, opts: &ResolveOptions) -> Result<ModSet> {
        srl_mods::resolve(meta, mode, Os::current(), opts)
    }

    /// The set Play installs for the mode: its own set with the optional mods and the player's
    /// changes from the Mods page (`Settings::mod_rules`).
    pub fn resolve(&self, meta: &ModMeta, mode: Mode, rules: &ModRules) -> Result<ModSet> {
        let own = self.mode_set(meta, mode, &rules.opts)?;
        Ok(srl_mods::apply(meta, own, &rules.choice(mode)))
    }

    /// [`Core::resolve`] for any instance: the practice instance's set is its explicit list
    /// (`practice::mod_set`), a custom instance's its base mode's with its own changes.
    pub fn resolve_instance(
        &self,
        meta: &ModMeta,
        instance: Instance,
        rules: &ModRules,
    ) -> Result<ModSet> {
        match instance {
            Instance::Mode(mode) => self.resolve(meta, mode, rules),
            Instance::Practice => super::practice::mod_set(meta, rules.opts.accessibility),
            Instance::Custom(_) => Ok(self.sets(meta, instance, rules)?.set),
        }
    }

    /// A mode's or a custom instance's sets: the base mode's own (with the optional mods), and
    /// the one Play installs (with the instance's changes, for its own `mods/`). The update
    /// check builds the Mods page's catalogue and concerns from both.
    pub fn sets(&self, meta: &ModMeta, instance: Instance, rules: &ModRules) -> Result<Sets> {
        let base = rules
            .base(instance)
            .with_context(|| format!("{} has no mode set of its own", instance.id()))?;
        let own = self.mode_set(meta, base, &rules.opts)?;
        let mut set = srl_mods::apply(meta, own.clone(), &rules.choice_of(instance));
        // srl-mods resolved it as the base mode; its files go to the instance's own directory.
        set.instance = instance;
        Ok(Sets { base, own, set })
    }

    /// Steps 1 to 3 of Play for a mode, the practice instance or a custom instance: metadata,
    /// game, mods, then the `tools` the game starts with. Idempotent and fast when everything is
    /// in place (a stat per game file, a hash of the mod jars and tool files). Reports
    /// `Metadata`, then `Java`, `Game` and `Assets` (in parallel), then `Mods`, then `Tools`
    /// when one downloads. A tool that can't be installed never fails Play: it comes back in
    /// `Prepared::tool_notes` and the game starts without it. Installs of every instance share
    /// `install_lock`: they use the same Java, libraries and assets.
    pub async fn prepare_instance(
        &self,
        instance: Instance,
        rules: &ModRules,
        tools: &ToolSelection,
        progress: Option<ProgressFn>,
    ) -> Result<Prepared> {
        // Any game of the directory: RSG can't swap the mods under a running SSG game.
        for i in instance.siblings() {
            if let Some(pid) = self.earlier_game(i) {
                bail!(
                    "Minecraft ({}) from an earlier launcher session is still running (pid {pid}); \
                     close it, then try again",
                    i.id()
                );
            }
        }
        let report = |stage: Stage| {
            if let Some(f) = &progress {
                f(Progress {
                    stage,
                    done: 0,
                    total: 0,
                    unit: Unit::Files,
                })
            }
        };
        // Queued behind another mode's install: say so, rather than "checking versions" for as
        // long as the other one takes.
        let _one_at_a_time = match self.install_lock.try_lock() {
            Ok(held) => held,
            Err(_) => {
                report(Stage::Waiting);
                self.install_lock.lock().await
            }
        };
        report(Stage::Metadata);

        let meta = self
            .mod_meta(META_FRESH)
            .await
            .context("loading the mod lists")?;
        let mut notes = meta.notes.clone();
        let rules_mode = match instance {
            Instance::Practice => super::practice::TOOLS_LIKE,
            _ => rules
                .base(instance)
                .with_context(|| format!("there is no instance {}", instance.id()))?,
        };
        let target = match instance {
            Instance::Mode(mode) => srl_game::Target::mode(mode),
            Instance::Practice => super::practice::target()?,
            // The base mode's game, in the instance's own directory.
            Instance::Custom(_) => srl_game::Target {
                instance,
                profile: srl_game::profile(rules_mode),
            },
        };
        let installed = self
            .installer
            .install_target(&target, progress.clone())
            .await
            .context("installing Minecraft")?;
        notes.extend(installed.warnings.iter().cloned());
        let mods = self.resolve_instance(&meta, instance, rules)?;
        notes.extend(mods.notes.iter().map(|n| n.to_string()));
        let report = srl_mods::install(
            &self.paths,
            &self.downloader,
            &mods,
            &self.guard,
            progress.clone(),
        )
        .await
        .context("installing the mods")?;
        if instance != Instance::Practice && rules_mode == Mode::Draftout {
            seed_options(&installed.game_dir).await?;
        }
        // The shared directory: this mode's Atum settings in place (`srl_core::shared`).
        if let Instance::Mode(mode) = instance
            && mode.is_shared()
        {
            let paths = self.paths.clone();
            tokio::task::spawn_blocking(move || srl_core::shared::switch_to(&paths, mode))
                .await?
                .context("putting the mode's own settings in place")?;
        }
        let record = self.write_record(&installed, &mods).await?;
        let tool_notes = {
            let _one_at_a_time = self.tool_lock.lock().await;
            self.tools
                .install_selected(rules_mode, tools, progress.clone())
                .await
        };
        Ok(Prepared {
            installed,
            mods,
            report,
            record,
            notes,
            tools: tools.clone(),
            tool_notes,
            rules_mode,
        })
    }

    /// Step 4: starts the game as `account` (`offline_account`, or the session
    /// `account::launch_account` signed in). The mode (or the practice instance) counts as
    /// running (no mods install, no map changes) until the returned `Game` is waited for or
    /// dropped.
    pub async fn launch(
        &self,
        prepared: &Prepared,
        settings: &Settings,
        account: GameAccount,
    ) -> Result<Game> {
        let instance = prepared.installed.instance;
        let rules = prepared.rules_mode;
        let token = self.guard.game_started(instance)?;
        let name = account.name().to_string();
        let microsoft = account.is_microsoft();
        let mut opts = LaunchOptions::with_profile(&prepared.installed.profile, account);
        // The mode's own setting (practice: RSG's; a custom instance's own, else its base's),
        // never below what the game needs (Draftout: 3 GB).
        opts.max_memory_mb = settings
            .memory_mb_of(instance)
            .max(prepared.installed.profile.min_memory_mb);
        // The tools' launch hook: waywall may wrap the command; what can't run is left out with
        // a note and the game starts anyway.
        let mut command = self
            .installer
            .command(&prepared.installed, &opts)
            .context("starting Minecraft")?;
        // Right before the main class, not in `extra_jvm_args`: those would replace the
        // version's default JVM arguments (Draftout's ZGC flags).
        // A custom instance's own arguments go there too, after the version's, so they win.
        let main_class = prepared.installed.version.main_class.as_deref();
        let mut jvm_args = settings.jvm_args_of(instance);
        if let Err(e) = super::validate_jvm_args(&jvm_args) {
            bail!("the instance's JVM arguments can't be used: {e}");
        }
        if let Some(home) = std::env::var_os(DEV_GAME_HOME_ENV).filter(|v| !v.is_empty()) {
            jvm_args.push(format!("-Duser.home={}", home.to_string_lossy()));
        }
        if !jvm_args.is_empty()
            && let Some(at) = main_class.and_then(|m| command.args.iter().position(|a| a == m))
        {
            command.args.splice(at..at, jvm_args);
        }
        // What the log may show of the command: the game's arguments, the token masked.
        let shown = command.redacted_args();
        let game_args = match main_class.and_then(|m| shown.iter().position(|a| a == m)) {
            Some(at) => shown[at + 1..].to_vec(),
            None => Vec::new(),
        };
        let hooked = self.tools.prepare(rules, command, &prepared.tools).await;
        let with = hooked.tools.clone();
        let process = self
            .installer
            .spawn(instance, &hooked.command)
            .await
            .context("starting Minecraft")?;
        if let Instance::Mode(mode) = instance
            && mode.is_shared()
            && let Err(e) = srl_core::shared::record_session(&self.paths, mode)
        {
            eprintln!("could not note the {} session: {e:#}", mode.id());
        }
        let pid_file = self.pid_file(instance);
        let stamp = PidFile {
            pid: process.pid(),
            started: unix_now(),
            game_dir: prepared.installed.game_dir.clone(),
        };
        if let Err(e) = write_json(&pid_file, &stamp).await {
            eprintln!("could not write {}: {e:#}", pid_file.display());
        }
        // Toolscreen into the game, Ninjabrain Bot next to it (unless the player runs one).
        let session = self
            .tools
            .start(hooked, &process, &prepared.installed.java.executable)
            .await;
        let tool_notes = merge_notes(&prepared.tool_notes, &session.notes);
        let failed = |t: &ToolId| {
            session
                .notes
                .iter()
                .any(|n| matches!(n, ToolNote::Failed(f, _) if f == t))
        };
        let tools = with.into_iter().filter(|t| !failed(t)).collect();
        Ok(Game {
            process,
            token: Some(token),
            pid_file,
            launched: Launched {
                player: name,
                memory_mb: opts.max_memory_mb,
                tools,
                tool_notes,
                microsoft,
                valid_until: None,
            },
            game_args,
            session: Some(session),
            account_in_use: None,
        })
    }

    /// What the mode looks like on disk, from its install record and a stat per file it names.
    /// No network and no hashing, so the launcher can call it for every mode before its window
    /// opens.
    pub fn local_state(&self, instance: impl Into<Instance>) -> Local {
        let instance = instance.into();
        let Some(record) = std::fs::read(self.record_file(instance))
            .ok()
            .and_then(|b| serde_json::from_slice::<InstallRecord>(&b).ok())
        else {
            return Local::NotInstalled;
        };
        let version_json = self
            .paths
            .version_dir(&record.version_id)
            .join(format!("{}.json", record.version_id));
        let game_files = record.java_executable.is_file()
            && version_json.is_file()
            && self.stamp_file(instance).is_file();
        if !game_files {
            return Local::NotInstalled;
        }
        // Another mode of the shared directory has its set in place: Play swaps it in from the
        // store, which is no repair.
        let elsewhere = srl_mods::owner(&self.paths, instance).is_some_and(|o| o != instance);
        let mods = srl_mods::mods_dir(&self.paths, instance);
        let mods_ok = elsewhere
            || record.mods.iter().all(|j| {
                std::fs::metadata(mods.join(&j.file_name))
                    .is_ok_and(|m| m.is_file() && m.len() == j.size)
            });
        if mods_ok {
            Local::Ready(record)
        } else {
            Local::Damaged(record)
        }
    }

    /// Copies the player's jar into the content store (`store/sha512/…`), where `install` links
    /// it from like any other jar, so the player may move or delete the original. Only a Fabric
    /// mod (a zip with `fabric.mod.json`) of at most `MAX_CUSTOM_JAR` bytes; its name comes from
    /// there. Blocking: call it off the UI thread.
    pub fn add_custom_jar(&self, path: &Path) -> Result<srl_mods::CustomJar> {
        add_custom_jar(&self.paths, path)
    }

    /// Compares the mode's `mods/` with `set` (hashes the jars, no downloads). `None` when
    /// nothing has to change.
    pub async fn mods_plan(&self, set: &ModSet) -> Result<Option<Plan>> {
        Ok(match srl_mods::status(&self.paths, set).await? {
            srl_mods::Status::UpToDate => None,
            srl_mods::Status::UpdateAvailable(plan)
            | srl_mods::Status::IllegalJarPresent { plan, .. } => Some(plan),
        })
    }

    /// Bytes of `plan` that aren't in the content store yet, i.e. what Play would download.
    pub fn plan_download(&self, plan: &Plan) -> u64 {
        plan.add
            .iter()
            .chain(plan.update.iter().map(|u| &u.to))
            .filter(|m| !self.paths.store_sha512(&m.sha512).is_file())
            .map(|m| m.size)
            .sum()
    }

    fn record_file(&self, instance: Instance) -> PathBuf {
        self.paths
            .meta()
            .join("launcher")
            .join(format!("{}.json", instance.id()))
    }

    /// srl-game's verification record of the instance's game files.
    fn stamp_file(&self, instance: Instance) -> PathBuf {
        self.paths
            .meta()
            .join("verified")
            .join(format!("{}.json", instance.id()))
    }

    fn pid_file(&self, instance: Instance) -> PathBuf {
        self.paths.logs().join(format!("{}.pid", instance.id()))
    }

    /// Whether a game of `instance` runs, started by this session or an earlier one. The import
    /// leaves such an instance alone: the game writes its options.txt again when it quits.
    pub fn game_running(&self, instance: Instance) -> bool {
        self.guard.is_game_running(instance) || self.earlier_game(instance).is_some()
    }

    /// The pid of a game of `instance` left running by an earlier launcher session (this
    /// session's games are covered by the `InstanceGuard`).
    pub fn earlier_game(&self, instance: Instance) -> Option<u32> {
        if self.guard.is_game_running(instance) {
            return None;
        }
        let stamp: PidFile =
            serde_json::from_slice(&std::fs::read(self.pid_file(instance)).ok()?).ok()?;
        is_our_game(&stamp).then_some(stamp.pid)
    }

    /// At launcher start: takes over the tools an earlier launcher session left running (it was
    /// closed or crashed while a game ran). They stop once the games of that session are gone,
    /// or now if none runs. Returns what it did, for the log.
    pub async fn adopt_earlier_tools(&self) -> Vec<(ToolId, srl_tools::Adopted)> {
        // The games of the earlier session, as their pid files say now (every instance's,
        // custom ones included); this session's games write theirs later and don't count.
        let stamps: Vec<PidFile> = std::fs::read_dir(self.paths.logs())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let instance = Instance::from_id(name.strip_suffix(".pid")?)?;
                serde_json::from_slice(&std::fs::read(self.pid_file(instance)).ok()?).ok()
            })
            .collect();
        self.tools
            .adopt_earlier(move || stamps.iter().any(is_our_game))
            .await
    }

    /// Writes the install record. The disk usage is walked again only when what is installed
    /// changed, so a repeat Play doesn't pay for it.
    async fn write_record(&self, installed: &Installed, mods: &ModSet) -> Result<InstallRecord> {
        let mut record = InstallRecord {
            minecraft_version: installed.minecraft_version.clone(),
            version_id: installed.version_id.clone(),
            fabric_loader: installed.fabric_loader.clone(),
            java_version: installed.java.version.clone(),
            java_executable: installed.java.executable.clone(),
            mods: mods
                .mods
                .iter()
                .map(|m| RecordedJar {
                    file_name: m.file_name.clone(),
                    size: m.size,
                })
                .collect(),
            disk: 0,
        };
        let path = self.record_file(installed.instance);
        let previous = tokio::fs::read(&path)
            .await
            .ok()
            .and_then(|b| serde_json::from_slice::<InstallRecord>(&b).ok());
        if let Some(p) = previous {
            record.disk = p.disk;
            if p.disk > 0 && p == record {
                return Ok(p);
            }
        }
        let (installed, paths) = (installed.clone(), self.paths.clone());
        record.disk = tokio::task::spawn_blocking(move || disk_usage(&installed, &paths)).await?;
        write_json(&path, &record).await?;
        Ok(record)
    }
}

/// A started game. Waiting for it (or dropping it) marks the mode as no longer running.
pub struct Game {
    pub process: GameProcess,
    token: Option<GameToken>,
    pid_file: PathBuf,
    /// The name, memory and tools it was started with.
    pub launched: Launched,
    /// The tools started for it; they stop when it exits.
    session: Option<ToolSession>,
    /// The game's arguments with the access token masked (`LaunchCommand::redacted_args`), for
    /// the log.
    pub game_args: Vec<String>,
    /// srl-auth's guard for the Microsoft account the game plays as: its background refresh
    /// leaves the account alone until the game has exited.
    pub account_in_use: Option<srl_auth::InUse>,
}

/// How a game's start went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Startup {
    /// The log shows the title screen loading, this long after the process started.
    Ready(Duration),
    /// Fabric Loader reported that the game can't start, for this reason (from the log). The
    /// process is still there, showing Fabric's error window; the caller ends it.
    Crashed(String),
    /// The game exited before either.
    Exited(GameExit),
    /// None of that within the time limit.
    TimedOut,
}

impl Game {
    /// Waits until the game's log shows the title screen loading (`READY_MARKERS`), or the game
    /// exits first, or `limit` passes. Reads only what the log gained since the last look, four
    /// times a second, and only during startup.
    pub async fn startup(&self, limit: Duration) -> Startup {
        let started = Instant::now();
        let mut log = LogScan::default();
        let path = self.process.log_path();
        loop {
            // Look at the log before the exit, so a marker written right before a quit counts.
            let exit = self.process.exit();
            match log.scan(path).await {
                Some(Seen::Ready) => return Startup::Ready(started.elapsed()),
                Some(Seen::Crashed) => {
                    // The reason follows the marker; give the game a moment to write all of it.
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    let text = tokio::fs::read(path).await.unwrap_or_default();
                    return Startup::Crashed(crash_reason(&String::from_utf8_lossy(&text)));
                }
                None => {}
            }
            if let Some(exit) = exit {
                return Startup::Exited(exit);
            }
            if started.elapsed() > limit {
                return Startup::TimedOut;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// Waits for the game to exit and for the tools started for it to close, then releases the
    /// mode.
    pub async fn wait(mut self) -> GameExit {
        let exit = self.process.wait().await;
        if let Some(session) = self.session.take() {
            // A tool gets srl-tools' grace to close before it is killed; a little more here, so
            // a stuck tool never keeps the mode from being Ready.
            let limit = srl_tools::TOOL_STOP_GRACE + Duration::from_secs(3);
            let _ = tokio::time::timeout(limit, session.finished()).await;
        }
        self.release().await;
        exit
    }

    async fn release(&mut self) {
        let _ = tokio::fs::remove_file(&self.pid_file).await;
        self.token = None;
        self.account_in_use = None;
    }
}

/// The notes of one launch: the install's (a tool that failed to download) and the launch hook's,
/// without a second note for a tool the install already explains.
pub fn merge_notes(install: &[ToolNote], launch: &[ToolNote]) -> Vec<ToolNote> {
    let tool_of = |n: &ToolNote| match n {
        ToolNote::NotAllowed(t, _)
        | ToolNote::NotInstalled(t)
        | ToolNote::Unverified(t, _)
        | ToolNote::AlreadyRunning(t)
        | ToolNote::LeftToWaywall(t)
        | ToolNote::Failed(t, _) => *t,
        ToolNote::Waywall(_) => ToolId::Waywall,
    };
    let mut out: Vec<ToolNote> = install.to_vec();
    for n in launch {
        let explained = install.iter().any(|i| tool_of(i) == tool_of(n));
        if !explained && !out.contains(n) {
            out.push(n.clone());
        }
    }
    out
}

/// Why a game died before its title screen: its exit code and the last meaningful thing its log
/// says (where the JVM says why, e.g. "Could not reserve enough space for object heap").
pub fn startup_failure(exit: GameExit, log: &Path) -> FailDetail {
    FailDetail::Exited {
        code: exit.code,
        said: last_message(log),
    }
}

/// A failure in English with the game log's path, for the command line and the launcher log.
pub fn describe(detail: &FailDetail, log: &Path) -> String {
    format!("{detail} Log: {}", log.display())
}

/// The longest message taken from a game log.
const SAID_MAX: usize = 300;

/// The game's last meaningful words, from the last 64 KB of its output, never log XML:
/// - the JVM's own lines after the last log event (or with no events at all), which say why
///   Java or Minecraft couldn't start;
/// - else the last ERROR or FATAL event, with the innermost cause of its exception. Realms
///   errors don't count: every offline start logs them.
fn last_message(log: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(log).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(64 * 1024)))
        .ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    last_message_in(&String::from_utf8_lossy(&buf))
}

fn last_message_in(text: &str) -> Option<String> {
    let entries = log_entries(text);
    let after_events = entries
        .iter()
        .rposition(|e| e.level.is_some())
        .map_or(0, |i| i + 1);
    let plain: Vec<&str> = entries[after_events..]
        .iter()
        .map(|e| e.message.as_str())
        .collect();
    let said = if !plain.is_empty() {
        plain[plain.len().saturating_sub(2)..].join(" / ")
    } else {
        let e = entries.iter().rev().find(|e| {
            matches!(e.level, Some("ERROR" | "FATAL")) && !e.logger.starts_with(REALMS)
        })?;
        let cause = e.throwable.map(throwable_reason).unwrap_or_default();
        if cause.is_empty() || e.message.contains(&cause) {
            e.message.clone()
        } else {
            format!("{}: {cause}", e.message)
        }
    };
    (!said.is_empty()).then(|| clip(said, SAID_MAX))
}

/// The logger of Realms, which can't sign in with an offline account and says so at every start.
const REALMS: &str = "com.mojang.realmsclient";

/// One thing the game's output says: a log4j event of the XML layout Mojang's log config gives
/// the game's stdout, or a plain line outside the events (the JVM itself, a crash before logging
/// started).
#[derive(Debug, PartialEq, Eq)]
struct LogEntry<'a> {
    /// The event's level; `None` for a plain line.
    level: Option<&'a str>,
    logger: &'a str,
    message: String,
    /// The event's exception and stack trace, as text.
    throwable: Option<&'a str>,
}

const EVENT: &str = "<log4j:Event";
const EVENT_END: &str = "</log4j:Event>";

/// The events and plain lines of a game's output, in order. A text that starts inside an event
/// (the tail of a long log) skips to its end. Plain lines are kept only when they read as text:
/// no markup, no stack frames, no OpenAL's goodbye.
fn log_entries(text: &str) -> Vec<LogEntry<'_>> {
    let mut out = Vec::new();
    let mut rest = text;
    if let Some(end) = rest.find(EVENT_END)
        && rest.find(EVENT).is_none_or(|start| end < start)
    {
        rest = &rest[end + EVENT_END.len()..];
    }
    loop {
        let (plain, event) = match rest.find(EVENT) {
            Some(i) => (&rest[..i], Some(&rest[i..])),
            None => (rest, None),
        };
        out.extend(
            plain
                .lines()
                .map(str::trim)
                .filter(|l| readable(l))
                .map(|l| LogEntry {
                    level: None,
                    logger: "",
                    message: l.to_string(),
                    throwable: None,
                }),
        );
        let Some(event) = event else { break };
        let (body, next) = match event.find(EVENT_END) {
            Some(e) => (&event[..e], &event[e + EVENT_END.len()..]),
            None => (event, ""),
        };
        let head = &body[..body.find('>').unwrap_or(body.len())];
        if let Some(message) = element(body, "log4j:Message") {
            out.push(LogEntry {
                level: attribute(head, "level"),
                logger: attribute(head, "logger").unwrap_or(""),
                message: message.trim().to_string(),
                throwable: element(body, "log4j:Throwable"),
            });
        }
        rest = next;
    }
    out
}

/// A plain output line that says something: not markup, a stack frame or OpenAL's cleanup note.
fn readable(line: &str) -> bool {
    !line.is_empty()
        && !line.starts_with('<')
        && !line.starts_with("]]>")
        && !line.contains("log4j:")
        && !(line.ends_with('>') && line.contains("=\""))
        && !line.starts_with("at ")
        && !line.starts_with("...")
        && !line.starts_with("AL lib:")
}

/// `name="value"` in a tag.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let at = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let len = tag[at..].find('"')?;
    Some(&tag[at..at + len])
}

/// The text of `<name>` in an event: its CDATA section (log4j's XML layout writes every message
/// as one). `None` without the element or its CDATA.
fn element<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let open = body.find(&format!("<{name}>"))? + name.len() + 2;
    let inner = body[open..].trim_start().strip_prefix("<![CDATA[")?;
    Some(&inner[..inner.find("]]>").unwrap_or(inner.len())])
}

/// `s` cut at `max` bytes (on a character boundary), with "…" when cut.
fn clip(mut s: String, max: usize) -> String {
    if s.len() > max {
        let cut = (0..=max)
            .rev()
            .find(|&i| s.is_char_boundary(i))
            .unwrap_or(0);
        s.truncate(cut);
        s.push('…');
    }
    s
}

/// The reason in an exception's text: the innermost `Caused by:` (without its exception type)
/// and the lines of its message, else the first line's message.
fn throwable_reason(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .rposition(|l| l.trim_start().starts_with("Caused by: "))
        .or_else(|| lines.iter().position(|l| !l.trim().is_empty()));
    let Some(start) = start else {
        return String::new();
    };
    let first = lines[start].trim();
    let first = first.rsplit("<![CDATA[").next().unwrap_or(first);
    let first = first.strip_prefix("Caused by: ").unwrap_or(first);
    // "java.lang.IllegalStateException: the message": keep the message.
    let first = match first.split_once(": ") {
        Some((kind, msg)) if !kind.contains(' ') && kind.contains('.') => msg,
        _ => first,
    };
    let mut parts = vec![first.trim().to_string()];
    for l in &lines[start + 1..] {
        let t = l.trim();
        if t.is_empty()
            || t.starts_with("at ")
            || t.starts_with("...")
            || t.starts_with("Caused by")
            || t.starts_with('<')
            || t.starts_with("]]>")
        {
            break;
        }
        parts.push(t.to_string());
    }
    clip(parts.join(" "), 400)
}

/// Whether an install failed because the network couldn't be reached (refused, timed out, no
/// DNS), as opposed to the disk or a bad file: the UI tells the player "no internet connection"
/// instead of the error chain.
pub fn install_failure_kind(err: &anyhow::Error) -> FailKind {
    let offline = err.chain().any(|e| {
        e.downcast_ref::<reqwest::Error>()
            .is_some_and(|r| r.is_connect() || r.is_timeout())
            || e.downcast_ref::<std::io::Error>().is_some_and(|io| {
                use std::io::ErrorKind::*;
                matches!(
                    io.kind(),
                    ConnectionRefused
                        | ConnectionReset
                        | ConnectionAborted
                        | NotConnected
                        | TimedOut
                        | HostUnreachable
                        | NetworkUnreachable
                        | NetworkDown
                )
            })
    });
    if offline {
        FailKind::Offline
    } else {
        FailKind::Install
    }
}

/// The reason for a Fabric startup crash, from the game log: the exception of the crash event
/// (its innermost `Caused by:` and the lines of its message). In plain logs, the lines after the
/// crash line.
fn crash_reason(log: &str) -> String {
    const UNKNOWN: &str = "Fabric Loader reported an error.";
    let event = log_entries(log)
        .into_iter()
        .rev()
        .find(|e| e.level.is_some() && CRASH_MARKERS.iter().any(|m| e.message.contains(m)));
    if let Some(reason) = event
        .and_then(|e| e.throwable)
        .map(throwable_reason)
        .filter(|r| !r.is_empty())
    {
        return reason;
    }
    let lines: Vec<&str> = log.lines().collect();
    let Some(at) = lines
        .iter()
        .rposition(|l| CRASH_MARKERS.iter().any(|m| l.contains(m)))
    else {
        return UNKNOWN.to_string();
    };
    let reason = throwable_reason(&lines[at + 1..].join("\n"));
    if reason.is_empty() {
        UNKNOWN.to_string()
    } else {
        reason
    }
}

/// The mod names a plan adds or updates, for "updates {mod} first".
pub fn describe_plan(plan: &Plan) -> Option<String> {
    let mut names: Vec<&str> = plan
        .add
        .iter()
        .chain(plan.update.iter().map(|u| &u.to))
        .map(|m| m.name.as_str())
        .collect();
    names.dedup();
    match names.len() {
        0 => None,
        1..=3 => Some(names.join(", ")),
        n => Some(format!("{}, {} +{}", names[0], names[1], n - 2)),
    }
}

/// What `LogScan` saw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Seen {
    Ready,
    Crashed,
}

/// Finds a marker in a growing log, reading only the new part each time.
#[derive(Default)]
struct LogScan {
    offset: u64,
    /// The end of the last read, so a marker split between two reads is still found.
    tail: String,
}

impl LogScan {
    async fn scan(&mut self, path: &Path) -> Option<Seen> {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        let mut file = tokio::fs::File::open(path).await.ok()?;
        file.seek(std::io::SeekFrom::Start(self.offset))
            .await
            .ok()?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).await.ok()?;
        if buf.is_empty() {
            return None;
        }
        self.offset += buf.len() as u64;
        let text = format!("{}{}", self.tail, String::from_utf8_lossy(&buf));
        if READY_MARKERS.iter().any(|m| text.contains(m)) {
            return Some(Seen::Ready);
        }
        if CRASH_MARKERS.iter().any(|m| text.contains(m)) {
            return Some(Seen::Crashed);
        }
        let keep = READY_MARKERS
            .iter()
            .chain(CRASH_MARKERS)
            .map(|m| m.len())
            .max()
            .unwrap_or(0);
        let cut = text.len().saturating_sub(keep);
        let cut = (cut..text.len())
            .find(|&i| text.is_char_boundary(i))
            .unwrap_or(text.len());
        self.tail = text[cut..].to_string();
        None
    }
}

/// The option that marks 26.x's first-start accessibility screen as seen (`seed_options`). The
/// settings import adds it to Draftout's options.txt too, if it's missing.
pub const DRAFTOUT_SEED: (&str, &str) = ("onboardAccessibility", "false");

/// 26.x opens an accessibility screen before the title screen on a first start. A speedrun
/// instance skips it: on the very first start only (no `options.txt` yet) the launcher writes
/// the one option that marks it as seen. Everything else stays the game's default.
async fn seed_options(game_dir: &Path) -> Result<()> {
    let path = game_dir.join("options.txt");
    if tokio::fs::try_exists(&path).await? {
        return Ok(());
    }
    tokio::fs::create_dir_all(game_dir).await?;
    let (key, value) = DRAFTOUT_SEED;
    tokio::fs::write(&path, format!("{key}:{value}\n"))
        .await
        .with_context(|| format!("writing {}", path.display()))
}

/// The largest jar the Mods page takes: a big Fabric mod is a few MB, Sodium about 1.
const MAX_CUSTOM_JAR: u64 = 64 * 1024 * 1024;

fn add_custom_jar(paths: &Paths, path: &Path) -> Result<srl_mods::CustomJar> {
    let meta = std::fs::metadata(path).with_context(|| format!("reading {}", path.display()))?;
    if !meta.is_file() {
        bail!("{} is not a file", path.display());
    }
    if meta.len() > MAX_CUSTOM_JAR {
        bail!(
            "{} is larger than {} MB",
            path.display(),
            MAX_CUSTOM_JAR >> 20
        );
    }
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| n.to_ascii_lowercase().ends_with(".jar") && !n.starts_with('.'))
        .with_context(|| format!("{} is not a .jar", path.display()))?;
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let (id, name) = fabric_mod(&bytes)
        .with_context(|| format!("{file_name} is not a Fabric mod (no fabric.mod.json)"))?;
    // Fabric API is banned in every mode (`srl_mods::forbidden`): not as the player's own jar
    // either.
    if let Some(id) = id.filter(|id| {
        Mode::ALL
            .iter()
            .all(|m| srl_mods::forbidden(*m).contains(&id.as_str()))
    }) {
        bail!(
            "{file_name} is {}, which speedrun rules ban in every mode",
            name.as_deref().unwrap_or(&id)
        );
    }
    let sha512 = {
        use sha2::{Digest, Sha512};
        hex::encode(Sha512::digest(&bytes))
    };
    let dest = paths.store_sha512(&sha512);
    if !dest.is_file() {
        let dir = dest.parent().expect("a store path has a parent");
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let tmp = dest.with_extension("tmp");
        std::fs::write(&tmp, &bytes).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &dest).with_context(|| format!("writing {}", dest.display()))?;
    }
    Ok(srl_mods::CustomJar {
        name: name.unwrap_or_else(|| file_name.trim_end_matches(".jar").to_string()),
        file_name,
        sha512,
        size: bytes.len() as u64,
    })
}

/// `Some((id, name))` from a jar's `fabric.mod.json` (`None` inside for what it doesn't say
/// usably), or `None` when the bytes are no zip with that file.
fn fabric_mod(bytes: &[u8]) -> Option<(Option<String>, Option<String>)> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    let mut file = zip.by_name("fabric.mod.json").ok()?;
    if file.size() > 1024 * 1024 {
        return Some((None, None));
    }
    let mut text = String::new();
    if file.read_to_string(&mut text).is_err() {
        return Some((None, None));
    }
    // Some mods put raw newlines inside strings, which strict JSON refuses; the id and the name
    // are all that is wanted.
    let json: Option<serde_json::Value> = serde_json::from_str(&text).ok();
    let id = json
        .as_ref()
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .map(|id| id.trim().to_ascii_lowercase());
    let name = json
        .as_ref()
        .and_then(|v| v.get("name").or_else(|| v.get("id")))
        .and_then(|v| v.as_str())
        .map(|n| {
            n.chars()
                .filter(|c| !c.is_control())
                .take(80)
                .collect::<String>()
        })
        .filter(|n| !n.trim().is_empty());
    Some((id, name))
}

/// Written next to the game logs while a game runs, so a later launcher session knows it is still
/// there.
#[derive(Serialize, Deserialize)]
struct PidFile {
    pid: u32,
    /// Seconds since the Unix epoch when the launcher started it.
    started: u64,
    game_dir: PathBuf,
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Whether the process in a pid file is still the game we started (and not another process that
/// got the same pid later).
#[cfg(target_os = "linux")]
fn is_our_game(stamp: &PidFile) -> bool {
    // The game's command line names its game directory (`--gameDir`).
    let Ok(cmdline) = std::fs::read(format!("/proc/{}/cmdline", stamp.pid)) else {
        return false;
    };
    let dir = stamp.game_dir.to_string_lossy();
    cmdline.split(|b| *b == 0).any(|arg| arg == dir.as_bytes())
}

#[cfg(all(unix, not(target_os = "linux")))]
fn is_our_game(stamp: &PidFile) -> bool {
    // No /proc: a live pid is the best this can tell.
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { libc::kill(stamp.pid as libc::pid_t, 0) == 0 }
}

#[cfg(windows)]
fn is_our_game(stamp: &PidFile) -> bool {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, STILL_ACTIVE},
        System::Threading::{
            GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        },
    };
    // SAFETY: plain Win32 calls on a handle we open and close here.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, stamp.pid);
        if handle.is_null() {
            return false;
        }
        let mut code = 0u32;
        let alive = GetExitCodeProcess(handle, &mut code) != 0 && code == STILL_ACTIVE as u32;
        let zero = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
        let times = GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) != 0;
        CloseHandle(handle);
        if !alive || !times {
            return false;
        }
        // FILETIME counts 100 ns steps since 1601; the process must have started when the
        // launcher started the game, not later under a reused pid.
        let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
        let created_unix = (ticks / 10_000_000).saturating_sub(11_644_473_600);
        created_unix.abs_diff(stamp.started) <= 30
    }
}

async fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let tmp = path.with_extension("json.part");
    tokio::fs::write(&tmp, serde_json::to_vec_pretty(value)?).await?;
    tokio::fs::rename(&tmp, path)
        .await
        .with_context(|| format!("writing {}", path.display()))
}

/// Bytes on disk of what the mode runs from: its Java, the classpath, natives, the assets its
/// index lists, the log config and its mods. Files shared with other modes count here too.
fn disk_usage(installed: &Installed, paths: &Paths) -> u64 {
    let len = |p: &Path| std::fs::symlink_metadata(p).map(|m| m.len()).unwrap_or(0);
    let mut total = dir_size(&installed.java.home) + dir_size(&installed.natives_dir);
    total += installed.classpath.iter().map(|p| len(p)).sum::<u64>();
    if let Some((_, config)) = &installed.log_config {
        total += len(config);
    }
    let index = paths
        .assets()
        .join("indexes")
        .join(format!("{}.json", installed.asset_index));
    if let Ok(bytes) = std::fs::read(&index) {
        total += bytes.len() as u64;
        #[derive(Deserialize)]
        struct Index {
            objects: std::collections::HashMap<String, Object>,
        }
        #[derive(Deserialize)]
        struct Object {
            hash: String,
            size: u64,
        }
        if let Ok(index) = serde_json::from_slice::<Index>(&bytes) {
            let mut seen = HashSet::new();
            total += index
                .objects
                .values()
                .filter(|o| seen.insert(o.hash.as_str()))
                .map(|o| o.size)
                .sum::<u64>();
        }
    }
    total + dir_size(&installed.game_dir.join("mods"))
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return 0;
    };
    rd.filter_map(|e| e.ok())
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn log_scan_finds_a_marker_split_between_reads() {
        let dir = std::env::temp_dir().join(format!("srl-app-logscan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("game.log");
        std::fs::write(&log, "[main] Loading Minecraft\n[Sound] Sound eng").unwrap();
        let mut scan = LogScan::default();
        assert_eq!(scan.scan(&log).await, None);
        let mut f = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
        std::io::Write::write_all(&mut f, b"ine started\n").unwrap();
        assert_eq!(scan.scan(&log).await, Some(Seen::Ready));
        std::fs::write(&log, "[main/ERROR]: A mod crashed on startup!\n").unwrap();
        assert_eq!(LogScan::default().scan(&log).await, Some(Seen::Crashed));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The tail of Draftout 1.18.0's crash in the game's output (log4j XML), from a real run.
    #[test]
    fn crash_reason_is_the_innermost_cause() {
        let log = r#"    <log4j:Message><![CDATA[Could not authorize you against Realms server]]></log4j:Message>
Caused by: java.io.IOException: unrelated, before the crash
    <log4j:Message><![CDATA[A mod crashed on startup!]]></log4j:Message>
    <log4j:Throwable><![CDATA[net.fabricmc.loader.impl.FormattedException: java.lang.RuntimeException: Could not execute entrypoint stage 'preLaunch'
	at net.fabricmc.loader.impl.launch.knot.KnotClient.main(KnotClient.java:23)
Caused by: java.lang.RuntimeException: Could not execute entrypoint stage 'preLaunch' due to errors
	at net.fabricmc.loader.impl.launch.knot.Knot.init(Knot.java:156)
	... 2 more
Caused by: java.lang.IllegalStateException: Draftout blocked launch because disallowed mods were found in your mods folder.
Disallowed mods (2): planifolia, speedrunigt
Remove the disallowed mods and restart Minecraft.
	at knot//com.draftoutmc.draftout.DisallowedModsChecker.scanAndThrow(DisallowedModsChecker.java:28)
]]></log4j:Throwable>
"#;
        assert_eq!(
            crash_reason(log),
            "Draftout blocked launch because disallowed mods were found in your mods folder. \
             Disallowed mods (2): planifolia, speedrunigt Remove the disallowed mods and restart \
             Minecraft."
        );
        let plain = "[main/ERROR]: Incompatible mods found!\n\
            net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible!\n\
            \tat net.fabricmc.loader.impl.launch.knot.Knot.init(Knot.java:1)\n";
        assert_eq!(crash_reason(plain), "Some of your mods are incompatible!");
    }

    #[test]
    fn startup_failure_names_the_code_and_the_log() {
        let exit = GameExit {
            code: Some(1),
            success: false,
        };
        let log = Path::new("/srl/logs/rsg-latest.log");
        let detail = startup_failure(exit, log);
        assert_eq!(
            detail,
            FailDetail::Exited {
                code: Some(1),
                said: None
            }
        );
        assert_eq!(
            describe(&detail, log),
            "Minecraft exited with code 1 while starting. Log: /srl/logs/rsg-latest.log"
        );
        // With a log, the JVM's last words are in the message.
        let dir = std::env::temp_dir().join(format!("srl-app-exit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("rsg-latest.log");
        std::fs::write(
            &log,
            "Error occurred during initialization of VM\nToo small maximum heap\n\n",
        )
        .unwrap();
        assert_eq!(
            startup_failure(exit, &log),
            FailDetail::Exited {
                code: Some(1),
                said: Some(
                    "Error occurred during initialization of VM / Too small maximum heap".into()
                )
            }
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The tail of a real log4j XML game output (M1, the game in `fix`), cut mid-event the way a
    /// tail read cuts it. Milestone 1 showed `thread="main">` from here as the game's words.
    const XML_TAIL: &str = r#"able" timestamp="1793000746751" level="INFO"
 thread="main">
    <log4j:Message><![CDATA[Reason: [HARD_DEP srlneeds263 1.0.0]]]></log4j:Message>
  </log4j:Event>
  <log4j:Event logger="com.mojang.realmsclient.RealmsAvailability" timestamp="1" level="ERROR" thread="Download-2">
    <log4j:Message><![CDATA[Couldn't connect to realms]]></log4j:Message>
  </log4j:Event>
  <log4j:Event logger="net.minecraft.client.Minecraft" timestamp="2" level="ERROR" thread="Render thread">
    <log4j:Message><![CDATA[Unreported exception thrown!]]></log4j:Message>
    <log4j:Throwable><![CDATA[java.lang.IllegalStateException: Failed to load the sound system
	at net.minecraft.class_1140.method_4875(class_1140.java:150)
Caused by: java.lang.OutOfMemoryError: Java heap space
	at java.base/java.util.Arrays.copyOf(Arrays.java:3541)
]]></log4j:Throwable>
  </log4j:Event>
  <log4j:Event logger="net.minecraft.class_310" timestamp="3" level="INFO" thread="Render thread">
    <log4j:Message><![CDATA[Stopping!]]></log4j:Message>
  </log4j:Event>
AL lib: (EE) alc_cleanup: 1 device not closed
"#;

    #[test]
    fn the_last_message_comes_from_the_log_events_never_their_xml() {
        let entries = log_entries(XML_TAIL);
        assert!(
            entries.iter().all(|e| !e.message.contains('<')
                && !e.message.contains("thread=")
                && !e.message.contains("CDATA")),
            "{entries:#?}"
        );
        assert_eq!(entries.len(), 3, "{entries:#?}");
        assert_eq!(entries[1].level, Some("ERROR"));
        assert_eq!(entries[1].logger, "net.minecraft.client.Minecraft");
        // The last error that isn't Realms, with its innermost cause.
        assert_eq!(
            last_message_in(XML_TAIL).as_deref(),
            Some("Unreported exception thrown!: Java heap space")
        );
        // Only INFO and Realms: nothing worth showing.
        let quiet = XML_TAIL.replace(
            r#"level="ERROR" thread="Render"#,
            r#"level="INFO" thread="Render"#,
        );
        assert_eq!(last_message_in(&quiet), None);
        // The JVM's own lines after the events win.
        let jvm =
            format!("{XML_TAIL}Exception in thread \"main\" java.lang.Error: boom\n\tat a.b(c)\n");
        assert_eq!(
            last_message_in(&jvm).as_deref(),
            Some("Exception in thread \"main\" java.lang.Error: boom")
        );
    }

    /// A real Fabric crash event, whole, from the M1 log in `fix` (shortened stack).
    #[test]
    fn crash_reason_from_a_whole_xml_event() {
        let log = r#"  <log4j:Event logger="FabricLoader" timestamp="1793000746756" level="ERROR" thread="main">
    <log4j:Message><![CDATA[Incompatible mods found!]]></log4j:Message>
    <log4j:Throwable><![CDATA[net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible with the game or each other!
A potential solution has been determined, this may resolve your problem:
	at net.fabricmc.loader.impl.FormattedException.ofLocalized(FormattedException.java:51)
]]></log4j:Throwable>
  </log4j:Event>
"#;
        assert_eq!(
            crash_reason(log),
            "Some of your mods are incompatible with the game or each other! A potential \
             solution has been determined, this may resolve your problem:"
        );
    }

    #[tokio::test]
    async fn refused_connections_count_as_offline() {
        let dl = Downloader::new().unwrap();
        // Port 9 (discard) is closed on the test machines: the connect is refused at once.
        let err = dl
            .refresh_bytes("http://127.0.0.1:9/x")
            .await
            .context("installing Minecraft")
            .unwrap_err();
        assert_eq!(install_failure_kind(&err), FailKind::Offline);
        let disk = anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::StorageFull))
            .context("writing a file");
        assert_eq!(install_failure_kind(&disk), FailKind::Install);
    }

    /// Fabric API is banned on /mc, and Ranked breaks with it: the Mods page's own jars must not
    /// be a way to put it in (`srl_mods::forbidden`).
    #[test]
    fn a_players_jar_can_not_be_fabric_api() {
        let root = std::env::temp_dir().join(format!("srl-app-own-jar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let jar = |file: &str, mod_json: &str| {
            let path = root.join(file);
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut zip, mod_json.as_bytes()).unwrap();
            zip.finish().unwrap();
            path
        };
        let paths = Paths::new(root.join("data"));
        let own = jar("mine.jar", r#"{"id":"mymod","name":"My Mod"}"#);
        assert_eq!(add_custom_jar(&paths, &own).unwrap().name, "My Mod");
        for (file, id) in [
            ("fabric-api-0.42.0+1.16.jar", "fabric"),
            ("fabric-api-0.92.2+1.20.1.jar", "fabric-api"),
        ] {
            let api = jar(file, &format!(r#"{{"id":"{id}","name":"Fabric API"}}"#));
            let err = add_custom_jar(&paths, &api).unwrap_err().to_string();
            assert!(err.contains("Fabric API"), "{err}");
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A custom instance resolves as its base mode, with its own changes, into its own
    /// directory; the practice instance has no base.
    #[test]
    fn a_custom_instance_gets_its_base_modes_set() {
        let root = std::env::temp_dir().join(format!("srl-app-sets-{}", std::process::id()));
        let core = Core::new(Paths::new(&root)).unwrap();
        let meta = super::super::fake::fake_meta();
        let mut settings = Settings::default();
        settings
            .custom_instances
            .push(super::super::CustomInstance {
                id: srl_core::CustomId(4),
                name: "Seeds".into(),
                base: Mode::Ssg,
                memory_mb: None,
                mods: srl_mods::ModChoice {
                    off: ["setspawnmod".to_string()].into(),
                    ..Default::default()
                },
                jvm_args: Vec::new(),
                created_ms: 0,
            });
        let rules = settings.mod_rules();
        let custom = Instance::Custom(srl_core::CustomId(4));
        let ssg = core.resolve(meta, Mode::Ssg, &rules).unwrap();
        let sets = core.sets(meta, custom, &rules).unwrap();
        assert_eq!(sets.base, Mode::Ssg);
        assert_eq!(sets.set.instance, custom);
        assert!(ssg.get("setspawnmod").is_some());
        assert!(sets.own.get("setspawnmod").is_some());
        assert!(sets.set.get("setspawnmod").is_none());
        assert_eq!(sets.set.mods.len() + 1, ssg.mods.len());
        let resolved = core.resolve_instance(meta, custom, &rules).unwrap();
        assert_eq!(resolved.instance, custom);
        assert_eq!(
            srl_mods::mods_dir(core.paths(), resolved.instance),
            root.join("instances/custom-4/mods")
        );
        assert!(core.sets(meta, Instance::Practice, &rules).is_err());
        assert!(
            core.sets(meta, Instance::Custom(srl_core::CustomId(5)), &rules)
                .is_err()
        );
    }

    #[test]
    fn local_state_needs_the_record_and_its_files() {
        let root = std::env::temp_dir().join(format!("srl-app-local-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let core = Core::new(Paths::new(&root)).unwrap();
        assert_eq!(core.local_state(Mode::Rsg), Local::NotInstalled);

        let java = root.join("java/bin/java");
        let id = "fabric-loader-0.20.1-1.16.1";
        let mods = root.join("instances/mcsr/mods");
        for (path, body) in [
            (java.clone(), &b"#!"[..]),
            (root.join(format!("versions/{id}/{id}.json")), b"{}"),
            (root.join("meta/verified/rsg.json"), b"{}"),
            (mods.join("a.jar"), b"abc"),
        ] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        let record = InstallRecord {
            minecraft_version: "1.16.1".into(),
            version_id: id.into(),
            fabric_loader: "0.20.1".into(),
            java_version: "21".into(),
            java_executable: java,
            mods: vec![RecordedJar {
                file_name: "a.jar".into(),
                size: 3,
            }],
            disk: 1,
        };
        std::fs::create_dir_all(root.join("meta/launcher")).unwrap();
        std::fs::write(
            root.join("meta/launcher/rsg.json"),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        assert_eq!(core.local_state(Mode::Rsg), Local::Ready(record.clone()));
        std::fs::write(mods.join("a.jar"), b"abcd").unwrap();
        assert_eq!(core.local_state(Mode::Rsg), Local::Damaged(record.clone()));
        // SSG's set is in the shared mods/ now: RSG swaps its own back in on Play, no repair.
        std::fs::create_dir_all(root.join("instances/mcsr/.srl-mods")).unwrap();
        std::fs::write(root.join("instances/mcsr/.srl-mods/owner"), "ssg").unwrap();
        assert_eq!(core.local_state(Mode::Rsg), Local::Ready(record.clone()));
        std::fs::remove_file(root.join("meta/verified/rsg.json")).unwrap();
        assert_eq!(core.local_state(Mode::Rsg), Local::NotInstalled);
        // A custom instance: its own record, stamp and mods/.
        let custom = Instance::Custom(srl_core::CustomId(1));
        assert_eq!(core.local_state(custom), Local::NotInstalled);
        let mods = root.join("instances/custom-1/mods");
        for (path, body) in [
            (root.join("meta/verified/custom-1.json"), &b"{}"[..]),
            (
                root.join("meta/launcher/custom-1.json"),
                &serde_json::to_vec(&record).unwrap(),
            ),
            (mods.join("a.jar"), b"abc"),
        ] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        assert_eq!(core.local_state(custom), Local::Ready(record));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
