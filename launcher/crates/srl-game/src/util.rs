//! Small helpers shared by the install steps.
use std::{
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result, bail};

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// A unique temporary sibling path next to `path`, so concurrent writers never collide.
pub(crate) fn unique_tmp_path(path: &Path) -> PathBuf {
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.{seq}.tmp", std::process::id()));
    PathBuf::from(tmp)
}

/// A path from a manifest or an archive, checked to stay inside the directory it is joined to:
/// no root, drive or `..`, no `:` (Windows drives and alternate data streams), and `\` counts as
/// a separator so a Windows-style `..\..\x` is caught on Linux too. `.` components are dropped.
/// `None` means the path must be rejected.
pub fn safe_relative_path(name: &str) -> Option<PathBuf> {
    if name.starts_with(['/', '\\']) {
        return None;
    }
    let mut out = PathBuf::new();
    for part in name.split(['/', '\\']) {
        match part {
            "" | "." => continue,
            ".." => return None,
            p if p.contains(':') => return None,
            p => {
                // Belt and braces: let std parse it too.
                let mut comps = Path::new(p).components();
                match (comps.next(), comps.next()) {
                    (Some(Component::Normal(_)), None) => out.push(p),
                    _ => return None,
                }
            }
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// `base.join(name)` if `name` is a safe relative path, else an error.
pub fn join_safe(base: &Path, name: &str) -> Result<PathBuf> {
    match safe_relative_path(name) {
        Some(rel) => Ok(base.join(rel)),
        None => bail!("refusing unsafe path {name:?}"),
    }
}

/// The Maven repository path of `group:artifact:version[:classifier][@ext]`.
pub fn maven_path(coordinate: &str) -> Result<String> {
    let (main, ext) = coordinate.split_once('@').unwrap_or((coordinate, "jar"));
    let mut parts = main.split(':');
    let (Some(group), Some(artifact), Some(version)) = (parts.next(), parts.next(), parts.next())
    else {
        bail!("not a Maven coordinate: {coordinate}");
    };
    let classifier = parts.next();
    let mut path = format!(
        "{}/{artifact}/{version}/{artifact}-{version}",
        group.replace('.', "/")
    );
    if let Some(c) = classifier {
        path.push('-');
        path.push_str(c);
    }
    path.push('.');
    path.push_str(ext);
    Ok(path)
}

/// `group:artifact[:classifier]`, the identity used to drop duplicate libraries.
pub fn library_key(coordinate: &str) -> String {
    let main = coordinate.split('@').next().unwrap_or(coordinate);
    let parts: Vec<&str> = main.split(':').collect();
    match parts.as_slice() {
        [group, artifact, _version, classifier, ..] => format!("{group}:{artifact}:{classifier}"),
        [group, artifact, ..] => format!("{group}:{artifact}"),
        _ => main.to_string(),
    }
}

/// Compares dotted versions numerically (`0.16.10 > 0.16.9`, `0.16.0 == 0.16`); a prerelease
/// (`0.16.0-beta.1`) compares less than its release (`0.16.0`).
pub fn cmp_versions(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    fn split(s: &str) -> (&str, Option<&str>, Option<&str>) {
        let (ver, build) = match s.split_once('+') {
            Some((v, b)) => (v, Some(b)),
            None => (s, None),
        };
        let (main, pre) = match ver.split_once('-') {
            Some((m, p)) => (m, Some(p)),
            None => (ver, None),
        };
        (main, pre, build)
    }

    fn cmp_numeric(a: &str, b: &str) -> Ordering {
        let a: Vec<u64> = a.split('.').map(|p| p.parse().unwrap_or(0)).collect();
        let b: Vec<u64> = b.split('.').map(|p| p.parse().unwrap_or(0)).collect();
        let n = a.len().max(b.len());
        for i in 0..n {
            let (x, y) = (
                a.get(i).copied().unwrap_or(0),
                b.get(i).copied().unwrap_or(0),
            );
            if x != y {
                return x.cmp(&y);
            }
        }
        Ordering::Equal
    }

    fn cmp_identifiers(a: &str, b: &str) -> Ordering {
        let mut ia = a.split(['.', '-']);
        let mut ib = b.split(['.', '-']);
        loop {
            match (ia.next(), ib.next()) {
                (Some(x), Some(y)) => {
                    let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
                        (Ok(nx), Ok(ny)) => nx.cmp(&ny),
                        _ => x.cmp(y),
                    };
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
                (Some(_), None) => return Ordering::Greater,
                (None, Some(_)) => return Ordering::Less,
                (None, None) => return Ordering::Equal,
            }
        }
    }

    let (am, ap, ab) = split(a);
    let (bm, bp, bb) = split(b);
    let main_ord = cmp_numeric(am, bm);
    if main_ord != Ordering::Equal {
        return main_ord;
    }
    let pre_ord = match (ap, bp) {
        (None, None) => Ordering::Equal,
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (Some(x), Some(y)) => cmp_identifiers(x, y),
    };
    if pre_ord != Ordering::Equal {
        return pre_ord;
    }
    match (ab, bb) {
        (None, None) => Ordering::Equal,
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (Some(x), Some(y)) => cmp_identifiers(x, y),
    }
}

/// Writes a file through a unique temporary file and a rename, so readers never see half a file.
pub async fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let tmp = unique_tmp_path(path);
    if let Err(e) = tokio::fs::write(&tmp, bytes).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e).with_context(|| format!("writing {}", tmp.display()));
    }
    if let Err(e) = tokio::fs::rename(&tmp, path).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e).with_context(|| format!("renaming to {}", path.display()));
    }
    Ok(())
}

/// The path as UTF-8, which every argument we pass to Java must be.
pub fn utf8(path: &Path) -> Result<&str> {
    path.to_str()
        .with_context(|| format!("path is not valid UTF-8: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_slip_guard() {
        assert_eq!(
            safe_relative_path("liblwjgl.so"),
            Some(PathBuf::from("liblwjgl.so"))
        );
        assert_eq!(
            safe_relative_path("./org/lwjgl/x.dll"),
            Some(PathBuf::from("org/lwjgl/x.dll"))
        );
        for evil in [
            "../evil.so",
            "a/../../evil.so",
            "/etc/passwd",
            "..\\..\\Windows\\evil.dll",
            "C:\\Windows\\evil.dll",
            "C:evil.dll",
            "file.txt:stream",
            "",
            "./",
        ] {
            assert_eq!(safe_relative_path(evil), None, "{evil:?}");
        }
        assert!(join_safe(Path::new("/srl/natives"), "../x").is_err());
    }

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.20.1").unwrap(),
            "net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar"
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.4.3:natives-linux").unwrap(),
            "org/lwjgl/lwjgl/3.4.3/lwjgl-3.4.3-natives-linux.jar"
        );
        assert_eq!(
            maven_path("a.b:c:1@zip").unwrap(),
            "a/b/c/1/c-1.zip".to_string()
        );
        assert!(maven_path("nonsense").is_err());
        assert_eq!(library_key("org.ow2.asm:asm:9.9"), "org.ow2.asm:asm");
        assert_eq!(
            library_key("org.lwjgl:lwjgl:3.4.3:natives-linux"),
            "org.lwjgl:lwjgl:natives-linux"
        );
    }

    #[test]
    fn version_order() {
        use std::cmp::Ordering::*;
        assert_eq!(cmp_versions("0.16.10", "0.16.9"), Greater);
        assert_eq!(cmp_versions("0.16.0", "0.16"), Equal);
        assert_eq!(cmp_versions("0.15.11", "0.16.0"), Less);
        assert_eq!(cmp_versions("0.16.0-beta.1", "0.16.0"), Less);
        assert_eq!(cmp_versions("0.16.0", "0.16.0-beta.1"), Greater);
        assert_eq!(cmp_versions("0.16.0-beta.2", "0.16.0-beta.10"), Less);
        assert_eq!(cmp_versions("0.16.0-alpha.1", "0.16.0-beta.1"), Less);
        assert_eq!(cmp_versions("21.0.7+6", "21.0.7+10"), Less);
        assert_ne!(
            unique_tmp_path(Path::new("/tmp/a.json")),
            unique_tmp_path(Path::new("/tmp/a.json"))
        );
    }
}
