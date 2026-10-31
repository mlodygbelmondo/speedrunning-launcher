//! Verified downloads. Every file the game runs (jars, natives, Java, mods) is checked against a
//! hash from its manifest before it is moved into place, and an existing file whose hash matches
//! is never downloaded again.
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, bail};
use futures::{StreamExt, TryStreamExt, stream};
use serde::de::DeserializeOwned;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use tokio::io::AsyncWriteExt;

use crate::{Progress, ProgressFn, Stage, USER_AGENT, Unit};

/// Expected hash of a file, lower-case hex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hash {
    Sha1(String),
    /// GitHub release assets publish a sha256 `digest`; tools pin it.
    Sha256(String),
    Sha512(String),
    /// Only for files no manifest hashes. Avoid: such a file is re-downloaded every time.
    None,
}

#[derive(Clone, Debug)]
pub struct Download {
    pub url: String,
    pub dest: PathBuf,
    pub hash: Hash,
    /// From the manifest, for progress in bytes. `None` counts the file as 1.
    pub size: Option<u64>,
}

/// How long `Downloader::refresh_bytes` may take in all, connect included. The biggest list it
/// fetches (mcsr-meta's `mods.json`, about 360 KB) needs under a second on a working connection.
pub const REFRESH_DEADLINE: Duration = Duration::from_secs(4);

#[derive(Clone)]
pub struct Downloader {
    client: reqwest::Client,
    /// Files fetched at the same time by `download_all`.
    concurrency: usize,
}

impl Downloader {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self {
            client,
            concurrency: 16,
        })
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub async fn fetch_bytes(&self, url: &str) -> Result<bytes::Bytes> {
        retry(|| async {
            let resp = self.client.get(url).send().await?.error_for_status()?;
            Ok(resp.bytes().await?)
        })
        .await
        .with_context(|| format!("GET {url}"))
    }

    /// One try with a short total deadline (`REFRESH_DEADLINE`), for refreshing a metadata list
    /// that has a cached copy to fall back to. When the network drops packets instead of refusing
    /// them, `fetch_bytes` waits out three 15 s connect timeouts (about 48 s per list, and Play
    /// asks for two sets of lists), while the cached copy would have done.
    pub async fn refresh_bytes(&self, url: &str) -> Result<bytes::Bytes> {
        async {
            let resp = self
                .client
                .get(url)
                .timeout(REFRESH_DEADLINE)
                .send()
                .await?
                .error_for_status()?;
            anyhow::Ok(resp.bytes().await?)
        }
        .await
        .with_context(|| format!("GET {url}"))
    }

    pub async fn fetch_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let bytes = self.fetch_bytes(url).await?;
        serde_json::from_slice(&bytes).with_context(|| format!("parsing JSON from {url}"))
    }

    /// Downloads one file unless `dest` already has the expected hash. Writes to `<dest>.part`,
    /// verifies, then renames, so a cancelled download never leaves a broken file behind.
    pub async fn download(&self, d: &Download) -> Result<()> {
        if matches_hash(&d.dest, &d.hash).await? {
            return Ok(());
        }
        if let Some(dir) = d.dest.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let part = part_path(&d.dest);
        retry(|| async {
            let resp = self.client.get(&d.url).send().await?.error_for_status()?;
            let mut file = tokio::fs::File::create(&part).await?;
            let mut body = resp.bytes_stream();
            let mut sha1 = Sha1::new();
            let mut sha256 = Sha256::new();
            let mut sha512 = Sha512::new();
            while let Some(chunk) = body.next().await {
                let chunk = chunk?;
                match d.hash {
                    Hash::Sha1(_) => sha1.update(&chunk),
                    Hash::Sha256(_) => sha256.update(&chunk),
                    Hash::Sha512(_) => sha512.update(&chunk),
                    Hash::None => {}
                }
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
            drop(file);
            let got = match d.hash {
                Hash::Sha1(_) => Some(hex::encode(sha1.finalize())),
                Hash::Sha256(_) => Some(hex::encode(sha256.finalize())),
                Hash::Sha512(_) => Some(hex::encode(sha512.finalize())),
                Hash::None => None,
            };
            if let (Hash::Sha1(want) | Hash::Sha256(want) | Hash::Sha512(want), Some(got)) =
                (&d.hash, got)
                && !want.eq_ignore_ascii_case(&got)
            {
                let _ = tokio::fs::remove_file(&part).await;
                bail!("hash mismatch: expected {want}, got {got}");
            }
            tokio::fs::rename(&part, &d.dest).await?;
            Ok(())
        })
        .await
        .with_context(|| format!("downloading {} to {}", d.url, d.dest.display()))
    }

    /// Downloads many files concurrently and reports progress for `stage`: bytes if every item
    /// has a size, else files.
    pub async fn download_all(
        &self,
        items: Vec<Download>,
        stage: Stage,
        progress: Option<ProgressFn>,
    ) -> Result<()> {
        let by_bytes = items.iter().all(|d| d.size.is_some());
        let unit = if by_bytes { Unit::Bytes } else { Unit::Files };
        let weight = move |d: &Download| if by_bytes { d.size.unwrap_or(1) } else { 1 };
        let total: u64 = items.iter().map(weight).sum();
        let done = Arc::new(AtomicU64::new(0));
        let report = |done: u64| {
            if let Some(p) = &progress {
                p(Progress {
                    stage,
                    done,
                    total,
                    unit,
                });
            }
        };
        report(0);
        stream::iter(items)
            .map(|d| {
                let done = done.clone();
                let report = &report;
                async move {
                    self.download(&d).await?;
                    report(done.fetch_add(weight(&d), Ordering::Relaxed) + weight(&d));
                    anyhow::Ok(())
                }
            })
            .buffer_unordered(self.concurrency)
            .try_collect::<Vec<()>>()
            .await?;
        Ok(())
    }
}

/// True if `path` exists and has the expected hash. Never true for `Hash::None`.
pub async fn matches_hash(path: &Path, hash: &Hash) -> Result<bool> {
    let want = match hash {
        Hash::Sha1(h) | Hash::Sha256(h) | Hash::Sha512(h) => h.clone(),
        Hash::None => return Ok(false),
    };
    if !tokio::fs::try_exists(path).await? {
        return Ok(false);
    }
    let path = path.to_path_buf();
    let hash = hash.clone();
    let got = tokio::task::spawn_blocking(move || match hash {
        Hash::Sha1(_) => file_sha1(&path),
        Hash::Sha256(_) => file_sha256(&path),
        _ => file_sha512(&path),
    })
    .await??;
    Ok(got.eq_ignore_ascii_case(&want))
}

pub fn file_sha1(path: &Path) -> Result<String> {
    hash_file::<Sha1>(path)
}

pub fn file_sha256(path: &Path) -> Result<String> {
    hash_file::<Sha256>(path)
}

pub fn file_sha512(path: &Path) -> Result<String> {
    hash_file::<Sha512>(path)
}

fn hash_file<D: Digest + std::io::Write>(path: &Path) -> Result<String> {
    let mut file =
        std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = D::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// 3 attempts, 0.5 s then 2 s apart. Hash mismatches are retried too: a CDN can serve a truncated
/// body with a 200.
async fn retry<T, F, Fut>(mut f: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last = None;
    for delay in [0, 500, 2000] {
        if delay > 0 {
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hash_of_existing_file() {
        let dir = std::env::temp_dir().join(format!("srl-core-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.txt");
        std::fs::write(&f, b"abc").unwrap();
        let sha1 = "a9993e364706816aba3e25717850c26c9cd0d89d".to_string();
        assert!(matches_hash(&f, &Hash::Sha1(sha1)).await.unwrap());
        assert!(!matches_hash(&f, &Hash::Sha1("00".into())).await.unwrap());
        let sha256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string();
        assert!(matches_hash(&f, &Hash::Sha256(sha256)).await.unwrap());
        assert!(
            !matches_hash(&dir.join("missing"), &Hash::Sha512("00".into()))
                .await
                .unwrap()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
