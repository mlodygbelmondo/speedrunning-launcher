//! Extracting a release zip. It comes over the network, so it is untrusted until proven otherwise
//! even after its checksum matched (the checksum comes from the same place): every entry name
//! passes srl-practice's zip-slip guard (`safe_relative`: no absolute paths, drive prefixes, `..`,
//! `:` or Windows device names), symlinks and duplicate names are refused, and the number of
//! entries and the bytes they expand to are capped. A zip that breaks a rule is refused as a
//! whole, before anything is written.
use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use srl_practice::archive::safe_relative;

/// Today's zip has about ten entries.
pub const MAX_ENTRIES: usize = 1_000;
/// And expands to about 60 MB.
pub const MAX_UNPACKED_BYTES: u64 = 512 << 20;

/// Extracts the zip at `zip_path` into `dest` (created; it should not exist yet) and returns the
/// files written, relative to `dest`.
pub fn extract(zip_path: &Path, dest: &Path) -> Result<Vec<PathBuf>> {
    let file = File::open(zip_path).with_context(|| format!("opening {}", zip_path.display()))?;
    let mut zip = zip::ZipArchive::new(file).context("not a zip")?;
    if zip.len() > MAX_ENTRIES {
        bail!("{} entries (the limit is {MAX_ENTRIES})", zip.len());
    }
    // Every name first, so a bad zip leaves nothing behind.
    let mut plan: Vec<(usize, PathBuf)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut declared: u64 = 0;
    for i in 0..zip.len() {
        let entry = zip.by_index_raw(i)?;
        let name = entry.name()?.into_owned();
        if entry.is_symlink() {
            bail!("symlink {name:?}");
        }
        if entry.is_dir() {
            continue;
        }
        let rel = safe_relative(&name).with_context(|| format!("unsafe entry name {name:?}"))?;
        // Windows compares names without case.
        if !seen.insert(rel.to_string_lossy().to_lowercase()) {
            bail!("{name:?} is in the zip twice");
        }
        declared = declared.saturating_add(entry.size());
        plan.push((i, rel));
    }
    if declared > MAX_UNPACKED_BYTES {
        bail!("expands to {declared} bytes (the limit is {MAX_UNPACKED_BYTES})");
    }
    std::fs::create_dir_all(dest)?;
    let mut written: u64 = 0;
    for (i, rel) in &plan {
        let target = dest.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut entry = zip.by_index(*i)?;
        let mut out =
            File::create(&target).with_context(|| format!("creating {}", target.display()))?;
        // The declared size can lie; count what actually comes out.
        let budget = MAX_UNPACKED_BYTES - written;
        let copied = std::io::copy(&mut (&mut entry).take(budget + 1), &mut out)
            .with_context(|| format!("extracting {}", rel.display()))?;
        if copied > budget {
            bail!("expands to more than {MAX_UNPACKED_BYTES} bytes");
        }
        out.flush()?;
        written += copied;
    }
    Ok(plan.into_iter().map(|(_, rel)| rel).collect())
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Write;

    use super::*;

    pub fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "srl-update-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(File::create(path).unwrap());
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn extracts_the_release_layout() {
        let dir = temp_dir("extract");
        let zip = dir.join("a.zip");
        make_zip(
            &zip,
            &[
                ("speedrunning-launcher.exe", b"MZ new"),
                ("README.txt", b"read me"),
                ("./licenses/OFL.txt", b"ofl"),
            ],
        );
        let files = extract(&zip, &dir.join("out")).unwrap();
        assert_eq!(
            files,
            [
                PathBuf::from("speedrunning-launcher.exe"),
                PathBuf::from("README.txt"),
                Path::new("licenses").join("OFL.txt"),
            ]
        );
        assert_eq!(
            std::fs::read(dir.join("out/licenses/OFL.txt")).unwrap(),
            b"ofl"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_path_traversal_and_writes_nothing() {
        let dir = temp_dir("traversal");
        for (i, bad) in [
            "../evil.exe",
            "licenses/../../evil.exe",
            "/etc/evil",
            "C:/Windows/evil.exe",
            "C:evil.exe",
            "..\\evil.exe",
            "\\\\server\\share\\evil",
            "NUL.txt",
        ]
        .into_iter()
        .enumerate()
        {
            let zip = dir.join(format!("{i}.zip"));
            make_zip(
                &zip,
                &[("speedrunning-launcher.exe", b"MZ"), (bad, b"evil")],
            );
            let out = dir.join(format!("out{i}"));
            let err = extract(&zip, &out).unwrap_err();
            assert!(
                format!("{err:#}").contains("unsafe entry name"),
                "{bad}: {err:#}"
            );
            // Refused before anything was written: not even the good entry.
            assert!(!out.exists(), "{bad}");
        }
        assert!(!dir.parent().unwrap().join("evil.exe").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_duplicates_and_garbage() {
        let dir = temp_dir("dupes");
        let zip = dir.join("d.zip");
        make_zip(&zip, &[("README.txt", b"a"), ("readme.TXT", b"b")]);
        assert!(format!("{:#}", extract(&zip, &dir.join("o")).unwrap_err()).contains("twice"));
        let junk = dir.join("junk.zip");
        std::fs::write(&junk, b"this is not a zip").unwrap();
        assert!(extract(&junk, &dir.join("o2")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
