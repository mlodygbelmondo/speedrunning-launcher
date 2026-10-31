//! `legal-dlls.csv` from the speedrun.com legal-builds repository: the DLLs and native libraries
//! that may be injected into or loaded by the game (rules A.8.4.c and A.8.16), by sha512.
//!
//! It lists Toolscreen.dll, liblogger and the custom GLFW builds (waywall's among them). The
//! launcher keeps the last good copy in `meta/tools/legal-dlls.csv` and refreshes it like the
//! mod lists: when older than [`MAX_AGE`], with one short try, falling back to the cached copy.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::{Context, Result, bail};
use srl_core::{Downloader, Paths};

pub const URL: &str = "https://raw.githubusercontent.com/Minecraft-Java-Edition-Speedrunning/legal-builds/main/legal-dlls.csv";

/// The same as the mod lists (srl-app refreshes those after 10 minutes).
pub const MAX_AGE: Duration = Duration::from_secs(600);

/// One row: the file name it is published under and its version label (`1.4.4`, `1.5.0-x64`,
/// `3.4-linux`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegalDll {
    pub file: String,
    pub version: String,
}

/// `legal-dlls.csv` indexed by sha512 (lower-case hex).
#[derive(Clone, Debug, Default)]
pub struct LegalDlls {
    by_sha512: HashMap<String, LegalDll>,
    /// Where it came from, for notes: `None` when fetched just now.
    pub cached_since: Option<SystemTime>,
}

impl LegalDlls {
    pub fn parse(text: &str) -> Result<Self> {
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        let header: Vec<&str> = lines
            .next()
            .context("legal-dlls.csv is empty")?
            .split(',')
            .map(str::trim)
            .collect();
        let col = |name: &str| {
            header
                .iter()
                .position(|h| h.eq_ignore_ascii_case(name))
                .with_context(|| format!("legal-dlls.csv has no {name} column"))
        };
        let (file, version, sha512) = (col("filename")?, col("version")?, col("sha512")?);
        let mut by_sha512 = HashMap::new();
        for line in lines {
            let cells: Vec<&str> = line.split(',').map(str::trim).collect();
            let (Some(f), Some(v), Some(h)) =
                (cells.get(file), cells.get(version), cells.get(sha512))
            else {
                continue;
            };
            if h.len() != 128 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
                continue;
            }
            by_sha512.insert(
                h.to_ascii_lowercase(),
                LegalDll {
                    file: f.to_string(),
                    version: v.to_string(),
                },
            );
        }
        if by_sha512.is_empty() {
            bail!("legal-dlls.csv has no rows");
        }
        Ok(Self {
            by_sha512,
            cached_since: None,
        })
    }

    pub fn get(&self, sha512: &str) -> Option<&LegalDll> {
        self.by_sha512.get(&sha512.to_ascii_lowercase())
    }

    pub fn contains(&self, sha512: &str) -> bool {
        self.get(sha512).is_some()
    }

    pub fn len(&self) -> usize {
        self.by_sha512.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_sha512.is_empty()
    }

    pub fn cache_path(paths: &Paths) -> PathBuf {
        paths.meta().join("tools").join("legal-dlls.csv")
    }

    /// The cached copy only, no network. `None` when there is none or it doesn't parse.
    pub fn cached(paths: &Paths) -> Option<Self> {
        let path = Self::cache_path(paths);
        let text = std::fs::read_to_string(&path).ok()?;
        let mut list = Self::parse(&text).ok()?;
        list.cached_since = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        Some(list)
    }

    /// The cached copy if younger than `max_age`, else a fresh one (one try, a few seconds at
    /// most), else the cached copy whatever its age. Fails only with neither.
    pub async fn load(paths: &Paths, downloader: &Downloader, max_age: Duration) -> Result<Self> {
        let path = Self::cache_path(paths);
        let age = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok());
        if age.is_some_and(|a| a < max_age)
            && let Some(list) = Self::cached(paths)
        {
            return Ok(list);
        }
        Self::settle(paths, downloader.refresh_bytes(URL).await).await
    }

    /// What [`Self::load`] makes of a fetch: the fetched list when it parses (it then becomes
    /// the cached copy), else the cached copy, else the error.
    async fn settle(paths: &Paths, fetched: Result<impl AsRef<[u8]>>) -> Result<Self> {
        let path = Self::cache_path(paths);
        match fetched {
            Ok(bytes) => match Self::parse(&String::from_utf8_lossy(bytes.as_ref())) {
                Ok(list) => {
                    write_atomic(&path, bytes.as_ref()).await?;
                    Ok(list)
                }
                // Not the CSV (a captive portal page, a changed header upstream): keep using the
                // last good copy, and never overwrite it with this.
                Err(err) => match Self::cached(paths) {
                    Some(list) => {
                        eprintln!(
                            "the fetched legal-dlls.csv doesn't parse ({err:#}); using the cached copy"
                        );
                        Ok(list)
                    }
                    None => Err(err.context("the fetched legal-dlls.csv")),
                },
            },
            Err(err) => Self::cached(paths).with_context(|| {
                format!("legal-dlls.csv can't be fetched ({err:#}) and there is no cached copy")
            }),
        }
    }
}

pub(crate) async fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The rows of the real legal-dlls.csv (2026-10-26, commit bc4735d) that the pins use, plus
    /// one GLFW row.
    pub const SAMPLE: &str = "filename,version,sha512
liblogger_x64.dll,1.0.2,946849cde44b52247534661eb798163c42b2a9e8e9e8311bb533457d2817ee47229ad793d0264902c0e9220fc543597836d37c043454bbd34d7e0e4ea5bd4834
liblogger_x64.dll,1.1.0,02a1396f69c836e44a4bc5ad2dc1440807c70026c5d6cb24ec4afad54caea503e6b221a45619fed75d0e266d32452d34497e4892a6a0727f16934be5ed6d3768
liblogger_arm64.dll,1.1.0,3503fc8af5aefe7c3fa5fa53c977b29f014d09ff0f9c3f840d1e57e24261006b668da8e6651c65d7145ce4aa2b8b183c97607b1301bd889a289996b626ecff01
Toolscreen.dll,1.4.4,833a6b0c0df01df057f17a6fb7ee0c2dc486a7de4d93fc04fc550c4270d6717695eec84d028e69bf5a67de3212f2c4cc15508c299fb421e8c3be983657c7d088
Toolscreen.dll,1.5.0-x64,1c9db0e0dab168f31f0ad41adccca451ac4a150b079a35bdcc3aacfae687fe7b505dc524238afa4141ebaec60b0f86b277e90ec9e5568cfc94010a4e2d4b53ec
Toolscreen.dll,1.5.0-arm64,c6b37cd3aeacc64100d1a4ab44d478a16d47253438377b297a752fab353129d180930b57fe91405ed861ac3c3e8ff201a3df3f7b947be6751d52570675b678ef
libglfw.so,3.4-linux,a0fafcbd8562c5be918338656415850ec3f4b1e4ebd1d40452fe52bf04255dfae897544dd3da8e0b81aff2d60316f6b42b13e46097dc66324401969a1fbcf8b9
";

    #[test]
    fn parses_by_header() {
        let l = LegalDlls::parse(SAMPLE).unwrap();
        assert_eq!(l.len(), 7);
        let row = l
            .get("833A6B0C0DF01DF057F17A6FB7EE0C2DC486A7DE4D93FC04FC550C4270D6717695EEC84D028E69BF5A67DE3212F2C4CC15508C299FB421E8C3BE983657C7D088")
            .unwrap();
        assert_eq!(row.file, "Toolscreen.dll");
        assert_eq!(row.version, "1.4.4");
        assert!(!l.contains("00"));
        // Column order comes from the header.
        let swapped = "sha512,filename,version\n946849cde44b52247534661eb798163c42b2a9e8e9e8311bb533457d2817ee47229ad793d0264902c0e9220fc543597836d37c043454bbd34d7e0e4ea5bd4834,liblogger_x64.dll,1.0.2\n";
        assert_eq!(
            LegalDlls::parse(swapped).unwrap().get("946849cde44b52247534661eb798163c42b2a9e8e9e8311bb533457d2817ee47229ad793d0264902c0e9220fc543597836d37c043454bbd34d7e0e4ea5bd4834").unwrap().version,
            "1.0.2"
        );
        assert!(LegalDlls::parse("").is_err());
        assert!(LegalDlls::parse("filename,version\n").is_err());
        assert!(LegalDlls::parse("filename,version,sha512\na,b,short\n").is_err());
    }

    /// A fetch that isn't the CSV falls back to the cached copy and leaves it as it is.
    #[tokio::test]
    async fn bad_fetch_keeps_the_cached_copy() {
        let root = std::env::temp_dir().join(format!("srl-tools-ld-{}", std::process::id()));
        let paths = Paths::new(&root);
        let portal = || -> Result<&'static [u8]> { Ok(b"<html>log in to the wifi</html>") };
        assert!(
            LegalDlls::settle(&paths, portal()).await.is_err(),
            "no cache yet"
        );
        let good: Result<&[u8]> = Ok(SAMPLE.as_bytes());
        assert_eq!(LegalDlls::settle(&paths, good).await.unwrap().len(), 7);
        assert_eq!(LegalDlls::settle(&paths, portal()).await.unwrap().len(), 7);
        let down: Result<&[u8]> = Err(anyhow::anyhow!("offline"));
        assert_eq!(LegalDlls::settle(&paths, down).await.unwrap().len(), 7);
        assert_eq!(
            std::fs::read_to_string(LegalDlls::cache_path(&paths)).unwrap(),
            SAMPLE
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
