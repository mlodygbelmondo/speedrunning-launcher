//! Reading a map zip: where its worlds are, and extracting them safely.
//!
//! Map zips come in three layouts, and all three exist in the wild: `level.dat` at the zip's root
//! (The MCSR Practice Map 2.0.0), one world folder (most maps), or several world folders. A world
//! is any folder with a `level.dat`; files outside every world (READMEs, a loose datapack zip)
//! are left out.
//!
//! A zip is untrusted data, so every entry name passes [`safe_relative`] (the zip-slip guard)
//! before anything is written, symlink entries are refused, and the total size is capped. A zip
//! that breaks a rule is refused as a whole rather than half-extracted.
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use crate::PracticeError;

/// Refuse zips that expand to more than this. The largest map today expands to 162 MB.
pub const MAX_UNPACKED_BYTES: u64 = 4 << 30;
/// And more entries than this. The largest map today has 7,925.
pub const MAX_ENTRIES: usize = 200_000;

/// Windows device names: a file called `CON` or `nul.txt` can't be created, or opens the device.
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// The zip-slip guard. Turns an entry name into a relative path that stays inside the folder it
/// is extracted to, on Linux and on Windows, or `None`:
///
/// - `\` counts as a separator (zips made on Windows use it, and Windows treats it as one);
/// - absolute paths, drive prefixes (`C:`), `..` and empty names are refused;
/// - a `:` anywhere is refused (drive-relative paths and NTFS alternate streams);
/// - Windows device names and names ending in a dot or space are refused, because Windows maps
///   them to another file than the one asked for;
/// - `.` components are dropped.
pub fn safe_relative(name: &str) -> Option<PathBuf> {
    let name = name.replace('\\', "/");
    if name.starts_with('/') || name.contains('\0') {
        return None;
    }
    let mut out = PathBuf::new();
    for part in name.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            _ => {}
        }
        if part.contains(':') || part.ends_with('.') || part.ends_with(' ') {
            return None;
        }
        let stem = part.split('.').next().unwrap_or(part);
        if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
            return None;
        }
        out.push(part);
    }
    // Belt and braces: whatever the rules above missed, the result must be plain components.
    let plain = out.components().all(|c| matches!(c, Component::Normal(_)));
    (plain && out.components().next().is_some()).then_some(out)
}

/// A world found in a zip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZipWorld {
    /// Its folder in the zip, `/`-separated; `""` when `level.dat` is at the zip's root.
    pub root: String,
}

/// Entry names as the rest of this module compares them: `/` separators, no leading `./`.
fn normalize(name: &str) -> String {
    let name = name.replace('\\', "/");
    name.trim_start_matches("./").to_string()
}

/// The worlds in a zip, outermost first. A `level.dat` inside another world (a backup copy, a
/// nested template) belongs to that world and is not a world of its own.
pub fn worlds_in(names: &[String]) -> Vec<ZipWorld> {
    let mut roots: Vec<String> = names
        .iter()
        .map(|n| normalize(n))
        .filter_map(|n| {
            if n == "level.dat" {
                Some(String::new())
            } else {
                n.strip_suffix("/level.dat")
                    .map(|r| r.trim_matches('/').to_string())
            }
        })
        .filter(|r| !r.starts_with("__MACOSX"))
        .collect();
    roots.sort_by_key(|r| (r.matches('/').count(), r.clone()));
    roots.dedup();
    let mut kept: Vec<String> = Vec::new();
    for r in roots {
        let inside = kept
            .iter()
            .any(|k| k.is_empty() || r == *k || r.starts_with(&format!("{k}/")));
        if !inside {
            kept.push(r);
        }
    }
    kept.into_iter().map(|root| ZipWorld { root }).collect()
}

/// Which world an entry belongs to, and its path inside that world.
fn place(name: &str, worlds: &[ZipWorld]) -> Option<(usize, String)> {
    let name = normalize(name);
    worlds.iter().enumerate().find_map(|(i, w)| {
        if w.root.is_empty() {
            Some((i, name.clone()))
        } else {
            name.strip_prefix(&format!("{}/", w.root))
                .map(|rest| (i, rest.to_string()))
        }
    })
}

/// Lists the worlds in the zip at `path` without extracting anything.
pub fn inspect(path: &Path) -> Result<Vec<ZipWorld>, PracticeError> {
    let mut zip = open(path)?;
    let names = entry_names(&mut zip)?;
    Ok(worlds_in(&names))
}

fn open(path: &Path) -> Result<zip::ZipArchive<File>, PracticeError> {
    let file = File::open(path)?;
    zip::ZipArchive::new(file).map_err(|e| PracticeError::BadArchive(e.to_string()))
}

fn entry_names(zip: &mut zip::ZipArchive<File>) -> Result<Vec<String>, PracticeError> {
    if zip.len() > MAX_ENTRIES {
        return Err(PracticeError::UnsafeArchive(format!(
            "{} entries (the limit is {MAX_ENTRIES})",
            zip.len()
        )));
    }
    (0..zip.len())
        .map(|i| {
            zip.name_for_index(i)
                .ok_or_else(|| PracticeError::BadArchive(format!("entry {i}")))?
                .map(|n| n.into_owned())
                .map_err(|e| PracticeError::BadArchive(e.to_string()))
        })
        .collect()
}

/// What [`extract_worlds`] wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extracted {
    /// One per world, in the order of `worlds`: the folder it was written to (`dest/<i>`) and its
    /// root in the zip.
    pub worlds: Vec<(PathBuf, ZipWorld)>,
    pub files: u64,
    pub bytes: u64,
}

/// Extracts every world of the zip into `dest/0`, `dest/1`, ... (`dest` must be a fresh, empty
/// staging folder). `session.lock` is skipped: the game makes its own, and several maps ship the
/// author's. Calls `on_file(done, total)` after each file.
pub fn extract_worlds(
    zip_path: &Path,
    dest: &Path,
    mut on_file: impl FnMut(u64, u64),
) -> Result<Extracted, PracticeError> {
    let mut zip = open(zip_path)?;
    let names = entry_names(&mut zip)?;
    let worlds = worlds_in(&names);
    if worlds.is_empty() {
        return Err(PracticeError::NoWorld);
    }
    // Check every name first, so a bad zip leaves nothing behind.
    let mut plan: Vec<(usize, usize, PathBuf)> = Vec::new();
    let mut declared: u64 = 0;
    for (i, name) in names.iter().enumerate() {
        let Some((w, rest)) = place(name, &worlds) else {
            continue;
        };
        if rest.is_empty() || rest.ends_with('/') || rest.ends_with('\\') {
            continue; // the world folder itself, or a directory entry
        }
        let rel = safe_relative(&rest)
            .ok_or_else(|| PracticeError::UnsafeArchive(format!("entry name {name:?}")))?;
        if rel.file_name().is_some_and(|f| f == "session.lock") && rel.components().count() == 1 {
            continue;
        }
        let entry = zip
            .by_index_raw(i)
            .map_err(|e| PracticeError::BadArchive(e.to_string()))?;
        if entry.is_symlink() {
            return Err(PracticeError::UnsafeArchive(format!("symlink {name:?}")));
        }
        if entry.is_dir() {
            continue;
        }
        declared = declared.saturating_add(entry.size());
        plan.push((i, w, rel));
    }
    if declared > MAX_UNPACKED_BYTES {
        return Err(PracticeError::UnsafeArchive(format!(
            "expands to {declared} bytes"
        )));
    }

    let out: Vec<PathBuf> = (0..worlds.len())
        .map(|w| dest.join(w.to_string()))
        .collect();
    for dir in &out {
        std::fs::create_dir_all(dir)?;
    }
    let total = plan.len() as u64;
    let mut written: u64 = 0;
    let mut seen: HashMap<PathBuf, ()> = HashMap::new();
    for (n, (i, w, rel)) in plan.into_iter().enumerate() {
        let target = out[w].join(&rel);
        // Two entries for one path (`a/b` and `a\b`): keep the first, like most unzip tools.
        if seen.insert(target.clone(), ()).is_some() {
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut entry = zip
            .by_index(i)
            .map_err(|e| PracticeError::BadArchive(e.to_string()))?;
        let mut file = File::create(&target)?;
        // The declared size can lie; count what actually comes out.
        let budget = MAX_UNPACKED_BYTES - written;
        let copied = std::io::copy(&mut (&mut entry).take(budget + 1), &mut file)
            .map_err(|e| PracticeError::BadArchive(format!("{rel:?}: {e}")))?;
        if copied > budget {
            return Err(PracticeError::UnsafeArchive(format!(
                "expands to more than {MAX_UNPACKED_BYTES} bytes"
            )));
        }
        file.flush()?;
        written += copied;
        on_file(n as u64 + 1, total);
    }
    Ok(Extracted {
        worlds: out.into_iter().zip(worlds).collect(),
        files: total,
        bytes: written,
    })
}

/// A folder name that is safe on every OS, from a map name: separators and characters Windows
/// refuses become `_`, trailing dots and spaces go, device names get a `_`.
pub fn folder_name(wanted: &str) -> String {
    let mut s: String = wanted
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    while s.ends_with('.') || s.ends_with(' ') {
        s.pop();
    }
    let s = s.trim_start().to_string();
    if s.is_empty() {
        return "Practice map".into();
    }
    let stem = s.split('.').next().unwrap_or(&s);
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return format!("{s}_");
    }
    s
}

/// `wanted`, or `wanted (2)`, `wanted (3)`, ... : the first name that `taken` doesn't hold. Never
/// reuses a folder that exists, so an install can't land on a world the player already has.
pub fn unique_name(wanted: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(wanted) {
        return wanted.to_string();
    }
    (2..)
        .map(|n| format!("{wanted} ({n})"))
        .find(|c| !taken(c))
        .expect("an unused name")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;

    pub fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "srl-practice-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A zip with these (name, contents) entries; names ending in `/` are directories.
    pub fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(File::create(path).unwrap());
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            if name.ends_with('/') {
                w.add_directory(*name, opts).unwrap();
            } else {
                w.start_file(*name, opts).unwrap();
                w.write_all(data).unwrap();
            }
        }
        w.finish().unwrap();
    }

    #[test]
    fn zip_slip_guard() {
        assert_eq!(
            safe_relative("World/region/r.0.0.mca"),
            Some(PathBuf::from("World/region/r.0.0.mca"))
        );
        assert_eq!(
            safe_relative("World\\data\\x.dat"),
            Some(PathBuf::from("World/data/x.dat"))
        );
        assert_eq!(safe_relative("./a/./b"), Some(PathBuf::from("a/b")));
        for bad in [
            "../evil",
            "a/../../evil",
            "a\\..\\..\\evil",
            "/etc/passwd",
            "\\Windows\\evil",
            "C:\\evil",
            "C:evil",
            "a/b:stream",
            "a/CON",
            "a/nul.txt",
            "a/trailing.",
            "a/trailing ",
            "",
            "./",
            "a\0b",
        ] {
            assert_eq!(safe_relative(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn finds_worlds_in_all_layouts() {
        let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let roots = |v: Vec<ZipWorld>| v.into_iter().map(|w| w.root).collect::<Vec<_>>();
        // level.dat at the root (The MCSR Practice Map).
        assert_eq!(
            roots(worlds_in(&names(&[
                "level.dat",
                "region/r.0.0.mca",
                "DIM1/level.dat"
            ]))),
            vec![""]
        );
        // One folder (most maps).
        assert_eq!(
            roots(worlds_in(&names(&[
                "LBP 3.15.0/",
                "LBP 3.15.0/level.dat",
                "README.txt"
            ]))),
            vec!["LBP 3.15.0"]
        );
        // Several worlds, one of them deeper, plus a nested copy and macOS junk.
        assert_eq!(
            roots(worlds_in(&names(&[
                "A/level.dat",
                "pack/B/level.dat",
                "A/backup/level.dat",
                "__MACOSX/A/level.dat",
                "C\\level.dat",
            ]))),
            vec!["A", "C", "pack/B"]
        );
        assert!(worlds_in(&names(&["README.md", "datapack.zip"])).is_empty());
    }

    #[test]
    fn extracts_each_layout() {
        let dir = temp_dir("layouts");
        // Root layout.
        let z = dir.join("root.zip");
        make_zip(
            &z,
            &[
                ("level.dat", b"L"),
                ("region/", b""),
                ("region/r.0.0.mca", b"R"),
                ("session.lock", b"S"),
            ],
        );
        let out = extract_worlds(&z, &dir.join("s1"), |_, _| {}).unwrap();
        assert_eq!(out.worlds.len(), 1);
        let w = &out.worlds[0].0;
        assert_eq!(std::fs::read(w.join("level.dat")).unwrap(), b"L");
        assert_eq!(std::fs::read(w.join("region/r.0.0.mca")).unwrap(), b"R");
        assert!(!w.join("session.lock").exists());

        // One folder, with a README outside it.
        let z = dir.join("one.zip");
        make_zip(
            &z,
            &[
                ("README.txt", b"hi"),
                ("Map/level.dat", b"L"),
                ("Map/data/x.dat", b"X"),
            ],
        );
        let out = extract_worlds(&z, &dir.join("s2"), |_, _| {}).unwrap();
        assert_eq!(out.worlds.len(), 1);
        assert_eq!(out.worlds[0].1.root, "Map");
        assert_eq!(
            std::fs::read(out.worlds[0].0.join("data/x.dat")).unwrap(),
            b"X"
        );
        assert!(!dir.join("s2/README.txt").exists());
        assert_eq!(out.files, 2);

        // Several worlds.
        let z = dir.join("many.zip");
        make_zip(
            &z,
            &[
                ("Pack/A/level.dat", b"A"),
                ("Pack/B/level.dat", b"B"),
                ("Pack/B/region/r.mca", b"BR"),
            ],
        );
        let out = extract_worlds(&z, &dir.join("s3"), |_, _| {}).unwrap();
        let got: Vec<_> = out
            .worlds
            .iter()
            .map(|(p, w)| (w.root.clone(), std::fs::read(p.join("level.dat")).unwrap()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("Pack/A".to_string(), b"A".to_vec()),
                ("Pack/B".to_string(), b"B".to_vec())
            ]
        );

        // No world at all.
        let z = dir.join("none.zip");
        make_zip(&z, &[("README.txt", b"hi")]);
        assert!(matches!(
            extract_worlds(&z, &dir.join("s4"), |_, _| {}),
            Err(PracticeError::NoWorld)
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_zip_slip_and_writes_nothing() {
        let dir = temp_dir("slip");
        for (i, evil) in [
            "Map/../../evil.txt",
            "Map/..\\..\\evil.txt",
            "Map/C:evil.txt",
        ]
        .into_iter()
        .enumerate()
        {
            let z = dir.join(format!("evil{i}.zip"));
            make_zip(&z, &[("Map/level.dat", b"L"), (evil, b"pwned")]);
            let stage = dir.join(format!("stage{i}"));
            let err = extract_worlds(&z, &stage, |_, _| {}).unwrap_err();
            assert!(
                matches!(err, PracticeError::UnsafeArchive(_)),
                "{evil}: {err}"
            );
            assert!(!stage.exists(), "{evil}: nothing may be written");
        }
        assert!(!dir.join("evil.txt").exists());
        assert!(!dir.parent().unwrap().join("evil.txt").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn folder_names() {
        assert_eq!(
            folder_name("The MCSR Practice Map v2.0.0"),
            "The MCSR Practice Map v2.0.0"
        );
        assert_eq!(folder_name("a/b:c*?"), "a_b_c__");
        assert_eq!(folder_name("dots..."), "dots");
        assert_eq!(folder_name("CON"), "CON_");
        assert_eq!(folder_name("  "), "Practice map");
        let taken = ["Map", "Map (2)"];
        assert_eq!(unique_name("Map", |n| taken.contains(&n)), "Map (3)");
        assert_eq!(unique_name("Other", |n| taken.contains(&n)), "Other");
    }
}
