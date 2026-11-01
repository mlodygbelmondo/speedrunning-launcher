//! Keeping `instances/<mode>/mods` equal to a `ModSet`.
//!
//! Jars are downloaded once into the content store (`store/sha512/<ab>/<hex>`, verified by the
//! `Downloader`) and materialised into `mods/` as hard links, or copies where a hard link is not
//! possible (another volume, FAT). On Windows always copies: there every mode's hard link would be
//! one file object, and a JVM holds its jars open without `FILE_SHARE_DELETE`, so while one mode's
//! game runs, another mode could not remove or replace its link to the same jar. Never symlinks:
//! Fabric and the Ranked anti-cheat read the file, and a dangling link would break the instance
//! when the store is cleaned.
//!
//! The launcher owns `mods/` in its own instances: any jar not in the set leaves it, so the
//! instance stays legal. A jar the store has a copy of is deleted; any other jar (one the player
//! put there) is moved to `mods-removed/<date>/` next to `mods/`, never deleted.
//!
//! Ranked, RSG, FSG and SSG share one game directory (`srl_core::shared`), so their sets take
//! turns in one `mods/`. After each install the instance's jars are noted in
//! `.srl-mods/<instance>.json` next to `mods/`, and `.srl-mods/owner` names the instance whose set
//! is in place. `plan` and `status` for another mode compare its set with its own note, not with
//! `mods/`: a switch between modes is not an update.
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use srl_core::{
    Download, Downloader, Hash, Instance, Paths, ProgressFn, Stage, download::file_sha512,
};

use crate::resolve::{KnownJar, ModEntry, ModSet};

/// Prefix of the temporary names used while materialising. Not a `.jar`, so Fabric ignores a
/// leftover from a crash, and the next install deletes it.
const TMP_PREFIX: &str = ".srl-";

/// Tells `install` which instances have a running game, and keeps a game from starting while
/// `install` changes its `mods/`. One per launcher process, shared (it is cheap to clone).
///
/// The game side (srl-game, wired by the integrator) calls `game_started` right before it spawns
/// the process and drops the token when the process exits. Only this process is covered: a game
/// left running by an earlier launcher session has to be detected by the caller.
///
/// Keyed by game directory ([`Instance::dir_id`]): the four 1.16.1 modes share one `mods/`, so a
/// running RSG game blocks an SSG install (and game) too; Draftout, the practice instance and
/// every custom instance have their own.
#[derive(Clone, Default)]
pub struct InstanceGuard {
    busy: Arc<Mutex<BusyDirs>>,
}

/// What runs in each game directory, by [`Instance::dir_id`].
type BusyDirs = HashMap<Cow<'static, str>, (Instance, Busy)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Busy {
    Game,
    Mods,
}

/// Why an instance's `mods/` cannot be touched (or its game started) right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstanceBusy {
    GameRunning(Instance),
    Installing(Instance),
}

impl std::fmt::Display for InstanceBusy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstanceBusy::GameRunning(m) => {
                write!(
                    f,
                    "the {} game is running; its mods stay as they are",
                    m.id()
                )
            }
            InstanceBusy::Installing(m) => write!(f, "the {} mods are being updated", m.id()),
        }
    }
}

impl std::error::Error for InstanceBusy {}

/// Held while a game runs; dropping it marks the game as exited.
pub struct GameToken {
    guard: InstanceGuard,
    instance: Instance,
}

impl Drop for GameToken {
    fn drop(&mut self) {
        self.guard.release(self.instance, Busy::Game);
    }
}

struct InstallToken {
    guard: InstanceGuard,
    instance: Instance,
}

impl Drop for InstallToken {
    fn drop(&mut self) {
        self.guard.release(self.instance, Busy::Mods);
    }
}

impl InstanceGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks the instance's game as running until the token is dropped. Fails while `install`
    /// is changing that instance's mods.
    pub fn game_started(&self, instance: impl Into<Instance>) -> Result<GameToken, InstanceBusy> {
        let instance = instance.into();
        self.acquire(instance, Busy::Game)?;
        Ok(GameToken {
            guard: self.clone(),
            instance,
        })
    }

    /// Whether this instance's own game runs (not another mode of its directory).
    pub fn is_game_running(&self, instance: impl Into<Instance>) -> bool {
        let instance = instance.into();
        self.lock().get(&instance.dir_id()) == Some(&(instance, Busy::Game))
    }

    /// The instance whose game runs in `instance`'s directory, if any (itself or another mode).
    pub fn game_in_dir(&self, instance: impl Into<Instance>) -> Option<Instance> {
        match self.lock().get(&instance.into().dir_id()) {
            Some((running, Busy::Game)) => Some(*running),
            _ => None,
        }
    }

    fn begin_install(&self, instance: Instance) -> Result<InstallToken, InstanceBusy> {
        self.acquire(instance, Busy::Mods)?;
        Ok(InstallToken {
            guard: self.clone(),
            instance,
        })
    }

    fn acquire(&self, instance: Instance, what: Busy) -> Result<(), InstanceBusy> {
        let mut busy = self.lock();
        match busy.get(&instance.dir_id()) {
            Some((other, Busy::Game)) => Err(InstanceBusy::GameRunning(*other)),
            Some((other, Busy::Mods)) => Err(InstanceBusy::Installing(*other)),
            None => {
                busy.insert(instance.dir_id(), (instance, what));
                Ok(())
            }
        }
    }

    fn release(&self, instance: Instance, what: Busy) {
        let mut busy = self.lock();
        if busy.get(&instance.dir_id()) == Some(&(instance, what)) {
            busy.remove(&instance.dir_id());
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BusyDirs> {
        // The map stays consistent even if a holder panicked: every critical section is one
        // insert or remove.
        self.busy.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// A jar found in `mods/`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledJar {
    pub file_name: String,
    pub sha512: String,
    pub size: u64,
    /// What the metadata knows about this exact build; `None` for a foreign jar.
    pub known: Option<KnownJar>,
}

impl InstalledJar {
    /// The jar must not be in this instance: unknown, illegalized, or not allowed in the mode.
    pub fn is_illegal(&self) -> bool {
        !self.known.as_ref().is_some_and(|k| k.allowed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Update {
    pub from: InstalledJar,
    pub to: ModEntry,
}

/// What `install` would change in `mods/`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// File names of the jars already right.
    pub keep: Vec<String>,
    pub add: Vec<ModEntry>,
    /// An older (or other) build of a mod in the set, replaced by the set's build.
    pub update: Vec<Update>,
    /// Jars that are not in the set.
    pub remove: Vec<InstalledJar>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.update.is_empty() && self.remove.is_empty()
    }

    /// Jars that make the instance illegal as it is now.
    pub fn illegal(&self) -> Vec<&InstalledJar> {
        self.update
            .iter()
            .map(|u| &u.from)
            .chain(&self.remove)
            .filter(|j| j.is_illegal())
            .collect()
    }

    /// Bytes `install` would have to put into `mods/` (download or link).
    pub fn bytes(&self) -> u64 {
        self.add
            .iter()
            .chain(self.update.iter().map(|u| &u.to))
            .map(|m| m.size)
            .sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    UpToDate,
    /// Mods to add, update or remove; nothing illegal in `mods/` now.
    UpdateAvailable(Plan),
    /// `mods/` holds a jar that must not be there (the plan removes or replaces it).
    IllegalJarPresent {
        illegal: Vec<InstalledJar>,
        plan: Plan,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallReport {
    pub added: Vec<ModEntry>,
    pub updated: Vec<Update>,
    pub removed: Vec<InstalledJar>,
    /// Where the jars that left `mods/` without a copy in the store went
    /// (`mods-removed/<date>/<name>`), so the player can get them back.
    pub set_aside: Vec<PathBuf>,
    /// Jars that were already right.
    pub unchanged: usize,
    /// How the new jars got into `mods/`.
    pub hard_linked: usize,
    pub copied: usize,
}

impl InstallReport {
    pub fn changed(&self) -> bool {
        !(self.added.is_empty() && self.updated.is_empty() && self.removed.is_empty())
    }
}

/// `instances/<mode>/mods`, or `instances/practice/mods`.
pub fn mods_dir(paths: &Paths, instance: impl Into<Instance>) -> PathBuf {
    paths.instance_dir(instance).join("mods")
}

/// Compares the instance's mods with the set, without downloading anything. Hashes every jar in
/// `mods/` (a full set is about 60 MB, well under a second). For a mode whose directory holds
/// another mode's set right now, compares with the jars it had after its own last install (a
/// jar the store lost counts as missing).
pub async fn plan(paths: &Paths, set: &ModSet) -> Result<Plan> {
    if owner(paths, set.instance).is_some_and(|o| o != set.instance) {
        // Its own last install, or (never installed here, or the note is gone) the set's jars
        // the store has: then only what Play would download counts.
        let noted = noted_jars(paths, set.instance).unwrap_or_else(|| {
            set.mods
                .iter()
                .map(|m| Found {
                    file_name: m.file_name.clone(),
                    sha512: m.sha512.clone(),
                    size: m.size,
                    symlink: false,
                })
                .collect()
        });
        let kept = noted
            .into_iter()
            .filter(|f| paths.store_sha512(&f.sha512).is_file())
            .collect();
        return Ok(plan_from(set, kept));
    }
    real_plan(paths, set).await
}

/// `plan` against what is in `mods/` now.
async fn real_plan(paths: &Paths, set: &ModSet) -> Result<Plan> {
    let installed = scan(&mods_dir(paths, set.instance)).await?;
    Ok(plan_from(set, installed))
}

/// Next to `mods/`: which instance's set is in place, and each instance's jars.
fn note_dir(paths: &Paths, instance: Instance) -> PathBuf {
    paths.instance_dir(instance).join(".srl-mods")
}

/// The instance whose set is in `mods/` now, if the launcher put one there. In the shared
/// directory before its first install, the mode it was moved over from
/// (`srl_core::shared::current`).
pub fn owner(paths: &Paths, instance: impl Into<Instance>) -> Option<Instance> {
    let instance = instance.into();
    let Ok(id) = std::fs::read_to_string(note_dir(paths, instance).join("owner")) else {
        return match instance {
            Instance::Mode(m) if m.is_shared() => {
                srl_core::shared::current(paths).map(Instance::Mode)
            }
            _ => None,
        };
    };
    instance
        .siblings()
        .into_iter()
        .find(|i| i.id() == id.trim())
}

/// The jars of the instance's last install, from its note.
fn noted_jars(paths: &Paths, instance: Instance) -> Option<Vec<Found>> {
    let bytes = std::fs::read(note_dir(paths, instance).join(format!("{}.json", instance.id())));
    let jars: Vec<NotedJar> = serde_json::from_slice(&bytes.ok()?).ok()?;
    Some(
        jars.into_iter()
            .map(|j| Found {
                file_name: j.file_name,
                sha512: j.sha512,
                size: j.size,
                symlink: false,
            })
            .collect(),
    )
}

#[derive(Serialize, Deserialize)]
struct NotedJar {
    file_name: String,
    sha512: String,
    size: u64,
}

async fn note_install(paths: &Paths, set: &ModSet) -> Result<()> {
    let dir = note_dir(paths, set.instance);
    tokio::fs::create_dir_all(&dir).await?;
    let jars: Vec<NotedJar> = set
        .mods
        .iter()
        .map(|m| NotedJar {
            file_name: m.file_name.clone(),
            sha512: m.sha512.clone(),
            size: m.size,
        })
        .collect();
    tokio::fs::write(
        dir.join(format!("{}.json", set.instance.id())),
        serde_json::to_vec_pretty(&jars)?,
    )
    .await?;
    tokio::fs::write(dir.join("owner"), set.instance.id().as_bytes()).await?;
    Ok(())
}

/// Up to date, updates available, or an illegal jar present. No downloads.
pub async fn status(paths: &Paths, set: &ModSet) -> Result<Status> {
    let plan = plan(paths, set).await?;
    let illegal: Vec<InstalledJar> = plan.illegal().into_iter().cloned().collect();
    Ok(if !illegal.is_empty() {
        Status::IllegalJarPresent { illegal, plan }
    } else if plan.is_empty() {
        Status::UpToDate
    } else {
        Status::UpdateAvailable(plan)
    })
}

/// Makes `mods/` of the set's instance exactly the set: downloads what the store lacks (verified
/// by sha512), links the new jars in, removes the rest. Fails with `InstanceBusy` (downcast the
/// error) if that instance's game is running, without touching anything.
pub async fn install(
    paths: &Paths,
    downloader: &Downloader,
    set: &ModSet,
    guard: &InstanceGuard,
    progress: Option<ProgressFn>,
) -> Result<InstallReport> {
    let _token = guard.begin_install(set.instance)?;
    let dir = mods_dir(paths, set.instance);
    tokio::fs::create_dir_all(&dir)
        .await
        .with_context(|| format!("creating {}", dir.display()))?;
    let plan = real_plan(paths, set).await?;

    let wanted: Vec<&ModEntry> = plan
        .add
        .iter()
        .chain(plan.update.iter().map(|u| &u.to))
        .collect();
    // The player's own jars have no URL: the store copy is the only one.
    if let Some(gone) = wanted.iter().find(|m| {
        matches!(m.source, crate::ModSource::Custom) && !paths.store_sha512(&m.sha512).is_file()
    }) {
        bail!(
            "{} is no longer in the launcher's store; add the jar again on the Mods page",
            gone.file_name
        );
    }
    let downloads = wanted
        .iter()
        .map(|m| Download {
            url: m.url.clone(),
            dest: paths.store_sha512(&m.sha512),
            hash: Hash::Sha512(m.sha512.clone()),
            size: Some(m.size),
        })
        .collect();
    downloader
        .download_all(downloads, Stage::Mods, progress)
        .await?;

    let mut report = InstallReport {
        unchanged: plan.keep.len(),
        ..Default::default()
    };
    let leaving: Vec<&InstalledJar> = plan
        .remove
        .iter()
        .chain(plan.update.iter().map(|u| &u.from))
        .collect();
    // A jar leaving under the name of a new one (another build the player put there) would be
    // destroyed by the rename below: it goes first, aside if the store has no copy.
    let taken = |jar: &InstalledJar| {
        wanted
            .iter()
            .any(|m| same_name(&m.file_name, &jar.file_name))
    };
    let aside_dir = removed_dir(paths, set.instance);
    for jar in leaving.iter().filter(|j| taken(j)) {
        take_out(paths, &dir, &aside_dir, jar, &mut report.set_aside).await?;
    }
    for m in &wanted {
        let store = paths.store_sha512(&m.sha512);
        let dir2 = dir.clone();
        let name = m.file_name.clone();
        let linked = tokio::task::spawn_blocking(move || materialise(&store, &dir2, &name))
            .await?
            .with_context(|| format!("putting {} into {}", m.file_name, dir.display()))?;
        if linked {
            report.hard_linked += 1;
        } else {
            report.copied += 1;
        }
    }

    // The rest last, so a failed download leaves the old (working) set in place.
    for jar in leaving.iter().filter(|j| !taken(j)) {
        take_out(paths, &dir, &aside_dir, jar, &mut report.set_aside).await?;
    }
    remove_leftovers(&dir).await;
    note_install(paths, set)
        .await
        .with_context(|| format!("noting the mods of {}", set.instance.id()))?;

    report.added = plan.add;
    report.updated = plan.update;
    report.removed = plan.remove;
    Ok(report)
}

/// Takes a jar out of `mods/`. The store has it (by content): deleting loses nothing. Anything
/// else may be the only copy of a jar the player added, so it is moved aside instead.
async fn take_out(
    paths: &Paths,
    dir: &Path,
    aside_dir: &Path,
    jar: &InstalledJar,
    set_aside: &mut Vec<PathBuf>,
) -> Result<()> {
    let path = dir.join(&jar.file_name);
    let in_store = !jar.sha512.is_empty() && paths.store_sha512(&jar.sha512).is_file();
    let result = if in_store {
        tokio::fs::remove_file(&path).await
    } else {
        let dest = aside_dir.join(&jar.file_name);
        let moved = async {
            tokio::fs::create_dir_all(aside_dir).await?;
            tokio::fs::rename(&path, &dest).await
        }
        .await;
        if moved.is_ok() {
            set_aside.push(dest);
        }
        moved
    };
    match result {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("removing {}", path.display())),
    }
}

/// Whether two names in `mods/` are one file: without case on Windows.
fn same_name(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// Where jars that leave `mods/` without a store copy go on this install:
/// `instances/<mode>/mods-removed/<UTC date and time>/`. Fabric loads only `mods/`, so they are
/// out of the game but still there.
pub fn removed_dir(paths: &Paths, instance: impl Into<Instance>) -> PathBuf {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    paths
        .instance_dir(instance)
        .join("mods-removed")
        .join(utc_stamp(secs))
}

/// `YYYY-MM-DD_HH-MM-SS` in UTC, for a folder name (no `:` on Windows).
fn utc_stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Days since 1970-01-01 to a civil date (Howard Hinnant's `civil_from_days`).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}_{:02}-{:02}-{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Puts the store file into `dir/name` atomically: link (or copy) to a temporary name, then
/// rename over the target. Returns whether it is a hard link.
fn materialise(store: &Path, dir: &Path, name: &str) -> std::io::Result<bool> {
    let tmp = dir.join(format!("{TMP_PREFIX}{name}.tmp"));
    let _ = std::fs::remove_file(&tmp);
    // Any hard-link failure (another volume, a file system without links, a permission quirk)
    // falls back to a copy; a copy failure is the real error. On btrfs and XFS `fs::copy` already
    // shares extents (copy_file_range), so an explicit reflink adds nothing. Windows always copies
    // (see the module doc): a few tens of MB per mode for instances that never block each other.
    let link = if cfg!(windows) {
        Err(std::io::ErrorKind::Unsupported.into())
    } else {
        std::fs::hard_link(store, &tmp)
    };
    let linked = match link {
        Ok(()) => true,
        Err(_) => {
            std::fs::copy(store, &tmp)?;
            false
        }
    };
    let target = dir.join(name);
    std::fs::rename(&tmp, &target)?;
    // rename() between two links of one file is a no-op that leaves `tmp` behind.
    let _ = std::fs::remove_file(&tmp);
    Ok(linked)
}

async fn remove_leftovers(dir: &Path) {
    let Ok(mut rd) = tokio::fs::read_dir(dir).await else {
        return;
    };
    while let Ok(Some(e)) = rd.next_entry().await {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with(TMP_PREFIX) && name.ends_with(".tmp") {
            let _ = tokio::fs::remove_file(e.path()).await;
        }
    }
}

/// A jar in `mods/` before it is matched against the set.
struct Found {
    file_name: String,
    sha512: String,
    size: u64,
    /// Symlinks are never kept, even with the right content.
    symlink: bool,
}

/// Every `*.jar` directly in `dir` (Fabric loads nothing else from there), with its sha512.
async fn scan(dir: &Path) -> Result<Vec<Found>> {
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut found = Vec::new();
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(found),
            Err(e) => return Err(e).with_context(|| format!("reading {}", dir.display())),
        };
        for e in rd {
            let e = e?;
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.to_ascii_lowercase().ends_with(".jar") {
                continue;
            }
            let meta = std::fs::symlink_metadata(e.path())?;
            let symlink = meta.file_type().is_symlink();
            if meta.is_dir() {
                continue;
            }
            // A dangling symlink has no content; it is removed like any other foreign jar.
            let (sha512, size) = match file_sha512(&e.path()) {
                Ok(h) => (h, std::fs::metadata(e.path()).map(|m| m.len()).unwrap_or(0)),
                Err(_) if symlink => (String::new(), 0),
                Err(err) => return Err(err),
            };
            found.push(Found {
                file_name: name,
                sha512,
                size,
                symlink,
            });
        }
        found.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(found)
    })
    .await?
}

fn plan_from(set: &ModSet, found: Vec<Found>) -> Plan {
    let mut plan = Plan::default();
    let mut satisfied: HashSet<&str> = HashSet::new();
    let mut leftovers = Vec::new();
    let links: HashSet<String> = found
        .iter()
        .filter(|f| f.symlink)
        .map(|f| f.file_name.clone())
        .collect();
    for f in found {
        let entry = set.mods.iter().find(|m| m.sha512 == f.sha512);
        match entry {
            // The right build. A second copy of it (another name) is removed.
            Some(m) if !f.symlink && satisfied.insert(m.modid.as_str()) => {
                plan.keep.push(f.file_name)
            }
            _ => leftovers.push(InstalledJar {
                known: set.known.get(&f.sha512).cloned(),
                file_name: f.file_name,
                sha512: f.sha512,
                size: f.size,
            }),
        }
    }
    // A mod whose newest build isn't legal yet keeps the legal build it has.
    for id in &set.keep_installed {
        let kept = leftovers.iter().position(|j| {
            !links.contains(&j.file_name)
                && j.known
                    .as_ref()
                    .is_some_and(|k| k.allowed && k.modid == *id)
        });
        if let Some(i) = kept {
            plan.keep.push(leftovers.remove(i).file_name);
        }
    }
    for m in &set.mods {
        if satisfied.contains(m.modid.as_str()) {
            continue;
        }
        // Pair it with an installed build of the same mod, if there is one: that is an update,
        // not an add and a remove.
        let old = leftovers
            .iter()
            .position(|j| j.known.as_ref().is_some_and(|k| k.modid == m.modid));
        match old {
            Some(i) => plan.update.push(Update {
                from: leftovers.remove(i),
                to: m.clone(),
            }),
            None => plan.add.push(m.clone()),
        }
    }
    plan.remove = leftovers;
    plan
}

#[cfg(test)]
mod tests {
    #[test]
    fn utc_stamps() {
        assert_eq!(super::utc_stamp(0), "1970-01-01_00-00-00");
        assert_eq!(super::utc_stamp(951_782_400), "2000-02-29_00-00-00");
        assert_eq!(super::utc_stamp(1_793_000_000), "2026-10-26_07-33-20");
    }
}
