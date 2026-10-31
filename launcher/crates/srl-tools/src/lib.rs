//! Speedrun tools: Ninjabrain Bot (every OS), Toolscreen (Windows) and waywall (Linux), installed
//! at legal versions, started with the game and stopped when it exits.
//!
//! # API
//!
//! ```no_run
//! # async fn demo(installed: srl_game::Installed, opts: srl_game::LaunchOptions) -> anyhow::Result<()> {
//! use srl_core::{Downloader, Mode, Paths};
//! use srl_game::GameInstaller;
//! use srl_tools::{ToolId, ToolSelection, Tools};
//!
//! let paths = Paths::from_env()?;
//! let downloader = Downloader::new()?;
//! // One per launcher process: it remembers which tool processes it started.
//! let tools = Tools::new(paths.clone(), downloader.clone());
//!
//! // The Tools page: what each tool is and its state (no network, no hashing).
//! for info in srl_tools::TOOLS.iter() {
//!     println!("{} {:?}", info.name, tools.status(info.id));
//! }
//! tools.install(ToolId::NinjabrainBot, None).await?; // install or update, Stage::Tools
//!
//! // Play. The app keeps a selection per mode; the defaults come from the rules.
//! let mode = Mode::Rsg;
//! let selection = ToolSelection::defaults(mode, srl_game::Platform::current().os);
//! let notes = tools.install_selected(mode, &selection, None).await; // after the mods
//! let installer = GameInstaller::new(paths, downloader);
//! let cmd = installer.command(&installed, &opts)?;
//! let prepared = tools.prepare(mode, cmd, &selection).await;   // waywall wraps the command here
//! let game = installer.spawn(mode, &prepared.command).await?;
//! let session = tools.start(prepared, &game, &installed.java.executable).await;
//! for note in &session.notes {
//!     eprintln!("{note}"); // what was skipped and why; the app translates `ToolNote`
//! }
//! // When the game exits the session stops the tool processes it started (Ninjabrain Bot),
//! // unless another running game still uses them. `session.finished()` waits for that.
//! # Ok(()) }
//! ```
//!
//! - **[`TOOLS`] / [`ToolInfo`]:** id, name, what it does (English, and a `copy_key` into
//!   `assets/copy.json`), the systems it runs on, the pinned version and why, licence, homepage
//!   and credit.
//! - **[`Tools::status`] / [`ToolStatus`]:** `NotAvailable` (wrong OS or CPU), `NotInstalled`,
//!   `Installed` (version, path, whether the launcher manages it) or `UpdateAvailable`.
//! - **[`Tools::install`]:** downloads the pinned version with its hash checked and removes older
//!   ones (so it is the update too), reporting [`srl_core::Stage::Tools`]. waywall isn't
//!   installed: the player installs it with their package manager.
//! - **Per-mode rules:** [`availability`] (Ninjabrain Bot is banned in Draftout; Toolscreen is
//!   only set up for the 1.16.1 modes) and [`ToolSelection::defaults`].
//! - **The launch hook:** [`Tools::prepare`] returns the command to start (wrapped in
//!   `waywall wrap --` when waywall is on) and checks what will run; [`Tools::start`], called
//!   right after the game process exists, injects Toolscreen (Windows) and starts Ninjabrain Bot
//!   unless one is already running, and returns a [`ToolSession`] that stops what it started
//!   when the game exits. A tool that can't run never blocks the game: it is left out with a
//!   [`ToolNote`].
//!
//! Details per tool, with the sources: [`ninjabrain`], [`toolscreen`], [`waywall`].
//!
//! # CLI
//!
//! `src/bin/srl-tools.rs` drives the same API for tests: `status`, `install <tool>`,
//! `fetch-toolscreen` (download and check the Toolscreen DLLs on any OS), `run-ninjabrain`.
pub mod legal_dlls;
pub mod ninjabrain;
pub mod registry;
pub mod side;
pub mod status;
pub mod toolscreen;
pub mod waywall;
#[cfg(windows)]
mod win;

use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Result, bail};
use srl_core::{Downloader, Mode, Paths, ProgressFn};
use srl_game::{GameProcess, LaunchCommand, Platform, rules::Os};
use tokio::sync::watch;

pub use legal_dlls::LegalDlls;
pub use registry::{Source, TOOLS, ToolId, ToolInfo, ToolSelection, Unavailable, availability};
pub use side::{PidFile, SideCommand, SideProcess, Stopped};
pub use status::{InstalledTool, ToolStatus};
pub use waywall::WrapIssue;

/// How long a tool gets to close before it is killed.
pub const TOOL_STOP_GRACE: Duration = Duration::from_secs(5);

/// Why a tool was left out of a launch, or something the player should know about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolNote {
    /// On in the selection, but this mode or system can't have it.
    NotAllowed(ToolId, Unavailable),
    /// On, but not installed (or, for waywall, not found).
    NotInstalled(ToolId),
    /// Its files don't pass the check before launch (changed on disk, or no longer legal).
    Unverified(ToolId, String),
    /// waywall can't wrap this launch (or wraps it with a GLFW legal-dlls.csv doesn't list).
    Waywall(WrapIssue),
    /// It was already running (started by the player): used as it is, never stopped by us.
    AlreadyRunning(ToolId),
    /// The player's waywall config starts it, so the launcher doesn't.
    LeftToWaywall(ToolId),
    /// Starting or injecting it failed.
    Failed(ToolId, String),
}

impl ToolNote {
    /// The line for the UI in `assets/copy.json`, with its placeholders (`{tool}`, `{path}`).
    /// `Display` is the English line for logs; the free-text detail of `Unverified` and `Failed`
    /// is English only and belongs under the translated line.
    pub fn copy(&self) -> (&'static str, Vec<(&'static str, String)>) {
        let tool = |t: &ToolId| vec![("tool", t.info().name.to_string())];
        match self {
            ToolNote::NotAllowed(t, Unavailable::Os) => ("tool.note.os", tool(t)),
            ToolNote::NotAllowed(t, Unavailable::Banned) => ("tool.note.banned", tool(t)),
            ToolNote::NotAllowed(t, Unavailable::NotSupported) => {
                ("tool.note.not_supported", tool(t))
            }
            ToolNote::NotInstalled(ToolId::Waywall) | ToolNote::Waywall(WrapIssue::NotFound) => {
                ("tool.note.waywall_not_found", vec![])
            }
            ToolNote::NotInstalled(t) => ("tool.note.not_installed", tool(t)),
            ToolNote::Unverified(t, _) => ("tool.note.unverified", tool(t)),
            ToolNote::Waywall(WrapIssue::NoWaylandSession) => ("tool.note.no_wayland", vec![]),
            ToolNote::Waywall(WrapIssue::NoGlfw) => ("tool.note.no_glfw", vec![]),
            ToolNote::Waywall(WrapIssue::GlfwNotLegal(p)) => (
                "tool.note.glfw_not_legal",
                vec![("path", p.display().to_string())],
            ),
            ToolNote::Waywall(WrapIssue::GlfwUnchecked(p)) => (
                "tool.note.glfw_unchecked",
                vec![("path", p.display().to_string())],
            ),
            ToolNote::AlreadyRunning(t) => ("tool.note.already_running", tool(t)),
            ToolNote::LeftToWaywall(t) => ("tool.note.left_to_waywall", tool(t)),
            ToolNote::Failed(t, _) => ("tool.note.failed", tool(t)),
        }
    }

    /// The English detail behind `Unverified` and `Failed`.
    pub fn detail(&self) -> Option<&str> {
        match self {
            ToolNote::Unverified(_, d) | ToolNote::Failed(_, d) => Some(d),
            _ => None,
        }
    }
}

impl fmt::Display for ToolNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = |t: &ToolId| t.info().name;
        match self {
            ToolNote::NotAllowed(t, Unavailable::Os) => {
                write!(f, "{} doesn't run on this system", name(t))
            }
            ToolNote::NotAllowed(t, Unavailable::Banned) => {
                write!(f, "{} is not allowed in this mode", name(t))
            }
            ToolNote::NotAllowed(t, Unavailable::NotSupported) => {
                write!(f, "{} isn't set up for this mode yet", name(t))
            }
            ToolNote::NotInstalled(ToolId::Waywall) => write!(f, "waywall is not on PATH"),
            ToolNote::NotInstalled(t) => write!(f, "{} is not installed", name(t)),
            ToolNote::Unverified(t, why) => write!(f, "{} left out: {why}", name(t)),
            ToolNote::Waywall(WrapIssue::NotFound) => write!(f, "waywall is not on PATH"),
            ToolNote::Waywall(WrapIssue::NoWaylandSession) => {
                write!(
                    f,
                    "waywall needs a Wayland session; the game starts without it"
                )
            }
            ToolNote::Waywall(WrapIssue::NoGlfw) => write!(
                f,
                "waywall needs its patched GLFW for 1.16.1 (/usr/local/lib64/waywall-glfw/libglfw.so); the game starts without waywall"
            ),
            ToolNote::Waywall(WrapIssue::GlfwNotLegal(p)) => write!(
                f,
                "the GLFW at {} is not in legal-dlls.csv: runs with it may be rejected",
                p.display()
            ),
            ToolNote::Waywall(WrapIssue::GlfwUnchecked(p)) => write!(
                f,
                "the GLFW at {} couldn't be checked: legal-dlls.csv couldn't be fetched",
                p.display()
            ),
            ToolNote::AlreadyRunning(t) => write!(f, "{} was already running", name(t)),
            ToolNote::LeftToWaywall(t) => {
                write!(f, "{} is started by your waywall config", name(t))
            }
            ToolNote::Failed(t, why) => write!(f, "{} failed: {why}", name(t)),
        }
    }
}

/// What [`Tools::adopt_earlier`] did with a tool an earlier launcher session left running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Adopted {
    /// No game of that session runs any more, so it was stopped.
    Stopped(Stopped),
    /// A game of that session still runs: kept (this pid) and stopped when it is gone.
    Kept(u32),
}

struct Owned {
    process: SideProcess,
    /// Running games that use it.
    users: usize,
}

/// The tools of one launcher process. Cheap to clone.
#[derive(Clone)]
pub struct Tools {
    paths: Paths,
    downloader: Downloader,
    platform: Platform,
    glfw: Option<PathBuf>,
    owned: Arc<Mutex<HashMap<ToolId, Owned>>>,
    starting: Arc<tokio::sync::Mutex<()>>,
}

/// What [`Tools::install`] would put on disk: the pinned version for this machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offer {
    pub version: &'static str,
    /// Bytes downloaded (the jar).
    pub download: u64,
}

/// What [`Tools::prepare`] decided for one launch.
#[derive(Debug)]
pub struct Prepared {
    pub mode: Mode,
    /// What to start: the game's command, wrapped by waywall when it is on.
    pub command: LaunchCommand,
    pub notes: Vec<ToolNote>,
    /// Tools that will run with this game.
    pub tools: Vec<ToolId>,
    ninjabrain: Option<PathBuf>,
    toolscreen: Option<toolscreen::Injection>,
}

/// The tools of one running game.
pub struct ToolSession {
    pub notes: Vec<ToolNote>,
    /// Started by the launcher for this game (or shared with another game it started them for).
    pub started: Vec<ToolId>,
    /// Injected into the game (Toolscreen).
    pub injected: Vec<ToolId>,
    done: watch::Receiver<bool>,
}

impl ToolSession {
    /// Resolves once the game has exited and the tools it started are stopped.
    pub async fn finished(&self) {
        let mut rx = self.done.clone();
        while !*rx.borrow_and_update() {
            if rx.changed().await.is_err() {
                return;
            }
        }
    }
}

impl Tools {
    pub fn new(paths: Paths, downloader: Downloader) -> Self {
        Self {
            paths,
            downloader,
            platform: Platform::current(),
            glfw: None,
            owned: Arc::default(),
            starting: Arc::default(),
        }
    }

    /// The patched GLFW for waywall, when the player keeps it somewhere else than
    /// [`waywall::GLFW_PATHS`].
    pub fn with_glfw(mut self, path: Option<PathBuf>) -> Self {
        self.glfw = path;
        self
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    /// From local files only: no network and no hashing.
    pub fn status(&self, tool: ToolId) -> ToolStatus {
        if !tool.info().runs_on(self.platform.os) {
            return ToolStatus::NotAvailable;
        }
        match tool {
            ToolId::NinjabrainBot => ninjabrain::status(&self.paths),
            ToolId::Toolscreen => toolscreen::status(
                &self.paths,
                self.platform.arch,
                LegalDlls::cached(&self.paths).as_ref(),
            ),
            ToolId::Waywall => waywall::status(),
        }
    }

    /// What an install of `tool` gets on this machine: the version and the bytes it downloads.
    /// `None` for tools the launcher doesn't install (waywall) or that have no legal build here.
    /// From the pins and the cached legal-dlls.csv: no network.
    pub fn offer(&self, tool: ToolId) -> Option<Offer> {
        if !tool.info().runs_on(self.platform.os) {
            return None;
        }
        match tool {
            ToolId::NinjabrainBot => Some(Offer {
                version: ninjabrain::VERSION,
                download: ninjabrain::SIZE,
            }),
            ToolId::Toolscreen => {
                toolscreen::offer(self.platform.arch, LegalDlls::cached(&self.paths).as_ref())
                    .map(|(version, download)| Offer { version, download })
            }
            ToolId::Waywall => None,
        }
    }

    /// Installs or updates a tool the launcher manages. Idempotent: verified files are not
    /// downloaded again.
    pub async fn install(&self, tool: ToolId, progress: Option<ProgressFn>) -> Result<()> {
        if !tool.info().runs_on(self.platform.os) {
            bail!("{} doesn't run on this system", tool.info().name);
        }
        match tool {
            ToolId::NinjabrainBot => {
                ninjabrain::install(&self.paths, &self.downloader, progress).await?;
            }
            ToolId::Toolscreen => {
                let legal =
                    LegalDlls::load(&self.paths, &self.downloader, legal_dlls::MAX_AGE).await?;
                toolscreen::install_for(
                    &self.paths,
                    &self.downloader,
                    self.platform.arch,
                    &legal,
                    progress,
                )
                .await?;
            }
            ToolId::Waywall => bail!("waywall is installed with the system's package manager"),
        }
        Ok(())
    }

    /// Before a launch: installs or updates every downloadable tool `selection` has on for
    /// `mode`. Never fails, so a tool can't keep the game from starting: what can't be used or
    /// installed comes back as notes (and `prepare` then leaves it out). Offline with the tools
    /// already installed it needs no network.
    pub async fn install_selected(
        &self,
        mode: Mode,
        selection: &ToolSelection,
        progress: Option<ProgressFn>,
    ) -> Vec<ToolNote> {
        let (usable, mut notes) = self.usable(mode, selection);
        for &tool in &usable.0 {
            if tool.info().source == Source::Download
                && let Err(e) = self.install(tool, progress.clone()).await
            {
                notes.push(ToolNote::Failed(tool, format!("installing: {e:#}")));
            }
        }
        // waywall isn't installed, but on 1.16.1 its GLFW is checked against legal-dlls.csv in
        // `prepare`, so the list must be on disk (fetched again when stale) on every system.
        // Without it `prepare` says it couldn't check, never that the GLFW isn't legal.
        if usable.has(ToolId::Waywall)
            && waywall::needs_glfw(mode)
            && let Err(e) =
                LegalDlls::load(&self.paths, &self.downloader, legal_dlls::MAX_AGE).await
        {
            eprintln!("legal-dlls.csv for waywall's GLFW: {e:#}");
        }
        notes
    }

    fn usable(&self, mode: Mode, selection: &ToolSelection) -> (ToolSelection, Vec<ToolNote>) {
        let mut notes = Vec::new();
        for &t in &selection.0 {
            if let Err(why) = availability(t, mode, self.platform.os) {
                notes.push(ToolNote::NotAllowed(t, why));
            }
        }
        (selection.usable(mode, self.platform.os), notes)
    }

    /// Decides what runs with this launch and checks it: Ninjabrain Bot's jar hash, Toolscreen's
    /// DLLs against their pins and legal-dlls.csv, waywall, its session and GLFW. Never fails:
    /// a tool that can't run is left out with a note, and the game starts anyway.
    pub async fn prepare(
        &self,
        mode: Mode,
        command: LaunchCommand,
        selection: &ToolSelection,
    ) -> Prepared {
        let (usable, mut notes) = self.usable(mode, selection);
        let mut prepared = Prepared {
            mode,
            command,
            notes: Vec::new(),
            tools: Vec::new(),
            ninjabrain: None,
            toolscreen: None,
        };

        if usable.has(ToolId::Toolscreen) {
            match LegalDlls::cached(&self.paths) {
                None => notes.push(ToolNote::NotInstalled(ToolId::Toolscreen)),
                Some(legal) => {
                    match toolscreen::for_launch(&self.paths, self.platform.arch, &legal).await {
                        Ok(inj) => {
                            prepared.toolscreen = Some(inj);
                            prepared.tools.push(ToolId::Toolscreen);
                        }
                        Err(e) => {
                            notes.push(ToolNote::Unverified(ToolId::Toolscreen, format!("{e:#}")))
                        }
                    }
                }
            }
        }

        let mut wrapped = false;
        if usable.has(ToolId::Waywall) {
            let ww = waywall::find();
            let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
            let glfw = if waywall::needs_glfw(mode) {
                let home = std::env::var_os("HOME").map(PathBuf::from);
                waywall::find_glfw(
                    self.glfw.as_deref(),
                    home.as_deref(),
                    LegalDlls::cached(&self.paths).as_ref(),
                )
            } else {
                None
            };
            match waywall::check(ww.as_ref(), wayland, mode, glfw.as_ref()) {
                Err(issue) => notes.push(ToolNote::Waywall(issue)),
                Ok(issue) => {
                    if let Some(issue) = issue {
                        notes.push(ToolNote::Waywall(issue));
                    }
                    let ww = ww.expect("check passed");
                    prepared.command = waywall::wrap(&prepared.command, &ww, mode, glfw.as_ref());
                    prepared.tools.push(ToolId::Waywall);
                    wrapped = true;
                }
            }
        }

        if usable.has(ToolId::NinjabrainBot) {
            if wrapped
                && waywall::config_dir().is_some_and(|d| waywall::config_starts_ninjabrain(&d))
            {
                notes.push(ToolNote::LeftToWaywall(ToolId::NinjabrainBot));
            } else {
                match ninjabrain::verify(&self.paths).await {
                    Ok(true) => {
                        prepared.ninjabrain = Some(ninjabrain::jar(&self.paths));
                        prepared.tools.push(ToolId::NinjabrainBot);
                    }
                    Ok(false) if !ninjabrain::jar(&self.paths).is_file() => {
                        notes.push(ToolNote::NotInstalled(ToolId::NinjabrainBot))
                    }
                    Ok(false) => notes.push(ToolNote::Unverified(
                        ToolId::NinjabrainBot,
                        "the jar was changed on disk".into(),
                    )),
                    Err(e) => notes.push(ToolNote::Unverified(
                        ToolId::NinjabrainBot,
                        format!("{e:#}"),
                    )),
                }
            }
        }
        prepared.notes = notes;
        prepared
    }

    /// Right after the game process started: injects Toolscreen into it (Windows) and starts
    /// Ninjabrain Bot with `java` unless one is running. The returned session stops what it
    /// started when `game` exits.
    pub async fn start(&self, prepared: Prepared, game: &GameProcess, java: &Path) -> ToolSession {
        let mut notes = prepared.notes;
        let mut injected = Vec::new();
        let mut started = Vec::new();

        if let Some(inj) = prepared.toolscreen {
            match inject(game.pid(), inj.dlls).await {
                Ok(()) => injected.push(ToolId::Toolscreen),
                Err(e) => notes.push(ToolNote::Failed(ToolId::Toolscreen, format!("{e:#}"))),
            }
        }

        if let Some(jar) = prepared.ninjabrain {
            match self.acquire_ninjabrain(java, &jar).await {
                Ok(true) => started.push(ToolId::NinjabrainBot),
                Ok(false) => notes.push(ToolNote::AlreadyRunning(ToolId::NinjabrainBot)),
                Err(e) => notes.push(ToolNote::Failed(ToolId::NinjabrainBot, format!("{e:#}"))),
            }
        }

        let (done_tx, done_rx) = watch::channel(false);
        let release = started.clone();
        let tools = self.clone();
        let game = game.clone();
        tokio::spawn(async move {
            game.wait().await;
            for tool in release {
                tools.release(tool).await;
            }
            let _ = done_tx.send(true);
        });
        ToolSession {
            notes,
            started,
            injected,
            done: done_rx,
        }
    }

    /// Ok(true): ours runs (started now, or shared with another game). Ok(false): the player's.
    async fn acquire_ninjabrain(&self, java: &Path, jar: &Path) -> Result<bool> {
        // One start at a time, so two games starting together don't both spawn one.
        let _starting = self.starting.lock().await;
        // Games that still use a Ninjabrain of ours the player has closed: the new one is theirs
        // too, so the first of them to exit doesn't stop it under the others.
        let mut users = 1;
        {
            let mut owned = self.owned.lock().unwrap();
            if let Some(o) = owned.get_mut(&ToolId::NinjabrainBot) {
                if o.process.is_running() {
                    o.users += 1;
                    return Ok(true);
                }
                // Ours, but the player closed it: start a new one below.
                users += o.users;
                owned.remove(&ToolId::NinjabrainBot);
            }
        }
        let others = tokio::task::spawn_blocking(|| ninjabrain::running(&[])).await?;
        if !others.is_empty() {
            return Ok(false);
        }
        let log = self
            .paths
            .logs()
            .join(format!("{}.log", ToolId::NinjabrainBot.id()));
        let process =
            SideProcess::spawn(ToolId::NinjabrainBot, &ninjabrain::command(java, jar), &log)
                .await?;
        let stamp = PidFile {
            pid: process.pid(),
            started: side::unix_now(),
            arg: jar.display().to_string(),
        };
        if let Err(e) = stamp.write(&self.paths, ToolId::NinjabrainBot) {
            eprintln!("Ninjabrain Bot's pid file: {e:#}");
        }
        self.owned
            .lock()
            .unwrap()
            .insert(ToolId::NinjabrainBot, Owned { process, users });
        Ok(true)
    }

    /// At launcher start: takes over the tools an earlier launcher session started and left
    /// running (it closed or crashed while a game ran), found through their pid files. While
    /// `earlier_games_run` says a game of that session still runs, they stay, count as ours (a
    /// Play shares them) and stop once those games are gone; otherwise they stop now.
    pub async fn adopt_earlier(
        &self,
        earlier_games_run: impl Fn() -> bool + Send + Sync + 'static,
    ) -> Vec<(ToolId, Adopted)> {
        let mut out = Vec::new();
        let tool = ToolId::NinjabrainBot;
        // A Play right at launcher start waits for this, so it doesn't take ours for the player's.
        let _starting = self.starting.lock().await;
        let Some(stamp) = PidFile::read(&self.paths, tool) else {
            return out;
        };
        if self.owned.lock().unwrap().contains_key(&tool) {
            return out;
        }
        if !tokio::task::spawn_blocking({
            let stamp = stamp.clone();
            move || stamp.is_live()
        })
        .await
        .unwrap_or(false)
        {
            PidFile::remove(&self.paths, tool);
            return out;
        }
        let process = SideProcess::adopt(tool, stamp.pid);
        let games = Arc::new(earlier_games_run);
        if !tokio::task::spawn_blocking({
            let games = games.clone();
            move || games()
        })
        .await
        .unwrap_or(false)
        {
            let how = process.stop(TOOL_STOP_GRACE).await;
            PidFile::remove(&self.paths, tool);
            out.push((tool, Adopted::Stopped(how)));
            return out;
        }
        self.owned
            .lock()
            .unwrap()
            .insert(tool, Owned { process, users: 1 });
        let tools = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let games = games.clone();
                if !tokio::task::spawn_blocking(move || games())
                    .await
                    .unwrap_or(false)
                {
                    break;
                }
            }
            // The earlier session's claim; a game of this session may still share it.
            tools.release(tool).await;
        });
        out.push((tool, Adopted::Kept(stamp.pid)));
        out
    }

    /// One game fewer uses `tool`; the last one stops it.
    async fn release(&self, tool: ToolId) {
        let stop = {
            let mut owned = self.owned.lock().unwrap();
            match owned.get_mut(&tool) {
                Some(o) if o.users > 1 => {
                    o.users -= 1;
                    None
                }
                Some(_) => owned.remove(&tool).map(|o| o.process),
                None => None,
            }
        };
        if let Some(p) = stop {
            let how = p.stop(TOOL_STOP_GRACE).await;
            PidFile::remove(&self.paths, tool);
            eprintln!("{} stopped: {how:?}", tool.info().name);
        }
    }

    /// The tool processes the launcher runs right now, for the Tools page.
    pub fn running(&self) -> Vec<(ToolId, u32)> {
        self.owned
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, o)| o.process.is_running())
            .map(|(t, o)| (*t, o.process.pid()))
            .collect()
    }

    /// Stops every tool process the launcher started, whoever uses it (launcher exit).
    pub async fn stop_all(&self) {
        let all: Vec<SideProcess> = self
            .owned
            .lock()
            .unwrap()
            .drain()
            .map(|(_, o)| o.process)
            .collect();
        for p in all {
            p.stop(TOOL_STOP_GRACE).await;
            PidFile::remove(&self.paths, p.tool());
        }
    }
}

/// Loads the DLLs into the game in order (liblogger first). Windows only.
#[cfg(windows)]
async fn inject(pid: u32, dlls: Vec<PathBuf>) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        for dll in &dlls {
            win::inject(pid, dll)?;
        }
        anyhow::Ok(())
    })
    .await?
}

#[cfg(not(windows))]
async fn inject(_pid: u32, _dlls: Vec<PathBuf>) -> Result<()> {
    bail!("Toolscreen runs on Windows only")
}

/// The OS the launcher runs on, as the registry sees it.
pub fn current_os() -> Os {
    Platform::current().os
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_read_well() {
        assert_eq!(
            ToolNote::NotAllowed(ToolId::NinjabrainBot, Unavailable::Banned).to_string(),
            "Ninjabrain Bot is not allowed in this mode"
        );
        assert_eq!(
            ToolNote::Waywall(WrapIssue::NoWaylandSession).to_string(),
            "waywall needs a Wayland session; the game starts without it"
        );
    }

    /// Every key the crate hands the UI is in both copy files, with the placeholders it fills.
    #[test]
    fn copy_keys_exist() {
        let en: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(include_str!("../../../assets/copy.json")).unwrap();
        let pl: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(include_str!("../../../assets/copy.pl.json")).unwrap();
        let t = ToolId::NinjabrainBot;
        let notes = [
            ToolNote::NotAllowed(t, Unavailable::Os),
            ToolNote::NotAllowed(t, Unavailable::Banned),
            ToolNote::NotAllowed(t, Unavailable::NotSupported),
            ToolNote::NotInstalled(t),
            ToolNote::NotInstalled(ToolId::Waywall),
            ToolNote::Unverified(t, String::new()),
            ToolNote::Waywall(WrapIssue::NotFound),
            ToolNote::Waywall(WrapIssue::NoWaylandSession),
            ToolNote::Waywall(WrapIssue::NoGlfw),
            ToolNote::Waywall(WrapIssue::GlfwNotLegal("/g".into())),
            ToolNote::Waywall(WrapIssue::GlfwUnchecked("/g".into())),
            ToolNote::AlreadyRunning(t),
            ToolNote::LeftToWaywall(t),
            ToolNote::Failed(t, String::new()),
        ];
        for (key, args) in notes.iter().map(ToolNote::copy) {
            for table in [&en, &pl] {
                let text = table[key]
                    .as_str()
                    .unwrap_or_else(|| panic!("{key} missing"));
                for (name, _) in &args {
                    assert!(
                        text.contains(&format!("{{{name}}}")),
                        "{key}: no {{{name}}}"
                    );
                }
            }
        }
        for info in TOOLS.iter() {
            assert!(en.contains_key(info.copy_key) && pl.contains_key(info.copy_key));
        }
    }

    /// A process not ours as a child (its parent exits at once), like a Ninjabrain Bot an
    /// earlier launcher session left behind. Returns its pid.
    #[cfg(target_os = "linux")]
    fn orphan(arg: &str) -> u32 {
        let out = std::process::Command::new("sh")
            .args(["-c", &format!("sleep {arg} >/dev/null 2>&1 & echo $!")])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn adopts_what_an_earlier_session_left_running() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let root = std::env::temp_dir().join(format!("srl-tools-adopt-{}", std::process::id()));
        let paths = Paths::new(&root);
        let tools = Tools::new(paths.clone(), Downloader::new().unwrap());
        let nb = ToolId::NinjabrainBot;
        assert!(tools.adopt_earlier(|| true).await.is_empty(), "no pid file");

        // A pid file whose process is gone (or is another program now) is dropped.
        let stale = PidFile {
            pid: orphan("301.25"),
            started: side::unix_now(),
            arg: "not-its-argument".into(),
        };
        stale.write(&paths, nb).unwrap();
        assert!(tools.adopt_earlier(|| true).await.is_empty());
        assert!(PidFile::read(&paths, nb).is_none());
        srl_game::stop::terminate(&[stale.pid]);

        // No earlier game runs: stopped right away.
        let pid = orphan("301.5");
        let stamp = PidFile {
            pid,
            started: side::unix_now(),
            arg: "301.5".into(),
        };
        stamp.write(&paths, nb).unwrap();
        let did = tools.adopt_earlier(|| false).await;
        assert!(matches!(
            did.as_slice(),
            [(ToolId::NinjabrainBot, Adopted::Stopped(_))]
        ));
        assert!(!stamp.is_live());
        assert!(PidFile::read(&paths, nb).is_none());

        // An earlier game still runs: kept, counted as ours, stopped once the game is gone.
        let pid = orphan("301.75");
        let stamp = PidFile {
            pid,
            started: side::unix_now(),
            arg: "301.75".into(),
        };
        stamp.write(&paths, nb).unwrap();
        let game = Arc::new(AtomicBool::new(true));
        let runs = game.clone();
        let did = tools
            .adopt_earlier(move || runs.load(Ordering::SeqCst))
            .await;
        assert_eq!(did, [(nb, Adopted::Kept(pid))]);
        assert_eq!(tools.running(), [(nb, pid)]);
        tokio::time::sleep(Duration::from_millis(2500)).await;
        assert!(stamp.is_live(), "the game still runs");
        game.store(false, Ordering::SeqCst);
        tokio::time::timeout(Duration::from_secs(10), async {
            while stamp.is_live() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("stopped after the earlier game exited");
        // The exit is noticed by polling, then the pid file goes.
        tokio::time::timeout(Duration::from_secs(3), async {
            while PidFile::read(&paths, nb).is_some() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("pid file removed");
        assert!(tools.running().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The first game to exit doesn't stop a Ninjabrain another running game uses, even when the
    /// player closed the first one and a new one was started for the second game.
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn a_replaced_tool_keeps_its_users() {
        let root = std::env::temp_dir().join(format!("srl-tools-users-{}", std::process::id()));
        let tools = Tools::new(Paths::new(&root), Downloader::new().unwrap());
        let nb = ToolId::NinjabrainBot;
        // Game 1 started P1, which the player has closed since.
        let p1 = SideProcess::adopt(nb, orphan("302.5"));
        p1.stop(Duration::from_secs(2)).await;
        tools.owned.lock().unwrap().insert(
            nb,
            Owned {
                process: p1,
                users: 1,
            },
        );
        // Game 2 starts P2 (a stand-in for `java -jar <jar>`).
        std::fs::create_dir_all(&root).unwrap();
        let java = root.join("java");
        std::fs::write(&java, "#!/bin/sh\nexec sleep 302.75\n").unwrap();
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let p2 = tools.acquire_ninjabrain(&java, &root.join("nb.jar")).await;
        // `ninjabrain::running` would see a real Ninjabrain on this machine; skip then.
        if p2.as_ref().is_ok_and(|ours| !ours) {
            let _ = std::fs::remove_dir_all(&root);
            return;
        }
        assert!(p2.unwrap());
        let pid2 = tools.running()[0].1;
        // Game 1 exits: P2 stays for game 2.
        tools.release(nb).await;
        assert_eq!(tools.running(), [(nb, pid2)]);
        // Game 2 exits: P2 stops.
        tools.release(nb).await;
        assert!(tools.running().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn prepare_leaves_out_what_cant_run() {
        let root = std::env::temp_dir().join(format!("srl-tools-prep-{}", std::process::id()));
        let tools = Tools::new(Paths::new(&root), Downloader::new().unwrap());
        let cmd = LaunchCommand {
            program: "/java".into(),
            args: vec!["Main".into()],
            cwd: root.clone(),
            env: vec![],
        };
        let mut sel = ToolSelection::default();
        sel.set(ToolId::NinjabrainBot, true);
        // Draftout bans Ninjabrain: nothing runs, the command is untouched.
        let p = tools.prepare(Mode::Draftout, cmd.clone(), &sel).await;
        assert_eq!(p.command, cmd);
        assert!(p.tools.is_empty());
        assert_eq!(
            p.notes,
            [ToolNote::NotAllowed(
                ToolId::NinjabrainBot,
                Unavailable::Banned
            )]
        );
        // RSG with nothing installed: a note, and the game still starts.
        let p = tools.prepare(Mode::Rsg, cmd.clone(), &sel).await;
        assert_eq!(p.command, cmd);
        assert_eq!(p.notes, [ToolNote::NotInstalled(ToolId::NinjabrainBot)]);
        // A jar that isn't the pinned one is refused.
        let jar = ninjabrain::jar(tools.paths());
        std::fs::create_dir_all(jar.parent().unwrap()).unwrap();
        std::fs::write(&jar, b"not ninjabrain").unwrap();
        let p = tools.prepare(Mode::Rsg, cmd.clone(), &sel).await;
        assert!(matches!(
            p.notes.as_slice(),
            [ToolNote::Unverified(ToolId::NinjabrainBot, _)]
        ));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
