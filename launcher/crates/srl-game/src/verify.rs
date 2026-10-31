//! Remembering which files were already verified, so a second install of a mode takes a stat per
//! file instead of hashing ~0.5 GB (assets, Java, libraries) every time Play is pressed.
//!
//! After a successful install we record each file's size, modification time and the sha1 it was
//! verified against in `meta/verified/<mode>.json`. Next time a file whose size, mtime and
//! expected sha1 are unchanged is trusted without hashing; anything else goes through the
//! normal hash-then-download path. A file edited in place keeps neither its mtime nor (usually)
//! its size, so this catches the same accidents a hash would, except deliberate tampering that
//! restores the mtime. One file per mode, so installs of two modes never write the same stamp.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};
use srl_core::{Download, Hash};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    size: u64,
    mtime_s: u64,
    mtime_ns: u32,
    sha1: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Stamp {
    files: HashMap<PathBuf, Entry>,
}

impl Stamp {
    /// The stamp at `path`, or an empty one if it is missing or unreadable.
    pub fn load(path: &Path) -> Stamp {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(self)?)?;
        std::fs::rename(tmp, path)
    }

    /// True if `d.dest` was verified against the same sha1 and hasn't changed since.
    pub fn is_fresh(&self, d: &Download) -> bool {
        let Hash::Sha1(want) = &d.hash else {
            return false;
        };
        match (self.files.get(&d.dest), entry_for(&d.dest, want)) {
            (Some(old), Some(now)) => *old == now,
            _ => false,
        }
    }

    /// Records `d.dest` as verified. Call only after the download (or hash check) succeeded.
    pub fn record(&mut self, d: &Download) {
        if let Hash::Sha1(sha1) = &d.hash
            && let Some(e) = entry_for(&d.dest, sha1)
        {
            self.files.insert(d.dest.clone(), e);
        }
    }
}

fn entry_for(path: &Path, sha1: &str) -> Option<Entry> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(Entry {
        size: meta.len(),
        mtime_s: mtime.as_secs(),
        mtime_ns: mtime.subsec_nanos(),
        sha1: sha1.to_ascii_lowercase(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_until_changed() {
        let dir = std::env::temp_dir().join(format!("srl-game-stamp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.jar");
        std::fs::write(&f, b"abc").unwrap();
        let d = Download {
            url: String::new(),
            dest: f.clone(),
            hash: Hash::Sha1("a9993e364706816aba3e25717850c26c9cd0d89d".into()),
            size: Some(3),
        };
        let mut stamp = Stamp::default();
        assert!(!stamp.is_fresh(&d));
        stamp.record(&d);
        assert!(stamp.is_fresh(&d));
        // Saved and loaded.
        let p = dir.join("stamp.json");
        stamp.save(&p).unwrap();
        let stamp = Stamp::load(&p);
        assert!(stamp.is_fresh(&d));
        // A different expected hash is not fresh.
        let other = Download {
            hash: Hash::Sha1("00".into()),
            ..d.clone()
        };
        assert!(!stamp.is_fresh(&other));
        // A changed file is not fresh.
        std::fs::write(&f, b"abcd").unwrap();
        assert!(!stamp.is_fresh(&d));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
