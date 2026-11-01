//! Where everything lives on disk. One root, shared by all modes:
//!
//! ```text
//! <root>/
//!   meta/                    cached manifests (Mojang, Fabric, mcsr-meta), refreshed on start
//!   versions/<id>/<id>.json  version JSON (vanilla and Fabric profiles)
//!   versions/<id>/<id>.jar   client jar
//!   libraries/               Maven layout, shared by path (sha1-verified)
//!   assets/indexes, objects/ shared by path (sha1-verified)
//!   natives/<version>/       extracted natives for versions that ship them in jars
//!   java/<component>-<version>/  Java runtimes, one directory per version
//!   store/sha512/<ab>/<hex>  content-addressed mod jars, materialised into instances
//!   instances/mcsr/          the game directory of Ranked, RSG, FSG and SSG (mods/, config/, saves/,
//!                            options.txt; mods-removed/ holds jars the launcher took out of mods/;
//!                            .srl-modes/ the per-mode files and which mode played when, `shared`)
//!   instances/draftout/      Draftout's game directory (26.3)
//!   instances/old-<mode>/    a mode's own directory from before the shared one (`shared::migrate`)
//!   tools/<tool>/<version>/  speedrun tools the launcher installs (Ninjabrain Bot, Toolscreen)
//!   instances/practice/      the practice instance: practice maps in saves/ (srl-practice)
//!   instances/custom-<n>/    a custom instance the player created (its definition is in
//!                            settings.json)
//!   trash/                   deleted custom instances, `custom-<n>-<unix time>/`, until the
//!                            player removes them
//!   logs/                    launcher log and the last game log of each mode
//! ```
//!
//! The root is `$SRL_HOME` if set (tests and agents always set it), else the OS's local data
//! directory: `%LOCALAPPDATA%\SpeedrunningLauncher\data` on Windows (not Roaming: the game, Java and
//! the instances are gigabytes that must not follow the player through a roaming profile),
//! `~/.local/share/speedrunninglauncher` on Linux. Older places are moved there on the first start
//! (`adopt_old_data`): the Roaming folder the launcher used until 0.2.3, and the `Speedrunluncher`
//! folders from before the rename.
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::Mode;

#[derive(Clone, Debug)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `$SRL_HOME`, else the OS's local data directory.
    pub fn from_env() -> Result<Self> {
        if let Some(dir) = std::env::var_os("SRL_HOME").filter(|v| !v.is_empty()) {
            return Ok(Self::new(dir));
        }
        let dirs = directories::ProjectDirs::from("", "", "SpeedrunningLauncher")
            .context("no home directory to put the launcher data in")?;
        let old = directories::ProjectDirs::from("", "", "Speedrunluncher");
        // Newest first: Roaming under the current name, then the pre-rename folder. Only Windows
        // tells the roaming and the local data directory apart; elsewhere they are one folder.
        let olds: Vec<&Path> = [Some(dirs.data_dir()), old.as_ref().map(|d| d.data_dir())]
            .into_iter()
            .flatten()
            .collect();
        Ok(Self::new(adopt_old_data(&olds, dirs.data_local_dir())))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn meta(&self) -> PathBuf {
        self.root.join("meta")
    }
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions().join(id)
    }
    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn natives(&self, version: &str) -> PathBuf {
        self.root.join("natives").join(version)
    }
    pub fn java(&self) -> PathBuf {
        self.root.join("java")
    }
    /// Content-addressed file for a sha512 (lower-case hex).
    pub fn store_sha512(&self, sha512: &str) -> PathBuf {
        let prefix = sha512.get(..2).unwrap_or("00");
        self.root
            .join("store")
            .join("sha512")
            .join(prefix)
            .join(sha512)
    }
    /// The game directory of a mode, passed to the game as `--gameDir`: `instances/mcsr` for the
    /// four 1.16.1 modes, `instances/draftout` for Draftout.
    pub fn instance(&self, mode: Mode) -> PathBuf {
        self.game_dir(crate::Instance::Mode(mode))
    }
    /// The game directory of any instance, passed to the game as `--gameDir`: `instances/mcsr`
    /// for the four 1.16.1 modes, else `instances/<id>` (`draftout`, `practice`, `custom-<n>`).
    /// The one place that maps an instance to its directory.
    pub fn game_dir(&self, instance: impl Into<crate::Instance>) -> PathBuf {
        self.instances().join(&*instance.into().dir_id())
    }
    /// `instances/`.
    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }
    /// Speedrun tools (srl-tools): `tools/<tool id>/<version>/`.
    pub fn tools(&self) -> PathBuf {
        self.root.join("tools")
    }
    /// The practice instance's game directory (`instances/practice`), apart from every mode so
    /// that practice maps and practice-only mods never end up in a run setup (research/04 §3).
    /// `practice` is not a `Mode` id, so it can't collide with one.
    pub fn practice_instance(&self) -> PathBuf {
        self.game_dir(crate::Instance::Practice)
    }
    /// The same as [`Paths::game_dir`], under its older name.
    pub fn instance_dir(&self, instance: impl Into<crate::Instance>) -> PathBuf {
        self.game_dir(instance)
    }
    /// Where a deleted custom instance's directory goes (`trash/`), so a mistake can be undone
    /// by moving it back by hand. On the same file system as `instances/`, so the move is a
    /// rename.
    pub fn trash(&self) -> PathBuf {
        self.root.join("trash")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
}

/// Until 0.2.3 the data was in the roaming data directory (`%APPDATA%` on Windows), and until
/// 2026-10-26 the launcher was called Speedrunluncher. If `new` doesn't exist yet, move the first
/// of `olds` that does to it, so nobody downloads Java and the game again. When the move fails (a
/// game from the old folder still holds its files open on Windows, or Roaming is redirected to
/// another drive, where a folder can't be renamed), keep using the old folder for now; the next
/// start tries again.
fn adopt_old_data(olds: &[&Path], new: &Path) -> PathBuf {
    if new.exists() {
        return new.to_path_buf();
    }
    let Some(old) = olds.iter().find(|old| **old != new && old.is_dir()) else {
        return new.to_path_buf();
    };
    if let Some(parent) = new.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::rename(old, new) {
        Ok(()) => {
            // On Windows the data sits in `<name>\data`; drop `<name>` if it is now empty. Never a
            // folder without that `data` level (`~/.local/share` on Linux).
            if let Some(parent) = old
                .parent()
                .filter(|p| old.file_name() == Some("data".as_ref()) && Some(*p) != new.parent())
            {
                let _ = std::fs::remove_dir(parent);
            }
            new.to_path_buf()
        }
        Err(_) => old.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout() {
        let p = Paths::new("/srl");
        assert_eq!(p.instance(Mode::Rsg), Path::new("/srl/instances/mcsr"));
        assert_eq!(p.instance(Mode::Ssg), p.instance(Mode::Ranked));
        assert_eq!(
            p.instance(Mode::Draftout),
            Path::new("/srl/instances/draftout")
        );
        assert_eq!(
            p.store_sha512("abcdef"),
            Path::new("/srl/store/sha512/ab/abcdef")
        );
        assert_eq!(p.version_dir("1.16.1"), Path::new("/srl/versions/1.16.1"));
        assert_eq!(p.practice_instance(), Path::new("/srl/instances/practice"));
        assert_eq!(
            p.instance_dir(crate::Instance::Practice),
            p.practice_instance()
        );
        assert_eq!(p.instance_dir(Mode::Rsg), p.instance(Mode::Rsg));
        assert_eq!(
            p.game_dir(crate::Instance::Custom(crate::CustomId(2))),
            Path::new("/srl/instances/custom-2")
        );
    }

    /// An empty folder under the system's temp directory, one per test.
    fn temp(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("srl-paths-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    /// Windows' three places: Roaming under the current name, Roaming under the old name, Local.
    fn windows_places(base: &Path) -> (PathBuf, PathBuf, PathBuf) {
        (
            base.join("Roaming/SpeedrunningLauncher/data"),
            base.join("Roaming/Speedrunluncher/data"),
            base.join("Local/SpeedrunningLauncher/data"),
        )
    }

    #[test]
    fn roaming_data_moves_to_local_once() {
        let base = temp("roaming");
        let (roaming, renamed, local) = windows_places(&base);
        std::fs::create_dir_all(roaming.join("java")).unwrap();
        std::fs::create_dir_all(&renamed).unwrap();
        assert_eq!(adopt_old_data(&[&roaming, &renamed], &local), local);
        assert!(local.join("java").is_dir());
        assert!(!base.join("Roaming/SpeedrunningLauncher").exists());
        // The new place exists now, so the pre-rename folder stays where it is.
        assert!(renamed.is_dir());
        // A Roaming folder again (an older launcher started once more): Local is kept as is.
        std::fs::create_dir_all(&roaming).unwrap();
        assert_eq!(adopt_old_data(&[&roaming, &renamed], &local), local);
        assert!(roaming.is_dir());
        assert!(local.join("java").is_dir());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn old_name_moves_without_a_roaming_folder() {
        let base = temp("rename");
        let (roaming, renamed, local) = windows_places(&base);
        std::fs::create_dir_all(renamed.join("java")).unwrap();
        assert_eq!(adopt_old_data(&[&roaming, &renamed], &local), local);
        assert!(local.join("java").is_dir());
        assert!(!base.join("Roaming/Speedrunluncher").exists());
        // Nothing old any more: the new folder is used as is.
        assert_eq!(adopt_old_data(&[&roaming, &renamed], &local), local);
        assert_eq!(adopt_old_data(&[], &local), local);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn same_place_stays_put() {
        // Linux and macOS: the roaming and the local data directory are one folder.
        let base = temp("same");
        let data = base.join("share/speedrunninglauncher");
        assert_eq!(adopt_old_data(&[&data], &data), data);
        assert!(!data.exists());
        std::fs::create_dir_all(data.join("java")).unwrap();
        assert_eq!(adopt_old_data(&[&data], &data), data);
        assert!(data.join("java").is_dir());
        // A folder without the `data` level keeps its parent even when the move empties it.
        std::fs::remove_dir_all(&data).unwrap();
        let old = base.join("old/speedrunluncher");
        std::fs::create_dir_all(&old).unwrap();
        assert_eq!(adopt_old_data(&[&old], &data), data);
        assert!(base.join("old").is_dir());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn failed_move_keeps_the_old_folder() {
        let base = temp("fail");
        let (roaming, renamed, local) = windows_places(&base);
        std::fs::create_dir_all(roaming.join("java")).unwrap();
        // The new place can't be made (its parent is a file), so the rename fails like one
        // across drives or with a file held open.
        std::fs::write(base.join("Local"), b"").unwrap();
        assert_eq!(adopt_old_data(&[&roaming, &renamed], &local), roaming);
        assert!(roaming.join("java").is_dir());
        // The next start tries again and gets through once the way is free.
        std::fs::remove_file(base.join("Local")).unwrap();
        assert_eq!(adopt_old_data(&[&roaming, &renamed], &local), local);
        assert!(local.join("java").is_dir());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
