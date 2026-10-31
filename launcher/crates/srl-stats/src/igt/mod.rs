//! SpeedRunIGT run records: where the mod writes them, parsing, attributing them to our modes and
//! turning them into a `RunHistory` per mode.
//!
//! # Where SpeedRunIGT 16.0+1.16.1 writes (read from its source, tag `16.0`)
//!
//! - **Global records:** `<user.home>/speedrunigt/records/<timer uuid>.json`, one file per run
//!   (the timer's random UUID), rewritten while the run goes on: when the player leaves the
//!   world, when the run completes, and when the next world starts. `user.home` is the OS user's
//!   home (`C:\Users\<name>` on Windows). The folder is shared by every instance of every
//!   launcher on the computer. The option "generate record" (default `EVERYTHING`) can limit it to
//!   completed runs or turn it off.
//! - **Per world:** `<game dir>/saves/<world>/speedrunigt/record.json`, the same JSON, rewritten on
//!   every pause too. Next to it SpeedRunIGT keeps `data/timer.igt` (its encrypted timer state),
//!   `logs/` and `events.log`; we read none of them.
//! - **`<user.home>/speedrunigt/latest_world.json`:** one line, rewritten whenever the current
//!   world logs an event: `{"world_path": "<game dir>/saves/<world>", "mods": [...], "version":
//!   "1.16.1", "mod_version": "16.0+1.16.1", "category": "ANY"}`.
//! - SpeedRunIGT after 16.0 (unreleased on 2026-10-26, commit b5d7fe9) moves the global folder to
//!   `$XDG_DATA_HOME/speedrunigt` or `~/.local/share/speedrunigt` on Linux and
//!   `%APPDATA%\speedrunigt` on Windows. Both places are read, so an update needs no change here.
//! - MCSR Ranked 6.0.4 bundles SpeedRunIGT 16.0 and cancels `writeRecordFile` during online
//!   matches (its `InGameTimerPrevent` mixin), so Ranked matches leave no records: the Ranked API
//!   is their source. Records in `instances/ranked` are the player's offline practice.
//!
//! # Attribution: which mode a record belongs to
//!
//! A record carries no game directory, only the world's name, and world names repeat across
//! instances (every Atum instance counts its own "Random Speedrun #N"). The rule, in order:
//!
//! 1. A world record (`instances/<mode>/saves/<world>/speedrunigt/record.json`) belongs to that
//!    mode. This is the only certain source.
//! 2. A global record that is the same write as a world record (same date, world name and times:
//!    the mod writes both from one string) is that run, so it gets that mode.
//! 3. Else, a global record whose world name matches exactly one of our world records (one not
//!    taken by rule 2), and which that world record continues (same category and run type, its
//!    timeline starts with the global one's, no time or date went down), is an older write of that
//!    run. This happens when the game stops before the player leaves the world: the world copy is
//!    rewritten on every pause, the global one only on leave. Because Atum names repeat across
//!    launchers, rule 3 also needs evidence: a non-empty shared timeline, or a global written
//!    after the world folder was created (an empty reset from before it is someone else's). And
//!    each world takes at most one global: if several qualify, none is merged (unless exactly one
//!    of them was already given this world's mode by an earlier scan, when it was the only one).
//! 4. Else, a global record we attributed in an earlier scan keeps its mode, so runs stay counted
//!    after the player deletes their worlds, provided the launcher scanned while the worlds
//!    existed (the app should scan when a game exits). The memory is
//!    `Paths::meta()/stats/attribution.json`, apart from the scan index so that an index format
//!    change can't erase it; see `scan`.
//! 5. Everything else is unattributed (`mode: None`): other launchers, or worlds deleted before
//!    we saw them. We never guess from the run type, the mods or the world name: "Random Speedrun
//!    #3" is the same in RSG and Ranked practice, and every 1.16.1 mode has the same base mods.
//!
//! `latest_world.json` is read for "the last world played" only. Its `world_path` would tie the
//! newest record to an instance, but that world is on disk anyway (rules 1 to 3), and once it is
//! deleted the file says nothing about which record was its run.
//!
//! # Speed
//!
//! Scanning stats every record and world folder and parses only the files whose size or mtime
//! changed since the last scan; the parsed results live in `Paths::meta()/stats/igt-index.json`.
//! See `scan`.
pub mod history;
pub mod record;
pub mod scan;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use srl_core::{Mode, Paths, shared::WorldModes};

pub use history::{
    BestSegment, CategoryKey, CategoryStats, Run, RunHistory, Segment, WorldRef, route_points,
    segment_times,
};
pub use record::{Record, RunType, Split, SplitKind};
pub use scan::{LocalStats, Skipped, scan};

/// Where to look. `IgtSources::for_user` is what the app uses; tests and the real-file check
/// point it at their own folders.
#[derive(Clone, Debug)]
pub struct IgtSources {
    /// Each `.../speedrunigt` global folder (holding `records/` and `latest_world.json`). Missing
    /// folders are fine.
    pub global_dirs: Vec<PathBuf>,
}

impl IgtSources {
    /// `~/speedrunigt` (16.0) and the OS data folder (`~/.local/share/speedrunigt` or
    /// `%APPDATA%\speedrunigt`, after 16.0).
    pub fn for_user() -> Self {
        let mut global_dirs = Vec::new();
        if let Some(base) = directories::BaseDirs::new() {
            global_dirs.push(base.home_dir().join("speedrunigt"));
            let data = base.data_dir().join("speedrunigt");
            if !global_dirs.contains(&data) {
                global_dirs.push(data);
            }
        }
        Self { global_dirs }
    }

    /// Only these folders (each a `speedrunigt` global folder).
    pub fn with_global_dirs(dirs: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            global_dirs: dirs.into_iter().collect(),
        }
    }
}

/// `latest_world.json`: the world SpeedRunIGT logged an event in last.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatestWorld {
    pub world_path: PathBuf,
    /// The mode whose instance holds the world, if any.
    pub mode: Option<Mode>,
    pub category: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub mods: Vec<String>,
}

/// The mode whose game directory's `saves/` holds `world_path`, and the world's folder name. A
/// world of the shared 1.16.1 directory belongs to the mode that created it
/// (`srl_core::shared::WorldModes`). Paths from SpeedRunIGT use `/` even on Windows, and may
/// differ from ours only in case there, so the comparison is component-wise and
/// case-insensitive on Windows.
pub fn mode_of_world(paths: &Paths, world_path: &Path) -> Option<(Mode, String)> {
    let world = normalize(world_path);
    game_dirs().into_iter().find_map(|dir_mode| {
        let saves = normalize(&paths.instance(dir_mode).join("saves"));
        let rest = world.strip_prefix(&saves)?;
        let rest = rest.strip_prefix('/')?;
        (!rest.is_empty() && !rest.contains('/')).then(|| {
            let name = world_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| rest.to_string());
            let mode = if dir_mode.is_shared() {
                WorldModes::load(paths)
                    .mode_of(&paths.instance(dir_mode).join("saves").join(&name), None)
            } else {
                dir_mode
            };
            (mode, name)
        })
    })
}

/// One mode per game directory: RSG stands for the shared one.
pub(crate) fn game_dirs() -> [Mode; 2] {
    [Mode::Rsg, Mode::Draftout]
}

fn normalize(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = s.trim_end_matches('/').to_string();
    if cfg!(windows) { s.to_lowercase() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_paths_map_to_modes() {
        let paths = Paths::new("/srl");
        assert_eq!(
            mode_of_world(
                &paths,
                Path::new("/srl/instances/mcsr/saves/FSG Speedrun #3")
            ),
            Some((Mode::Rsg, "FSG Speedrun #3".into()))
        );
        assert_eq!(
            mode_of_world(&paths, Path::new("/srl/instances/mcsr/saves/W/")),
            Some((Mode::Rsg, "W".into()))
        );
        assert_eq!(
            mode_of_world(&paths, Path::new("/srl/instances/mcsr/saves")),
            None
        );
        assert_eq!(
            mode_of_world(&paths, Path::new("/srl/instances/mcsr/saves/a/b")),
            None
        );
        assert_eq!(
            mode_of_world(&paths, Path::new("/other/instances/rsg/saves/a")),
            None
        );
        assert_eq!(
            mode_of_world(&paths, Path::new("/srl/instances/mcsrx/saves/a")),
            None
        );
    }
}
