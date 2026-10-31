//! Merging a loader profile into its vanilla version and turning the merged library list into
//! the files to download: classpath jars in launch order and, for versions before 1.19, the
//! native jars to extract.
//!
//! The duplicate handling and the natives lookup follow Pandora Launcher (MIT, commit 2d83d6f),
//! `crates/backend/src/launch/mod.rs` (`collect_libraries`) and `crates/schema/src/version.rs`
//! (`PartialMinecraftVersion::apply_to`). See THIRD_PARTY_NOTICES.md. Changes: the loader's
//! libraries come first and win over vanilla ones with the same `group:artifact[:classifier]`
//! (the vanilla launcher's `inheritsFrom` behaviour) instead of "highest version wins", and the
//! JSON order is kept so the classpath is deterministic.
use std::collections::HashSet;

use anyhow::Result;

use crate::{
    rules::{Features, Platform, rules_allow},
    schema::{Arguments, Artifact, Library, VersionJson},
    util::{library_key, maven_path},
};

/// One file under `libraries/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryFile {
    /// Maven-layout path relative to `libraries/`.
    pub path: String,
    pub url: String,
    /// `None` only for Fabric jars before their Maven `.sha1` sidecar is fetched.
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// A pre-1.19 classifier jar whose contents are extracted into the natives directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeJar {
    pub file: LibraryFile,
    /// Path prefixes to skip, e.g. `META-INF/`.
    pub exclude: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolvedLibraries {
    /// In classpath order. The client jar is not included; it goes last.
    pub classpath: Vec<LibraryFile>,
    pub natives: Vec<NativeJar>,
}

/// Merges a loader profile (`child`, e.g. Fabric) into the vanilla version it inherits from.
/// The child's libraries go first, its main class wins and its arguments are appended.
pub fn merge(parent: VersionJson, child: VersionJson) -> VersionJson {
    let mut v = parent;
    v.id = child.id;
    v.inherits_from = None;
    let mut libraries = child.libraries;
    libraries.append(&mut v.libraries);
    v.libraries = libraries;
    if child.main_class.is_some() {
        v.main_class = child.main_class;
    }
    if let Some(extra) = child.arguments {
        let args = v.arguments.get_or_insert_with(Arguments::default);
        args.game.extend(extra.game);
        args.jvm.extend(extra.jvm);
        args.default_user_jvm.extend(extra.default_user_jvm);
    }
    if child.minecraft_arguments.is_some() {
        v.minecraft_arguments = child.minecraft_arguments;
    }
    if child.asset_index.is_some() {
        v.asset_index = child.asset_index;
        v.assets = child.assets;
    }
    if child.downloads.is_some() {
        v.downloads = child.downloads;
    }
    if child.logging.is_some() {
        v.logging = child.logging;
    }
    v
}

/// The libraries that apply on `platform`, in classpath order, without duplicates.
pub fn resolve(
    libraries: &[Library],
    platform: &Platform,
    features: &Features,
) -> Result<ResolvedLibraries> {
    let mut out = ResolvedLibraries::default();
    let mut seen_keys = HashSet::new();
    let mut seen_paths = HashSet::new();
    let mut seen_natives = HashSet::new();
    for lib in libraries {
        if let Some(rules) = &lib.rules
            && !rules_allow(rules, platform, features)
        {
            continue;
        }
        // Old-style natives: a classifier jar per OS, extracted rather than put on the classpath.
        // Collected before the duplicate check, because 1.16.1 lists e.g. `lwjgl:3.2.2` twice:
        // once for the jar and once for its natives.
        if let Some(natives) = &lib.natives
            && let Some(classifier) = natives.get(platform.mojang_os_name())
        {
            let classifier = classifier.replace("${arch}", platform.bits());
            let artifact = lib
                .downloads
                .as_ref()
                .and_then(|d| d.classifiers.as_ref())
                .and_then(|c| c.get(&classifier));
            if let Some(a) = artifact
                && seen_natives.insert(a.path.clone())
            {
                out.natives.push(NativeJar {
                    file: from_artifact(a),
                    exclude: lib
                        .extract
                        .as_ref()
                        .map(|e| e.exclude.clone())
                        .unwrap_or_default(),
                });
            }
        }
        let Some(file) = classpath_file(lib)? else {
            continue;
        };
        // The first library with a given identity wins: loader libraries come first, so they
        // replace vanilla ones (e.g. a newer ASM).
        if !seen_keys.insert(library_key(&lib.name)) || !seen_paths.insert(file.path.clone()) {
            continue;
        }
        out.classpath.push(file);
    }
    Ok(out)
}

/// The jar a library puts on the classpath, if any.
fn classpath_file(lib: &Library) -> Result<Option<LibraryFile>> {
    if let Some(downloads) = &lib.downloads {
        // Vanilla style. A library with only classifiers (natives) has no classpath jar.
        return Ok(downloads.artifact.as_ref().map(from_artifact));
    }
    // Maven style (Fabric): name + repository URL.
    let path = maven_path(&lib.name)?;
    let base = lib
        .url
        .as_deref()
        .unwrap_or(crate::schema::MOJANG_LIBRARIES_URL);
    let sep = if base.ends_with('/') { "" } else { "/" };
    Ok(Some(LibraryFile {
        url: format!("{base}{sep}{path}"),
        path,
        sha1: lib.sha1.clone(),
        size: lib.size,
    }))
}

fn from_artifact(a: &Artifact) -> LibraryFile {
    LibraryFile {
        path: a.path.clone(),
        url: a.url.clone(),
        sha1: a.sha1.clone(),
        size: a.size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Arch, Os};

    fn linux() -> Platform {
        Platform {
            os: Os::Linux,
            arch: Arch::X86_64,
            os_version: "6.8.0".into(),
        }
    }

    fn windows() -> Platform {
        Platform {
            os: Os::Windows,
            arch: Arch::X86_64,
            os_version: "10.0.22631".into(),
        }
    }

    /// A cut-down 1.16.1: a plain library, LWJGL 3.2.1 (macOS) and 3.2.2 (everything else),
    /// the 3.2.2 natives entry repeating the same name, and a library with an old ASM.
    fn vanilla() -> VersionJson {
        serde_json::from_str(
            r#"{
            "id": "1.16.1",
            "mainClass": "net.minecraft.client.main.Main",
            "arguments": {"game": ["--username", "${auth_player_name}"], "jvm": ["-cp", "${classpath}"]},
            "libraries": [
              {"name": "com.mojang:patchy:1.3.9", "downloads": {"artifact": {"path": "com/mojang/patchy/1.3.9/patchy-1.3.9.jar", "sha1": "aa", "size": 1, "url": "https://libraries.minecraft.net/com/mojang/patchy/1.3.9/patchy-1.3.9.jar"}}},
              {"name": "org.ow2.asm:asm:7.0", "downloads": {"artifact": {"path": "org/ow2/asm/asm/7.0/asm-7.0.jar", "sha1": "bb", "size": 1, "url": "u"}}},
              {"name": "org.lwjgl:lwjgl:3.2.1", "rules": [{"action": "allow", "os": {"name": "osx"}}],
               "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.2.1/lwjgl-3.2.1.jar", "sha1": "cc", "size": 1, "url": "u"}}},
              {"name": "org.lwjgl:lwjgl:3.2.2", "rules": [{"action": "allow"}, {"action": "disallow", "os": {"name": "osx"}}],
               "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2.jar", "sha1": "dd", "size": 1, "url": "u"}}},
              {"name": "org.lwjgl:lwjgl:3.2.2", "rules": [{"action": "allow"}, {"action": "disallow", "os": {"name": "osx"}}],
               "natives": {"linux": "natives-linux", "windows": "natives-windows"},
               "extract": {"exclude": ["META-INF/"]},
               "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2.jar", "sha1": "dd", "size": 1, "url": "u"},
                             "classifiers": {
                               "natives-linux": {"path": "org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2-natives-linux.jar", "sha1": "ee", "size": 1, "url": "u"},
                               "natives-windows": {"path": "org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2-natives-windows.jar", "sha1": "ff", "size": 1, "url": "u"}}}}
            ]}"#,
        )
        .unwrap()
    }

    fn fabric() -> VersionJson {
        serde_json::from_str(
            r#"{
            "id": "fabric-loader-0.20.1-1.16.1",
            "inheritsFrom": "1.16.1",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "arguments": {"game": [], "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "]},
            "libraries": [
              {"name": "org.ow2.asm:asm:9.9", "url": "https://maven.fabricmc.net/", "sha1": "99", "size": 2},
              {"name": "net.fabricmc:intermediary:1.16.1", "url": "https://maven.fabricmc.net/"},
              {"name": "net.fabricmc:fabric-loader:0.20.1", "url": "https://maven.fabricmc.net/"}
            ]}"#,
        )
        .unwrap()
    }

    fn paths(r: &ResolvedLibraries) -> Vec<&str> {
        r.classpath.iter().map(|f| f.path.as_str()).collect()
    }

    #[test]
    fn classpath_order_and_duplicates() {
        let merged = merge(vanilla(), fabric());
        assert_eq!(merged.id, "fabric-loader-0.20.1-1.16.1");
        assert_eq!(
            merged.main_class.as_deref(),
            Some("net.fabricmc.loader.impl.launch.knot.KnotClient")
        );
        let args = merged.arguments.as_ref().unwrap();
        assert_eq!(args.jvm.len(), 3, "Fabric's JVM argument is appended");

        let r = resolve(&merged.libraries, &linux(), &Features::default()).unwrap();
        assert_eq!(
            paths(&r),
            [
                // Fabric first, in profile order; its ASM 9.9 replaces vanilla's 7.0.
                "org/ow2/asm/asm/9.9/asm-9.9.jar",
                "net/fabricmc/intermediary/1.16.1/intermediary-1.16.1.jar",
                "net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar",
                // Then vanilla in JSON order, LWJGL 3.2.2 once.
                "com/mojang/patchy/1.3.9/patchy-1.3.9.jar",
                "org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2.jar",
            ]
        );
        assert_eq!(
            r.classpath[1].url,
            "https://maven.fabricmc.net/net/fabricmc/intermediary/1.16.1/intermediary-1.16.1.jar"
        );
        assert_eq!(r.classpath[1].sha1, None);
        assert_eq!(r.natives.len(), 1);
        assert_eq!(
            r.natives[0].file.path,
            "org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2-natives-linux.jar"
        );
        assert_eq!(r.natives[0].exclude, ["META-INF/"]);
    }

    #[test]
    fn os_specific_natives() {
        let v = vanilla();
        let w = resolve(&v.libraries, &windows(), &Features::default()).unwrap();
        assert_eq!(
            w.natives[0].file.path,
            "org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2-natives-windows.jar"
        );
        let mac = Platform {
            os: Os::MacOs,
            arch: Arch::Aarch64,
            os_version: String::new(),
        };
        let m = resolve(&v.libraries, &mac, &Features::default()).unwrap();
        assert!(paths(&m).contains(&"org/lwjgl/lwjgl/3.2.1/lwjgl-3.2.1.jar"));
        assert!(!paths(&m).contains(&"org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2.jar"));
        assert!(m.natives.is_empty());
    }

    #[test]
    fn new_style_natives_are_classpath_jars() {
        // 26.x: every native jar is its own library with an OS rule.
        let libs: Vec<Library> = serde_json::from_str(
            r#"[
            {"name": "org.lwjgl:lwjgl:3.4.3", "downloads": {"artifact": {"path": "l.jar", "sha1": "1", "size": 1, "url": "u"}}},
            {"name": "org.lwjgl:lwjgl:3.4.3:natives-linux", "rules": [{"action": "allow", "os": {"name": "linux"}}],
             "downloads": {"artifact": {"path": "l-natives-linux.jar", "sha1": "2", "size": 1, "url": "u"}}},
            {"name": "org.lwjgl:lwjgl:3.4.3:natives-windows", "rules": [{"action": "allow", "os": {"name": "windows"}}],
             "downloads": {"artifact": {"path": "l-natives-windows.jar", "sha1": "3", "size": 1, "url": "u"}}}
            ]"#,
        )
        .unwrap();
        let r = resolve(&libs, &linux(), &Features::default()).unwrap();
        assert_eq!(paths(&r), ["l.jar", "l-natives-linux.jar"]);
        assert!(r.natives.is_empty());
        let w = resolve(&libs, &windows(), &Features::default()).unwrap();
        assert_eq!(paths(&w), ["l.jar", "l-natives-windows.jar"]);
    }
}
