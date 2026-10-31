//! The self-update extension of the facade (srl-update). Settings → About and the top nav read
//! `Updates::snapshot` once, then apply `Event::Update`.
//!
//! Only a Windows build from CI updates itself: a local build (no `SRL_COMMIT`) or another system
//! says so and does nothing (`Unavailable`). `SRL_UPDATE_URL` points the check at another release
//! JSON than GitHub's (a test server) and then lets the check run on any system. `SRL_FAKE` and
//! `SRL_DEMO` use `FakeUpdates`, which never touches the network.
//!
//! **When the swap happens:** right after the download is verified and staged, not on Restart.
//! The new exe is then in place whatever the player does next, so the update applies on any next
//! start; "Restart to update" only makes it sooner. A failure anywhere leaves the running version
//! as it was (`srl_update::swap`).
//!
//! The check runs once on start when Settings allow it (`Settings::auto_update`, on by default),
//! and by hand from About. An automatic check that fails only logs; one by hand says why.
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use srl_update::{Check, ErrorKind, SwapError, Updater, swap};

use super::{Event, Events, real::log};

/// Why this build doesn't update itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unavailable {
    /// Built on a developer's machine: its version doesn't say what code it has.
    LocalBuild,
    /// Releases have a Windows build only.
    Platform,
}

/// Why a check by hand failed, as About words it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateFailure {
    Offline,
    RateLimited,
    Checksum,
    /// Anything else; `UpdateState::Failed::detail` has the English chain.
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Unavailable(Unavailable),
    /// Not checked in this session (the automatic check is off, or it failed quietly).
    Idle,
    Checking,
    UpToDate,
    /// The new version is downloading (`size` bytes).
    Downloading {
        version: String,
        size: u64,
    },
    /// The new version is in place and starts with the next start.
    Ready {
        version: String,
    },
    /// A newer version is out, but the launcher's folder can't be written: the player
    /// downloads it from `page`.
    Manual {
        version: String,
        dir: PathBuf,
        page: String,
    },
    Failed {
        why: UpdateFailure,
        detail: String,
    },
}

impl UpdateState {
    /// A check or download is under way.
    pub fn busy(&self) -> bool {
        matches!(
            self,
            UpdateState::Checking | UpdateState::Downloading { .. }
        )
    }
}

/// The updates side of the facade. Like `Backend`: methods return at once, changes arrive as
/// `Event::Update`.
pub trait Updates {
    fn snapshot(&self) -> UpdateState;
    /// Checks now (About's button), and downloads and installs a newer version. Does nothing
    /// while a check runs or once an update is ready.
    fn check(&self);
    /// Starts the new version. `true`: it started and the launcher should quit now. The UI
    /// offers it only while no game runs and nothing installs.
    fn restart(&self) -> bool;
}

/// The states the fake starts in (`SRL_SCREEN=update-*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdatePreset {
    /// Checked on start, nothing new. Check now finds 0.2.0, downloads it and makes it ready.
    Default,
    Checking,
    Downloading,
    Ready,
    Manual,
    Failed,
    Local,
}

/// The version the fake offers.
const FAKE_VERSION: &str = "0.2.0";

fn preset_state(preset: UpdatePreset, data_dir: &std::path::Path) -> UpdateState {
    let version = FAKE_VERSION.to_string();
    match preset {
        UpdatePreset::Default => UpdateState::UpToDate,
        UpdatePreset::Checking => UpdateState::Checking,
        UpdatePreset::Downloading => UpdateState::Downloading {
            version,
            size: 26 << 20,
        },
        UpdatePreset::Ready => UpdateState::Ready { version },
        UpdatePreset::Manual => UpdateState::Manual {
            version,
            // Where a player would have put it by hand.
            dir: if cfg!(windows) {
                PathBuf::from(r"C:\Program Files\Speedrunning Launcher")
            } else {
                data_dir.join("app")
            },
            page: srl_update::RELEASES_PAGE.into(),
        },
        UpdatePreset::Failed => UpdateState::Failed {
            why: UpdateFailure::Offline,
            detail: "GET https://api.github.com/…: connection refused (simulated)".into(),
        },
        UpdatePreset::Local => UpdateState::Unavailable(Unavailable::LocalBuild),
    }
}

fn set(state: &Mutex<UpdateState>, events: &Events, new: UpdateState) {
    let mut s = state.lock().unwrap();
    if *s != new {
        *s = new.clone();
        let _ = events.unbounded_send(Event::Update(new));
    }
}

/// Where the update comes from and which exe it replaces, or why there is none.
fn detect() -> Result<(String, PathBuf), Unavailable> {
    if option_env!("SRL_COMMIT").is_none() {
        return Err(Unavailable::LocalBuild);
    }
    let url = std::env::var("SRL_UPDATE_URL")
        .ok()
        .filter(|u| !u.trim().is_empty());
    if !cfg!(windows) && url.is_none() {
        return Err(Unavailable::Platform);
    }
    let exe = std::env::current_exe().map_err(|e| {
        log(format_args!("update: no path for the launcher's exe: {e}"));
        Unavailable::Platform
    })?;
    Ok((url.unwrap_or_else(|| srl_update::LATEST_URL.into()), exe))
}

/// `RealBackend`'s updates: srl-update on the backend's runtime.
pub struct RealUpdates {
    state: Arc<Mutex<UpdateState>>,
    /// `None` when this build doesn't update itself.
    job: Option<Arc<Job>>,
    events: Events,
    rt: tokio::runtime::Handle,
}

struct Job {
    updater: Updater,
    /// The exe the launcher started from, read once at start: after a swap the running file has
    /// another name (Linux follows it in `current_exe`), and this path holds the new version.
    exe: PathBuf,
}

impl RealUpdates {
    pub fn new(
        paths: &srl_core::Paths,
        downloader: srl_core::Downloader,
        events: Events,
        rt: tokio::runtime::Handle,
    ) -> Self {
        let job = detect().and_then(|(url, exe)| {
            let updater = Updater::new(
                downloader,
                url,
                env!("CARGO_PKG_VERSION"),
                paths.root().join("updates"),
            )
            .map_err(|e| {
                log(format_args!("update: {e:#}"));
                Unavailable::LocalBuild
            })?;
            Ok(Arc::new(Job { updater, exe }))
        });
        let (state, job) = match job {
            Ok(job) => (UpdateState::Idle, Some(job)),
            Err(why) => (UpdateState::Unavailable(why), None),
        };
        Self {
            state: Arc::new(Mutex::new(state)),
            job,
            events,
            rt,
        }
    }

    /// On start: deletes what the last swap left next to the exe, then checks if `auto`.
    pub fn start(&self, auto: bool) {
        let Some(job) = self.job.clone() else {
            return;
        };
        log(format_args!(
            "update: version {}, exe {}, automatic check {}",
            job.updater.current(),
            job.exe.display(),
            if auto { "on" } else { "off" }
        ));
        self.rt.spawn(async move {
            // The previous launcher may take a moment to quit after Restart: Windows doesn't
            // delete an exe that still runs.
            for wait in [0, 2, 10] {
                tokio::time::sleep(Duration::from_secs(wait)).await;
                let exe = job.exe.clone();
                let tried = tokio::task::spawn_blocking(move || swap::cleanup_old(&exe))
                    .await
                    .unwrap_or_default();
                let mut left = false;
                for (path, r) in tried {
                    match r {
                        Ok(()) => log(format_args!("update: removed {}", path.display())),
                        Err(e) => {
                            left = true;
                            log(format_args!(
                                "update: can't remove {} yet: {e}",
                                path.display()
                            ))
                        }
                    }
                }
                if !left {
                    break;
                }
            }
        });
        if auto {
            self.run(false);
        }
    }

    fn run(&self, by_hand: bool) {
        let Some(job) = self.job.clone() else {
            return;
        };
        let (state, events) = (self.state.clone(), self.events.clone());
        {
            // Checked and claimed under one lock: two clicks start one check.
            let mut s = state.lock().unwrap();
            if s.busy() || matches!(*s, UpdateState::Ready { .. }) {
                return;
            }
            *s = UpdateState::Checking;
            let _ = events.unbounded_send(Event::Update(UpdateState::Checking));
        }
        self.rt.spawn(async move {
            let started = std::time::Instant::now();
            let end = match update(&job, &|s| set(&state, &events, s)).await {
                Ok(end) => end,
                Err(e) => {
                    log(format_args!(
                        "update: {} failed: {}",
                        if by_hand {
                            "the check"
                        } else {
                            "the automatic check"
                        },
                        e.detail
                    ));
                    if by_hand {
                        let why = match e.kind {
                            ErrorKind::Offline => UpdateFailure::Offline,
                            ErrorKind::RateLimited => UpdateFailure::RateLimited,
                            ErrorKind::Checksum => UpdateFailure::Checksum,
                            ErrorKind::Other => UpdateFailure::Other,
                        };
                        UpdateState::Failed {
                            why,
                            detail: e.detail,
                        }
                    } else {
                        UpdateState::Idle
                    }
                }
            };
            log(format_args!(
                "update: {end:?} after {:.1}s",
                started.elapsed().as_secs_f64()
            ));
            set(&state, &events, end);
        });
    }
}

/// One check: the release, and when it is newer, the download, the checks and the swap.
async fn update(
    job: &Job,
    progress: &(dyn Fn(UpdateState) + Sync),
) -> Result<UpdateState, srl_update::UpdateError> {
    let offer = match job.updater.check().await? {
        Check::UpToDate { latest } => {
            log(format_args!(
                "update: up to date (latest release: {})",
                latest.as_deref().unwrap_or("none")
            ));
            return Ok(UpdateState::UpToDate);
        }
        Check::Newer(offer) => offer,
    };
    let version = offer.version.to_string();
    log(format_args!(
        "update: {} is out ({} bytes)",
        offer.tag, offer.zip.size
    ));
    let dir = job.exe.parent().map(PathBuf::from).unwrap_or_default();
    let manual = |why: &dyn std::fmt::Display| {
        log(format_args!(
            "update: {} can't be written ({why}); the player downloads {} by hand",
            dir.display(),
            offer.tag
        ));
        UpdateState::Manual {
            version: version.clone(),
            dir: dir.clone(),
            page: offer.page.clone(),
        }
    };
    // Program Files or a read-only drive: no point downloading what can't be put in place.
    if let Err(e) = swap::writable(&dir) {
        return Ok(manual(&e));
    }
    progress(UpdateState::Downloading {
        version: version.clone(),
        size: offer.zip.size,
    });
    let staged = job.updater.download(&offer).await?;
    log(format_args!(
        "update: {} verified and staged in {}",
        offer.tag,
        staged.dir.display()
    ));
    let exe = job.exe.clone();
    let swap_staged = staged.clone();
    let swapped = tokio::task::spawn_blocking(move || swap::install(&swap_staged, &exe))
        .await
        .map_err(|e| srl_update::UpdateError {
            kind: ErrorKind::Other,
            detail: e.to_string(),
        })?;
    job.updater.forget(&staged.version);
    match swapped {
        Ok(done) => {
            for note in &done.notes {
                log(format_args!("update: not copied: {note}"));
            }
            log(format_args!(
                "update: {} is in place; the previous exe is {}",
                offer.tag,
                done.old.display()
            ));
            Ok(UpdateState::Ready { version })
        }
        Err(SwapError::NotWritable(e)) => Ok(manual(&e)),
        Err(e) => Err(srl_update::UpdateError {
            kind: ErrorKind::Other,
            detail: e.to_string(),
        }),
    }
}

impl Updates for RealUpdates {
    fn snapshot(&self) -> UpdateState {
        self.state.lock().unwrap().clone()
    }

    fn check(&self) {
        self.run(true);
    }

    fn restart(&self) -> bool {
        let Some(job) = &self.job else {
            return false;
        };
        if !matches!(*self.state.lock().unwrap(), UpdateState::Ready { .. }) {
            return false;
        }
        log(format_args!(
            "update: restarting into {}",
            job.exe.display()
        ));
        match std::process::Command::new(&job.exe)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => true,
            Err(e) => {
                log(format_args!(
                    "update: couldn't start {}: {e}",
                    job.exe.display()
                ));
                set(
                    &self.state,
                    &self.events,
                    UpdateState::Failed {
                        why: UpdateFailure::Other,
                        detail: format!("starting {}: {e}", job.exe.display()),
                    },
                );
                false
            }
        }
    }
}

/// Simulated updates for screenshots and UI work, on a thread of its own.
pub struct FakeUpdates {
    state: Arc<Mutex<UpdateState>>,
    events: Events,
}

impl FakeUpdates {
    pub fn new(events: Events, preset: UpdatePreset, data_dir: &std::path::Path) -> Self {
        Self {
            state: Arc::new(Mutex::new(preset_state(preset, data_dir))),
            events,
        }
    }
}

impl Updates for FakeUpdates {
    fn snapshot(&self) -> UpdateState {
        self.state.lock().unwrap().clone()
    }

    fn check(&self) {
        {
            let s = self.state.lock().unwrap();
            if s.busy() || matches!(*s, UpdateState::Ready { .. } | UpdateState::Unavailable(_)) {
                return;
            }
        }
        let (state, events) = (self.state.clone(), self.events.clone());
        set(&state, &events, UpdateState::Checking);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(1200));
            set(
                &state,
                &events,
                UpdateState::Downloading {
                    version: FAKE_VERSION.into(),
                    size: 26 << 20,
                },
            );
            thread::sleep(Duration::from_secs(2));
            set(
                &state,
                &events,
                UpdateState::Ready {
                    version: FAKE_VERSION.into(),
                },
            );
        });
    }

    fn restart(&self) -> bool {
        eprintln!("fake: would start the new version and quit");
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A build from this machine has no `SRL_COMMIT`: it never checks, and says why.
    #[test]
    fn a_local_build_never_checks() {
        if option_env!("SRL_COMMIT").is_none() {
            assert_eq!(detect(), Err(Unavailable::LocalBuild));
        }
    }

    #[test]
    fn the_fake_checks_downloads_and_gets_ready() {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let fake = FakeUpdates::new(tx, UpdatePreset::Default, std::path::Path::new("/srl"));
        assert_eq!(fake.snapshot(), UpdateState::UpToDate);
        fake.check();
        let mut seen = Vec::new();
        while let Some(Event::Update(s)) =
            futures::executor::block_on(futures::StreamExt::next(&mut rx))
        {
            let ready = matches!(s, UpdateState::Ready { .. });
            seen.push(s);
            if ready {
                break;
            }
        }
        assert!(
            matches!(
                seen[..],
                [
                    UpdateState::Checking,
                    UpdateState::Downloading { .. },
                    UpdateState::Ready { .. }
                ]
            ),
            "{seen:?}"
        );
        // Ready: a second check does nothing, and the fake never really restarts.
        fake.check();
        assert!(!fake.restart());
        assert!(matches!(fake.snapshot(), UpdateState::Ready { .. }));
    }
}
