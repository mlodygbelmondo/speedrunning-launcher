//! Install records: what the launcher put into the practice saves, one JSON file per map in
//! `meta/practice/<id>.json`. A world is the launcher's to change only if a record names it; the
//! record also holds the fingerprint that tells a fresh world from a played one.
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use srl_core::Paths;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecord {
    pub id: String,
    /// The pristine copy: the verified zip in the content store (`Paths::store_sha512`). Reset
    /// extracts it again; an update replaces it.
    pub pristine: Pristine,
    /// The version of the worlds in `saves/`. Behind `pristine.version` after an update the
    /// player chose to apply later (`UpdateChoice::StoreOnly`).
    pub worlds_version: String,
    pub worlds: Vec<WorldRecord>,
    /// Worlds the launcher installed earlier and handed over to the player on an update
    /// (`UpdateChoice::KeepBoth`). Listed for the UI; never touched again.
    #[serde(default)]
    pub handed_over: Vec<String>,
    /// Unix seconds.
    pub installed_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pristine {
    pub version: String,
    pub sha512: String,
    pub size: u64,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldRecord {
    /// The folder under `saves/`: one plain path component.
    pub folder: String,
    /// Its folder in the zip (`""` for the zip's root).
    pub zip_root: String,
    /// sha512 of `level.dat` as extracted. The game rewrites level.dat whenever the world is
    /// opened, so a different hash means the world has been played.
    pub level_dat_sha512: String,
    /// `LevelName` without colour codes: what the Singleplayer list shows.
    #[serde(default)]
    pub level_name: Option<String>,
    #[serde(default)]
    pub data_version: Option<i32>,
}

pub fn dir(paths: &Paths) -> PathBuf {
    paths.meta().join("practice")
}

pub fn path(paths: &Paths, id: &str) -> PathBuf {
    dir(paths).join(format!("{id}.json"))
}

/// A folder name from a record is used to delete things, so it must be exactly one plain
/// component: a hand-edited record can't point the launcher at `../../`.
pub fn is_plain_folder(folder: &str) -> bool {
    crate::archive::safe_relative(folder).is_some_and(|p| {
        p.components().count() == 1 && p.as_os_str() == std::ffi::OsStr::new(folder)
    })
}

pub fn load(paths: &Paths, id: &str) -> Result<Option<InstallRecord>> {
    let p = path(paths, id);
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("reading {}", p.display())),
    };
    let r: InstallRecord =
        serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", p.display()))?;
    ensure!(r.id == id, "{} is the record of {}", p.display(), r.id);
    // The hash names a store file that uninstall and update delete: it must be a hash.
    ensure!(
        crate::catalogue::is_sha512_hex(&r.pristine.sha512),
        "{}: bad sha512",
        p.display()
    );
    for f in r.worlds.iter().map(|w| &w.folder).chain(&r.handed_over) {
        ensure!(is_plain_folder(f), "{}: bad folder {f:?}", p.display());
    }
    Ok(Some(r))
}

/// Writes atomically (temporary file, then rename), so a crash leaves the old record or the new
/// one, never half of one.
pub fn save(paths: &Paths, r: &InstallRecord) -> Result<()> {
    let p = path(paths, &r.id);
    std::fs::create_dir_all(dir(paths))?;
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(r)?)?;
    std::fs::rename(&tmp, &p).with_context(|| format!("writing {}", p.display()))?;
    Ok(())
}

pub fn remove(paths: &Paths, id: &str) -> Result<()> {
    match std::fs::remove_file(path(paths, id)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// Every record on disk (unreadable ones are skipped: they name nothing we may touch).
pub fn all(paths: &Paths) -> Vec<InstallRecord> {
    let Ok(entries) = std::fs::read_dir(dir(paths)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let id = name.strip_suffix(".json")?;
            load(paths, id).ok().flatten()
        })
        .collect()
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `saves/` of the practice instance.
pub fn saves(paths: &Paths) -> PathBuf {
    paths.practice_instance().join("saves")
}

/// Where worlds are extracted and deleted through, inside the instance so that renames stay on
/// one file system. The game ignores it: it is not in `saves/`.
pub fn staging(paths: &Paths, tag: &str) -> PathBuf {
    paths.practice_instance().join(".srl-staging").join(tag)
}

pub fn world_dir(paths: &Paths, folder: &str) -> PathBuf {
    saves(paths).join(folder)
}

pub fn exists_no_follow(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_folders_only() {
        assert!(is_plain_folder("LBP 3.15.0"));
        assert!(is_plain_folder("Blaze Practice (2)"));
        for bad in [
            "", "..", "../x", "a/b", "a\\b", "C:", "/abs", "x.", "CON", "./x",
        ] {
            assert!(!is_plain_folder(bad), "{bad:?}");
        }
    }
}
