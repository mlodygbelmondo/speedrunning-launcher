//! Downloading a map zip into the content store, verified against the catalogue's sha512 and
//! size. `srl_core::Downloader::download` does the same for small files but reports no bytes;
//! map zips run to 92 MB, so this streams with byte progress, on the same HTTP client.
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use futures::StreamExt;
use sha2::{Digest, Sha512};
use srl_core::{Downloader, Paths};
use tokio::io::AsyncWriteExt;

use crate::{Catalogue, PracticeError, Progress, ProgressFn, Step, record};

/// sha512 of a file, off the async threads.
pub async fn sha512_file(path: &Path) -> Result<String, PracticeError> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || srl_core::download::file_sha512(&path))
        .await
        .map_err(anyhow::Error::from)?
        .map_err(PracticeError::Other)
}

/// True if `path` exists with this size and sha512. The size is checked first: a stat is free,
/// hashing 92 MB is not.
pub async fn is_verified(path: &Path, sha512: &str, size: u64) -> Result<bool, PracticeError> {
    match tokio::fs::metadata(path).await {
        Ok(m) if m.len() == size => Ok(sha512_file(path).await?.eq_ignore_ascii_case(sha512)),
        Ok(_) => Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Deletes `<dest>.part` when dropped, unless disarmed: a cancelled download (the future dropped
/// by the UI's Cancel) leaves nothing behind. On Windows the delete can fail while the file is
/// still open; [`remove_stale_parts`] catches that at the next call.
struct PartGuard {
    path: PathBuf,
    armed: bool,
}

impl Drop for PartGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// The `.part` file of a store path.
fn part_of(dest: &Path) -> PathBuf {
    let mut part = dest.as_os_str().to_owned();
    part.push(".part");
    PathBuf::from(part)
}

/// Deletes the `.part` files of every map zip this crate may download (the catalogue's and the
/// install records'), left behind if the app was killed mid-download. Only these: other
/// downloads into the store are not ours to clean.
pub fn remove_stale_parts(paths: &Paths, catalogue: &Catalogue) {
    let shas = catalogue
        .maps
        .iter()
        .filter_map(|m| m.download.as_ref().map(|d| d.sha512.clone()))
        .chain(record::all(paths).into_iter().map(|r| r.pristine.sha512));
    for sha in shas {
        let _ = std::fs::remove_file(part_of(&paths.store_sha512(&sha)));
    }
}

/// Downloads `url` to `dest` unless `dest` is already verified. Writes `<dest>.part` and renames
/// only after the hash and size match, so a cancelled or bad download never leaves a file that
/// looks valid. Three tries for network errors, like srl-core's downloads. Not retried: a 404 or
/// 410, and a full-size body with the wrong hash (another file under the pinned URL, as when an
/// author moves a tag), which would only come again.
pub async fn fetch(
    downloader: &Downloader,
    url: &str,
    dest: &Path,
    sha512: &str,
    size: u64,
    progress: Option<&ProgressFn>,
) -> Result<(), PracticeError> {
    let report = |step, done, total| {
        if let Some(p) = progress {
            p(Progress { step, done, total });
        }
    };
    report(Step::Verifying, 0, size);
    if is_verified(dest, sha512, size).await? {
        report(Step::Downloading, size, size);
        return Ok(());
    }
    if let Some(dir) = dest.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let part = part_of(dest);
    let mut guard = PartGuard {
        path: part.clone(),
        armed: true,
    };
    let mut last = None;
    for delay in [0, 500, 2000] {
        if delay > 0 {
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        match try_once(downloader, url, &part, sha512, size, &report).await {
            Ok(()) => {
                tokio::fs::rename(&part, dest).await?;
                guard.armed = false;
                return Ok(());
            }
            Err(e @ (PracticeError::SourceGone { .. } | PracticeError::HashMismatch { .. })) => {
                return Err(e);
            }
            Err(e) => last = Some(e),
        }
    }
    Err(last.expect("three tries"))
}

async fn try_once(
    downloader: &Downloader,
    url: &str,
    part: &Path,
    sha512: &str,
    size: u64,
    report: &impl Fn(Step, u64, u64),
) -> Result<(), PracticeError> {
    let net = |e: reqwest::Error| PracticeError::Network(e.to_string());
    let resp = downloader.client().get(url).send().await.map_err(net)?;
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::GONE {
        return Err(PracticeError::SourceGone { url: url.into() });
    }
    let resp = resp.error_for_status().map_err(net)?;
    let mut file = tokio::fs::File::create(part).await?;
    let mut body = resp.bytes_stream();
    let mut hasher = Sha512::new();
    let mut done: u64 = 0;
    report(Step::Downloading, 0, size);
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(net)?;
        done += chunk.len() as u64;
        // A server sending more than the pinned size is wrong; stop early instead of filling
        // the disk.
        if done > size {
            return Err(PracticeError::HashMismatch {
                expected: format!("{size} bytes"),
                got: format!("more than {size} bytes"),
            });
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        report(Step::Downloading, done, size);
    }
    file.flush().await?;
    drop(file);
    if done < size {
        // The connection ended early without an error: a network problem, worth a retry.
        return Err(PracticeError::Network(format!(
            "the download ended after {done} of {size} bytes"
        )));
    }
    let got = hex::encode(hasher.finalize());
    if !got.eq_ignore_ascii_case(sha512) {
        return Err(PracticeError::HashMismatch {
            expected: format!("{sha512} ({size} bytes)"),
            got: format!("{got} ({done} bytes)"),
        });
    }
    Ok(())
}
