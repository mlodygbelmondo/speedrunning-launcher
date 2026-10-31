//! Java runtimes from Mojang's runtime manifest, installed file by file into
//! `java/<component>-<version>/` (see `home`) with a sha1 per file.
//!
//! The manifest handling follows Pandora Launcher (MIT, commit 2d83d6f),
//! `crates/backend/src/launch/mod.rs` (`load_mojang_java_binary`, `do_java_runtime_load`). See
//! THIRD_PARTY_NOTICES.md. Changes: downloads go through srl-core's verified `Downloader` (raw
//! files, no LZMA), links are created with their relative target after checking it stays inside
//! the runtime, and on Windows we launch `javaw.exe` so no console window opens.
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use srl_core::{Download, Hash};

use crate::{
    rules::{Arch, Os, Platform},
    schema::{JavaRuntimeFile, JavaRuntimeManifest},
    util::{join_safe, safe_relative_path},
};

/// An installed Java runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaRuntime {
    /// Mojang component, e.g. `java-runtime-delta`.
    pub component: String,
    /// E.g. `21.0.7`.
    pub version: String,
    /// The runtime's root (`JAVA_HOME`).
    pub home: PathBuf,
    /// What we start the game with: `bin/javaw.exe` on Windows, `bin/java` elsewhere.
    pub executable: PathBuf,
}

/// The platform key of Mojang's runtime manifest. Linux on ARM has no Mojang runtimes.
pub fn platform_key(p: &Platform) -> Result<&'static str> {
    Ok(match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => "linux",
        (Os::Linux, Arch::X86) => "linux-i386",
        (Os::Windows, Arch::X86_64) => "windows-x64",
        (Os::Windows, Arch::X86) => "windows-x86",
        (Os::Windows, Arch::Aarch64) => "windows-arm64",
        (Os::MacOs, Arch::Aarch64) => "mac-os-arm64",
        (Os::MacOs, _) => "mac-os",
        (Os::Linux, Arch::Aarch64) => {
            bail!("Mojang publishes no Java runtime for Linux on ARM; set a custom Java")
        }
    })
}

/// The files of a runtime: what to download, which files need the executable bit, and links.
#[derive(Clone, Debug)]
pub struct RuntimePlan {
    pub downloads: Vec<Download>,
    pub executables: Vec<PathBuf>,
    pub directories: Vec<PathBuf>,
    /// (link path, relative target)
    pub links: Vec<(PathBuf, String)>,
}

pub fn plan(manifest: &JavaRuntimeManifest, home: &Path) -> Result<RuntimePlan> {
    let mut plan = RuntimePlan {
        downloads: Vec::new(),
        executables: Vec::new(),
        directories: Vec::new(),
        links: Vec::new(),
    };
    // Each link target is checked on its own name, lexically. A path that goes through another
    // link (`lib/up/x` with `lib/up -> ..`) would be resolved on disk through that link, so it
    // could end up outside the runtime although every check passed. No runtime needs that.
    let link_names: Vec<&Path> = manifest
        .files
        .iter()
        .filter(|(_, f)| matches!(f, JavaRuntimeFile::Link { .. }))
        .map(|(name, _)| Path::new(name.as_str()))
        .collect();
    for (name, file) in &manifest.files {
        if let Some(via) = Path::new(name)
            .ancestors()
            .skip(1)
            .find(|a| link_names.contains(a))
        {
            bail!(
                "Java runtime entry {name} goes through the link {}",
                via.display()
            );
        }
        let path = join_safe(home, name)?;
        match file {
            JavaRuntimeFile::Directory => plan.directories.push(path),
            JavaRuntimeFile::File {
                executable,
                downloads,
            } => {
                if *executable {
                    plan.executables.push(path.clone());
                }
                plan.downloads.push(Download {
                    url: downloads.raw.url.clone(),
                    dest: path,
                    hash: Hash::Sha1(downloads.raw.sha1.clone()),
                    size: Some(downloads.raw.size),
                });
            }
            JavaRuntimeFile::Link { target } => {
                let parent = Path::new(name).parent().unwrap_or(Path::new(""));
                if !link_stays_inside(parent, target) {
                    bail!("Java runtime link {name} -> {target} points outside the runtime");
                }
                plan.links.push((path, target.clone()));
            }
        }
    }
    Ok(plan)
}

/// After the downloads: directories, executable bits and links.
pub fn finish(plan: &RuntimePlan) -> Result<()> {
    for dir in &plan.directories {
        std::fs::create_dir_all(dir)?;
    }
    #[cfg(unix)]
    for exe in &plan.executables {
        use anyhow::Context as _;
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::metadata(exe)?.permissions();
        if perms.mode() & 0o111 != 0o111 {
            std::fs::set_permissions(exe, std::fs::Permissions::from_mode(0o755))
                .with_context(|| format!("making {} executable", exe.display()))?;
        }
    }
    for (link, target) in &plan.links {
        make_link(link, target)?;
    }
    Ok(())
}

#[cfg(unix)]
fn make_link(link: &Path, target: &str) -> Result<()> {
    use anyhow::Context as _;
    if let Ok(existing) = std::fs::read_link(link) {
        if existing == Path::new(target) {
            return Ok(());
        }
        std::fs::remove_file(link)?;
    } else if link.exists() {
        std::fs::remove_file(link)?;
    }
    if let Some(dir) = link.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::os::unix::fs::symlink(target, link)
        .with_context(|| format!("linking {} -> {target}", link.display()))
}

/// Windows runtimes have no links today. If one ever does, copy the target: symlinks need
/// developer mode or admin rights there.
#[cfg(not(unix))]
fn make_link(link: &Path, target: &str) -> Result<()> {
    let source = link.parent().unwrap_or(Path::new("")).join(target);
    if !link.exists() && source.is_file() {
        std::fs::copy(&source, link)?;
    }
    Ok(())
}

/// True if `parent/target` stays inside the runtime root.
fn link_stays_inside(parent: &Path, target: &str) -> bool {
    let mut depth: i64 = parent.components().count() as i64;
    for part in target.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => depth -= 1,
            p if p.contains(':') => return false,
            _ => depth += 1,
        }
        if depth < 0 {
            return false;
        }
    }
    !target.starts_with('/') && !target.starts_with('\\')
}

/// Where the launcher binary lives inside a runtime.
pub fn executable(home: &Path, platform: &Platform) -> PathBuf {
    match platform.os {
        Os::Windows => home.join("bin").join("javaw.exe"),
        Os::MacOs => {
            let bundle = home.join("jre.bundle/Contents/Home/bin/java");
            if bundle.exists() {
                bundle
            } else {
                home.join("bin").join("java")
            }
        }
        Os::Linux => home.join("bin").join("java"),
    }
}

/// Checks a component name before it becomes a directory name.
pub fn check_component(component: &str) -> Result<()> {
    match safe_relative_path(component) {
        Some(p) if p.components().count() == 1 => Ok(()),
        _ => bail!("bad Java component name {component:?}"),
    }
}

/// Where a runtime version lives: `<java root>/<component>-<version>/`, one directory per
/// version. A new build (Mojang moving delta from 21.0.7 to 21.0.8) goes into a directory of its
/// own, so it never rewrites files of the runtime a running game has loaded (Windows refuses to
/// replace a loaded `jvm.dll`; Linux would let the running JVM load libraries of the newer
/// build), and an interrupted update leaves the previous runtime whole.
pub fn home(java_root: &Path, component: &str, version: &str) -> Result<PathBuf> {
    let name = format!("{component}-{version}");
    match safe_relative_path(&name) {
        Some(p) if p.components().count() == 1 => Ok(java_root.join(name)),
        _ => bail!("bad Java runtime version {version:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_must_stay_inside() {
        assert!(link_stays_inside(
            Path::new("legal/java.compiler"),
            "../java.base/LICENSE"
        ));
        assert!(!link_stays_inside(Path::new("legal"), "../../etc/passwd"));
        assert!(!link_stays_inside(Path::new(""), "/etc/passwd"));
    }

    #[test]
    fn plan_from_manifest() {
        let m: JavaRuntimeManifest = serde_json::from_str(
            r#"{"files": {
              "bin": {"type": "directory"},
              "bin/java": {"type": "file", "executable": true, "downloads": {"raw": {"sha1": "aa", "size": 10, "url": "u"}}},
              "legal/a/LICENSE": {"type": "link", "target": "../b/LICENSE"}
            }}"#,
        )
        .unwrap();
        let p = plan(&m, Path::new("/srl/java/java-runtime-delta")).unwrap();
        assert_eq!(p.downloads.len(), 1);
        assert_eq!(
            p.executables,
            [PathBuf::from("/srl/java/java-runtime-delta/bin/java")]
        );
        assert_eq!(p.links.len(), 1);
        let evil: JavaRuntimeManifest = serde_json::from_str(
            r#"{"files": {"../../x": {"type": "file", "executable": false, "downloads": {"raw": {"sha1": "aa", "size": 1, "url": "u"}}}}}"#,
        )
        .unwrap();
        assert!(plan(&evil, Path::new("/srl/java/x")).is_err());
        assert!(check_component("java-runtime-delta").is_ok());
        assert!(check_component("../x").is_err());
        assert_eq!(
            home(Path::new("/srl/java"), "java-runtime-delta", "21.0.7").unwrap(),
            Path::new("/srl/java/java-runtime-delta-21.0.7")
        );
        assert!(home(Path::new("/srl/java"), "java-runtime-delta", "1/../../x").is_err());
    }

    /// Two links that each stay inside on paper, chained so that a file entry under them would
    /// be written outside the runtime.
    #[test]
    fn entries_under_a_link_are_refused() {
        let chained: JavaRuntimeManifest = serde_json::from_str(
            r#"{"files": {
              "lib/up": {"type": "link", "target": ".."},
              "lib/up/up2": {"type": "link", "target": ".."},
              "lib/up/up2/x": {"type": "file", "executable": false, "downloads": {"raw": {"sha1": "aa", "size": 1, "url": "u"}}}
            }}"#,
        )
        .unwrap();
        let err = plan(&chained, Path::new("/srl/java/x")).unwrap_err();
        assert!(err.to_string().contains("goes through the link"), "{err}");
    }

    #[test]
    fn platform_keys() {
        let p = |os, arch| Platform {
            os,
            arch,
            os_version: String::new(),
        };
        assert_eq!(platform_key(&p(Os::Linux, Arch::X86_64)).unwrap(), "linux");
        assert_eq!(
            platform_key(&p(Os::Windows, Arch::X86_64)).unwrap(),
            "windows-x64"
        );
        assert!(platform_key(&p(Os::Linux, Arch::Aarch64)).is_err());
        assert!(
            executable(Path::new("/j"), &p(Os::Windows, Arch::X86_64)).ends_with("bin/javaw.exe")
        );
    }
}
