//! Cached metadata. Lists that change (Mojang's version manifest, its Java runtime list, Fabric's
//! loader list) are kept on disk and refreshed when older than `max_age`; when the network is
//! down, the cached copy is used and a warning is recorded. Documents named by a hash (version
//! JSONs, asset indexes, runtime manifests) are verified against that hash and never refetched
//! while they match.
use std::{
    path::Path,
    sync::Mutex,
    time::{Duration, SystemTime},
};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use srl_core::{Download, Downloader, Hash, download::matches_hash};

use crate::util::write_atomic;

pub struct MetaCache<'a> {
    pub downloader: &'a Downloader,
    pub max_age: Duration,
    /// Things the user may want to know (stale metadata, offline fallbacks).
    pub warnings: Mutex<Vec<String>>,
}

impl<'a> MetaCache<'a> {
    pub fn new(downloader: &'a Downloader, max_age: Duration) -> Self {
        Self {
            downloader,
            max_age,
            warnings: Mutex::new(Vec::new()),
        }
    }

    pub fn warn(&self, msg: String) {
        self.warnings.lock().unwrap().push(msg);
    }

    /// A list that changes over time, cached at `path`. With a cached copy, the refresh gets one
    /// short try (`Downloader::refresh_bytes`), and a failed fetch or a body that doesn't parse
    /// (a captive portal's page, a truncated or changed response) falls back to that copy; the
    /// cache is replaced only by a body that parsed.
    pub async fn list<T: DeserializeOwned>(&self, url: &str, path: &Path) -> Result<T> {
        let modified = tokio::fs::metadata(path)
            .await
            .ok()
            .and_then(|m| m.modified().ok());
        let fresh = modified
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age < self.max_age);
        if fresh && let Ok(v) = read_json(path).await {
            return Ok(v);
        }
        let fetched = if modified.is_some() {
            self.downloader.refresh_bytes(url).await
        } else {
            self.downloader.fetch_bytes(url).await
        };
        let parsed = fetched.and_then(|bytes| {
            let v = serde_json::from_slice::<T>(&bytes)
                .with_context(|| format!("parsing JSON from {url}"))?;
            Ok((v, bytes))
        });
        match parsed {
            Ok((v, bytes)) => {
                write_atomic(path, &bytes).await?;
                Ok(v)
            }
            Err(err) => match read_json(path).await {
                Ok(v) => {
                    self.warn(format!(
                        "using the cached {} because the download failed: {err:#}",
                        path.display()
                    ));
                    Ok(v)
                }
                Err(_) => Err(err.context(format!("no cached copy at {}", path.display()))),
            },
        }
    }

    /// A document with a known sha1, downloaded unless the cached copy matches.
    pub async fn hashed<T: DeserializeOwned>(
        &self,
        url: &str,
        path: &Path,
        sha1: &str,
    ) -> Result<T> {
        let d = Download {
            url: url.to_string(),
            dest: path.to_path_buf(),
            hash: Hash::Sha1(sha1.to_string()),
            size: None,
        };
        self.downloader.download(&d).await?;
        read_json(path).await
    }

    /// Like `hashed`, but without a hash from upstream: used for the version JSON when even the
    /// manifest is unavailable. Accepts any cached copy.
    pub async fn cached_only<T: DeserializeOwned>(&self, path: &Path) -> Result<T> {
        read_json(path).await
    }
}

pub async fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
}

/// True if `path` exists with the given sha1.
pub async fn has_sha1(path: &Path, sha1: &str) -> bool {
    matches_hash(path, &Hash::Sha1(sha1.to_string()))
        .await
        .unwrap_or(false)
}
