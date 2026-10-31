//! The world side of installing, resetting, updating and removing maps: synchronous file work,
//! run on a blocking thread by `Practice`. Nothing here downloads; the zip it gets is verified.
//!
//! Safety rules, in one place:
//! - a world is deleted or replaced only if an install record names it ([`crate::record`]);
//! - a new world never lands on an existing folder ([`crate::archive::unique_name`]);
//! - worlds are extracted into a staging folder first and moved into `saves/` whole, and
//!   deleted by moving them out of `saves/` first, so the game never lists a half-written world;
//! - a replaced world leaves `saves/` only after its successor is extracted and ready, and is
//!   deleted only after the record names the successor. Any failure before that puts it back, and
//!   so does the next call after a crash ([`recover_staging`]);
//! - a symlink in `saves/` is never followed.
use std::path::{Path, PathBuf};

use anyhow::Context;
use srl_core::Paths;

use crate::{
    CarriedOver, CarryOver, MapEntry, PracticeError, Progress, ProgressFn, Step, Warning,
    WorldState, WorldView,
    archive::{self, folder_name, unique_name},
    catalogue::DATA_VERSION_1_16_1,
    level,
    record::{self, WorldRecord},
};

/// Written into a swap folder once the record names the new worlds: from then on the old worlds
/// in it are garbage, not something to put back.
const COMMITTED: &str = ".committed";

/// The state of one recorded world, from its level.dat fingerprint.
pub fn world_state(paths: &Paths, w: &WorldRecord) -> WorldState {
    let dir = record::world_dir(paths, &w.folder);
    // Missing only when the folder is gone (or something that isn't a real folder took its name:
    // not ours either). A folder without a readable level.dat still holds the player's progress:
    // the game saves level.dat by renaming files, and a crash between the renames leaves only
    // level.dat_old, which the game then loads.
    if !std::fs::symlink_metadata(&dir).is_ok_and(|m| m.is_dir()) {
        return WorldState::Missing;
    }
    match srl_core::download::file_sha512(&dir.join("level.dat")) {
        Ok(h) if h == w.level_dat_sha512 => WorldState::Fresh,
        // Changed, gone, or unreadable (locked by the game on Windows): treat it as played.
        _ => WorldState::Played,
    }
}

pub fn view(paths: &Paths, w: &WorldRecord) -> WorldView {
    WorldView {
        folder: w.folder.clone(),
        name: w.level_name.clone().unwrap_or_else(|| w.folder.clone()),
        state: world_state(paths, w),
        data_version: w.data_version,
    }
}

/// What happens to the map's current worlds when new ones go in.
#[derive(Default)]
pub struct Swap<'a> {
    /// Recorded worlds the new ones replace (ReplaceWorlds, Reset).
    pub replace: &'a [String],
    /// Worlds to copy the carry-over files from. Read right before the swap, so the newest
    /// loadouts are the ones kept.
    pub carry_from: &'a [String],
    pub carry: &'a [CarryOver],
}

pub struct Placed {
    pub worlds: Vec<WorldRecord>,
    pub carried: Vec<CarriedOver>,
}

/// Extracts every world of `zip` and moves it into `saves/`, under names nothing else uses (the
/// first world of a zip with `level.dat` at its root is named after the map and version).
///
/// The order is what keeps the player's data safe: extract into staging and write the carried
/// files there; then `check` (the caller's last word, e.g. "has a world been played meanwhile?")
/// and the open-world check; then move the replaced worlds out, the new ones in, and `commit`
/// (which saves the record). If anything fails, the new worlds go and the old ones come back.
/// The old worlds are deleted only after `commit` succeeded.
#[allow(clippy::too_many_arguments)]
pub fn place_worlds(
    paths: &Paths,
    entry: &MapEntry,
    version: &str,
    zip: &Path,
    swap: Swap<'_>,
    progress: Option<&ProgressFn>,
    warnings: &mut Vec<Warning>,
    check: impl FnOnce() -> Result<(), PracticeError>,
    commit: impl FnOnce(&[WorldRecord]) -> Result<(), PracticeError>,
) -> Result<Placed, PracticeError> {
    let stage = record::staging(paths, &format!("new-{}", entry.id));
    clear(&stage)?;
    let result = place_from_stage(
        paths, entry, version, zip, &stage, swap, progress, warnings, check, commit,
    );
    let _ = std::fs::remove_dir_all(&stage);
    tidy_staging(paths);
    result
}

/// A world extracted into staging, ready to move in.
struct Staged {
    dir: PathBuf,
    zip_root: String,
    hash: String,
    info: Option<level::LevelInfo>,
    carried: Vec<CarryOver>,
}

#[allow(clippy::too_many_arguments)]
fn place_from_stage(
    paths: &Paths,
    entry: &MapEntry,
    version: &str,
    zip: &Path,
    stage: &Path,
    swap: Swap<'_>,
    progress: Option<&ProgressFn>,
    warnings: &mut Vec<Warning>,
    check: impl FnOnce() -> Result<(), PracticeError>,
    commit: impl FnOnce(&[WorldRecord]) -> Result<(), PracticeError>,
) -> Result<Placed, PracticeError> {
    let extracted = archive::extract_worlds(zip, stage, |done, total| {
        if let Some(p) = progress {
            p(Progress {
                step: Step::Extracting,
                done,
                total,
            });
        }
    })?;
    // The last checks, after the slow part and right before anything in saves/ changes.
    check()?;
    ensure_closed(paths, swap.replace)?;
    let files = read_carry_over(paths, swap.carry_from, swap.carry);
    let mut staged = Vec::new();
    for (dir, zw) in extracted.worlds {
        let carried = write_carry_over(&dir, &files)?;
        let level_dat = dir.join("level.dat");
        staged.push(Staged {
            hash: srl_core::download::file_sha512(&level_dat)?,
            info: level::read_level_dat_file(&level_dat).ok(),
            dir,
            zip_root: zw.root,
            carried,
        });
    }

    let saves = record::saves(paths);
    std::fs::create_dir_all(&saves)?;
    let swap_dir = record::staging(paths, &format!("swap-{}-{}", record::now(), entry.id));
    let mut moved_out: Vec<String> = Vec::new();
    let mut placed: Vec<WorldRecord> = Vec::new();
    let mut carried: Vec<CarriedOver> = Vec::new();
    let mut new_warnings: Vec<Warning> = Vec::new();
    let result = (|| -> Result<(), PracticeError> {
        for f in swap.replace {
            if !record::is_plain_folder(f) {
                return Err(anyhow::anyhow!("refusing to move {f:?}").into());
            }
            let dir = record::world_dir(paths, f);
            if !std::fs::symlink_metadata(&dir).is_ok_and(|m| m.is_dir()) {
                continue;
            }
            std::fs::create_dir_all(&swap_dir)?;
            std::fs::rename(&dir, swap_dir.join(f))
                .with_context(|| format!("saves/{f} is in use (is the game running?)"))?;
            moved_out.push(f.clone());
        }
        for s in staged {
            let wanted = match s.zip_root.rsplit('/').next() {
                Some(last) if !last.is_empty() => folder_name(last),
                _ => folder_name(&format!("{} v{version}", entry.name)),
            };
            let folder = unique_name(&wanted, |n| record::exists_no_follow(&saves.join(n)));
            if folder != wanted {
                new_warnings.push(Warning::Renamed {
                    wanted,
                    folder: folder.clone(),
                });
            }
            let data_version = s.info.as_ref().and_then(|i| i.data_version);
            match data_version {
                Some(dv) if dv > DATA_VERSION_1_16_1 => new_warnings.push(Warning::NewerWorld {
                    folder: folder.clone(),
                    data_version: dv,
                }),
                None => new_warnings.push(Warning::NoDataVersion {
                    folder: folder.clone(),
                }),
                _ => {}
            }
            std::fs::rename(&s.dir, saves.join(&folder))
                .with_context(|| format!("moving the world into saves/{folder}"))?;
            carried.extend(s.carried.into_iter().map(|c| CarriedOver {
                folder: folder.clone(),
                what: c.what,
                then_run: c.then_run,
            }));
            placed.push(WorldRecord {
                folder,
                zip_root: s.zip_root,
                level_dat_sha512: s.hash,
                level_name: s.info.and_then(|i| i.display_name()),
                data_version,
            });
        }
        commit(&placed)
    })();

    if let Err(e) = result {
        // Undo: the new worlds out (they are ours and fresh), the old ones back where they were.
        for (i, w) in placed.iter().enumerate() {
            let gone = stage.join(format!("undo-{i}"));
            if std::fs::rename(saves.join(&w.folder), &gone).is_ok() {
                let _ = std::fs::remove_dir_all(&gone);
            }
        }
        // What can't be put back stays in the swap folder, and the next call tries again.
        for f in &moved_out {
            let _ = std::fs::rename(swap_dir.join(f), record::world_dir(paths, f));
        }
        let _ = std::fs::remove_dir(&swap_dir);
        return Err(e);
    }
    warnings.append(&mut new_warnings);
    if !moved_out.is_empty() {
        // The record names the new worlds now. Mark the swap done before deleting, so a crash
        // during the delete doesn't bring half of an old world back.
        let _ = std::fs::write(swap_dir.join(COMMITTED), b"");
        let _ = std::fs::remove_dir_all(&swap_dir);
    }
    Ok(Placed {
        worlds: placed,
        carried,
    })
}

/// True if a running game has the world open. Minecraft 1.16.1 locks the whole of
/// `session.lock` (Java `FileChannel.tryLock`) for as long as the world is loaded: a POSIX
/// record lock on Linux and macOS, `LockFileEx` on Windows. Checked without taking the lock,
/// so the check can't get in the game's way.
pub fn world_in_use(dir: &Path) -> bool {
    let Ok(file) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.join("session.lock"))
    else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: `flock` is plain data, all-zero is a valid value, and F_GETLK only writes into
        // the struct we pass, on a descriptor that stays open for the call.
        let mut fl: libc::flock = unsafe { std::mem::zeroed() };
        fl.l_type = libc::F_WRLCK as _;
        fl.l_whence = libc::SEEK_SET as _;
        let r = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETLK, &mut fl) };
        r == 0 && i32::from(fl.l_type) != libc::F_UNLCK
    }
    #[cfg(windows)]
    {
        // Reading a range another process locked fails (ERROR_LOCK_VIOLATION). The game writes
        // a character into the file before locking it, so there is a byte to read.
        use std::io::Read;
        let mut byte = [0u8; 1];
        (&file).read(&mut byte).is_err()
    }
    #[cfg(not(any(unix, windows)))]
    {
        drop(file);
        false
    }
}

/// Fails with `WorldInUse` if the game has any of these worlds open.
pub fn ensure_closed(paths: &Paths, folders: &[String]) -> Result<(), PracticeError> {
    match folders
        .iter()
        .find(|f| world_in_use(&record::world_dir(paths, f)))
    {
        Some(f) => Err(PracticeError::WorldInUse { folder: f.clone() }),
        None => Ok(()),
    }
}

/// Removes a recorded world from `saves/`: moved out first, then deleted. A missing folder is
/// fine; a symlink is left alone (the player put it there).
pub fn remove_world(paths: &Paths, folder: &str) -> Result<bool, PracticeError> {
    if !record::is_plain_folder(folder) {
        return Err(anyhow::anyhow!("refusing to delete {folder:?}").into());
    }
    let dir = record::world_dir(paths, folder);
    match std::fs::symlink_metadata(&dir) {
        Ok(m) if m.is_dir() => {}
        _ => return Ok(false),
    }
    let trash = record::staging(paths, &format!("trash-{}-{folder}", record::now()));
    if let Some(parent) = trash.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(&dir, &trash)
        .with_context(|| format!("saves/{folder} is in use (is the game running?)"))?;
    std::fs::remove_dir_all(&trash)?;
    tidy_staging(paths);
    Ok(true)
}

/// The carry-over files found in the first world that has each, read into memory (they are
/// loadouts and PBs: kilobytes).
pub fn read_carry_over(
    paths: &Paths,
    folders: &[String],
    rules: &[CarryOver],
) -> Vec<(CarryOver, Vec<u8>)> {
    rules
        .iter()
        .filter_map(|c| {
            let from = archive::safe_relative(&c.from)?;
            folders.iter().find_map(|f| {
                let p = record::world_dir(paths, f).join(&from);
                let meta = std::fs::symlink_metadata(&p).ok()?;
                meta.is_file()
                    .then(|| std::fs::read(&p).ok())
                    .flatten()
                    .map(|b| (c.clone(), b))
            })
        })
        .collect()
}

/// Writes carried files into a world folder (a staged one, before it moves into `saves/`).
fn write_carry_over(
    world: &Path,
    files: &[(CarryOver, Vec<u8>)],
) -> Result<Vec<CarryOver>, PracticeError> {
    let mut out = Vec::new();
    for (c, bytes) in files {
        let Some(to) = archive::safe_relative(&c.to) else {
            continue;
        };
        let p = world.join(to);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, bytes)?;
        out.push(c.clone());
    }
    Ok(out)
}

fn clear(dir: &Path) -> Result<(), PracticeError> {
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// Leftovers of an interrupted operation (the process died, or the app closed mid-extract). A
/// swap that never committed still holds the player's old worlds: they go back into `saves/`
/// (under a new name if theirs is taken). Everything else is safe to delete: the game never sees
/// it. Called under the lock at the start of every mutating call.
pub fn recover_staging(paths: &Paths) {
    let root = paths.practice_instance().join(".srl-staging");
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    let saves = record::saves(paths);
    for e in entries.flatten() {
        let path = e.path();
        let is_swap = e.file_name().to_string_lossy().starts_with("swap-")
            && e.file_type().is_ok_and(|t| t.is_dir())
            && !path.join(COMMITTED).exists();
        if is_swap && !restore_swap(&path, &saves) {
            continue; // keep it: deleting would lose a world we couldn't put back
        }
        let _ = std::fs::remove_dir_all(&path).or_else(|_| std::fs::remove_file(&path));
    }
    let _ = std::fs::remove_dir(&root);
}

/// Moves every world of an uncommitted swap back into `saves/`. True if all went back.
fn restore_swap(swap: &Path, saves: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(swap) else {
        return false;
    };
    let _ = std::fs::create_dir_all(saves);
    let mut all = true;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !record::is_plain_folder(&name) {
            continue;
        }
        let folder = unique_name(&name, |n| record::exists_no_follow(&saves.join(n)));
        all &= std::fs::rename(e.path(), saves.join(folder)).is_ok();
    }
    all
}

/// Removes the staging folder once it is empty, so the instance holds only what the game uses.
fn tidy_staging(paths: &Paths) {
    let _ = std::fs::remove_dir(paths.practice_instance().join(".srl-staging"));
}
