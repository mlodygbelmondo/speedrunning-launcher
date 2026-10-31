//! Tool processes that run next to the game (Ninjabrain Bot): started detached from the
//! launcher's terminal, logged to `logs/<tool>.log`, stopped politely, then killed.
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anyhow::{Context, Result};
use srl_game::QuitRequest;
use tokio::sync::{mpsc, watch};

use crate::ToolId;

/// A command line for a side process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

/// A running side process. Cheap to clone; dropping every clone leaves it running.
#[derive(Clone, Debug)]
pub struct SideProcess {
    tool: ToolId,
    pid: u32,
    exit: watch::Receiver<bool>,
    kill: mpsc::UnboundedSender<()>,
}

/// Variables that inject JVM options (a Java agent among them), never passed to a Java tool.
const SCRUBBED_ENV: &[&str] = &["JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS", "JDK_JAVA_OPTIONS"];

impl SideProcess {
    pub async fn spawn(tool: ToolId, cmd: &SideCommand, log: &Path) -> Result<SideProcess> {
        if let Some(dir) = log.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        tokio::fs::create_dir_all(&cmd.cwd).await?;
        let out =
            std::fs::File::create(log).with_context(|| format!("creating {}", log.display()))?;
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
        #[cfg(unix)]
        command.process_group(0);
        #[cfg(windows)]
        {
            // No console window, and a Ctrl+C in the launcher's console doesn't reach it.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("starting {}", cmd.program.display()))?;
        let pid = child.id().context("the process exited at once")?;
        let (exit_tx, exit_rx) = watch::channel(false);
        let (kill_tx, mut kill_rx) = mpsc::unbounded_channel::<()>();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = child.wait() => break,
                    msg = kill_rx.recv() => match msg {
                        Some(()) => { let _ = child.start_kill(); }
                        None => { let _ = child.wait().await; break; }
                    },
                }
            }
            let _ = exit_tx.send(true);
        });
        Ok(SideProcess {
            tool,
            pid,
            exit: exit_rx,
            kill: kill_tx,
        })
    }

    /// A process an earlier launcher session started (found through its pid file), so it is
    /// no child of ours: its exit is noticed by polling, and a kill goes by pid.
    pub fn adopt(tool: ToolId, pid: u32) -> SideProcess {
        let (exit_tx, exit_rx) = watch::channel(!sys::alive(pid));
        let (kill_tx, mut kill_rx) = mpsc::unbounded_channel::<()>();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(500));
            loop {
                tokio::select! {
                    _ = tick.tick() => if !sys::alive(pid) { break },
                    msg = kill_rx.recv() => match msg {
                        Some(()) => sys::kill(pid),
                        // Nobody holds it any more: stop watching, leave it running.
                        None => return,
                    },
                }
            }
            let _ = exit_tx.send(true);
        });
        SideProcess {
            tool,
            pid,
            exit: exit_rx,
            kill: kill_tx,
        }
    }

    pub fn tool(&self) -> ToolId {
        self.tool
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn is_running(&self) -> bool {
        !*self.exit.borrow()
    }

    pub async fn wait(&self) {
        let mut rx = self.exit.clone();
        while !*rx.borrow_and_update() {
            if rx.changed().await.is_err() {
                return;
            }
        }
    }

    /// Closes its window (srl-game's quit request: `WM_DELETE_WINDOW` on X11, `WM_CLOSE` on
    /// Windows, else SIGTERM; Ninjabrain Bot saves its window position and state either way),
    /// waits up to `grace`, then kills it.
    pub async fn stop(&self, grace: Duration) -> Stopped {
        if !self.is_running() {
            return Stopped::AlreadyExited;
        }
        let pid = self.pid;
        let request = tokio::task::spawn_blocking(move || srl_game::stop::request_quit(&[pid]))
            .await
            .unwrap_or(QuitRequest::Signal);
        if tokio::time::timeout(grace, self.wait()).await.is_ok() {
            return Stopped::Quit(request);
        }
        let _ = self.kill.send(());
        self.wait().await;
        Stopped::Killed
    }
}

/// Written next to the game logs (`logs/tools/<tool>.pid`) while a tool the launcher started
/// runs, so a later launcher session can find it, take it over and stop it when it isn't needed.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PidFile {
    pub pid: u32,
    /// Seconds since the Unix epoch when the launcher started it.
    pub started: u64,
    /// An argument of its command line (Ninjabrain Bot's jar), to tell it from a process that
    /// got the same pid later.
    pub arg: String,
}

impl PidFile {
    pub fn path(paths: &srl_core::Paths, tool: ToolId) -> PathBuf {
        paths
            .logs()
            .join("tools")
            .join(format!("{}.pid", tool.id()))
    }

    pub fn read(paths: &srl_core::Paths, tool: ToolId) -> Option<PidFile> {
        serde_json::from_slice(&std::fs::read(Self::path(paths, tool)).ok()?).ok()
    }

    pub fn write(&self, paths: &srl_core::Paths, tool: ToolId) -> Result<()> {
        let path = Self::path(paths, tool);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("pid.part");
        std::fs::write(&tmp, serde_json::to_vec(self)?)?;
        std::fs::rename(&tmp, &path).with_context(|| format!("writing {}", path.display()))
    }

    pub fn remove(paths: &srl_core::Paths, tool: ToolId) {
        let _ = std::fs::remove_file(Self::path(paths, tool));
    }

    /// Whether the process it names is still the one the launcher started.
    pub fn is_live(&self) -> bool {
        sys::is_same(self)
    }
}

pub(crate) fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(unix)]
mod sys {
    pub fn alive(pid: u32) -> bool {
        // SAFETY: signal 0 only checks that the process exists.
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }

    pub fn kill(pid: u32) {
        // SAFETY: a plain signal to one pid.
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }
    }

    #[cfg(target_os = "linux")]
    pub fn is_same(stamp: &super::PidFile) -> bool {
        std::fs::read(format!("/proc/{}/cmdline", stamp.pid))
            .is_ok_and(|raw| raw.split(|b| *b == 0).any(|a| a == stamp.arg.as_bytes()))
    }

    #[cfg(not(target_os = "linux"))]
    pub fn is_same(stamp: &super::PidFile) -> bool {
        alive(stamp.pid)
    }
}

#[cfg(windows)]
mod sys {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE, STILL_ACTIVE},
        System::Threading::{
            GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_TERMINATE, TerminateProcess,
        },
    };

    fn open(pid: u32, access: u32) -> Option<HANDLE> {
        // SAFETY: a plain Win32 call; the caller closes the handle.
        let h = unsafe { OpenProcess(access, 0, pid) };
        (!h.is_null()).then_some(h)
    }

    pub fn alive(pid: u32) -> bool {
        let Some(h) = open(pid, PROCESS_QUERY_LIMITED_INFORMATION) else {
            return false;
        };
        let mut code = 0u32;
        // SAFETY: `h` is a live handle we opened and close here.
        unsafe {
            let ok = GetExitCodeProcess(h, &mut code) != 0;
            CloseHandle(h);
            ok && code == STILL_ACTIVE as u32
        }
    }

    pub fn kill(pid: u32) {
        if let Some(h) = open(pid, PROCESS_TERMINATE) {
            // SAFETY: `h` is a live handle we opened and close here.
            unsafe {
                TerminateProcess(h, 1);
                CloseHandle(h);
            }
        }
    }

    /// Alive, and created when the launcher started it (not later, under a reused pid).
    pub fn is_same(stamp: &super::PidFile) -> bool {
        if !alive(stamp.pid) {
            return false;
        }
        let Some(h) = open(stamp.pid, PROCESS_QUERY_LIMITED_INFORMATION) else {
            return false;
        };
        let zero = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
        // SAFETY: `h` is a live handle we opened and close here.
        let ok = unsafe {
            let ok = GetProcessTimes(h, &mut created, &mut exited, &mut kernel, &mut user) != 0;
            CloseHandle(h);
            ok
        };
        // FILETIME counts 100 ns steps since 1601.
        let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
        let created_unix = (ticks / 10_000_000).saturating_sub(11_644_473_600);
        ok && created_unix.abs_diff(stamp.started) <= 30
    }
}

/// How [`SideProcess::stop`] ended the process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stopped {
    /// It quit after the request (a window close or SIGTERM).
    Quit(QuitRequest),
    /// It ignored the request and was killed after the grace period.
    Killed,
    AlreadyExited,
}
