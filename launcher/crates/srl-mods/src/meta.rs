//! The mod metadata: mcsr-meta schema 7 (`mods.json`, `extra.json`), the legal authority
//! (`legal-builds.csv`), `conditional-mods.json`, and the Modrinth version lists of the non-legal
//! extras the modes need (MCSR Ranked, the FSG mod, Draftout).
//!
//! Every file is cached under `Paths::meta()/mcsr/`. A fetched file replaces the cached one only
//! after it parsed, so the cache always holds the last good copy, and an offline start (or a
//! broken upstream) falls back to it.
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use srl_core::{Downloader, Paths};

/// The consumable legal manifest (CDN, byte-identical to the `schema-7` branch of mcsr-meta).
pub const MODS_URL: &str = "https://meta.mc.sr/schema-7/mods.json";
/// Practice and modded-category mods: not legal for /mc, but the source of the FSG mod and
/// Draftout's allowed extras.
pub const EXTRA_URL: &str = "https://meta.mc.sr/schema-7/extra.json";
/// The legal authority (rule A.8.5): a jar not listed here by sha512 is not legal.
pub const LEGAL_BUILDS_URL: &str = "https://raw.githubusercontent.com/Minecraft-Java-Edition-Speedrunning/legal-mods/main/legal-builds.csv";
/// Mods legal only in some categories, e.g. `{"ssg-only": ["chunkcacher", "setspawnmod"]}`.
pub const CONDITIONAL_URL: &str = "https://raw.githubusercontent.com/Minecraft-Java-Edition-Speedrunning/legal-mods/main/conditional-mods.json";

/// Modrinth projects of the non-legal extras. `extra.json` mirrors them but lags behind (on
/// 2026-10-26 it had mcsrranked 5.8.26 while Modrinth had 6.0.4, and only the newest Ranked
/// build can connect), so Modrinth is the primary source and `extra.json` the fallback.
pub const MODRINTH_PROJECTS: [(&str, &str); 3] = [
    ("mcsrranked", "mcsr-ranked"),
    ("fsg-mod", "fsg-mod"),
    ("draftout", "draftout"),
];

const SCHEMA_VERSION: u32 = 7;

/// `mods.json` or `extra.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub mods: Vec<MetaMod>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetaMod {
    pub modid: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub homepage: Option<String>,
    /// Kept as strings: an unknown future trait must not break parsing, and `resolve` leaves
    /// such a mod out rather than guess what it means.
    #[serde(default)]
    pub traits: Vec<String>,
    #[serde(default)]
    pub incompatibilities: Vec<String>,
    #[serde(default = "yes")]
    pub recommended: bool,
    #[serde(default)]
    pub obsolete: bool,
    pub versions: Vec<MetaVersion>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetaVersion {
    pub target_version: Vec<String>,
    pub version: String,
    pub url: String,
    pub sha1: String,
    pub sha512: String,
    pub size: u64,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default = "yes")]
    pub recommended: bool,
    #[serde(default)]
    pub obsolete: bool,
    #[serde(default)]
    pub intermediary: Vec<String>,
}

fn yes() -> bool {
    true
}

impl MetaMod {
    /// The version for a Minecraft version. Schema 7 lists at most one per target, newest first,
    /// so the first match is the one mc.sr shows.
    pub fn version_for(&self, minecraft: &str) -> Option<&MetaVersion> {
        self.versions
            .iter()
            .find(|v| v.target_version.iter().any(|t| t == minecraft))
    }
}

impl MetaVersion {
    /// The file name the jar gets in `mods/`: the last URL segment, percent-decoded
    /// (`FSG-Mod-5.3.0%2BMC1.16.1.jar` becomes `FSG-Mod-5.3.0+MC1.16.1.jar`).
    pub fn file_name(&self) -> Option<String> {
        file_name_from_url(&self.url)
    }
}

/// One row of `legal-builds.csv`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegalBuild {
    pub modid: String,
    pub filename: String,
    /// `YYYY-MM-DD`, when the build was added.
    pub date: String,
}

/// `legal-builds.csv` indexed by sha512 (lower-case hex).
#[derive(Clone, Debug, Default)]
pub struct LegalBuilds {
    pub by_sha512: HashMap<String, LegalBuild>,
}

impl LegalBuilds {
    /// Columns `modid,filename,timestamp,date,hash`. Located by header name, because the research
    /// snapshot called the last one `sha512` and the live file calls it `hash`.
    pub fn parse(text: &str) -> Result<Self> {
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let header: Vec<&str> = lines
            .next()
            .context("legal-builds.csv is empty")?
            .split(',')
            .map(str::trim)
            .collect();
        let col = |names: &[&str]| {
            header
                .iter()
                .position(|h| names.contains(h))
                .with_context(|| format!("legal-builds.csv has no {names:?} column"))
        };
        let (modid, filename, date, hash) = (
            col(&["modid"])?,
            col(&["filename"])?,
            col(&["date"])?,
            col(&["hash", "sha512"])?,
        );
        let mut by_sha512 = HashMap::new();
        for line in lines {
            // No field contains a comma or quotes (checked on the live file), so a plain split
            // is enough; a malformed row is skipped rather than failing the whole file.
            let fields: Vec<&str> = line.split(',').map(str::trim).collect();
            if fields.len() != header.len() {
                continue;
            }
            by_sha512.insert(
                fields[hash].to_ascii_lowercase(),
                LegalBuild {
                    modid: fields[modid].to_string(),
                    filename: fields[filename].to_string(),
                    date: fields[date].to_string(),
                },
            );
        }
        if by_sha512.is_empty() {
            bail!("legal-builds.csv has no rows");
        }
        Ok(Self { by_sha512 })
    }

    pub fn contains(&self, sha512: &str) -> bool {
        self.by_sha512.contains_key(&sha512.to_ascii_lowercase())
    }
}

/// A Modrinth version, the fields we use from `GET /v2/project/{slug}/version`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModrinthVersion {
    pub id: String,
    pub project_id: String,
    pub version_number: String,
    /// `release`, `beta` or `alpha`. Not filtered on: every Draftout build is a beta.
    pub version_type: String,
    /// RFC 3339; all values share one format, so they sort as strings.
    pub date_published: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<ModrinthFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModrinthFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    pub size: u64,
    pub hashes: ModrinthHashes,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModrinthHashes {
    pub sha512: String,
    #[serde(default)]
    pub sha1: Option<String>,
}

impl ModrinthVersion {
    /// The jar of the version: the primary file, else the first `.jar`.
    pub fn jar(&self) -> Option<&ModrinthFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.iter().find(|f| f.filename.ends_with(".jar")))
    }
}

/// Everything `resolve` needs, loaded once and shared between modes.
#[derive(Clone, Debug)]
pub struct ModMeta {
    pub mods: Manifest,
    pub extra: Manifest,
    pub legal: LegalBuilds,
    /// Category key (`ssg-only`) to the modids it applies to.
    pub conditional: BTreeMap<String, Vec<String>>,
    /// Modrinth versions (Fabric only) by modid. A project that could not be fetched and has no
    /// cached copy is missing; `resolve` then uses `extra.json`.
    pub modrinth: BTreeMap<String, Vec<ModrinthVersion>>,
    /// What did not come fresh from the network: files served from the cache after a failed
    /// fetch, Modrinth projects that are missing. For the log and the UI; not errors.
    pub notes: Vec<String>,
}

/// One cached file and where it comes from.
struct Source {
    file: String,
    url: String,
    /// Without it (and without a cached copy) loading fails.
    required: bool,
}

fn sources() -> Vec<Source> {
    let mut v = vec![
        Source {
            file: "mods.json".into(),
            url: MODS_URL.into(),
            required: true,
        },
        Source {
            file: "extra.json".into(),
            url: EXTRA_URL.into(),
            required: true,
        },
        Source {
            file: "legal-builds.csv".into(),
            url: LEGAL_BUILDS_URL.into(),
            required: true,
        },
        Source {
            file: "conditional-mods.json".into(),
            url: CONDITIONAL_URL.into(),
            required: true,
        },
    ];
    for (modid, slug) in MODRINTH_PROJECTS {
        v.push(Source {
            file: modrinth_file(modid),
            url: format!(
                "https://api.modrinth.com/v2/project/{slug}/version?loaders=%5B%22fabric%22%5D"
            ),
            required: false,
        });
    }
    v
}

fn modrinth_file(modid: &str) -> String {
    format!("modrinth-{modid}.json")
}

/// Where the mod metadata is cached.
pub fn cache_dir(paths: &Paths) -> PathBuf {
    paths.meta().join("mcsr")
}

impl ModMeta {
    /// Loads the metadata. With `refresh`, every file is fetched again (do it on start and before
    /// an install); without it, only files missing from the cache are. A failed fetch falls back
    /// to the cached copy and adds a note; it is an error only when a required file has no
    /// cached copy either. A file with a cached copy gets one short try
    /// (`Downloader::refresh_bytes`), so a network that drops packets delays Play by seconds,
    /// not minutes; only a missing file gets the full retries.
    pub async fn load(paths: &Paths, downloader: &Downloader, refresh: bool) -> Result<Self> {
        let dir = cache_dir(paths);
        tokio::fs::create_dir_all(&dir)
            .await
            .with_context(|| format!("creating {}", dir.display()))?;
        let sources = sources();
        let fetched = futures::future::join_all(sources.iter().map(|s| {
            let path = dir.join(&s.file);
            async move {
                let cached = tokio::fs::try_exists(&path).await.unwrap_or(false);
                if !refresh && cached {
                    return None;
                }
                Some(if cached {
                    downloader.refresh_bytes(&s.url).await
                } else {
                    downloader.fetch_bytes(&s.url).await
                })
            }
        }))
        .await;

        let mut notes = Vec::new();
        for (s, result) in sources.iter().zip(fetched) {
            let path = dir.join(&s.file);
            match result {
                None => {}
                Some(Ok(bytes)) => match validate(&s.file, &bytes) {
                    // Write only what parses, so the cache keeps the last good copy.
                    Ok(()) => write_atomic(&path, &bytes).await?,
                    Err(e) => notes.push(format!("{}: bad download ({e:#}), using cache", s.file)),
                },
                Some(Err(e)) => notes.push(format!("{}: offline ({e:#}), using cache", s.file)),
            }
            if !s.required && !tokio::fs::try_exists(&path).await.unwrap_or(false) {
                notes.push(format!("{}: not available", s.file));
            }
        }
        // Without a cached copy, the fetch errors are the useful part of the message.
        let mut meta = Self::from_dir(&dir).with_context(|| notes.join("; "))?;
        notes.append(&mut meta.notes);
        meta.notes = notes;
        Ok(meta)
    }

    /// Loads the metadata from a directory with the cache layout, without any network: the cache
    /// itself, or test fixtures.
    pub fn from_dir(dir: &Path) -> Result<Self> {
        let read = |file: &str| {
            let path = dir.join(file);
            std::fs::read(&path).with_context(|| format!("reading {}", path.display()))
        };
        let modrinth: Vec<(&str, Vec<u8>)> = MODRINTH_PROJECTS
            .iter()
            .filter_map(|(modid, _)| {
                Some((*modid, std::fs::read(dir.join(modrinth_file(modid))).ok()?))
            })
            .collect();
        Self::from_files(
            &read("mods.json")?,
            &read("extra.json")?,
            &read("legal-builds.csv")?,
            &read("conditional-mods.json")?,
            modrinth.iter().map(|(m, b)| (*m, b.as_slice())),
        )
    }

    /// The metadata from the bytes of its files (the cache layout's names), e.g. fixtures built
    /// into the app's fake backend. `modrinth`: (modid, its Modrinth versions file).
    pub fn from_files<'a>(
        mods: &[u8],
        extra: &[u8],
        legal_builds: &[u8],
        conditional: &[u8],
        modrinth: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    ) -> Result<Self> {
        let mods = parse_manifest(mods).context("mods.json")?;
        let extra = parse_manifest(extra).context("extra.json")?;
        let legal = LegalBuilds::parse(&String::from_utf8_lossy(legal_builds))?;
        let conditional =
            serde_json::from_slice(conditional).context("parsing conditional-mods.json")?;
        let mut by_mod = BTreeMap::new();
        let mut notes = Vec::new();
        for (modid, bytes) in modrinth {
            match serde_json::from_slice::<Vec<ModrinthVersion>>(bytes) {
                Ok(v) => {
                    by_mod.insert(modid.to_string(), v);
                }
                Err(e) => notes.push(format!("{}: unreadable ({e})", modrinth_file(modid))),
            }
        }
        Ok(Self {
            mods,
            extra,
            legal,
            conditional,
            modrinth: by_mod,
            notes,
        })
    }

    pub fn legal_mod(&self, modid: &str) -> Option<&MetaMod> {
        self.mods.mods.iter().find(|m| m.modid == modid)
    }

    pub fn extra_mod(&self, modid: &str) -> Option<&MetaMod> {
        self.extra.mods.iter().find(|m| m.modid == modid)
    }

    /// Traits of a mod from `mods.json` plus the categories `conditional-mods.json` gives it.
    pub fn traits_of(&self, m: &MetaMod) -> Vec<String> {
        let mut traits = m.traits.clone();
        for (category, modids) in &self.conditional {
            if modids.iter().any(|id| id == &m.modid) && !traits.contains(category) {
                traits.push(category.clone());
            }
        }
        traits
    }

    /// The newest Modrinth version of an extra for a Minecraft version.
    pub fn modrinth_latest(&self, modid: &str, minecraft: &str) -> Option<&ModrinthVersion> {
        self.modrinth
            .get(modid)?
            .iter()
            .filter(|v| {
                v.game_versions.iter().any(|g| g == minecraft)
                    && v.loaders.iter().any(|l| l == "fabric")
                    && v.jar().is_some()
            })
            .max_by(|a, b| a.date_published.cmp(&b.date_published))
    }
}

fn parse_manifest(bytes: &[u8]) -> Result<Manifest> {
    let m: Manifest = serde_json::from_slice(bytes)?;
    if m.schema_version != SCHEMA_VERSION {
        bail!(
            "schema version {} (expected {SCHEMA_VERSION})",
            m.schema_version
        );
    }
    Ok(m)
}

/// Checks a fetched file before it may replace the cached copy.
fn validate(file: &str, bytes: &[u8]) -> Result<()> {
    match file {
        "mods.json" | "extra.json" => parse_manifest(bytes).map(drop),
        "legal-builds.csv" => LegalBuilds::parse(&String::from_utf8_lossy(bytes)).map(drop),
        "conditional-mods.json" => {
            serde_json::from_slice::<BTreeMap<String, Vec<String>>>(bytes)?;
            Ok(())
        }
        _ => {
            serde_json::from_slice::<Vec<ModrinthVersion>>(bytes)?;
            Ok(())
        }
    }
}

async fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, bytes)
        .await
        .with_context(|| format!("writing {}", tmp.display()))?;
    tokio::fs::rename(&tmp, path)
        .await
        .with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

/// The last path segment of a URL, percent-decoded. `None` if it is not a plain `.jar` file name
/// (no separators, no `..`), so a manifest can never make us write outside `mods/`.
pub fn file_name_from_url(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next()?;
    let segment = path.rsplit('/').next()?;
    let name = percent_decode(segment)?;
    sanitize_file_name(&name)
}

pub(crate) fn sanitize_file_name(name: &str) -> Option<String> {
    let ok = !name.is_empty()
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
        && name.to_ascii_lowercase().ends_with(".jar");
    ok.then(|| name.to_string())
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_from_urls() {
        assert_eq!(
            file_name_from_url(
                "https://cdn.modrinth.com/data/XZOGBIpM/versions/qc4OUmcd/FSG-Mod-5.3.0%2BMC1.16.1.jar"
            )
            .as_deref(),
            Some("FSG-Mod-5.3.0+MC1.16.1.jar")
        );
        assert_eq!(
            file_name_from_url("https://x/legal-mods/seedqueue/1.16.1/seedqueue-1.8+1.16.1.jar")
                .as_deref(),
            Some("seedqueue-1.8+1.16.1.jar")
        );
        assert_eq!(file_name_from_url("https://x/a/..%2F..%2Fevil.jar"), None);
        assert_eq!(file_name_from_url("https://x/a/readme.txt"), None);
        assert_eq!(file_name_from_url("https://x/a/%2"), None);
    }

    #[test]
    fn legal_builds_by_header() {
        let csv = "modid,filename,timestamp,date,hash\n\
                   atum,atum-2.7.2.jar,1,2026-01-01,ABCDEF\n\
                   broken,row\n";
        let legal = LegalBuilds::parse(csv).unwrap();
        assert!(legal.contains("abcdef"));
        assert_eq!(legal.by_sha512.len(), 1);
        assert_eq!(legal.by_sha512["abcdef"].modid, "atum");
        assert!(LegalBuilds::parse("modid,filename\n").is_err());
    }

    #[test]
    fn rejects_other_schema() {
        assert!(parse_manifest(br#"{"schemaVersion":6,"mods":[]}"#).is_err());
        assert!(parse_manifest(br#"{"schemaVersion":7,"mods":[]}"#).is_ok());
    }
}
