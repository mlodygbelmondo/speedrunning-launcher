//! Self-update for Speedrunning Launcher. The app decides when it runs (Windows release builds,
//! `srl-app/src/backend/update.rs`); this crate does the work:
//!
//! 1. **Check** (`Updater::check`): the latest GitHub release (`releases/latest`, which never
//!    returns a prerelease, so the `launcher-latest` test build is never offered) against the
//!    running version, by semver. Only a newer version counts, and only with both assets:
//!    `speedrunning-launcher-windows.zip` and `speedrunning-launcher-windows.zip.sha256`
//!    (sha256sum's format, written by CI).
//! 2. **Download** (`Updater::download`): the checksum first, then the zip into
//!    `<data>/updates/<version>/` through srl-core's `Downloader`, which checks the SHA-256 before
//!    the file gets its name.
//! 3. **Stage:** the zip is untrusted data. Every entry name passes the zip-slip guard, symlinks
//!    are refused and entries and sizes are capped (`archive`); the staged folder must hold
//!    `speedrunning-launcher.exe`.
//! 4. **Swap** (`swap::install`): the running exe is renamed to `<name>.old.exe` (Windows lets a
//!    running exe be renamed, not replaced), the new one takes its name and the zip's other files
//!    are copied next to it. A failed step puts the old exe back. The next start deletes the
//!    `.old.exe` (`swap::cleanup_old`).
//!
//! Every failure is an `UpdateError` whose kind the UI can word (no network, GitHub's rate limit,
//! a bad checksum); the rest is the English detail for the log.
pub mod archive;
pub mod release;
pub mod swap;

use std::{
    fmt,
    path::{Path, PathBuf},
    time::Duration,
};

use semver::Version;
use srl_core::{Download, Downloader, Hash, HashMismatch};

pub use release::{Asset, Offer, Release};
pub use swap::{Installed, SwapError};

/// The latest release of the public repository, as GitHub's API describes it.
pub const LATEST_URL: &str =
    "https://api.github.com/repos/mlodygbelmondo/speedrunning-launcher/releases/latest";
/// Where a player downloads a release by hand.
pub const RELEASES_PAGE: &str =
    "https://github.com/mlodygbelmondo/speedrunning-launcher/releases/latest";
/// The release's Windows build (CI's `Package` step).
pub const ZIP_NAME: &str = "speedrunning-launcher-windows.zip";
/// Its SHA-256, one `sha256sum` line: `<64 hex>  speedrunning-launcher-windows.zip`.
pub const CHECKSUM_NAME: &str = "speedrunning-launcher-windows.zip.sha256";
/// The launcher in the zip, at its root.
pub const EXE_NAME: &str = "speedrunning-launcher.exe";
/// A bigger zip is refused before it is downloaded. Today's is about 25 MB.
pub const MAX_ZIP_BYTES: u64 = 256 << 20;
/// A checksum file is one line.
const MAX_CHECKSUM_BYTES: usize = 4096;
/// The whole release check, connect included. The answer is a few KB.
const CHECK_DEADLINE: Duration = Duration::from_secs(20);

/// What went wrong, as the UI words it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// GitHub or the download could not be reached (refused, timed out, no DNS).
    Offline,
    /// GitHub's API refused the request (403 or 429): 60 requests an hour without a token.
    RateLimited,
    /// The zip's SHA-256 is not the one in the checksum asset. The file is deleted.
    Checksum,
    /// Anything else: a release without the assets, bad JSON, a bad zip, the disk.
    Other,
}

#[derive(Debug)]
pub struct UpdateError {
    pub kind: ErrorKind,
    /// The English error chain, for the log.
    pub detail: String,
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for UpdateError {}

impl UpdateError {
    fn other(detail: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Other,
            detail: detail.into(),
        }
    }

    /// From an error chain: a hash mismatch or a network error anywhere in it decides the kind.
    fn from_chain(e: &anyhow::Error) -> Self {
        let kind = if e.chain().any(|c| c.is::<HashMismatch>()) {
            ErrorKind::Checksum
        } else if e.chain().any(is_network) {
            ErrorKind::Offline
        } else if e.chain().any(|c| {
            c.downcast_ref::<reqwest::Error>()
                .and_then(reqwest::Error::status)
                .is_some_and(rate_limited)
        }) {
            ErrorKind::RateLimited
        } else {
            ErrorKind::Other
        };
        Self {
            kind,
            detail: format!("{e:#}"),
        }
    }
}

fn rate_limited(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::TOO_MANY_REQUESTS
}

/// A connection that could not be made or timed out, as the app's install errors count it.
fn is_network(e: &(dyn std::error::Error + 'static)) -> bool {
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
}

/// The answer of a check.
#[derive(Clone, Debug, PartialEq)]
pub enum Check {
    /// Nothing newer. `latest` is the newest release's tag, `None` when there is no release yet.
    UpToDate {
        latest: Option<String>,
    },
    Newer(Offer),
}

/// A downloaded, verified and extracted release, ready for `swap::install`.
#[derive(Clone, Debug, PartialEq)]
pub struct Staged {
    pub version: Version,
    /// The zip's files, extracted.
    pub dir: PathBuf,
    /// Every file in `dir`, relative to it (the exe among them).
    pub files: Vec<PathBuf>,
}

impl Staged {
    pub fn exe(&self) -> PathBuf {
        self.dir.join(EXE_NAME)
    }
}

pub struct Updater {
    downloader: Downloader,
    latest_url: String,
    current: Version,
    /// `<data>/updates`: one folder per version being downloaded.
    dir: PathBuf,
}

impl Updater {
    /// `latest_url` is `LATEST_URL` (or a test server's), `current` the running version
    /// (`CARGO_PKG_VERSION`), `dir` where downloads and staging go.
    pub fn new(
        downloader: Downloader,
        latest_url: impl Into<String>,
        current: &str,
        dir: PathBuf,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            downloader,
            latest_url: latest_url.into(),
            current: Version::parse(current)?,
            dir,
        })
    }

    pub fn current(&self) -> &Version {
        &self.current
    }

    /// Asks GitHub for the latest release, once, with a short deadline: a check that fails is
    /// tried again on the next start or by hand.
    pub async fn check(&self) -> Result<Check, UpdateError> {
        let resp = self
            .downloader
            .client()
            .get(&self.latest_url)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .timeout(CHECK_DEADLINE)
            .send()
            .await
            .map_err(|e| {
                UpdateError::from_chain(
                    &anyhow::Error::new(e).context(format!("GET {}", self.latest_url)),
                )
            })?;
        let status = resp.status();
        // No release published yet.
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(Check::UpToDate { latest: None });
        }
        if rate_limited(status) {
            return Err(UpdateError {
                kind: ErrorKind::RateLimited,
                detail: format!("GET {}: HTTP {status}", self.latest_url),
            });
        }
        let body = resp
            .error_for_status()
            .map_err(|e| UpdateError::from_chain(&e.into()))?
            .bytes()
            .await
            .map_err(|e| UpdateError::from_chain(&e.into()))?;
        let release: Release = serde_json::from_slice(&body).map_err(|e| {
            UpdateError::other(format!("the release from {}: {e}", self.latest_url))
        })?;
        match release.offer(&self.current).map_err(UpdateError::other)? {
            Some(offer) => Ok(Check::Newer(offer)),
            None => Ok(Check::UpToDate {
                latest: Some(release.tag_name),
            }),
        }
    }

    /// Downloads the offer's checksum and zip, verifies the zip and extracts it into
    /// `<dir>/<version>/staged`. Folders of other versions in `dir` are removed first: one
    /// download is kept at a time, and an unfinished one of the same version is resumed from
    /// its verified zip.
    pub async fn download(&self, offer: &Offer) -> Result<Staged, UpdateError> {
        let vdir = self.dir.join(offer.version.to_string());
        self.forget_others(&offer.version);
        tokio::fs::create_dir_all(&vdir)
            .await
            .map_err(|e| UpdateError::other(format!("creating {}: {e}", vdir.display())))?;
        let sum = self
            .downloader
            .fetch_bytes(&offer.checksum.browser_download_url)
            .await
            .map_err(|e| UpdateError::from_chain(&e))?;
        if sum.len() > MAX_CHECKSUM_BYTES {
            return Err(UpdateError::other(format!(
                "{} is {} bytes, not one line",
                CHECKSUM_NAME,
                sum.len()
            )));
        }
        let want = release::parse_checksum(&String::from_utf8_lossy(&sum), ZIP_NAME)
            .map_err(UpdateError::other)?;
        let zip = vdir.join(ZIP_NAME);
        self.downloader
            .download(&Download {
                url: offer.zip.browser_download_url.clone(),
                dest: zip.clone(),
                hash: Hash::Sha256(want),
                size: Some(offer.zip.size),
            })
            .await
            .map_err(|e| UpdateError::from_chain(&e))?;
        let staged = vdir.join("staged");
        let version = offer.version.clone();
        tokio::task::spawn_blocking(move || stage(&zip, &staged, version))
            .await
            .map_err(|e| UpdateError::other(e.to_string()))?
    }

    /// Removes a version's download and staging (after its swap, or a failed one).
    pub fn forget(&self, version: &Version) {
        let _ = std::fs::remove_dir_all(self.dir.join(version.to_string()));
    }

    fn forget_others(&self, keep: &Version) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for e in entries.flatten() {
            if e.file_name() != keep.to_string().as_str() {
                let p = e.path();
                let _ = if p.is_dir() {
                    std::fs::remove_dir_all(&p)
                } else {
                    std::fs::remove_file(&p)
                };
            }
        }
    }
}

/// Extracts the verified zip into a fresh `staged` folder and checks that the exe is there.
fn stage(zip: &Path, staged: &Path, version: Version) -> Result<Staged, UpdateError> {
    if staged.exists() {
        std::fs::remove_dir_all(staged)
            .map_err(|e| UpdateError::other(format!("clearing {}: {e}", staged.display())))?;
    }
    let files = archive::extract(zip, staged)
        .map_err(|e| UpdateError::other(format!("{}: {e:#}", zip.display())))?;
    let exe = staged.join(EXE_NAME);
    let size = std::fs::metadata(&exe).map(|m| m.len()).unwrap_or(0);
    if !exe.is_file() || size == 0 {
        return Err(UpdateError::other(format!(
            "{} has no {EXE_NAME} at its root",
            zip.display()
        )));
    }
    Ok(Staged {
        version,
        dir: staged.to_path_buf(),
        files,
    })
}
