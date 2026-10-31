//! Extracting pre-1.19 native jars (LWJGL 3.2.2 for 1.16.1) into `natives/<version>/`.
//!
//! Newer versions ship natives as ordinary classpath jars and LWJGL unpacks them itself, so this
//! only runs when the resolved libraries list native jars.
use std::{
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::util::safe_relative_path;

/// Written after a successful extraction: the sha1s of the source jars, a `---` line, then one
/// `<size>\t<path>` line per extracted file. If the sha1s match and every file is there with its
/// size, the directory is already right and nothing is extracted again. The file list matters:
/// an antivirus that quarantines `lwjgl.dll`, or a user who deletes a file, would otherwise leave
/// every 1.16.1 mode failing with "Failed to locate library" while the launcher says Ready.
const MARKER: &str = ".srl-natives";
const MARKER_FILES: &str = "\n---\n";

/// One jar to extract: its path on disk, its sha1 and the prefixes to skip.
pub struct NativeSource {
    pub jar: PathBuf,
    pub sha1: String,
    pub exclude: Vec<String>,
}

/// Extracts all jars into `dir`. Skips `META-INF/` always (signatures and manifests are not
/// natives), plus each jar's own `extract.exclude`, and refuses entries that would land outside
/// `dir` (zip slip).
pub fn extract_all(sources: &[NativeSource], dir: &Path) -> Result<()> {
    let mut stamp: Vec<&str> = sources.iter().map(|s| s.sha1.as_str()).collect();
    stamp.sort_unstable();
    let stamp = stamp.join("\n");
    let marker = dir.join(MARKER);
    if std::fs::read_to_string(&marker).is_ok_and(|s| marker_holds(&s, &stamp, dir)) {
        return Ok(());
    }
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let mut files = Vec::new();
    for s in sources {
        files.extend(
            extract_jar(&s.jar, dir, &s.exclude)
                .with_context(|| format!("extracting natives from {}", s.jar.display()))?,
        );
    }
    let mut text = stamp;
    text.push_str(MARKER_FILES);
    for (rel, size) in files {
        text.push_str(&format!("{size}\t{rel}\n"));
    }
    std::fs::write(&marker, text)?;
    Ok(())
}

/// Whether a marker names these source jars and every file it lists is still there, unchanged
/// in size. A marker from before the file list counts as stale, so it is extracted once more.
fn marker_holds(marker: &str, stamp: &str, dir: &Path) -> bool {
    let Some((sources, files)) = marker.split_once(MARKER_FILES) else {
        return false;
    };
    sources == stamp
        && files.lines().filter(|l| !l.is_empty()).all(|l| {
            let Some((size, rel)) = l.split_once('\t') else {
                return false;
            };
            let (Ok(size), Some(rel)) = (size.parse::<u64>(), safe_relative_path(rel)) else {
                return false;
            };
            std::fs::metadata(dir.join(rel)).is_ok_and(|m| m.is_file() && m.len() == size)
        })
}

/// Extracts one jar into `dir` and returns what it wrote: each file's path inside `dir` (with
/// `/` separators, as in the jar) and its size.
pub fn extract_jar(jar: &Path, dir: &Path, exclude: &[String]) -> Result<Vec<(String, u64)>> {
    let file = std::fs::File::open(jar)?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))?;
    let mut written = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name()?.to_string();
        if entry.is_dir() || is_excluded(&name, exclude) {
            continue;
        }
        let Some(rel) = safe_relative_path(&name) else {
            anyhow::bail!("refusing to extract {name:?}: it points outside the natives directory");
        };
        let out = dir.join(rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        // Already right: leave it. A re-extraction (one file went missing) then never replaces a
        // library that a running game of another mode has loaded, which Windows refuses.
        if std::fs::metadata(&out).is_ok_and(|m| m.len() == bytes.len() as u64)
            && std::fs::read(&out).is_ok_and(|old| old == bytes)
        {
            written.push((name, bytes.len() as u64));
            continue;
        }
        // Through a temporary file: a running game may have the old library mapped.
        let mut tmp = out.as_os_str().to_owned();
        tmp.push(".tmp");
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, &out)?;
        written.push((name, bytes.len() as u64));
    }
    Ok(written)
}

fn is_excluded(name: &str, exclude: &[String]) -> bool {
    name.starts_with("META-INF/") || exclude.iter().any(|e| name.starts_with(e.as_str()))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn jar(path: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, data) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("srl-game-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn extracts_and_skips_meta_inf() {
        let dir = scratch("natives-ok");
        let src = dir.join("natives.jar");
        jar(
            &src,
            &[
                ("liblwjgl.so", b"elf"),
                ("linux/x64/libglfw.so", b"elf2"),
                ("META-INF/MANIFEST.MF", b"m"),
                ("skipme/x.txt", b"x"),
            ],
        );
        let out = dir.join("out");
        let sources = [NativeSource {
            jar: src,
            sha1: "aa".into(),
            exclude: vec!["skipme/".into()],
        }];
        extract_all(&sources, &out).unwrap();
        assert_eq!(std::fs::read(out.join("liblwjgl.so")).unwrap(), b"elf");
        assert!(out.join("linux/x64/libglfw.so").exists());
        assert!(!out.join("META-INF").exists());
        assert!(!out.join("skipme").exists());
        // Second run: the marker matches and every file is there, so nothing is touched.
        let marker = out.join(MARKER);
        let before = std::fs::metadata(&marker).unwrap().modified().unwrap();
        extract_all(&sources, &out).unwrap();
        assert_eq!(
            std::fs::metadata(&marker).unwrap().modified().unwrap(),
            before
        );
        // A file gone (an antivirus, a user) or changed: extracted again.
        std::fs::remove_file(out.join("liblwjgl.so")).unwrap();
        extract_all(&sources, &out).unwrap();
        assert_eq!(std::fs::read(out.join("liblwjgl.so")).unwrap(), b"elf");
        std::fs::write(out.join("linux/x64/libglfw.so"), b"elf2 damaged").unwrap();
        extract_all(&sources, &out).unwrap();
        assert_eq!(
            std::fs::read(out.join("linux/x64/libglfw.so")).unwrap(),
            b"elf2"
        );
        // A marker from before the file list counts as stale.
        std::fs::write(&marker, "aa").unwrap();
        std::fs::remove_file(out.join("liblwjgl.so")).unwrap();
        extract_all(&sources, &out).unwrap();
        assert!(out.join("liblwjgl.so").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_zip_slip() {
        let dir = scratch("natives-slip");
        let src = dir.join("evil.jar");
        jar(&src, &[("../../evil.so", b"boom")]);
        let out = dir.join("a").join("out");
        let err = extract_jar(&src, &out, &[]).unwrap_err();
        assert!(err.to_string().contains("outside"), "{err}");
        assert!(!dir.join("evil.so").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
