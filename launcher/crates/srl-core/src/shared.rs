//! The shared 1.16.1 game directory, `instances/mcsr`.
//!
//! Ranked, RSG, FSG and SSG play in one directory, the way a player keeps one MultiMC instance
//! and turns mods on and off: the same options, keys, configs, resource packs and worlds. A mode
//! is the set of mods Play puts into `mods/` (srl-mods) plus the few files that must differ per
//! mode, kept here:
//!
//! - **Atum's settings** (`PER_MODE_FILES`): an SSG seed typed into Atum must never carry over
//!   into an RSG run. On a switch the outgoing mode's copy goes to `.srl-modes/<mode>/` and the
//!   incoming mode's comes back (or none: Atum starts from its defaults).
//! - **Which mode played when**: Stats count a world for the mode that created it. Worlds from
//!   before the shared directory are listed by name; newer ones go to the mode whose game was
//!   started last before the world was created.
//!
//! Everything lives in `instances/mcsr/.srl-modes/` (`state.json` and one folder per mode), which
//! the game ignores.
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{Mode, Paths};

/// The shared directory's name under `instances/`.
pub const DIR: &str = "mcsr";

/// The launcher's own folder in the shared directory.
pub const STATE_DIR: &str = ".srl-modes";

/// Files each mode keeps for itself, relative to the game directory: Atum's settings, which hold
/// the seed and the world type (SpeedrunAPI versions, then the older Atum properties).
pub const PER_MODE_FILES: [&str; 2] = ["config/mcsr/atum.json", "config/atum/atum.properties"];

/// Sessions kept for Stats; older ones only matter for worlds that are long deleted.
const MAX_SESSIONS: usize = 5000;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct State {
    /// The mode whose per-mode files are in place now.
    #[serde(default)]
    current: Option<Mode>,
    /// Game starts, oldest first.
    #[serde(default)]
    sessions: Vec<Session>,
    /// Worlds moved in from a mode's own directory, by folder name.
    #[serde(default)]
    worlds: BTreeMap<String, Mode>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Session {
    mode: Mode,
    /// Milliseconds since the Unix epoch.
    started_ms: i64,
}

fn shared_dir(paths: &Paths) -> PathBuf {
    paths.instances().join(DIR)
}

fn state_file(paths: &Paths) -> PathBuf {
    shared_dir(paths).join(STATE_DIR).join("state.json")
}

fn mode_files(paths: &Paths, mode: Mode) -> PathBuf {
    shared_dir(paths).join(STATE_DIR).join(mode.id())
}

fn load(paths: &Paths) -> State {
    fs::read(state_file(paths))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(paths: &Paths, state: &State) -> Result<()> {
    let file = state_file(paths);
    let dir = file.parent().expect("state file has a parent");
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let tmp = file.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(state)?)
        .with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, &file).with_context(|| format!("writing {}", file.display()))
}

/// The mode whose per-mode files are in place, if the launcher knows.
pub fn current(paths: &Paths) -> Option<Mode> {
    load(paths).current
}

/// Puts `mode`'s per-mode files into the shared directory, the previous mode's aside. Call it
/// before the mode's game starts (never while a game of the directory runs).
pub fn switch_to(paths: &Paths, mode: Mode) -> Result<()> {
    let mut state = load(paths);
    if state.current == Some(mode) {
        return Ok(());
    }
    let game = shared_dir(paths);
    // With no known previous mode the files in place are the ones the player has always used:
    // they become this mode's.
    if let Some(prev) = state.current {
        let aside = mode_files(paths, prev);
        for rel in PER_MODE_FILES {
            move_file(&game.join(rel), &aside.join(rel))?;
        }
        let own = mode_files(paths, mode);
        for rel in PER_MODE_FILES {
            move_file(&own.join(rel), &game.join(rel))?;
        }
    }
    state.current = Some(mode);
    save(paths, &state)
}

/// Moves `from` to `to` (replacing it) if `from` exists, else removes `to`'s stale copy: after
/// this `to` is exactly what `from` was.
fn move_file(from: &Path, to: &Path) -> Result<()> {
    match fs::remove_file(to) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!("removing {}", to.display())),
    }
    if !from.is_file() {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    fs::rename(from, to).with_context(|| format!("moving {} to {}", from.display(), to.display()))
}

/// Notes that a game of `mode` started now, for [`WorldModes`].
pub fn record_session(paths: &Paths, mode: Mode) -> Result<()> {
    record_session_at(paths, mode, now_ms())
}

fn record_session_at(paths: &Paths, mode: Mode, started_ms: i64) -> Result<()> {
    let mut state = load(paths);
    state.sessions.push(Session { mode, started_ms });
    if state.sessions.len() > MAX_SESSIONS {
        let cut = state.sessions.len() - MAX_SESSIONS;
        state.sessions.drain(..cut);
    }
    save(paths, &state)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Which mode a world of the shared directory belongs to. Loaded once per Stats scan.
#[derive(Clone, Debug, Default)]
pub struct WorldModes {
    state: State,
}

impl WorldModes {
    pub fn load(paths: &Paths) -> Self {
        Self { state: load(paths) }
    }

    /// The mode of the world in `world_dir` (a folder in `instances/mcsr/saves`), created at
    /// `created_ms` if known (else the folder's own time is read).
    pub fn mode_of(&self, world_dir: &Path, created_ms: Option<i64>) -> Mode {
        let name = world_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Some(mode) = self.state.worlds.get(&name) {
            return *mode;
        }
        let created = created_ms.or_else(|| folder_time_ms(world_dir));
        created
            .and_then(|at| {
                self.state
                    .sessions
                    .iter()
                    .rev()
                    .find(|s| s.started_ms <= at)
                    .map(|s| s.mode)
            })
            .or(self.state.current)
            .unwrap_or(Mode::Rsg)
    }
}

/// When the folder was created (or, where the file system can't say, last changed).
pub fn folder_time_ms(dir: &Path) -> Option<i64> {
    let meta = fs::metadata(dir).ok()?;
    let at = meta.created().or_else(|_| meta.modified()).ok()?;
    Some(at.duration_since(UNIX_EPOCH).ok()?.as_millis() as i64)
}

/// What [`migrate`] did, for the launcher log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Migration {
    /// The mode whose directory became `instances/mcsr`.
    pub base: Option<Mode>,
    /// Worlds moved into the shared `saves/`, per mode.
    pub moved_worlds: Vec<(Mode, usize)>,
    /// Old directories kept as `instances/old-<mode>`.
    pub kept: Vec<PathBuf>,
}

impl Migration {
    pub fn is_empty(&self) -> bool {
        self.base.is_none() && self.moved_worlds.is_empty() && self.kept.is_empty()
    }
}

/// Moves the per-mode directories of earlier versions (`instances/ranked`, `rsg`, `fsg`, `ssg`)
/// into the shared one. The most recently played becomes `instances/mcsr` as it is (its options,
/// configs and resource packs are the player's newest); the others give their worlds (renamed on
/// a clash) and Atum settings, and the rest of each stays as `instances/old-<mode>`, nothing
/// deleted. Safe to call on every start: a step that failed (a file held open on Windows) is
/// tried again next time. Call it before anything else touches the instances.
pub fn migrate(paths: &Paths) -> Result<Migration> {
    let instances = paths.instances();
    let shared = shared_dir(paths);
    let olds: Vec<Mode> = Mode::SHARED
        .into_iter()
        .filter(|m| instances.join(m.id()).is_dir())
        .collect();
    let mut done = Migration::default();
    if olds.is_empty() {
        return Ok(done);
    }
    let mut state = load(paths);
    let mut rest = olds.clone();
    if !shared.exists() {
        let base = *olds
            .iter()
            .max_by_key(|m| last_played(&instances.join(m.id())))
            .expect("not empty");
        fs::rename(instances.join(base.id()), &shared)
            .with_context(|| format!("moving instances/{} to instances/{DIR}", base.id()))?;
        for name in world_names(&shared.join("saves")) {
            state.worlds.insert(name, base);
        }
        state.current = Some(base);
        save(paths, &state)?;
        done.base = Some(base);
        rest.retain(|m| *m != base);
    }
    for mode in rest {
        let old = instances.join(mode.id());
        let saves = shared.join("saves");
        let mut moved = 0;
        for name in world_names(&old.join("saves")) {
            fs::create_dir_all(&saves).with_context(|| format!("creating {}", saves.display()))?;
            let to = free_name(&saves, &name, mode);
            let to_name = to.file_name().unwrap().to_string_lossy().into_owned();
            fs::rename(old.join("saves").join(&name), &to)
                .with_context(|| format!("moving {name} into {}", saves.display()))?;
            state.worlds.insert(to_name, mode);
            moved += 1;
        }
        let own = mode_files(paths, mode);
        for rel in PER_MODE_FILES {
            let (from, to) = (old.join(rel), own.join(rel));
            if from.is_file() && !to.exists() {
                fs::create_dir_all(to.parent().unwrap())?;
                fs::copy(&from, &to)
                    .with_context(|| format!("copying {} to {}", from.display(), to.display()))?;
            }
        }
        save(paths, &state)?;
        let keep = free_dir(&instances, &format!("old-{}", mode.id()));
        fs::rename(&old, &keep)
            .with_context(|| format!("moving instances/{} to {}", mode.id(), keep.display()))?;
        if moved > 0 {
            done.moved_worlds.push((mode, moved));
        }
        done.kept.push(keep);
    }
    Ok(done)
}

/// The newest of the times a mode's directory was played in: its options.txt (the game writes it
/// on every start) and its saves folder.
fn last_played(dir: &Path) -> std::time::SystemTime {
    ["options.txt", "saves"]
        .iter()
        .filter_map(|f| fs::metadata(dir.join(f)).and_then(|m| m.modified()).ok())
        .max()
        .unwrap_or(UNIX_EPOCH)
}

/// The world folders in `saves` (no symlinks, no files).
fn world_names(saves: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(saves) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// `saves/<name>`, or `<name> (<MODE>)`, `<name> (<MODE> 2)`… if that is taken.
fn free_name(saves: &Path, name: &str, mode: Mode) -> PathBuf {
    let plain = saves.join(name);
    if !taken(&plain) {
        return plain;
    }
    let tag = mode.id().to_uppercase();
    (1..)
        .map(|n| match n {
            1 => saves.join(format!("{name} ({tag})")),
            n => saves.join(format!("{name} ({tag} {n})")),
        })
        .find(|p| !taken(p))
        .expect("an unused name")
}

fn free_dir(parent: &Path, name: &str) -> PathBuf {
    (1..)
        .map(|n| match n {
            1 => parent.join(name),
            n => parent.join(format!("{name}-{n}")),
        })
        .find(|p| !taken(p))
        .expect("an unused name")
}

/// Whether something (even a dangling link) has that name. Windows file names are
/// case-insensitive, so there a name differing only in case counts as taken too.
fn taken(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("srl-shared-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn atum_settings_stay_with_their_mode() {
        let root = temp("switch");
        let paths = Paths::new(&root);
        let game = paths.instance(Mode::Rsg);
        let atum = game.join(PER_MODE_FILES[0]);
        write(&atum, r#"{"seed":""}"#);
        // The first mode keeps what is there.
        switch_to(&paths, Mode::Rsg).unwrap();
        assert_eq!(fs::read_to_string(&atum).unwrap(), r#"{"seed":""}"#);
        // SSG starts from Atum's defaults, and its seed never reaches RSG.
        switch_to(&paths, Mode::Ssg).unwrap();
        assert!(!atum.exists());
        write(&atum, r#"{"seed":"2483313382402348964"}"#);
        switch_to(&paths, Mode::Rsg).unwrap();
        assert_eq!(fs::read_to_string(&atum).unwrap(), r#"{"seed":""}"#);
        switch_to(&paths, Mode::Ssg).unwrap();
        assert_eq!(
            fs::read_to_string(&atum).unwrap(),
            r#"{"seed":"2483313382402348964"}"#
        );
        switch_to(&paths, Mode::Ssg).unwrap();
        assert_eq!(current(&paths), Some(Mode::Ssg));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn worlds_go_to_the_mode_that_made_them() {
        let root = temp("worlds");
        let paths = Paths::new(&root);
        record_session_at(&paths, Mode::Rsg, 1_000).unwrap();
        record_session_at(&paths, Mode::Ssg, 5_000).unwrap();
        let w = WorldModes::load(&paths);
        let saves = paths.instance(Mode::Rsg).join("saves");
        assert_eq!(w.mode_of(&saves.join("a"), Some(2_000)), Mode::Rsg);
        assert_eq!(w.mode_of(&saves.join("b"), Some(5_001)), Mode::Ssg);
        // Older than every session: the fallback.
        assert_eq!(w.mode_of(&saves.join("c"), Some(10)), Mode::Rsg);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn old_mode_directories_become_one() {
        let root = temp("migrate");
        let paths = Paths::new(&root);
        let inst = paths.instances();
        write(&inst.join("rsg/options.txt"), "fov:0.0\n");
        write(&inst.join("rsg/saves/Random Speedrun #1/level.dat"), "r");
        write(&inst.join("ssg/saves/Random Speedrun #1/level.dat"), "s");
        write(&inst.join("ssg/saves/Set Speedrun #4/level.dat"), "s");
        write(&inst.join("ssg/config/mcsr/atum.json"), r#"{"seed":"1"}"#);
        write(&inst.join("ranked/options.txt"), "fov:1.0\n");
        // RSG was played last (files only: Windows opens no directory to set its time).
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
        fs::File::options()
            .write(true)
            .open(inst.join("rsg/options.txt"))
            .unwrap()
            .set_modified(later)
            .unwrap();
        let m = migrate(&paths).unwrap();
        assert_eq!(m.base, Some(Mode::Rsg));
        assert_eq!(m.moved_worlds, vec![(Mode::Ssg, 2)]);
        let shared = inst.join("mcsr");
        assert_eq!(
            fs::read_to_string(shared.join("options.txt")).unwrap(),
            "fov:0.0\n"
        );
        assert!(
            shared
                .join("saves/Random Speedrun #1 (SSG)/level.dat")
                .is_file()
        );
        assert!(inst.join("old-ranked/options.txt").is_file());
        assert!(inst.join("old-ssg").is_dir());
        assert!(!inst.join("rsg").exists() && !inst.join("ssg").exists());
        let w = WorldModes::load(&paths);
        let saves = shared.join("saves");
        assert_eq!(
            w.mode_of(&saves.join("Random Speedrun #1"), None),
            Mode::Rsg
        );
        assert_eq!(w.mode_of(&saves.join("Set Speedrun #4"), None), Mode::Ssg);
        assert_eq!(current(&paths), Some(Mode::Rsg));
        // SSG's seed comes back when SSG plays.
        switch_to(&paths, Mode::Ssg).unwrap();
        assert_eq!(
            fs::read_to_string(shared.join(PER_MODE_FILES[0])).unwrap(),
            r#"{"seed":"1"}"#
        );
        // Nothing left to do.
        assert!(migrate(&paths).unwrap().is_empty());
        fs::remove_dir_all(&root).unwrap();
    }
}
