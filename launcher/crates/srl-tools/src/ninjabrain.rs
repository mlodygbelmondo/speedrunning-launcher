//! Ninjabrain Bot: a Java (8+) Swing app that reads F3+C from the clipboard and finds the
//! stronghold.
//!
//! - **Version:** 1.5.2. speedrun.com rules v8 (2026-10-17), A.11.7.a, allow "NinjabrainBot (up
//!   to v1.5.2)", and 1.5.2 is also its newest GitHub release (2026-03-24).
//! - **Download:** the release asset `Ninjabrain-Bot-1.5.2.jar`, checked against the sha256 that
//!   GitHub publishes as the asset's `digest`, pinned here so no API call is needed.
//! - **Java:** the runtime of the mode it starts with (`srl_game::Installed::java`): Mojang's
//!   `java-runtime-delta`, Java 21, for the 1.16.1 modes, the only ones that allow it. It is
//!   already installed and verified for the game, so the tool costs no second runtime, and
//!   Ninjabrain is built for Java 8 and runs on newer ones (waywall's docs even advise Java 17+
//!   against a blank window). On Windows that is `javaw.exe`, so no console opens.
//! - **Already running:** a Ninjabrain the player started (found by its command line on Linux,
//!   by its window on Windows) is used as it is and never stopped by the launcher.
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use srl_core::{Download, Downloader, Hash, Paths, ProgressFn, Stage};

use crate::{
    ToolId,
    side::SideCommand,
    status::{InstalledTool, ToolStatus},
};

pub const VERSION: &str = "1.5.2";
const JAR: &str = "Ninjabrain-Bot-1.5.2.jar";
const URL: &str = "https://github.com/Ninjabrain1/Ninjabrain-Bot/releases/download/1.5.2/Ninjabrain-Bot-1.5.2.jar";
/// GitHub's asset digest for `Ninjabrain-Bot-1.5.2.jar` (`gh api repos/Ninjabrain1/Ninjabrain-Bot/releases`).
pub const SHA256: &str = "98099f6321a90d4acec13400e86d05f7af8d60e5639c2f38427e968dc71450ff";
pub const SIZE: u64 = 7_618_461;

pub fn dir(paths: &Paths) -> PathBuf {
    paths.tools().join(ToolId::NinjabrainBot.id())
}

pub fn jar(paths: &Paths) -> PathBuf {
    dir(paths).join(VERSION).join(JAR)
}

pub fn download() -> Download {
    Download {
        url: URL.into(),
        dest: PathBuf::new(),
        hash: Hash::Sha256(SHA256.into()),
        size: Some(SIZE),
    }
}

/// From the files on disk: the pinned jar with its size, or an older version's directory.
pub fn status(paths: &Paths) -> ToolStatus {
    let jar = jar(paths);
    if std::fs::metadata(&jar).is_ok_and(|m| m.len() == SIZE) {
        return ToolStatus::Installed(InstalledTool {
            version: Some(VERSION.into()),
            path: jar,
            managed: true,
        });
    }
    match other_versions(paths).into_iter().next() {
        Some(old) => ToolStatus::UpdateAvailable {
            installed: old,
            latest: VERSION.into(),
        },
        None => ToolStatus::NotInstalled,
    }
}

/// Version directories other than the pinned one that hold a jar.
fn other_versions(paths: &Paths) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir(paths)) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|v| v != VERSION)
        .filter(|v| {
            std::fs::read_dir(dir(paths).join(v)).is_ok_and(|mut it| {
                it.any(|f| f.is_ok_and(|f| f.file_name().to_string_lossy().ends_with(".jar")))
            })
        })
        .collect();
    out.sort();
    out.reverse();
    out
}

/// Downloads the pinned jar (verified) unless it is already there, then removes other versions.
pub async fn install(
    paths: &Paths,
    downloader: &Downloader,
    progress: Option<ProgressFn>,
) -> Result<PathBuf> {
    let dest = jar(paths);
    let d = Download {
        dest: dest.clone(),
        ..download()
    };
    downloader
        .download_all(vec![d], Stage::Tools, progress)
        .await
        .context("downloading Ninjabrain Bot")?;
    for old in other_versions(paths) {
        let _ = tokio::fs::remove_dir_all(dir(paths).join(old)).await;
    }
    Ok(dest)
}

/// True if the jar on disk has the pinned hash (hashes 7.6 MB, a few ms).
pub async fn verify(paths: &Paths) -> Result<bool> {
    srl_core::download::matches_hash(&jar(paths), &Hash::Sha256(SHA256.into())).await
}

/// `java -jar Ninjabrain-Bot-1.5.2.jar`, run from its version directory.
pub fn command(java: &Path, jar: &Path) -> SideCommand {
    SideCommand {
        program: java.to_path_buf(),
        args: vec!["-jar".into(), jar.display().to_string()],
        cwd: jar.parent().map(Path::to_path_buf).unwrap_or_default(),
    }
}

/// True if a command line is a Ninjabrain Bot: a `-jar` whose file name starts with
/// `Ninjabrain` (any version, any folder: the player's own copy counts).
pub fn is_ninjabrain_cmdline(args: &[&str]) -> bool {
    args.iter().any(|a| {
        let name = a
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(a)
            .to_ascii_lowercase();
        name.starts_with("ninjabrain") && name.ends_with(".jar")
    })
}

/// The pids of running Ninjabrain Bots, except `ignore` (the launcher's own).
pub fn running(ignore: &[u32]) -> Vec<u32> {
    imp::running()
        .into_iter()
        .filter(|p| !ignore.contains(p))
        .collect()
}

#[cfg(target_os = "linux")]
mod imp {
    pub fn running() -> Vec<u32> {
        let Ok(dir) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        dir.flatten()
            .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
            .filter(|pid| {
                std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|raw| {
                    let text = String::from_utf8_lossy(&raw);
                    let args: Vec<&str> = text.split('\0').collect();
                    super::is_ninjabrain_cmdline(&args)
                })
            })
            .collect()
    }
}

#[cfg(windows)]
mod imp {
    pub fn running() -> Vec<u32> {
        crate::win::java_windows_titled("Ninjabrain Bot")
    }
}

#[cfg(all(not(target_os = "linux"), not(windows)))]
mod imp {
    /// macOS: `ps` lists the command lines.
    pub fn running() -> Vec<u32> {
        let Ok(out) = std::process::Command::new("ps")
            .args(["-axo", "pid=,args="])
            .output()
        else {
            return Vec::new();
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| {
                let l = l.trim_start();
                let (pid, args) = l.split_once(' ')?;
                let args: Vec<&str> = args.split_whitespace().collect();
                super::is_ninjabrain_cmdline(&args).then(|| pid.parse().ok())?
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_and_command() {
        let paths = Paths::new("/srl");
        assert_eq!(
            jar(&paths),
            Path::new("/srl/tools/ninjabrain-bot/1.5.2/Ninjabrain-Bot-1.5.2.jar")
        );
        let cmd = command(Path::new("/srl/java/delta/bin/java"), &jar(&paths));
        assert_eq!(cmd.program, Path::new("/srl/java/delta/bin/java"));
        assert_eq!(
            cmd.args,
            ["-jar".to_string(), jar(&paths).display().to_string()]
        );
        assert_eq!(cmd.cwd, Path::new("/srl/tools/ninjabrain-bot/1.5.2"));
        assert_eq!(download().hash, Hash::Sha256(SHA256.into()));
        assert!(URL.ends_with(JAR) && URL.contains(&format!("/download/{VERSION}/")));
    }

    #[test]
    fn finds_any_ninjabrain_jar() {
        assert!(is_ninjabrain_cmdline(&[
            "java",
            "-jar",
            "/home/a/Downloads/Ninjabrain-Bot-1.4.3.jar"
        ]));
        assert!(is_ninjabrain_cmdline(&[
            "javaw.exe",
            "-jar",
            r"C:\mc\ninjabrain-bot.jar"
        ]));
        assert!(!is_ninjabrain_cmdline(&[
            "java",
            "-jar",
            "Jingle-2.0.0.jar"
        ]));
        assert!(!is_ninjabrain_cmdline(&["vim", "ninjabrain.txt"]));
    }

    #[test]
    fn status_from_disk() {
        let root = std::env::temp_dir().join(format!("srl-tools-nb-{}", std::process::id()));
        let paths = Paths::new(&root);
        assert_eq!(status(&paths), ToolStatus::NotInstalled);
        let old = dir(&paths).join("1.5.1");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("Ninjabrain-Bot-1.5.1.jar"), b"x").unwrap();
        assert_eq!(
            status(&paths),
            ToolStatus::UpdateAvailable {
                installed: "1.5.1".into(),
                latest: "1.5.2".into()
            }
        );
        // A truncated pinned jar doesn't count as installed.
        std::fs::create_dir_all(jar(&paths).parent().unwrap()).unwrap();
        std::fs::write(jar(&paths), b"short").unwrap();
        assert!(matches!(status(&paths), ToolStatus::UpdateAvailable { .. }));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
