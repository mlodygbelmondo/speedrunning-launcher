//! Building the launch command and starting and watching the game process.
//!
//! Argument handling (rules on arguments, `${placeholder}` substitution, the pre-1.13 fallback)
//! follows Pandora Launcher (MIT, commit 2d83d6f), `crates/backend/src/launch/mod.rs`
//! (`LaunchContext::launch`, `process_arguments`, `expand_argument`, `resolve_expansion`). See
//! THIRD_PARTY_NOTICES.md. Changes: no wrapper class or stdin protocol (we start the real main
//! class directly, which keeps us clear of speedrun rules A.8.17 to A.8.19), unknown
//! placeholders are left as they are instead of panicking, offline-account values, and the
//! 26.x `default-user-jvm` arguments.
//!
//! The account is a [`GameAccount`]: an offline account, or a Microsoft account's session as the
//! placeholder values srl-auth hands over (`MinecraftSession::placeholders`). srl-game does not
//! depend on srl-auth: the values are plain strings here, and the access token among them is
//! masked in everything this crate prints or shows ([`GameAccount::redact`],
//! [`LaunchCommand::redacted_args`], the `Debug` impls).
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use srl_core::{Instance, Mode, OfflineAccount};
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

use crate::{
    Installed,
    rules::{Features, Platform, rules_allow},
    schema::Argument,
    stop::QuitRequest,
    util::utf8,
};

/// Placeholder access token for offline accounts. The game only uses it online.
pub const OFFLINE_ACCESS_TOKEN: &str = "0";

/// Lines in the game's output that mean it has loaded its resources and is showing the title
/// screen (on a first 26.x start, the accessibility screen in front of it). 1.16.1 and 26.3 both
/// log them; they are how the launcher tells a game that started from one that died starting.
pub const READY_MARKERS: &[&str] = &["Sound engine started", "textures/atlas/blocks.png-atlas"];

/// What a masked access token reads as in logs and on screen.
pub const HIDDEN_TOKEN: &str = "<access token>";

/// Who the game plays as.
#[derive(Clone, Debug)]
pub enum GameAccount {
    /// Singleplayer only: the offline UUID of the name and a placeholder token.
    Offline(OfflineAccount),
    /// A Microsoft account signed in to Minecraft.
    Microsoft(OnlineSession),
}

impl From<OfflineAccount> for GameAccount {
    fn from(a: OfflineAccount) -> Self {
        Self::Offline(a)
    }
}

/// A Microsoft account's session for one launch, as the version JSON placeholders it fills.
/// `placeholders` is srl-auth's `MinecraftSession::placeholders()` as it is (`auth_player_name`,
/// `auth_uuid`, `auth_access_token`, `auth_session`, `user_type` = `msa`, `auth_xuid`, `clientid`,
/// `user_properties`); `build_command` lets them win over its own values. Its `Debug` prints only
/// the name, the UUID and the placeholder names.
#[derive(Clone)]
pub struct OnlineSession {
    pub name: String,
    pub uuid: Uuid,
    pub placeholders: Vec<(String, String)>,
}

impl std::fmt::Debug for OnlineSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnlineSession")
            .field("name", &self.name)
            .field("uuid", &self.uuid)
            .field(
                "placeholders",
                &self.placeholders.iter().map(|(k, _)| k).collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// The placeholders a Microsoft session must fill: without them the game would start with our
/// offline values under a Microsoft name.
const ONLINE_REQUIRED: [&str; 4] = [
    "auth_player_name",
    "auth_uuid",
    "auth_access_token",
    "user_type",
];

impl OnlineSession {
    fn value(&self, key: &str) -> Option<&str> {
        self.placeholders
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// The access token, for masking it.
    fn token(&self) -> Option<&str> {
        self.value("auth_access_token").filter(|t| !t.is_empty())
    }
}

impl GameAccount {
    pub fn name(&self) -> &str {
        match self {
            Self::Offline(a) => &a.name,
            Self::Microsoft(s) => &s.name,
        }
    }

    pub fn uuid(&self) -> Uuid {
        match self {
            Self::Offline(a) => a.uuid,
            Self::Microsoft(s) => s.uuid,
        }
    }

    pub fn is_microsoft(&self) -> bool {
        matches!(self, Self::Microsoft(_))
    }

    /// `text` with the access token masked (`HIDDEN_TOKEN`), for any log line or command shown.
    /// Offline accounts have nothing to hide.
    pub fn redact(&self, text: &str) -> String {
        match self {
            Self::Microsoft(s) => match s.token() {
                Some(token) => text.replace(token, HIDDEN_TOKEN),
                None => text.to_string(),
            },
            Self::Offline(_) => text.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LaunchOptions {
    pub account: GameAccount,
    /// `-Xmx`, in MB.
    pub max_memory_mb: u32,
    /// `-Xms`, in MB. `None` leaves the JVM default.
    pub min_memory_mb: Option<u32>,
    /// The user's own JVM arguments. When empty, the version's `default-user-jvm` arguments are
    /// used instead (26.x: ZGC, compact object headers), the way the official launcher does.
    pub extra_jvm_args: Vec<String>,
    /// Initial window size (`--width`/`--height`). `None` lets the game decide.
    pub window_size: Option<(u32, u32)>,
    /// Use this `java`/`javaw` instead of the installed runtime.
    pub java_override: Option<PathBuf>,
}

impl LaunchOptions {
    /// The mode's default memory, no extra arguments, the game's default window.
    pub fn new(mode: Mode, account: impl Into<GameAccount>) -> Self {
        Self::with_profile(&crate::profile::profile(mode), account)
    }

    /// [`LaunchOptions::new`] for any target's profile (the practice instance's too).
    pub fn with_profile(profile: &crate::ModeProfile, account: impl Into<GameAccount>) -> Self {
        Self {
            account: account.into(),
            max_memory_mb: profile.default_memory_mb,
            min_memory_mb: None,
            extra_jvm_args: Vec::new(),
            window_size: None,
            java_override: None,
        }
    }
}

/// A fully built command line. Its `Debug` masks the access token (`redacted_args`).
#[derive(Clone, PartialEq, Eq)]
pub struct LaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// The game directory, also the working directory.
    pub cwd: PathBuf,
    /// Set on top of the launcher's environment (after `SCRUBBED_ENV` is removed). Empty for the
    /// plain game; a wrapper such as waywall adds what it needs (srl-tools).
    pub env: Vec<(String, String)>,
}

impl LaunchCommand {
    /// The arguments with the access token masked, wherever the command came from (a wrapper
    /// such as waywall keeps the game's arguments as they are): the value after `--accessToken`
    /// and after `--session`, and a pre-1.7 `token:<token>:<uuid>` session. For logs and for any
    /// command the UI shows.
    pub fn redacted_args(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(self.args.len());
        let mut hide_next = false;
        for a in &self.args {
            if hide_next {
                out.push(HIDDEN_TOKEN.to_string());
                hide_next = false;
                continue;
            }
            hide_next = a == "--accessToken" || a == "--session";
            if a.starts_with("token:") {
                out.push(format!("token:{HIDDEN_TOKEN}"));
            } else {
                out.push(a.clone());
            }
        }
        out
    }
}

impl std::fmt::Debug for LaunchCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaunchCommand")
            .field("program", &self.program)
            .field("args", &self.redacted_args())
            .field("cwd", &self.cwd)
            .field("env", &self.env)
            .finish()
    }
}

/// Replaces every `${name}` that `vars` knows. Unknown placeholders are left as they are, so a
/// new Mojang placeholder shows up in the log instead of crashing the launcher.
pub fn substitute(template: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match vars.get(key) {
                    Some(v) => out.push_str(v),
                    None => {
                        out.push_str("${");
                        out.push_str(key);
                        out.push('}');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Applies rules and substitution to a list of arguments.
pub fn expand(
    args: &[Argument],
    vars: &HashMap<&str, String>,
    platform: &Platform,
    features: &Features,
) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Argument::Plain(s) => out.push(substitute(s, vars)),
            Argument::Ruled { rules, value } => {
                if rules_allow(rules, platform, features) {
                    out.extend(value.values().iter().map(|s| substitute(s, vars)));
                }
            }
        }
    }
    out
}

/// Builds the command that starts `installed` with `opts` on `platform`. Pure: no I/O.
pub fn build_command(
    installed: &Installed,
    opts: &LaunchOptions,
    platform: &Platform,
) -> Result<LaunchCommand> {
    let v = &installed.version;
    let features = Features {
        is_demo_user: false,
        has_custom_resolution: opts.window_size.is_some(),
    };
    let classpath = installed
        .classpath
        .iter()
        .map(|p| utf8(p).map(str::to_string))
        .collect::<Result<Vec<_>>>()?
        .join(platform.classpath_separator());
    let (width, height) = opts.window_size.unwrap_or((854, 480));
    let uuid = opts.account.uuid().simple().to_string();
    let assets_root = utf8(&installed.assets_dir)?.to_string();
    let mut vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", opts.account.name().to_string()),
        // The Minecraft version, not the Fabric profile id: what MultiMC and Prism pass, and what
        // F3 shows next to the brand ("1.16.1/fabric").
        ("version_name", installed.minecraft_version.clone()),
        ("game_directory", utf8(&installed.game_dir)?.to_string()),
        ("assets_root", assets_root.clone()),
        ("game_assets", assets_root),
        ("assets_index_name", installed.asset_index.clone()),
        ("auth_uuid", uuid.clone()),
        ("auth_access_token", OFFLINE_ACCESS_TOKEN.to_string()),
        (
            "auth_session",
            format!("token:{OFFLINE_ACCESS_TOKEN}:{uuid}"),
        ),
        // "legacy" is the offline account type every version understands; 1.16.1 predates "msa".
        // 1.21.9+ no longer passes it at all.
        ("user_type", "legacy".to_string()),
        ("user_properties", "{}".to_string()),
        ("clientid", String::new()),
        ("auth_xuid", String::new()),
        ("version_type", "release".to_string()),
        (
            "natives_directory",
            utf8(&installed.natives_dir)?.to_string(),
        ),
        (
            "library_directory",
            utf8(&installed.libraries_dir)?.to_string(),
        ),
        ("classpath", classpath),
        (
            "classpath_separator",
            platform.classpath_separator().to_string(),
        ),
        ("launcher_name", "SpeedrunningLauncher".to_string()),
        ("launcher_version", env!("CARGO_PKG_VERSION").to_string()),
        ("resolution_width", width.to_string()),
        ("resolution_height", height.to_string()),
    ]);
    // A Microsoft account: srl-auth's values replace the offline ones above (session.rs in
    // srl-auth documents each: `--userType msa`, the UUID, the token, `--xuid`, `--clientId`).
    if let GameAccount::Microsoft(session) = &opts.account {
        if let Some(missing) = ONLINE_REQUIRED
            .iter()
            .find(|k| session.value(k).is_none_or(str::is_empty))
        {
            bail!("the Microsoft session has no {missing}");
        }
        for (key, value) in &session.placeholders {
            vars.insert(key.as_str(), value.clone());
        }
    }

    let mut args = Vec::new();
    // 1. The version's JVM arguments, ending in `-cp <classpath>`. Pre-1.13 versions have none
    //    and get the implied ones.
    match &v.arguments {
        Some(a) if !a.jvm.is_empty() => args.extend(expand(&a.jvm, &vars, platform, &features)),
        _ => {
            if platform.os == crate::rules::Os::MacOs {
                args.push("-XstartOnFirstThread".into());
            }
            args.push(substitute(
                "-Djava.library.path=${natives_directory}",
                &vars,
            ));
            args.push("-cp".into());
            args.push(vars["classpath"].clone());
        }
    }
    // 2. The log4j configuration from the version's `logging` section.
    if let Some((argument, path)) = &installed.log_config {
        let path = HashMap::from([("path", utf8(path)?.to_string())]);
        args.push(substitute(argument, &path));
    }
    // 3. The mode's fixes, unless the version already passes them.
    for a in installed.profile.jvm_args {
        if !args.iter().any(|x| x == a) {
            args.push(a.to_string());
        }
    }
    // 4. Memory, then the user's arguments (last, so they can override anything above).
    if let Some(min) = opts.min_memory_mb {
        args.push(format!("-Xms{}m", min.min(opts.max_memory_mb)));
    }
    args.push(format!("-Xmx{}m", opts.max_memory_mb));
    if opts.extra_jvm_args.is_empty() {
        if let Some(a) = &v.arguments {
            let defaults = expand(&a.default_user_jvm, &vars, platform, &features);
            // Memory comes from the options.
            args.extend(
                defaults
                    .into_iter()
                    .filter(|x| !x.starts_with("-Xmx") && !x.starts_with("-Xms")),
            );
        }
    } else {
        args.extend(opts.extra_jvm_args.iter().cloned());
    }
    // 5. Main class and game arguments.
    let Some(main_class) = &v.main_class else {
        bail!("{} has no main class", v.id);
    };
    args.push(main_class.clone());
    match (&v.arguments, &v.minecraft_arguments) {
        (Some(a), _) if !a.game.is_empty() => {
            args.extend(expand(&a.game, &vars, platform, &features))
        }
        (_, Some(legacy)) => {
            args.extend(legacy.split_whitespace().map(|s| substitute(s, &vars)));
            if let Some((w, h)) = opts.window_size {
                args.extend(["--width".into(), w.to_string()]);
                args.extend(["--height".into(), h.to_string()]);
            }
        }
        _ => bail!("{} has no game arguments", v.id),
    }

    Ok(LaunchCommand {
        program: opts
            .java_override
            .clone()
            .unwrap_or_else(|| installed.java.executable.clone()),
        args,
        cwd: installed.game_dir.clone(),
        env: Vec::new(),
    })
}

/// How the game ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameExit {
    /// `None` when the process was ended by a signal (Unix).
    pub code: Option<i32>,
    pub success: bool,
}

/// A running (or finished) game. Cheap to clone: every clone watches the same process. The game
/// is not tied to this handle: dropping every clone leaves it running.
#[derive(Clone, Debug)]
pub struct GameProcess {
    pid: u32,
    instance: Instance,
    log_path: PathBuf,
    exit: watch::Receiver<Option<GameExit>>,
    kill: mpsc::UnboundedSender<()>,
}

impl GameProcess {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// The mode (or the practice instance) this game runs.
    pub fn instance(&self) -> Instance {
        self.instance
    }

    /// Where stdout and stderr go: `logs/<id>-latest.log`. With Mojang's log4j config the
    /// game's own lines are log4j XML events here; the plain-text log is
    /// `instances/<id>/logs/latest.log`.
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    pub fn is_running(&self) -> bool {
        self.exit.borrow().is_none()
    }

    /// The exit, if the game has ended.
    pub fn exit(&self) -> Option<GameExit> {
        *self.exit.borrow()
    }

    /// Waits until the game exits.
    pub async fn wait(&self) -> GameExit {
        let mut rx = self.exit.clone();
        loop {
            if let Some(e) = *rx.borrow_and_update() {
                return e;
            }
            if rx.changed().await.is_err() {
                // The watcher is gone, which only happens after it sent the exit.
                return rx.borrow().unwrap_or(GameExit {
                    code: None,
                    success: false,
                });
            }
        }
    }

    /// Ends the game at once and waits for it. On Unix the whole process group goes (SIGKILL), so
    /// a wrapper's child (`waywall wrap -- java ...`) can't outlive it; on Windows
    /// TerminateProcess.
    pub async fn kill(&self) -> GameExit {
        let _ = self.kill.send(());
        self.wait().await
    }

    /// Asks the game to quit so it saves (see [`crate::stop`]: its window is closed on X11 and
    /// Windows, else it gets SIGTERM), waits up to `grace`, sends SIGTERM if the window close
    /// didn't end it (another [`TERM_GRACE`]), then kills it.
    pub async fn stop(&self, grace: Duration) -> GameExit {
        self.stop_detailed(grace).await.exit
    }

    /// [`GameProcess::stop`], saying how the game ended. While waiting, the window close is
    /// sent again every [`CLOSE_RESEND`]: a close that reaches the game in the middle of a
    /// stall can be lost, and a second one is what a player would do.
    pub async fn stop_detailed(&self, grace: Duration) -> StopReport {
        if !self.is_running() {
            return StopReport {
                exit: self.wait().await,
                request: None,
                escalated: None,
            };
        }
        let pid = self.pid;
        let targets = tokio::task::spawn_blocking(move || crate::stop::game_pids(pid))
            .await
            .unwrap_or_else(|_| vec![pid]);
        let ask = |targets: Vec<u32>| async move {
            tokio::task::spawn_blocking(move || crate::stop::request_quit(&targets))
                .await
                .unwrap_or(QuitRequest::Signal)
        };
        let request = ask(targets.clone()).await;
        let deadline = tokio::time::Instant::now() + grace;
        loop {
            let step = match request {
                QuitRequest::Window(_) => CLOSE_RESEND,
                QuitRequest::Signal => grace,
            };
            let until = (tokio::time::Instant::now() + step).min(deadline);
            if let Ok(exit) = tokio::time::timeout_at(until, self.wait()).await {
                return StopReport {
                    exit,
                    request: Some(request),
                    escalated: None,
                };
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            ask(targets.clone()).await;
        }
        if cfg!(unix) && matches!(request, QuitRequest::Window(_)) && self.is_running() {
            crate::stop::terminate(&targets);
            if let Ok(exit) = tokio::time::timeout(TERM_GRACE, self.wait()).await {
                return StopReport {
                    exit,
                    request: Some(request),
                    escalated: Some(Escalation::Terminated),
                };
            }
        }
        StopReport {
            exit: self.kill().await,
            request: Some(request),
            escalated: Some(Escalation::Killed),
        }
    }
}

/// How [`GameProcess::stop_detailed`] ended the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopReport {
    pub exit: GameExit,
    /// The first request; `None` when the game had already exited.
    pub request: Option<QuitRequest>,
    /// `None` when the request was enough.
    pub escalated: Option<Escalation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Escalation {
    /// The window close was ignored for the whole grace period; SIGTERM ended it.
    Terminated,
    /// Killed after everything else.
    Killed,
}

/// How often [`GameProcess::stop_detailed`] repeats a window close while it waits.
pub const CLOSE_RESEND: Duration = Duration::from_secs(3);

/// How long [`GameProcess::stop`] waits after SIGTERM when closing the window didn't end the game.
pub const TERM_GRACE: Duration = Duration::from_secs(5);

#[cfg(windows)]
mod windows_flags {
    pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    pub const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
}

/// Environment variables that inject JVM options behind our back (a `-javaagent` there would
/// break speedrun rule A.8.19), so the game never inherits them.
const SCRUBBED_ENV: &[&str] = &["JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS", "JDK_JAVA_OPTIONS"];

/// Starts the game. stdout and stderr go to `log_path` (the previous log is kept as
/// `<id>-previous.log`).
pub async fn spawn(
    cmd: &LaunchCommand,
    instance: impl Into<Instance>,
    log_path: PathBuf,
) -> Result<GameProcess> {
    let instance = instance.into();
    if let Some(dir) = log_path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    if tokio::fs::try_exists(&log_path).await.unwrap_or(false) {
        let previous = log_path.with_file_name(format!("{}-previous.log", instance.id()));
        let _ = tokio::fs::rename(&log_path, previous).await;
    }
    tokio::fs::create_dir_all(&cmd.cwd).await?;
    let out = std::fs::File::create(&log_path)
        .with_context(|| format!("creating {}", log_path.display()))?;
    let err = out.try_clone()?;

    let mut command = tokio::process::Command::new(&cmd.program);
    command
        .args(&cmd.args)
        .current_dir(&cmd.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err))
        .kill_on_drop(false);
    for var in SCRUBBED_ENV {
        command.env_remove(var);
    }
    for (key, value) in &cmd.env {
        command.env(key, value);
    }
    // Its own process group: a Ctrl+C in the launcher's terminal must not reach the game.
    #[cfg(unix)]
    command.process_group(0);
    #[cfg(windows)]
    command
        .creation_flags(windows_flags::CREATE_NO_WINDOW | windows_flags::CREATE_NEW_PROCESS_GROUP);

    let mut child = command
        .spawn()
        .with_context(|| format!("starting {}", cmd.program.display()))?;
    let pid = child.id().context("the game exited at once")?;
    let (exit_tx, exit_rx) = watch::channel(None);
    let (kill_tx, mut kill_rx) = mpsc::unbounded_channel::<()>();
    tokio::spawn(async move {
        let status = loop {
            tokio::select! {
                status = child.wait() => break status,
                msg = kill_rx.recv() => match msg {
                    Some(()) => kill_tree(&mut child, pid),
                    // Every handle is gone: keep the game running, just reap it when it ends.
                    None => break child.wait().await,
                },
            }
        };
        let exit = match status {
            Ok(s) => GameExit {
                code: s.code(),
                success: s.success(),
            },
            Err(_) => GameExit {
                code: None,
                success: false,
            },
        };
        let _ = exit_tx.send(Some(exit));
    });
    Ok(GameProcess {
        pid,
        instance,
        log_path,
        exit: exit_rx,
        kill: kill_tx,
    })
}

/// How often a game from an earlier launcher session is checked for its exit.
const ADOPTED_POLL: Duration = Duration::from_secs(1);

impl GameProcess {
    /// Watches a game this process didn't start: one an earlier launcher session left running
    /// (the caller checked that `pid` is still that game). Its exit code can't be read, so its
    /// end counts as a normal exit; the exit is seen within [`ADOPTED_POLL`]. Stop and kill work
    /// as for a game started here.
    pub fn adopt(pid: u32, instance: impl Into<Instance>, log_path: PathBuf) -> GameProcess {
        let (exit_tx, exit_rx) = watch::channel(None);
        let (kill_tx, mut kill_rx) = mpsc::unbounded_channel::<()>();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(ADOPTED_POLL);
            loop {
                tokio::select! {
                    _ = tick.tick() => {}
                    msg = kill_rx.recv() => match msg {
                        Some(()) => adopted::kill(pid),
                        // Every handle is gone: nobody waits for the exit any more.
                        None => return,
                    },
                }
                if !adopted::alive(pid) {
                    break;
                }
            }
            let _ = exit_tx.send(Some(GameExit {
                code: None,
                success: true,
            }));
        });
        GameProcess {
            pid,
            instance: instance.into(),
            log_path,
            exit: exit_rx,
            kill: kill_tx,
        }
    }
}

/// Liveness and kill for a game that isn't this process's child.
mod adopted {
    #[cfg(target_os = "linux")]
    pub fn alive(pid: u32) -> bool {
        // A zombie has exited; its parent (not us) hasn't reaped it yet.
        std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
            stat.rsplit_once(')')
                .and_then(|(_, rest)| rest.trim_start().chars().next())
                .is_some_and(|state| state != 'Z')
        })
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    pub fn alive(pid: u32) -> bool {
        // SAFETY: signal 0 only checks that the process exists.
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }

    /// SIGKILL to its process group (the launcher that started it made it lead one), so a
    /// wrapper's children go too.
    #[cfg(unix)]
    pub fn kill(pid: u32) {
        // SAFETY: plain syscalls on a game the caller checked is ours.
        unsafe {
            if libc::killpg(pid as libc::pid_t, libc::SIGKILL) != 0 {
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
        }
    }

    #[cfg(windows)]
    pub fn alive(pid: u32) -> bool {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, STILL_ACTIVE},
            System::Threading::{
                GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        };
        // SAFETY: plain Win32 calls on a handle opened and closed here.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return false;
            }
            let mut code = 0u32;
            let alive = GetExitCodeProcess(handle, &mut code) != 0 && code == STILL_ACTIVE as u32;
            CloseHandle(handle);
            alive
        }
    }

    #[cfg(windows)]
    pub fn kill(pid: u32) {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess},
        };
        // SAFETY: plain Win32 calls on a handle opened and closed here.
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if !handle.is_null() {
                TerminateProcess(handle, 1);
                CloseHandle(handle);
            }
        }
    }
}

/// SIGKILL to the game's process group (it leads one, see `spawn`), so a wrapper's children go
/// too; elsewhere `start_kill`.
fn kill_tree(child: &mut tokio::process::Child, pid: u32) {
    #[cfg(unix)]
    // SAFETY: plain syscall. The group is the one `spawn` created for this child, and the child
    // isn't reaped yet (this runs in its watcher before `wait` returns).
    unsafe {
        libc::killpg(pid as libc::pid_t, libc::SIGKILL);
    }
    #[cfg(not(unix))]
    let _ = pid;
    let _ = child.start_kill();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        java::JavaRuntime,
        rules::{Arch, Os},
        schema::VersionJson,
    };

    #[test]
    fn substitution() {
        let vars = HashMap::from([("a", "1".to_string()), ("bb", "two".to_string())]);
        assert_eq!(substitute("${a}", &vars), "1");
        assert_eq!(substitute("x=${a},y=${bb}!", &vars), "x=1,y=two!");
        assert_eq!(substitute("${unknown}", &vars), "${unknown}");
        assert_eq!(substitute("$a ${a", &vars), "$a ${a");
        assert_eq!(substitute("no placeholders", &vars), "no placeholders");
    }

    /// The paths are spelled out for the simulated OS rather than joined, because `Path::join`
    /// uses the separator of the OS the tests run on: on Windows a joined `/srl` path would mix
    /// `\` into a simulated Linux command line.
    fn installed(version_json: &str, os: Os) -> (Installed, Platform) {
        let path = |rel: &str| {
            if os == Os::Windows {
                PathBuf::from(format!("C:\\srl\\{}", rel.replace('/', "\\")))
            } else {
                PathBuf::from(format!("/srl/{rel}"))
            }
        };
        let version: VersionJson = serde_json::from_str(version_json).unwrap();
        let platform = Platform {
            os,
            arch: Arch::X86_64,
            os_version: "10.0.22631".into(),
        };
        let i = Installed {
            instance: Mode::Rsg.into(),
            profile: crate::profile(Mode::Rsg),
            minecraft_version: "1.16.1".into(),
            version_id: version.id.clone(),
            fabric_loader: "0.20.1".into(),
            java: JavaRuntime {
                component: "java-runtime-delta".into(),
                version: "21.0.7".into(),
                home: path("java"),
                executable: path("java/bin/java"),
            },
            version,
            classpath: vec![path("libraries/a.jar"), path("versions/1.16.1/1.16.1.jar")],
            natives_dir: path("natives/1.16.1"),
            libraries_dir: path("libraries"),
            assets_dir: path("assets"),
            asset_index: "1.16".into(),
            log_config: Some((
                "-Dlog4j.configurationFile=${path}".into(),
                path("assets/log_configs/client-1.12.xml"),
            )),
            game_dir: path("instances/rsg"),
            warnings: Vec::new(),
        };
        (i, platform)
    }

    const V1161: &str = r#"{
        "id": "fabric-loader-0.20.1-1.16.1",
        "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
        "arguments": {
          "game": ["--username", "${auth_player_name}", "--version", "${version_name}",
                   "--gameDir", "${game_directory}", "--assetsDir", "${assets_root}",
                   "--assetIndex", "${assets_index_name}", "--uuid", "${auth_uuid}",
                   "--accessToken", "${auth_access_token}", "--userType", "${user_type}",
                   "--versionType", "${version_type}",
                   {"rules": [{"action": "allow", "features": {"is_demo_user": true}}], "value": "--demo"},
                   {"rules": [{"action": "allow", "features": {"has_custom_resolution": true}}],
                    "value": ["--width", "${resolution_width}", "--height", "${resolution_height}"]}],
          "jvm": [{"rules": [{"action": "allow", "os": {"name": "osx"}}], "value": ["-XstartOnFirstThread"]},
                  {"rules": [{"action": "allow", "os": {"name": "windows"}}], "value": "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump"},
                  "-Djava.library.path=${natives_directory}", "-cp", "${classpath}",
                  "-DFabricMcEmu= net.minecraft.client.main.Main "]
        }}"#;

    #[test]
    fn offline_args_linux() {
        let (i, p) = installed(V1161, Os::Linux);
        let account = OfflineAccount::new("Violet_Runner");
        let mut opts = LaunchOptions::new(Mode::Rsg, account.clone());
        opts.window_size = Some((1280, 720));
        let cmd = build_command(&i, &opts, &p).unwrap();
        assert_eq!(cmd.program, PathBuf::from("/srl/java/bin/java"));
        assert_eq!(cmd.cwd, PathBuf::from("/srl/instances/rsg"));
        let a = &cmd.args;
        let after = |flag: &str| a[a.iter().position(|x| x == flag).unwrap() + 1].as_str();
        assert_eq!(
            after("-cp"),
            "/srl/libraries/a.jar:/srl/versions/1.16.1/1.16.1.jar"
        );
        assert!(a.contains(&"-Djava.library.path=/srl/natives/1.16.1".to_string()));
        assert!(a.contains(
            &"-Dlog4j.configurationFile=/srl/assets/log_configs/client-1.12.xml".to_string()
        ));
        assert!(a.contains(&"-Xmx2048m".to_string()));
        assert!(!a.iter().any(|x| x.starts_with("-XstartOnFirstThread")));
        assert!(!a.iter().any(|x| x.starts_with("-XX:HeapDumpPath")));
        // JVM arguments, then the main class, then game arguments.
        let main = a
            .iter()
            .position(|x| x == "net.fabricmc.loader.impl.launch.knot.KnotClient")
            .unwrap();
        assert!(a.iter().position(|x| x == "-Xmx2048m").unwrap() < main);
        assert!(a.iter().position(|x| x == "--username").unwrap() > main);
        assert_eq!(after("--username"), "Violet_Runner");
        assert_eq!(after("--uuid"), account.uuid.simple().to_string());
        assert_eq!(after("--accessToken"), OFFLINE_ACCESS_TOKEN);
        assert_eq!(after("--userType"), "legacy");
        assert_eq!(after("--version"), "1.16.1");
        assert_eq!(after("--gameDir"), "/srl/instances/rsg");
        assert_eq!(after("--assetsDir"), "/srl/assets");
        assert_eq!(after("--assetIndex"), "1.16");
        assert_eq!(after("--width"), "1280");
        assert_eq!(after("--height"), "720");
        assert!(!a.contains(&"--demo".to_string()));
        assert!(!a.iter().any(|x| x.contains("${")), "{a:?}");
    }

    #[test]
    fn windows_separator_and_rules() {
        let (i, p) = installed(V1161, Os::Windows);
        let opts = LaunchOptions::new(Mode::Rsg, OfflineAccount::new("Steve"));
        let cmd = build_command(&i, &opts, &p).unwrap();
        let a = &cmd.args;
        let cp = &a[a.iter().position(|x| x == "-cp").unwrap() + 1];
        let want: Vec<&str> = i.classpath.iter().map(|p| p.to_str().unwrap()).collect();
        assert_eq!(cp, &want.join(";"));
        assert!(a.iter().any(|x| x.starts_with("-XX:HeapDumpPath")));
        assert!(!a.contains(&"--width".to_string()));
    }

    #[test]
    fn default_user_jvm_only_without_user_args() {
        let json = r#"{
            "id": "26.3", "mainClass": "net.minecraft.client.main.Main",
            "arguments": {
              "game": ["--username", "${auth_player_name}"],
              "jvm": ["-XX:StackShadowPages=32", "-cp", "${classpath}"],
              "default-user-jvm": [
                {"value": ["-Xms2G", "-Xmx4G", "-XX:+UseCompactObjectHeaders"]},
                {"rules": [{"action": "allow", "os": {"name": "linux"}}], "value": ["-XX:+UseZGC"]}
              ]
            }}"#;
        let (mut i, p) = installed(json, Os::Linux);
        i.instance = Mode::Draftout.into();
        i.profile = crate::profile(Mode::Draftout);
        let mut opts = LaunchOptions::new(Mode::Draftout, OfflineAccount::new("Steve"));
        let a = build_command(&i, &opts, &p).unwrap().args;
        assert!(a.contains(&"-XX:+UseZGC".to_string()));
        assert!(a.contains(&"-XX:+UseCompactObjectHeaders".to_string()));
        assert!(a.contains(&"-Xmx4096m".to_string()));
        assert!(!a.contains(&"-Xmx4G".to_string()) && !a.contains(&"-Xms2G".to_string()));
        // Draftout's crash workaround is added once; StackShadowPages isn't repeated.
        assert_eq!(
            a.iter()
                .filter(|x| x.starts_with("-XX:CompileCommand=exclude,io/netty"))
                .count(),
            1
        );
        assert_eq!(
            a.iter().filter(|x| *x == "-XX:StackShadowPages=32").count(),
            1
        );
        opts.extra_jvm_args = vec!["-XX:+UseG1GC".into()];
        let a = build_command(&i, &opts, &p).unwrap().args;
        assert!(a.contains(&"-XX:+UseG1GC".to_string()));
        assert!(!a.contains(&"-XX:+UseZGC".to_string()));
    }

    #[test]
    fn legacy_minecraft_arguments() {
        let json = r#"{"id": "1.8.9", "mainClass": "net.minecraft.client.main.Main",
            "minecraftArguments": "--username ${auth_player_name} --session ${auth_session} --userProperties ${user_properties}"}"#;
        let (i, p) = installed(json, Os::Linux);
        let opts = LaunchOptions::new(Mode::Rsg, OfflineAccount::new("Steve"));
        let a = build_command(&i, &opts, &p).unwrap().args;
        assert!(a.contains(&"-Djava.library.path=/srl/natives/1.16.1".to_string()));
        assert!(a.contains(&"-cp".to_string()));
        assert!(a.contains(&"{}".to_string()));
        assert!(a.iter().any(|x| x.starts_with("token:0:")));
    }

    /// The placeholders srl-auth's `MinecraftSession::placeholders` returns for a session.
    fn online() -> OnlineSession {
        let uuid = Uuid::parse_str("069a79f4-44e9-4726-a5be-fca90e38aaf5").unwrap();
        let token = "eyJhbGciOiJIUzI1NiJ9.e30.MC-SECRET-7";
        let simple = uuid.simple().to_string();
        OnlineSession {
            name: "Violet_Runner".into(),
            uuid,
            placeholders: vec![
                ("auth_player_name".into(), "Violet_Runner".into()),
                ("auth_uuid".into(), simple.clone()),
                ("auth_session".into(), format!("token:{token}:{simple}")),
                ("auth_access_token".into(), token.into()),
                ("user_type".into(), "msa".into()),
                ("auth_xuid".into(), "2535400000000001".into()),
                ("clientid".into(), "install-id".into()),
                ("user_properties".into(), "{}".into()),
            ],
        }
    }

    /// A Microsoft session fills exactly what srl-auth's session.rs documents, and nothing this
    /// crate prints or shows carries the token.
    #[test]
    fn microsoft_args_and_redaction() {
        let (i, p) = installed(V1161, Os::Linux);
        let account = GameAccount::Microsoft(online());
        let opts = LaunchOptions::new(Mode::Rsg, account.clone());
        let cmd = build_command(&i, &opts, &p).unwrap();
        let a = &cmd.args;
        let after = |flag: &str| a[a.iter().position(|x| x == flag).unwrap() + 1].as_str();
        assert_eq!(after("--username"), "Violet_Runner");
        assert_eq!(after("--uuid"), "069a79f444e94726a5befca90e38aaf5");
        assert_eq!(
            after("--accessToken"),
            "eyJhbGciOiJIUzI1NiJ9.e30.MC-SECRET-7"
        );
        assert_eq!(after("--userType"), "msa");
        assert!(!a.iter().any(|x| x.contains("${")), "{a:?}");

        let secret = "MC-SECRET-7";
        let shown = cmd.redacted_args().join(" ");
        assert!(!shown.contains(secret), "{shown}");
        assert!(shown.contains("--accessToken <access token> --userType msa"));
        assert!(!format!("{cmd:?}").contains(secret));
        assert!(!format!("{opts:?}").contains(secret));
        assert!(!account.redact(&a.join(" ")).contains(secret));
        assert_eq!(account.name(), "Violet_Runner");

        // Newer versions also ask for --clientId and --xuid; pre-1.7 ones for --session.
        let json = r#"{"id": "26.3", "mainClass": "net.minecraft.client.main.Main",
            "arguments": {"jvm": ["-cp", "${classpath}"],
              "game": ["--uuid", "${auth_uuid}", "--accessToken", "${auth_access_token}",
                       "--clientId", "${clientid}", "--xuid", "${auth_xuid}",
                       "--userType", "${user_type}", "--session", "${auth_session}"]}}"#;
        let (mut i, p) = installed(json, Os::Linux);
        i.instance = Mode::Draftout.into();
        i.profile = crate::profile(Mode::Draftout);
        let opts = LaunchOptions::new(Mode::Draftout, account.clone());
        let cmd = build_command(&i, &opts, &p).unwrap();
        let a = &cmd.args;
        let after = |flag: &str| a[a.iter().position(|x| x == flag).unwrap() + 1].as_str();
        assert_eq!(after("--clientId"), "install-id");
        assert_eq!(after("--xuid"), "2535400000000001");
        assert!(after("--session").starts_with("token:"));
        assert!(!cmd.redacted_args().join(" ").contains(secret));

        // A session without a token is refused, not started with the offline values.
        let mut broken = online();
        broken
            .placeholders
            .retain(|(k, _)| k != "auth_access_token");
        let opts = LaunchOptions::new(Mode::Rsg, GameAccount::Microsoft(broken));
        let (i, p) = installed(V1161, Os::Linux);
        let err = build_command(&i, &opts, &p).unwrap_err().to_string();
        assert!(err.contains("auth_access_token"), "{err}");
    }

    /// The masking works on any command, a wrapped one included, without knowing the account.
    #[test]
    fn redacted_args_mask_the_token_positions() {
        let cmd = LaunchCommand {
            program: "/usr/bin/waywall".into(),
            args: [
                "wrap",
                "--",
                "java",
                "Main",
                "--accessToken",
                "abc",
                "--session",
                "x",
            ]
            .map(String::from)
            .to_vec(),
            cwd: "/srl".into(),
            env: vec![],
        };
        assert_eq!(
            cmd.redacted_args(),
            [
                "wrap",
                "--",
                "java",
                "Main",
                "--accessToken",
                HIDDEN_TOKEN,
                "--session",
                HIDDEN_TOKEN
            ]
        );
    }

    /// Opt-in, Windows (or wine): `stop` closes a GUI program's window with `taskkill` (no
    /// `/F`) instead of killing it.
    #[cfg(windows)]
    #[tokio::test]
    #[ignore]
    async fn stop_closes_the_window_on_windows() {
        let dir = std::env::temp_dir().join(format!("srl-game-stop-{}", std::process::id()));
        let cmd = LaunchCommand {
            program: r"C:\windows\system32\notepad.exe".into(),
            args: vec![],
            cwd: dir.clone(),
            env: vec![],
        };
        let game = spawn(&cmd, Mode::Rsg, dir.join("notepad.log"))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;
        let report = game.stop_detailed(Duration::from_secs(10)).await;
        assert_eq!(report.request, Some(QuitRequest::Window(1)));
        assert_eq!(report.escalated, None, "{report:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A game from an earlier session: running while its process lives, ended by `kill`.
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn adopted_game_is_watched_and_killed() {
        use std::os::unix::process::CommandExt as _;
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let game = GameProcess::adopt(child.id(), Mode::Rsg, PathBuf::from("/nowhere.log"));
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert!(game.is_running());
        let exit = tokio::time::timeout(Duration::from_secs(5), game.kill())
            .await
            .expect("the kill is seen within a poll");
        assert!(exit.success);
        let _ = child.wait();
        assert!(!adopted::alive(child.id()));
    }
}
