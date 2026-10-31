//! Asking a process to quit the way a user would, so Minecraft saves before it exits.
//!
//! Minecraft saves the world and its options when its window closes: the main loop sees the close
//! request, logs `Stopping!` and shuts the integrated server down. A signal skips that, so the
//! launcher asks for a close first:
//!
//! - **Linux (X11, XWayland):** a `WM_DELETE_WINDOW` client message to every top-level window
//!   whose `_NET_WM_PID` is the process, the same message a window manager sends for the close
//!   button. GLFW (1.16.1) and SDL3 (26.x) both answer it. A game with no X11 window (a native
//!   Wayland one, e.g. under waywall) gets `SIGTERM` instead.
//! - **Windows:** `taskkill /PID <pid>` without `/F`, which posts `WM_CLOSE` to the process's
//!   top-level windows.
//! - **macOS and other Unix:** `SIGTERM`.
//!
//! [`crate::GameProcess::stop`] waits a grace period after the request, sends `SIGTERM` if the
//! window close didn't end the game, and kills it after that.

/// How a quit request reached the process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuitRequest {
    /// This many windows were asked to close (`WM_DELETE_WINDOW` on X11, `WM_CLOSE` on Windows).
    Window(usize),
    /// No window to close: `SIGTERM` was sent.
    Signal,
}

/// Asks `pids` to quit: closes their windows where the platform allows it, else signals them.
/// Never blocks for long: one X11 round trip per window on Linux, a detached `taskkill` on
/// Windows.
pub fn request_quit(pids: &[u32]) -> QuitRequest {
    imp::request_quit(pids)
}

/// Sends `SIGTERM` to `pids` (Unix). On Windows it does what [`request_quit`] does.
pub fn terminate(pids: &[u32]) {
    imp::terminate(pids)
}

/// The processes that are the game in the tree started as `root`: `root` itself, or, when `root`
/// is a wrapper (`waywall wrap -- java ...`), the Java processes under it that run Minecraft
/// (their command line has `--gameDir`). Falls back to `[root]` when nothing matches or the
/// platform has no `/proc`.
pub fn game_pids(root: u32) -> Vec<u32> {
    imp::game_pids(root)
}

/// True if a command line (`/proc/<pid>/cmdline` split at NUL) is a Minecraft JVM.
pub fn is_game_cmdline(args: &[&str]) -> bool {
    args.contains(&"--gameDir")
}

#[cfg(target_os = "linux")]
mod imp {
    use std::collections::HashMap;

    use super::QuitRequest;

    pub fn request_quit(pids: &[u32]) -> QuitRequest {
        match x11::close_windows(pids) {
            Ok(n) if n > 0 => QuitRequest::Window(n),
            _ => {
                terminate(pids);
                QuitRequest::Signal
            }
        }
    }

    pub fn terminate(pids: &[u32]) {
        for &pid in pids {
            // SAFETY: plain syscall. Callers pass processes they watch and haven't reaped.
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
        }
    }

    pub fn game_pids(root: u32) -> Vec<u32> {
        let tree = descendants(root);
        let games: Vec<u32> = tree
            .into_iter()
            .filter(|pid| {
                std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|raw| {
                    let text = String::from_utf8_lossy(&raw);
                    let args: Vec<&str> = text.split('\0').collect();
                    super::is_game_cmdline(&args)
                })
            })
            .collect();
        if games.is_empty() { vec![root] } else { games }
    }

    /// `root` and every process below it, from the parent pids in `/proc/<pid>/stat`.
    fn descendants(root: u32) -> Vec<u32> {
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        if let Ok(dir) = std::fs::read_dir("/proc") {
            for entry in dir.flatten() {
                let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|s| s.parse::<u32>().ok())
                else {
                    continue;
                };
                if let Some(ppid) = std::fs::read_to_string(format!("/proc/{pid}/stat"))
                    .ok()
                    .and_then(|s| parent_of(&s))
                {
                    children.entry(ppid).or_default().push(pid);
                }
            }
        }
        let mut out = vec![root];
        let mut i = 0;
        while i < out.len() {
            if let Some(kids) = children.get(&out[i]) {
                for &k in kids {
                    if !out.contains(&k) {
                        out.push(k);
                    }
                }
            }
            i += 1;
        }
        out
    }

    /// The 4th field of `/proc/<pid>/stat`. The 2nd (the name) is in parentheses and may contain
    /// spaces, so parsing starts after the last `)`.
    pub(super) fn parent_of(stat: &str) -> Option<u32> {
        let rest = &stat[stat.rfind(')')? + 1..];
        rest.split_whitespace().nth(1)?.parse().ok()
    }

    mod x11 {
        use anyhow::Result;
        use x11rb::{
            CURRENT_TIME,
            connection::Connection,
            protocol::xproto::{
                AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, Window,
            },
        };

        /// Sends `WM_DELETE_WINDOW` to every window of `pids` that takes it. Returns how many.
        /// Fails when there is no X server (`DISPLAY` unset or unreachable).
        pub fn close_windows(pids: &[u32]) -> Result<usize> {
            if std::env::var_os("DISPLAY").is_none_or(|d| d.is_empty()) {
                return Ok(0);
            }
            let (conn, _) = x11rb::connect(None)?;
            let atom = |name: &str| -> Result<u32> {
                Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom)
            };
            let wm_protocols = atom("WM_PROTOCOLS")?;
            let wm_delete = atom("WM_DELETE_WINDOW")?;
            let net_wm_pid = atom("_NET_WM_PID")?;

            let mut found = 0;
            for screen in &conn.setup().roots {
                // Breadth first, so a window manager's frame windows don't hide the clients.
                let mut queue: Vec<Window> = vec![screen.root];
                let mut i = 0;
                while i < queue.len() {
                    let w = queue[i];
                    i += 1;
                    if let Ok(tree) = conn.query_tree(w)?.reply() {
                        queue.extend(tree.children);
                    }
                    if w == screen.root {
                        continue;
                    }
                    let pid = conn
                        .get_property(false, w, net_wm_pid, AtomEnum::CARDINAL, 0, 1)?
                        .reply()
                        .ok()
                        .and_then(|r| r.value32().and_then(|mut v| v.next()));
                    if !pid.is_some_and(|p| pids.contains(&p)) {
                        continue;
                    }
                    let takes_delete = conn
                        .get_property(false, w, wm_protocols, AtomEnum::ATOM, 0, 32)?
                        .reply()
                        .ok()
                        .and_then(|r| r.value32().map(|mut v| v.any(|a| a == wm_delete)))
                        .unwrap_or(false);
                    if !takes_delete {
                        continue;
                    }
                    let event = ClientMessageEvent::new(
                        32,
                        w,
                        wm_protocols,
                        [wm_delete, CURRENT_TIME, 0, 0, 0],
                    );
                    // No event mask: the X server delivers it to the client that owns the
                    // window, which is what a window manager does for the close button.
                    conn.send_event(false, w, EventMask::NO_EVENT, event)?;
                    found += 1;
                }
            }
            conn.flush()?;
            Ok(found)
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::{os::windows::process::CommandExt as _, process::Stdio};

    use super::QuitRequest;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub fn request_quit(pids: &[u32]) -> QuitRequest {
        terminate(pids);
        QuitRequest::Window(pids.len())
    }

    /// `taskkill` without `/F` posts `WM_CLOSE` to the process's windows. Fire and forget: the
    /// caller waits for the process, not for taskkill.
    pub fn terminate(pids: &[u32]) {
        for &pid in pids {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string()])
                .creation_flags(CREATE_NO_WINDOW)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
        }
    }

    pub fn game_pids(root: u32) -> Vec<u32> {
        vec![root]
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
mod imp {
    use super::QuitRequest;

    pub fn request_quit(pids: &[u32]) -> QuitRequest {
        terminate(pids);
        QuitRequest::Signal
    }

    pub fn terminate(pids: &[u32]) {
        for &pid in pids {
            // SAFETY: plain syscall on a process the caller watches.
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
        }
    }

    pub fn game_pids(root: u32) -> Vec<u32> {
        vec![root]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_cmdline() {
        assert!(is_game_cmdline(&[
            "/srl/java/bin/java",
            "-Xmx2048M",
            "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "--gameDir",
            "/srl/instances/rsg",
        ]));
        assert!(!is_game_cmdline(&[
            "java",
            "-jar",
            "Ninjabrain-Bot-1.5.2.jar"
        ]));
        assert!(!is_game_cmdline(&["waywall", "wrap", "--"]));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parent_from_stat() {
        assert_eq!(
            imp::parent_of("1234 (Render thread) S 99 1234 1234 0 -1"),
            Some(99)
        );
        assert_eq!(imp::parent_of("7 (a) b) R 3 7 7"), Some(3));
        assert_eq!(imp::parent_of("garbage"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn own_process_is_its_own_game_fallback() {
        let me = std::process::id();
        assert_eq!(game_pids(me), vec![me]);
    }
}
