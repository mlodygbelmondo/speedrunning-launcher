//! Custom instances on disk: reserving a new one's directory, copying another instance's settings
//! or its whole game directory into it, and moving a deleted one to the trash. Plain blocking
//! file work; `RealBackend` runs it on tokio's blocking pool and reports through its events.
//!
//! - **Create** (`Backend::create_instance`) makes an empty `instances/custom-<n>`; with
//!   `NewInstance::copy_settings_from` it gets that instance's option files (`options.txt`,
//!   `optionsof.txt`, any `options*.txt`), `config/` and `resourcepacks/`, never its worlds or
//!   mods.
//! - **Duplicate** (`Backend::duplicate_instance`) copies the whole game directory but the parts
//!   the launcher makes again or that belong to the source alone (`NOT_COPIED`: `mods/`, logs,
//!   crash reports, the launcher's notes), with `saves/` only when asked. Files are counted
//!   first, then copied one by one with progress; one that can't be read (a world the running
//!   game holds open on Windows) is left out with a note, never failing the copy. Links are
//!   never followed.
//! - **The shared 1.16.1 directory** holds the Atum settings of whichever mode played last, so a
//!   copy of it takes the source mode's own (`shared::per_mode_file`): an SSG seed must not end
//!   up in an RSG-based instance.
//! - **Delete** moves the directory to `<data>/trash/custom-<n>-<unix time>` (a rename on the same
//!   file system, nothing deleted), with the launcher's logs of it, so a mistake can be undone by
//!   moving it back by hand; the install record, verification stamp and pid file go.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use srl_core::{CustomId, Instance, Mode, Paths};

/// What a duplicate copies: one of the launcher's instances, or another launcher's game
/// directory (an import), played like the given mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CopyFrom {
    Instance(Instance),
    Folder(PathBuf, Mode),
}

/// The error a duplicate ends with when `Backend::cancel_copy` stopped it.
pub const COPY_CANCELLED: &str = "copy cancelled";

/// What `Backend::create_instance` makes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewInstance {
    /// Checked with `Settings::validate_instance_name` (trimmed, 1 to 40 characters, unique).
    pub name: String,
    /// The mode it plays like: its game, Java, legal mods and tool rules.
    pub base: Mode,
    /// Start with this instance's options, mod settings (`config/`) and resource packs.
    pub copy_settings_from: Option<Instance>,
}

/// A custom instance that `create_instance` or `duplicate_instance` made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Created {
    pub id: CustomId,
    /// Its name as saved (trimmed; a duplicate whose name was taken while it copied gets a
    /// number, `Settings::unique_instance_name`).
    pub name: String,
    /// Files the copy left out, `<path in the game directory>: <why>`. Empty for a plain create.
    pub skipped: Vec<String>,
}

/// How far a `duplicate_instance` is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyProgress {
    /// The instance being made (not in the settings until the copy is done).
    pub id: CustomId,
    pub name: String,
    pub files: u64,
    pub files_total: u64,
    pub bytes: u64,
    pub bytes_total: u64,
}

/// Entries at the top of a game directory a duplicate never copies: `mods/` is the launcher's to
/// fill from the store (and `mods-removed/` its leftovers), the logs and crash reports are the
/// source's history, `.srl-mods/` and `.srl-modes/` the launcher's notes about the source's mods
/// and modes, `.fabric/` Fabric's cache, made again on the first start.
pub const NOT_COPIED: [&str; 7] = [
    "mods",
    "mods-removed",
    ".srl-mods",
    ".srl-modes",
    "logs",
    "crash-reports",
    ".fabric",
];

/// The worlds folder, copied by a duplicate only when asked.
pub const WORLDS: &str = "saves";

/// The folders `copy_settings_from` takes besides the option files: mod settings and resource
/// packs.
pub const SETTINGS_DIRS: [&str; 2] = ["config", "resourcepacks"];

/// The game's option files at the top of a game directory: `options.txt`, OptiFine's
/// `optionsof.txt`, `optionsshaders.txt` and the like.
pub fn is_option_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("options") && lower.ends_with(".txt")
}

/// Which entries at the top of the source a copy takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyWhat {
    /// `create_instance` with `copy_settings_from`: option files, `config/`, `resourcepacks/`.
    Settings,
    /// `duplicate_instance`: everything but `NOT_COPIED`, and `saves/` only with `worlds`.
    Everything { worlds: bool },
    /// `import_instance`: what `Settings` takes, and `saves/` with `worlds`. Never the rest of
    /// another launcher's folder: the official launcher's `.minecraft` also holds the game's
    /// assets, libraries and versions (gigabytes) and its account files.
    Import { worlds: bool },
}

impl CopyWhat {
    fn takes(self, top: &str, is_dir: bool) -> bool {
        // Windows folders may be "Saves" or "Mods": the names match in any case.
        let top = top.to_ascii_lowercase();
        let top = top.as_str();
        let settings = if is_dir {
            SETTINGS_DIRS.contains(&top)
        } else {
            is_option_file(top)
        };
        match self {
            CopyWhat::Settings => settings,
            CopyWhat::Everything { worlds } => {
                !NOT_COPIED.contains(&top) && (worlds || top != WORLDS)
            }
            CopyWhat::Import { worlds } => settings || (worlds && is_dir && top == WORLDS),
        }
    }
}

/// The files and folders a copy will make, counted before it starts so it can show progress.
#[derive(Debug, Default)]
pub struct CopyPlan {
    /// Folders to create, relative to the game directory, parents first.
    dirs: Vec<PathBuf>,
    /// Files to copy with their sizes.
    files: Vec<(PathBuf, u64)>,
    /// What was left out while counting (links, unreadable folders).
    skipped: Vec<String>,
}

impl CopyPlan {
    /// Walks `from` (no links followed) for what `what` takes.
    pub fn new(from: &Path, what: CopyWhat) -> CopyPlan {
        let mut plan = CopyPlan::default();
        let Ok(top) = fs::read_dir(from) else {
            // A source that was never started has no directory yet: nothing to copy.
            return plan;
        };
        let mut stack: Vec<PathBuf> = Vec::new();
        for entry in top.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                if what.takes(&name, true) || what.takes(&name, false) {
                    plan.skipped.push(format!("{name}: a link, not followed"));
                }
                continue;
            }
            if !what.takes(&name, kind.is_dir()) {
                continue;
            }
            let rel = PathBuf::from(&name);
            if kind.is_dir() {
                plan.dirs.push(rel.clone());
                stack.push(rel);
            } else if kind.is_file() {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                plan.files.push((rel, size));
            }
        }
        while let Some(dir) = stack.pop() {
            let entries = match fs::read_dir(from.join(&dir)) {
                Ok(e) => e,
                Err(e) => {
                    plan.skipped.push(format!("{}: {e}", shown(&dir)));
                    continue;
                }
            };
            for entry in entries.flatten() {
                let rel = dir.join(entry.file_name());
                match entry.file_type() {
                    Ok(t) if t.is_symlink() => plan
                        .skipped
                        .push(format!("{}: a link, not followed", shown(&rel))),
                    Ok(t) if t.is_dir() => {
                        plan.dirs.push(rel.clone());
                        stack.push(rel);
                    }
                    Ok(t) if t.is_file() => {
                        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        plan.files.push((rel, size));
                    }
                    Ok(_) => {}
                    Err(e) => plan.skipped.push(format!("{}: {e}", shown(&rel))),
                }
            }
        }
        // Parents before children (the walk is depth-first from a stack).
        plan.dirs.sort();
        plan
    }

    pub fn files(&self) -> u64 {
        self.files.len() as u64
    }

    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|(_, s)| s).sum()
    }

    /// Copies the plan from `from` into `to` (which exists). `progress(files, bytes)` is called
    /// after every file. Returns what was left out, the walk's notes first.
    pub fn run(self, from: &Path, to: &Path, progress: impl FnMut(u64, u64)) -> Vec<String> {
        self.run_until(from, to, progress, &AtomicBool::new(false))
            .unwrap_or_default()
    }

    /// `run`, checking `stop` before every file: `None` once it is set (what was copied stays,
    /// for the caller to remove).
    pub fn run_until(
        self,
        from: &Path,
        to: &Path,
        mut progress: impl FnMut(u64, u64),
        stop: &AtomicBool,
    ) -> Option<Vec<String>> {
        let mut skipped = self.skipped;
        let mut failed_dirs: Vec<PathBuf> = Vec::new();
        for dir in &self.dirs {
            if failed_dirs.iter().any(|f| dir.starts_with(f)) {
                continue;
            }
            if let Err(e) = fs::create_dir_all(to.join(dir)) {
                skipped.push(format!("{}: {e}", shown(dir)));
                failed_dirs.push(dir.clone());
            }
        }
        let (mut files, mut bytes) = (0, 0);
        for (rel, size) in &self.files {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            if !failed_dirs.iter().any(|f| rel.starts_with(f))
                && let Err(e) = fs::copy(from.join(rel), to.join(rel))
            {
                skipped.push(format!("{}: {e}", shown(rel)));
                let _ = fs::remove_file(to.join(rel));
            }
            files += 1;
            bytes += size;
            progress(files, bytes);
        }
        Some(skipped)
    }
}

/// A relative path as the player reads it, with `/` on every system.
fn shown(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// After copying from `source` into `to`: a copy of the shared 1.16.1 directory gets the source
/// mode's own Atum settings (`shared::PER_MODE_FILES`), or none when that mode has none yet,
/// instead of whichever mode's were in place. Nothing for other sources.
pub fn fix_per_mode_files(paths: &Paths, source: Instance, to: &Path) -> Result<()> {
    let Instance::Mode(mode) = source else {
        return Ok(());
    };
    if !mode.is_shared() {
        return Ok(());
    }
    for rel in srl_core::shared::PER_MODE_FILES {
        let (from, dest) = (
            srl_core::shared::per_mode_file(paths, mode, rel),
            to.join(rel),
        );
        match fs::remove_file(&dest) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("removing {}", dest.display())),
        }
        if from.is_file() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, &dest)
                .with_context(|| format!("copying {} to {}", from.display(), dest.display()))?;
        }
    }
    Ok(())
}

/// Picks the number for a new custom instance and creates its empty directory, so nothing else
/// takes it: `first` (`Settings::next_custom_id`), or the next one whose directory doesn't
/// exist. Leftovers of an earlier instance with that number (its install record, verification
/// stamp, pid file) are removed, so the new one starts as not installed.
pub fn reserve(paths: &Paths, first: CustomId) -> Result<CustomId> {
    let instances = paths.instances();
    fs::create_dir_all(&instances).with_context(|| format!("creating {}", instances.display()))?;
    let mut id = first;
    loop {
        let dir = paths.game_dir(Instance::Custom(id));
        match fs::create_dir(&dir) {
            Ok(()) => break,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                id = CustomId(id.0.checked_add(1).context("no free instance number")?);
            }
            Err(e) => return Err(e).with_context(|| format!("creating {}", dir.display())),
        }
    }
    for f in records(paths, Instance::Custom(id)) {
        let _ = fs::remove_file(f);
    }
    Ok(id)
}

/// The launcher's files about an instance outside its directory: the install record, srl-game's
/// verification stamp and the pid file.
fn records(paths: &Paths, instance: Instance) -> [PathBuf; 3] {
    let id = instance.id();
    [
        paths.meta().join("launcher").join(format!("{id}.json")),
        paths.meta().join("verified").join(format!("{id}.json")),
        paths.logs().join(format!("{id}.pid")),
    ]
}

/// Removes a half-made instance's directory (a failed create or duplicate) and its records.
pub fn discard(paths: &Paths, id: CustomId) {
    let _ = fs::remove_dir_all(paths.game_dir(Instance::Custom(id)));
    for f in records(paths, Instance::Custom(id)) {
        let _ = fs::remove_file(f);
    }
}

/// Moves a deleted custom instance's directory to `trash/custom-<n>-<unix time>/` (a rename,
/// nothing deleted), with the launcher's logs of its game in `launcher-logs/` there, and removes
/// its install record, stamp and pid file. `Ok(None)` when it had no directory. Fails, changing
/// nothing, when the directory can't be moved (a file held open on Windows).
pub fn move_to_trash(paths: &Paths, id: CustomId) -> Result<Option<PathBuf>> {
    let instance = Instance::Custom(id);
    let dir = paths.game_dir(instance);
    let moved = if fs::symlink_metadata(&dir).is_ok() {
        let trash = paths.trash();
        fs::create_dir_all(&trash).with_context(|| format!("creating {}", trash.display()))?;
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let name = format!("{}-{secs}", instance.id());
        let dest = (1..)
            .map(|n| match n {
                1 => trash.join(&name),
                n => trash.join(format!("{name}-{n}")),
            })
            .find(|p| fs::symlink_metadata(p).is_err())
            .expect("a free name");
        fs::rename(&dir, &dest)
            .with_context(|| format!("moving {} to {}", dir.display(), dest.display()))?;
        let logs = dest.join("launcher-logs");
        for which in ["latest", "previous"] {
            let log = paths.logs().join(format!("{}-{which}.log", instance.id()));
            if log.is_file() && fs::create_dir_all(&logs).is_ok() {
                let _ = fs::rename(&log, logs.join(log.file_name().expect("a file name")));
            }
        }
        Some(dest)
    } else {
        None
    };
    for f in records(paths, instance) {
        let _ = fs::remove_file(f);
    }
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("srl-instances-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn tree(root: &Path) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![PathBuf::new()];
        while let Some(rel) = stack.pop() {
            for e in fs::read_dir(root.join(&rel)).unwrap().flatten() {
                let r = rel.join(e.file_name());
                if e.file_type().unwrap().is_dir() {
                    stack.push(r);
                } else {
                    out.push(shown(&r));
                }
            }
        }
        out.sort();
        out
    }

    #[test]
    fn a_copy_stops_when_asked() {
        let root = temp("stop");
        let (src, to) = (root.join("src"), root.join("to"));
        for i in 0..5 {
            write(&src.join(format!("saves/w{i}/level.dat")), "x");
        }
        fs::create_dir_all(&to).unwrap();
        let stop = AtomicBool::new(false);
        let mut copied = 0;
        let answer = CopyPlan::new(&src, CopyWhat::Everything { worlds: true }).run_until(
            &src,
            &to,
            |files, _| {
                copied = files;
                if files == 2 {
                    stop.store(true, Ordering::Relaxed);
                }
            },
            &stop,
        );
        assert_eq!(answer, None);
        assert_eq!(copied, 2);
        assert_eq!(tree(&to).len(), 2);
        let _ = fs::remove_dir_all(&root);
    }

    fn source(root: &Path) -> PathBuf {
        let src = root.join("instances/mcsr");
        for f in [
            "options.txt",
            "optionsof.txt",
            "servers.dat",
            "config/sodium.json",
            "resourcepacks/pack.zip",
            "saves/World 1/level.dat",
            "saves/World 1/region/r.0.0.mca",
            "mods/sodium.jar",
            "mods-removed/x.jar",
            "logs/latest.log",
            "crash-reports/c.txt",
            ".srl-mods/owner",
            ".srl-modes/state.json",
            ".fabric/cache",
            "speedrunigt/records.json",
        ] {
            write(&src.join(f), f);
        }
        src
    }

    #[test]
    fn settings_copy_takes_options_config_and_packs() {
        let root = temp("settings");
        let src = source(&root);
        let to = root.join("instances/custom-1");
        fs::create_dir_all(&to).unwrap();
        let plan = CopyPlan::new(&src, CopyWhat::Settings);
        assert_eq!(plan.files(), 4);
        let skipped = plan.run(&src, &to, |_, _| {});
        assert!(skipped.is_empty(), "{skipped:?}");
        assert_eq!(
            tree(&to),
            [
                "config/sodium.json",
                "options.txt",
                "optionsof.txt",
                "resourcepacks/pack.zip"
            ]
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_duplicate_leaves_out_mods_logs_and_the_launchers_notes() {
        let root = temp("dup");
        let src = source(&root);
        for worlds in [false, true] {
            let to = root.join(format!("instances/custom-{worlds}"));
            fs::create_dir_all(&to).unwrap();
            let plan = CopyPlan::new(&src, CopyWhat::Everything { worlds });
            let total = (plan.files(), plan.bytes());
            let mut last = (0, 0);
            let skipped = plan.run(&src, &to, |f, b| last = (f, b));
            assert!(skipped.is_empty(), "{skipped:?}");
            assert_eq!(last, total);
            let mut want = vec![
                "config/sodium.json",
                "options.txt",
                "optionsof.txt",
                "resourcepacks/pack.zip",
                "servers.dat",
                "speedrunigt/records.json",
            ];
            if worlds {
                want.extend(["saves/World 1/level.dat", "saves/World 1/region/r.0.0.mca"]);
                want.sort();
            }
            assert_eq!(tree(&to), want);
        }
        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn links_are_never_followed_and_unreadable_files_are_noted() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp("links");
        let src = root.join("src");
        write(&src.join("saves/a/level.dat"), "a");
        write(&root.join("outside/secret.txt"), "s");
        std::os::unix::fs::symlink(root.join("outside"), src.join("saves/link")).unwrap();
        write(&src.join("saves/b/level.dat"), "b");
        let locked = src.join("saves/b/level.dat");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let readable = fs::read(&locked).is_ok(); // root reads anything
        let to = root.join("to");
        fs::create_dir_all(&to).unwrap();
        let skipped =
            CopyPlan::new(&src, CopyWhat::Everything { worlds: true }).run(&src, &to, |_, _| {});
        assert!(skipped.iter().any(|s| s.starts_with("saves/link: a link")));
        assert_eq!(skipped.len(), if readable { 1 } else { 2 }, "{skipped:?}");
        assert!(to.join("saves/a/level.dat").is_file());
        assert!(!to.join("saves/link").exists());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_copy_of_the_shared_directory_takes_its_modes_atum_settings() {
        let root = temp("atum");
        let paths = Paths::new(&root);
        let shared = paths.instance(Mode::Rsg);
        let rel = srl_core::shared::PER_MODE_FILES[0];
        write(&shared.join(rel), "rsg");
        srl_core::shared::switch_to(&paths, Mode::Rsg).unwrap();
        srl_core::shared::switch_to(&paths, Mode::Ssg).unwrap();
        write(&shared.join(rel), "ssg seed");
        let to = root.join("instances/custom-1");
        fs::create_dir_all(&to).unwrap();
        CopyPlan::new(&shared, CopyWhat::Everything { worlds: false }).run(&shared, &to, |_, _| {});
        assert_eq!(fs::read_to_string(to.join(rel)).unwrap(), "ssg seed");
        fix_per_mode_files(&paths, Instance::Mode(Mode::Rsg), &to).unwrap();
        assert_eq!(fs::read_to_string(to.join(rel)).unwrap(), "rsg");
        // FSG never played: Atum's defaults, so no file.
        fix_per_mode_files(&paths, Instance::Mode(Mode::Fsg), &to).unwrap();
        assert!(!to.join(rel).exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn reserve_skips_taken_numbers_and_trash_keeps_everything() {
        let root = temp("trash");
        let paths = Paths::new(&root);
        fs::create_dir_all(paths.game_dir(Instance::Custom(CustomId(1)))).unwrap();
        write(&root.join("meta/launcher/custom-2.json"), "{}");
        let id = reserve(&paths, CustomId(1)).unwrap();
        assert_eq!(id, CustomId(2));
        assert!(!root.join("meta/launcher/custom-2.json").exists());
        let dir = paths.game_dir(Instance::Custom(id));
        write(&dir.join("saves/w/level.dat"), "w");
        write(&root.join("logs/custom-2-latest.log"), "log");
        write(&root.join("logs/custom-2.pid"), "{}");
        let moved = move_to_trash(&paths, id).unwrap().unwrap();
        assert!(!dir.exists());
        assert!(moved.starts_with(paths.trash()));
        assert!(
            moved
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("custom-2-")
        );
        assert!(moved.join("saves/w/level.dat").is_file());
        assert!(moved.join("launcher-logs/custom-2-latest.log").is_file());
        assert!(!root.join("logs/custom-2.pid").exists());
        // No directory: nothing to move, still fine.
        assert_eq!(move_to_trash(&paths, CustomId(9)).unwrap(), None);
        fs::remove_dir_all(&root).unwrap();
    }
}
