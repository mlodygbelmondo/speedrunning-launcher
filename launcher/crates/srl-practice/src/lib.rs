//! Practice maps: the install catalogue, the practice instance, and installing, resetting,
//! updating and removing maps. See `NOTES.md` in this crate for the decisions and the checks.
//!
//! # API
//!
//! ```no_run
//! # async fn demo() -> Result<(), srl_practice::PracticeError> {
//! use srl_core::{Downloader, Paths};
//! use srl_practice::{MapStatus, Practice, UpdatePlan};
//!
//! let practice = Practice::new(Paths::from_env()?, Downloader::new()?)?;
//! let catalogue = practice.catalogue(); // built in, no I/O: credit, licence, requirements
//! let statuses = practice.statuses().await?; // one per catalogue entry, no network
//!
//! let id = "llama-s-bastion-practice-llamapag";
//! let done = practice.install(id, None).await?; // download, verify, extract, record
//! println!("in Singleplayer: {}", done.worlds[0].name);
//!
//! match practice.plan_update(id).await? {
//!     UpdatePlan::UpToDate => {}
//!     UpdatePlan::Replace { .. } => { practice.update(id, None, None).await?; }
//!     UpdatePlan::Ask { .. } => { /* ask the player, then update(id, Some(choice), ..) */ }
//! }
//! let plan = practice.plan_reset(id).await?; // for the confirmation
//! practice.reset(id, true, None).await?;     // fresh world, loadouts kept
//! practice.uninstall(id).await?;
//! # Ok(()) }
//! ```
//!
//! - [`Practice`] is `Clone + Send + Sync`. Every method is async and does its file work on
//!   tokio's blocking pool, so a GPUI app can call it from its background runtime and never
//!   block. Mutating calls (install, update, reset, uninstall) run one at a time; a second one
//!   waits for the first. Dropping a future cancels it: a download stops and its `.part` file is
//!   deleted at once. File work already running on the blocking pool (an extraction, a swap)
//!   finishes first and keeps the lock until it ends, so the next call never overlaps it; any
//!   staging leftovers are cleaned up by the next call, and `saves/` is never left half-written.
//!   For a Cancel button: drop the future during `Step::Downloading` only; once
//!   `Step::Extracting` starts, the call finishes on its own (a few seconds at most).
//! - Replacing worlds (update, reset) extracts the new ones first and swaps them in only when
//!   they are ready. A failure at any point (a bad zip, a full disk, an open world) leaves the
//!   old worlds and the record as they were; a crash mid-swap is undone by the next call.
//! - [`Catalogue`] (`data/catalogue.json`, embedded): the app's cards come from it. Each map
//!   has either a pinned download (the author's GitHub release, sha512 and size) or `link_only`
//!   with a reason. `tools/refresh_catalogue.py` re-pins the data and reports new releases.
//! - [`PRACTICE`] ([`PracticeProfile`]): Minecraft 1.16.1, Fabric, Java 21 and the mod ids for
//!   the practice instance, `Paths::practice_instance()` (`instances/practice`). The app
//!   installs them through srl-game (`Target::practice`) and srl-mods (`resolve_ids`); this crate
//!   does not install the game or mods.
//! - Errors are [`PracticeError`], one variant per thing the UI says differently.
//!
//! ## Where things go
//!
//! - The verified zip (the pristine copy) in the content store, `store/sha512/<ab>/<hex>`.
//! - Its worlds in `instances/practice/saves/<folder>`.
//! - A record per map in `meta/practice/<id>.json`: the worlds it installed and a fingerprint of
//!   each `level.dat`, which tells fresh worlds from played ones (the game rewrites level.dat
//!   whenever a world is opened).
//!
//! The launcher changes only worlds a record names. A world the player made or copied in is
//! never touched; a new world never overwrites an existing folder (it gets ` (2)`).
//!
//! ## Rules for callers
//!
//! - Grey out Reset, Update and Remove while the practice instance's game runs (srl-game owns
//!   the process, this crate can't see it). As a safety net, those calls check the world's
//!   `session.lock`, which the game locks while the world is open, and fail with
//!   [`PracticeError::WorldInUse`] before changing anything. Install never touches an existing
//!   world, so it is safe at any time.
//! - Ask before reset and uninstall (they delete progress in the map's world, which
//!   [`WorldState::Played`] shows), and before an update whose plan is [`UpdatePlan::Ask`].
//!   `update` refuses to choose for the player: without a choice it returns
//!   [`PracticeError::NeedsDecision`].
//! - A future content-store clean-up must keep the files [`Practice::store_files_in_use`] lists.
//!
//! # For the UI
//!
//! **What the Practice page shows.** A card per catalogue entry (copy `map.<id>`), with a state
//! and an action from [`Practice::statuses`]:
//!
//! - [`MapStatus::NotInstalled`]: **Install**, with the size ([`MapDownload::size`]).
//! - [`MapStatus::Installed`]: **Play** (starts the practice instance), **Reset map**,
//!   **Remove**, and **Update to vX** when [`InstalledMap::update`] is set. When
//!   [`InstalledMap::version`] is behind [`InstalledMap::stored_version`] (the player chose to
//!   update later), Reset installs the newer version, so the button reads "Reset to vX".
//! - [`MapStatus::LinkOnly`]: **Open the author's page** with the reason
//!   ([`LinkOnlyReason`]): "made for Minecraft 1.16.5" or "only on a page that needs a
//!   browser".
//! - On every map: the credit line ([`MapEntry::credit`]), the author's page, support links,
//!   the requirements (optional ones marked optional; a mod with `provided: false` is not
//!   installed by the launcher) and the licence note.
//!
//! **Finding the map in the game.** Minecraft 1.16.1 can't open a world from the launch command
//! (Quick Play arrived in 1.20), so the honest flow is: Play starts the practice instance, the
//! player clicks **Singleplayer** and picks the world. Show the name to look for:
//! [`WorldView::name`] is the world's `LevelName` as the list shows it, without colour codes
//! (e.g. "Llama's Bastion Practice v3.15.0"), with the folder ([`WorldView::folder`]) as the
//! subtitle. When carried-over data names a command ([`CarriedOver::then_run`]), tell the player
//! to run it once in the new world. No map in today's catalogue needs one: Mescht's Portal
//! Practice imports old data with `/trigger import` only from v1.4 on (not released as of
//! 2026-10-26); v1.3.1 has no importer.
//!
//! **Loading.** Install and update report [`Progress`]: [`Step::Verifying`] (checking a copy
//! already in the store), [`Step::Downloading`] in bytes, then [`Step::Extracting`] in files.
//! The biggest map (The MCSR Practice Map) is 92 MB and 7,251 files. `statuses` reads small
//! files only (a few KB per installed map) and needs no network.
//!
//! **Empty state.** Nothing installed is the normal first state: every card shows Install.
//!
//! **Errors**, by [`PracticeError`] variant:
//! - `Network`: "Couldn't download. Check the connection and try again."
//! - `SourceGone`: "The author moved this download. Open their page to get it; the launcher
//!   will catch up in an update."
//! - `HashMismatch`: "The download didn't match the version we checked, so nothing was
//!   installed." (Never offer to install it anyway.)
//! - `UnsafeArchive`, `BadArchive`, `NoWorld`: "This download isn't a world we can install."
//! - `WorldInUse`: "The map is open in the game. Leave the world (Save and Quit to Title)
//!   first."
//! - `Other`: disk errors; show the message. "is in use (is the game running?)" means the game
//!   or another program holds a file in the world.
//!
//! **What the copy may honestly claim.**
//! - "Downloaded from the author's own release and checked against the version we tested": yes
//!   (sha512 pinned per version, verified before anything is extracted).
//! - "Installs into a separate Practice instance, never into your run setups": yes.
//! - "Reset gives you the fresh map and keeps your other worlds": yes; with `keep_data` it also
//!   keeps the map's own player-data file where there is one ([`ResetPlan::carry_over`] lists
//!   it): loadouts in Zero Practice and Blaze Practice by Mescht; loadouts, PBs and level
//!   settings in Portal Practice by Mescht. Other maps' records (BT Practice's, for example, kept
//!   in the scoreboard) are reset with the world; say so in the confirmation.
//! - "Updates never touch a world you played without asking": yes. An update carries a file over
//!   only where the author documents it for a new version (Zero Practice, Blaze Practice by
//!   Mescht); Portal Practice's file is kept only by a Reset of the same version.
//! - "If an update or reset fails, your world is as it was": yes.
//! - Don't claim the map opens by itself, that maps are mirrored or hosted by us, or that maps
//!   are approved by the authors: we download from their public releases, as Map-Check does, but
//!   have no written permission (research/04 §4.2 suggests asking).
//! - All 13 installable maps are made for 1.16.1; their worlds were saved by 1.16.1 (DataVersion
//!   2567, checked 2026-10-26).
pub mod archive;
pub mod catalogue;
mod error;
mod fetch;
pub mod level;
pub mod profile;
pub mod record;
mod worlds;

use std::sync::Arc;

use srl_core::{Downloader, Paths};

pub use catalogue::{
    CarryOver, Catalogue, Licence, LinkOnly, LinkOnlyReason, MapDownload, MapEntry, Redistribution,
    Requirement, RequirementKind, SupportLink,
};
pub use error::PracticeError;
pub use profile::{Loader, PRACTICE, PracticeProfile, ProfileMod};
use record::{InstallRecord, Pristine};

/// Progress of an install, update or reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub step: Step,
    pub done: u64,
    pub total: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Hashing a copy that is already in the store (bytes).
    Verifying,
    /// Bytes.
    Downloading,
    /// Files.
    Extracting,
}

/// Called from any thread, often: store the value and notify the UI.
pub type ProgressFn = Arc<dyn Fn(Progress) + Send + Sync>;

/// One card's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapStatus {
    /// No download we may use; open the author's page.
    LinkOnly,
    NotInstalled,
    Installed(InstalledMap),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledMap {
    /// The version of the worlds in `saves/`.
    pub version: String,
    /// The version of the stored pristine copy, which Reset installs. Newer than `version` after
    /// the player chose to update later.
    pub stored_version: String,
    /// The pristine copy is in the store, so Reset works offline. (Checked by size, not hash:
    /// Reset verifies the hash and downloads again if needed.)
    pub stored: bool,
    /// A newer version in the catalogue than the stored one.
    pub update: Option<String>,
    pub worlds: Vec<WorldView>,
    /// Older worlds the player kept on an update; theirs now, never changed by the launcher.
    pub handed_over: Vec<String>,
}

impl InstalledMap {
    pub fn any_played(&self) -> bool {
        self.worlds.iter().any(|w| w.state == WorldState::Played)
    }
    /// The player deleted every world: Install puts the map back.
    pub fn all_missing(&self) -> bool {
        self.worlds.iter().all(|w| w.state == WorldState::Missing)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldView {
    /// The folder under `instances/practice/saves/`.
    pub folder: String,
    /// The name the Singleplayer list shows (LevelName without colour codes), else the folder.
    pub name: String,
    pub state: WorldState,
    pub data_version: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldState {
    /// As installed: never opened.
    Fresh,
    /// Opened in the game since it was installed. Also a folder whose level.dat is unreadable or
    /// gone (a crash mid-save leaves only level.dat_old): it may hold progress, so it is treated
    /// the same way.
    Played,
    /// The folder is gone: the player deleted or moved it.
    Missing,
}

/// What an update would do, for the UI to act on or ask about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdatePlan {
    UpToDate,
    /// No world has been played: replacing them loses nothing. `update(id, None, ..)` does it.
    Replace {
        from: String,
        to: String,
    },
    /// A world has been played. Ask the player for an [`UpdateChoice`].
    Ask {
        from: String,
        to: String,
        played: Vec<WorldView>,
        /// What would be carried into the new world on `ReplaceWorlds` or `KeepBoth`.
        carry_over: Vec<CarryOver>,
    },
}

/// The player's answer to [`UpdatePlan::Ask`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateChoice {
    /// Delete the played worlds and install the new version, carrying over the files the author
    /// documents (loadouts, PBs).
    ReplaceWorlds,
    /// Install the new version next to the played worlds, which become the player's own (the
    /// launcher never touches them again).
    KeepBoth,
    /// Only replace the stored copy; the played worlds stay. Reset installs the new version later.
    StoreOnly,
}

/// What Reset would do, for the confirmation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResetPlan {
    /// The worlds that will be replaced (Played ones lose progress).
    pub worlds: Vec<WorldView>,
    /// The version the fresh worlds will be.
    pub version: String,
    /// The stored copy is missing, so Reset downloads it.
    pub needs_download: bool,
    /// Files `reset(.., keep_data = true, ..)` keeps.
    pub carry_over: Vec<CarryOver>,
}

/// What install, update and reset did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub version: String,
    /// The map's worlds now (empty after `StoreOnly`: they didn't change).
    pub worlds: Vec<WorldView>,
    /// Played worlds handed over to the player (`KeepBoth`).
    pub handed_over: Vec<String>,
    pub carried: Vec<CarriedOver>,
    pub warnings: Vec<Warning>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CarriedOver {
    pub folder: String,
    pub what: String,
    /// The player runs this once in the new world (the map imports the file).
    pub then_run: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    /// The folder name was taken by another world, so the map went to `folder`.
    Renamed { wanted: String, folder: String },
    /// Saved by a newer Minecraft than 1.16.1: the game will warn before opening it.
    NewerWorld { folder: String, data_version: i32 },
    /// level.dat has no DataVersion (or couldn't be read).
    NoDataVersion { folder: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uninstalled {
    pub removed: Vec<String>,
    /// Worlds handed over to the player earlier: left in place.
    pub kept: Vec<String>,
}

/// The practice maps of one data folder. Cheap to clone.
#[derive(Clone)]
pub struct Practice {
    paths: Paths,
    downloader: Downloader,
    catalogue: Arc<Catalogue>,
    lock: Arc<tokio::sync::Mutex<()>>,
}

/// Runs file work off the async threads.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, PracticeError> + Send + 'static,
) -> Result<T, PracticeError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| PracticeError::Other(e.into()))?
}

/// The lock of a mutating call. Shared with every blocking task the call starts, so it is
/// released only when the call's future is gone *and* its file work has ended: dropping the
/// future (the UI's Cancel) can't let a second call run next to an extraction that goes on.
type Guard = Arc<tokio::sync::OwnedMutexGuard<()>>;

/// [`blocking`], holding the call's lock until `f` returns.
async fn locked<T: Send + 'static>(
    guard: &Guard,
    f: impl FnOnce() -> Result<T, PracticeError> + Send + 'static,
) -> Result<T, PracticeError> {
    let guard = guard.clone();
    blocking(move || {
        let _guard = guard;
        f()
    })
    .await
}

impl Practice {
    /// With the built-in catalogue.
    pub fn new(paths: Paths, downloader: Downloader) -> Result<Self, PracticeError> {
        Ok(Self::with_catalogue(
            paths,
            downloader,
            Catalogue::builtin()?,
        ))
    }

    /// With another catalogue (tests, or a hosted one later).
    pub fn with_catalogue(paths: Paths, downloader: Downloader, catalogue: Catalogue) -> Self {
        Self {
            paths,
            downloader,
            catalogue: Arc::new(catalogue),
            lock: Arc::default(),
        }
    }

    pub fn catalogue(&self) -> &Catalogue {
        &self.catalogue
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    fn entry(&self, id: &str) -> Result<MapEntry, PracticeError> {
        self.catalogue
            .get(id)
            .cloned()
            .ok_or_else(|| PracticeError::UnknownMap(id.into()))
    }

    fn download_of(entry: &MapEntry) -> Result<MapDownload, PracticeError> {
        entry
            .download
            .clone()
            .ok_or_else(|| PracticeError::LinkOnly(entry.id.clone()))
    }

    /// Takes the lock of a mutating call, then cleans up after an interrupted one: puts back the
    /// worlds of a swap that never committed, deletes staging leftovers and stale `.part` files.
    async fn begin(&self) -> Result<Guard, PracticeError> {
        let guard = Arc::new(self.lock.clone().lock_owned().await);
        let (paths, cat) = (self.paths.clone(), self.catalogue.clone());
        locked(&guard, move || {
            worlds::recover_staging(&paths);
            fetch::remove_stale_parts(&paths, &cat);
            Ok(())
        })
        .await?;
        Ok(guard)
    }

    /// Every catalogue entry's state, in catalogue order. Local files only.
    pub async fn statuses(&self) -> Result<Vec<(String, MapStatus)>, PracticeError> {
        let (paths, cat) = (self.paths.clone(), self.catalogue.clone());
        blocking(move || {
            cat.maps
                .iter()
                .map(|m| Ok((m.id.clone(), status_of(&paths, m)?)))
                .collect()
        })
        .await
    }

    pub async fn status(&self, id: &str) -> Result<MapStatus, PracticeError> {
        let (paths, entry) = (self.paths.clone(), self.entry(id)?);
        blocking(move || status_of(&paths, &entry)).await
    }

    /// Downloads (unless the store has a verified copy), verifies and extracts the map into the
    /// practice saves, and records it. If the map is installed but the player deleted all its
    /// worlds, this puts them back; otherwise an installed map is `AlreadyInstalled`.
    pub async fn install(
        &self,
        id: &str,
        progress: Option<ProgressFn>,
    ) -> Result<Outcome, PracticeError> {
        let entry = self.entry(id)?;
        let dl = Self::download_of(&entry)?;
        let guard = self.begin().await?;
        let paths = self.paths.clone();
        let old = {
            let (paths, id) = (paths.clone(), entry.id.clone());
            locked(&guard, move || {
                let old = record::load(&paths, &id)?;
                if let Some(old) = &old
                    && !old
                        .worlds
                        .iter()
                        .all(|w| worlds::world_state(&paths, w) == WorldState::Missing)
                {
                    return Err(PracticeError::AlreadyInstalled(id));
                }
                Ok(old)
            })
            .await?
        };
        let zip = paths.store_sha512(&dl.sha512);
        fetch::fetch(
            &self.downloader,
            &dl.url,
            &zip,
            &dl.sha512,
            dl.size,
            progress.as_ref(),
        )
        .await?;
        locked(&guard, move || {
            let mut warnings = Vec::new();
            let rec = InstallRecord {
                id: entry.id.clone(),
                pristine: pristine_of(&dl),
                worlds_version: dl.version.clone(),
                worlds: Vec::new(),
                handed_over: old
                    .as_ref()
                    .map(|o| o.handed_over.clone())
                    .unwrap_or_default(),
                installed_at: record::now(),
            };
            let placed = worlds::place_worlds(
                &paths,
                &entry,
                &dl.version,
                &zip,
                worlds::Swap::default(),
                progress.as_ref(),
                &mut warnings,
                || Ok(()),
                |placed| {
                    let rec = InstallRecord {
                        worlds: placed.to_vec(),
                        ..rec
                    };
                    Ok(record::save(&paths, &rec)?)
                },
            )?;
            if let Some(old) = old {
                drop_unused_store_file(&paths, &old.pristine.sha512);
            }
            Ok(Outcome {
                version: dl.version,
                worlds: placed
                    .worlds
                    .iter()
                    .map(|w| worlds::view(&paths, w))
                    .collect(),
                handed_over: Vec::new(),
                carried: Vec::new(),
                warnings,
            })
        })
        .await
    }

    /// What an update of an installed map would do. No network: compares the stored version with
    /// the catalogue's.
    pub async fn plan_update(&self, id: &str) -> Result<UpdatePlan, PracticeError> {
        let entry = self.entry(id)?;
        let paths = self.paths.clone();
        blocking(move || plan_update_of(&paths, &entry)).await
    }

    /// Updates the map to the catalogue's version. With [`UpdatePlan::Replace`] `choice` may be
    /// `None`; with [`UpdatePlan::Ask`] it must be the player's answer, else
    /// [`PracticeError::NeedsDecision`]. Without a choice, the plan is checked again right before
    /// the worlds are replaced, so a world played during the download is never replaced without
    /// asking. The stored copy is always replaced.
    pub async fn update(
        &self,
        id: &str,
        choice: Option<UpdateChoice>,
        progress: Option<ProgressFn>,
    ) -> Result<Outcome, PracticeError> {
        let entry = self.entry(id)?;
        let dl = Self::download_of(&entry)?;
        let guard = self.begin().await?;
        let paths = self.paths.clone();
        let plan = {
            let (paths, entry) = (paths.clone(), entry.clone());
            locked(&guard, move || plan_update_of(&paths, &entry)).await?
        };
        let implied = choice.is_none();
        let choice = match (&plan, choice) {
            (UpdatePlan::UpToDate, _) => {
                return Err(PracticeError::Other(anyhow::anyhow!(
                    "{id} is already at {}",
                    dl.version
                )));
            }
            (UpdatePlan::Replace { .. }, c) => c.unwrap_or(UpdateChoice::ReplaceWorlds),
            (UpdatePlan::Ask { .. }, Some(c)) => c,
            (UpdatePlan::Ask { .. }, None) => {
                return Err(PracticeError::NeedsDecision(Box::new(plan)));
            }
        };
        if choice == UpdateChoice::ReplaceWorlds {
            // Before downloading: no point fetching an update the open world blocks.
            let (paths, id) = (paths.clone(), entry.id.clone());
            locked(&guard, move || {
                let folders = record::load(&paths, &id)?
                    .map(|r| r.worlds.into_iter().map(|w| w.folder).collect::<Vec<_>>())
                    .unwrap_or_default();
                worlds::ensure_closed(&paths, &folders)
            })
            .await?;
        }
        let zip = paths.store_sha512(&dl.sha512);
        fetch::fetch(
            &self.downloader,
            &dl.url,
            &zip,
            &dl.sha512,
            dl.size,
            progress.as_ref(),
        )
        .await?;
        locked(&guard, move || {
            let mut rec = record::load(&paths, &entry.id)?
                .ok_or_else(|| PracticeError::NotInstalled(entry.id.clone()))?;
            let old_sha = rec.pristine.sha512.clone();
            let mut out = Outcome {
                version: dl.version.clone(),
                worlds: Vec::new(),
                handed_over: Vec::new(),
                carried: Vec::new(),
                warnings: Vec::new(),
            };
            let folders: Vec<String> = rec.worlds.iter().map(|w| w.folder.clone()).collect();
            let rules = carry_rules(&entry, &rec.worlds_version, &dl.version);
            match choice {
                UpdateChoice::StoreOnly => {
                    rec.pristine = pristine_of(&dl);
                    record::save(&paths, &rec)?;
                    out.version = rec.worlds_version.clone();
                }
                UpdateChoice::ReplaceWorlds => {
                    // The player said nothing because nothing was played when they asked. If a
                    // world has been played since (during the download), ask now.
                    let check = || match plan_update_of(&paths, &entry)? {
                        plan @ UpdatePlan::Ask { .. } if implied => {
                            Err(PracticeError::NeedsDecision(Box::new(plan)))
                        }
                        _ => Ok(()),
                    };
                    let placed = worlds::place_worlds(
                        &paths,
                        &entry,
                        &dl.version,
                        &zip,
                        worlds::Swap {
                            replace: &folders,
                            carry_from: &folders,
                            carry: &rules,
                        },
                        progress.as_ref(),
                        &mut out.warnings,
                        check,
                        |placed| {
                            rec.worlds = placed.to_vec();
                            rec.worlds_version = dl.version.clone();
                            rec.pristine = pristine_of(&dl);
                            Ok(record::save(&paths, &rec)?)
                        },
                    )?;
                    out.carried = placed.carried;
                }
                UpdateChoice::KeepBoth => {
                    let handed_over = &mut out.handed_over;
                    let placed = worlds::place_worlds(
                        &paths,
                        &entry,
                        &dl.version,
                        &zip,
                        worlds::Swap {
                            replace: &[],
                            carry_from: &folders,
                            carry: &rules,
                        },
                        progress.as_ref(),
                        &mut out.warnings,
                        || Ok(()),
                        |placed| {
                            for w in std::mem::replace(&mut rec.worlds, placed.to_vec()) {
                                if worlds::world_state(&paths, &w) != WorldState::Missing {
                                    handed_over.push(w.folder.clone());
                                    rec.handed_over.push(w.folder);
                                }
                            }
                            rec.worlds_version = dl.version.clone();
                            rec.pristine = pristine_of(&dl);
                            Ok(record::save(&paths, &rec)?)
                        },
                    )?;
                    out.carried = placed.carried;
                }
            }
            drop_unused_store_file(&paths, &old_sha);
            out.worlds = if choice == UpdateChoice::StoreOnly {
                Vec::new()
            } else {
                rec.worlds.iter().map(|w| worlds::view(&paths, w)).collect()
            };
            Ok(out)
        })
        .await
    }

    /// What Reset would do, for the confirmation dialog.
    pub async fn plan_reset(&self, id: &str) -> Result<ResetPlan, PracticeError> {
        let entry = self.entry(id)?;
        let paths = self.paths.clone();
        blocking(move || {
            let rec = record::load(&paths, &entry.id)?
                .ok_or_else(|| PracticeError::NotInstalled(entry.id.clone()))?;
            let stored = paths.store_sha512(&rec.pristine.sha512);
            Ok(ResetPlan {
                worlds: rec.worlds.iter().map(|w| worlds::view(&paths, w)).collect(),
                version: rec.pristine.version.clone(),
                needs_download: std::fs::metadata(&stored)
                    .map(|m| m.len() != rec.pristine.size)
                    .unwrap_or(true),
                carry_over: carry_rules(&entry, &rec.worlds_version, &rec.pristine.version),
            })
        })
        .await
    }

    /// Replaces the map's worlds with fresh ones from the stored copy (verified first, and
    /// downloaded again if it is missing or damaged). Other saves are not touched. With
    /// `keep_data`, the files the author documents as player data (loadouts, PBs) are copied
    /// into the fresh world.
    pub async fn reset(
        &self,
        id: &str,
        keep_data: bool,
        progress: Option<ProgressFn>,
    ) -> Result<Outcome, PracticeError> {
        let entry = self.entry(id)?;
        let guard = self.begin().await?;
        let paths = self.paths.clone();
        let rec = {
            let (paths, id) = (paths.clone(), entry.id.clone());
            locked(&guard, move || {
                let rec = record::load(&paths, &id)?.ok_or(PracticeError::NotInstalled(id))?;
                let folders: Vec<String> = rec.worlds.iter().map(|w| w.folder.clone()).collect();
                worlds::ensure_closed(&paths, &folders)?;
                Ok(rec)
            })
            .await?
        };
        // The catalogue's URL when it is the same file (it is checked by the catalogue's rules);
        // the record's only for a version the catalogue no longer lists. Either way the file
        // must match the record's sha512.
        let url = match &entry.download {
            Some(d) if d.sha512 == rec.pristine.sha512 => d.url.clone(),
            _ => rec.pristine.url.clone(),
        };
        let zip = paths.store_sha512(&rec.pristine.sha512);
        fetch::fetch(
            &self.downloader,
            &url,
            &zip,
            &rec.pristine.sha512,
            rec.pristine.size,
            progress.as_ref(),
        )
        .await?;
        locked(&guard, move || {
            let mut rec = rec;
            let folders: Vec<String> = rec.worlds.iter().map(|w| w.folder.clone()).collect();
            let version = rec.pristine.version.clone();
            let rules = if keep_data {
                carry_rules(&entry, &rec.worlds_version, &version)
            } else {
                Vec::new()
            };
            let mut out = Outcome {
                version: version.clone(),
                worlds: Vec::new(),
                handed_over: Vec::new(),
                carried: Vec::new(),
                warnings: Vec::new(),
            };
            // The open-world check runs again inside, right before the swap: the game may have
            // opened the world while the copy was downloading or extracting.
            let placed = worlds::place_worlds(
                &paths,
                &entry,
                &version,
                &zip,
                worlds::Swap {
                    replace: &folders,
                    carry_from: &folders,
                    carry: &rules,
                },
                progress.as_ref(),
                &mut out.warnings,
                || Ok(()),
                |placed| {
                    rec.worlds = placed.to_vec();
                    rec.worlds_version = version.clone();
                    Ok(record::save(&paths, &rec)?)
                },
            )?;
            out.carried = placed.carried;
            out.worlds = rec.worlds.iter().map(|w| worlds::view(&paths, w)).collect();
            Ok(out)
        })
        .await
    }

    /// Removes the map's worlds, its record and its stored copy. Worlds handed over to the player
    /// stay.
    pub async fn uninstall(&self, id: &str) -> Result<Uninstalled, PracticeError> {
        let entry = self.entry(id)?;
        let guard = self.begin().await?;
        let paths = self.paths.clone();
        locked(&guard, move || {
            let rec = record::load(&paths, &entry.id)?
                .ok_or_else(|| PracticeError::NotInstalled(entry.id.clone()))?;
            let folders: Vec<String> = rec.worlds.iter().map(|w| w.folder.clone()).collect();
            worlds::ensure_closed(&paths, &folders)?;
            let mut removed = Vec::new();
            for w in &rec.worlds {
                if worlds::remove_world(&paths, &w.folder)? {
                    removed.push(w.folder.clone());
                }
            }
            record::remove(&paths, &entry.id)?;
            drop_unused_store_file(&paths, &rec.pristine.sha512);
            Ok(Uninstalled {
                removed,
                kept: rec.handed_over,
            })
        })
        .await
    }

    /// The store files (sha512) that installed maps keep as their pristine copies. A store
    /// clean-up must keep them.
    pub async fn store_files_in_use(&self) -> Result<Vec<String>, PracticeError> {
        let paths = self.paths.clone();
        blocking(move || {
            Ok(record::all(&paths)
                .into_iter()
                .map(|r| r.pristine.sha512)
                .collect())
        })
        .await
    }
}

fn pristine_of(dl: &MapDownload) -> Pristine {
    Pristine {
        version: dl.version.clone(),
        sha512: dl.sha512.clone(),
        size: dl.size,
        url: dl.url.clone(),
    }
}

fn status_of(paths: &Paths, entry: &MapEntry) -> Result<MapStatus, PracticeError> {
    let Some(dl) = &entry.download else {
        return Ok(MapStatus::LinkOnly);
    };
    let Some(rec) = record::load(paths, &entry.id)? else {
        return Ok(MapStatus::NotInstalled);
    };
    let stored = std::fs::metadata(paths.store_sha512(&rec.pristine.sha512))
        .is_ok_and(|m| m.len() == rec.pristine.size);
    Ok(MapStatus::Installed(InstalledMap {
        version: rec.worlds_version.clone(),
        stored_version: rec.pristine.version.clone(),
        stored,
        update: (dl.sha512 != rec.pristine.sha512).then(|| dl.version.clone()),
        worlds: rec.worlds.iter().map(|w| worlds::view(paths, w)).collect(),
        handed_over: rec.handed_over.clone(),
    }))
}

fn plan_update_of(paths: &Paths, entry: &MapEntry) -> Result<UpdatePlan, PracticeError> {
    let dl = Practice::download_of(entry)?;
    let rec = record::load(paths, &entry.id)?
        .ok_or_else(|| PracticeError::NotInstalled(entry.id.clone()))?;
    Ok(decide_update(
        &rec,
        &dl,
        rec.worlds.iter().map(|w| worlds::view(paths, w)).collect(),
        &carry_rules(entry, &rec.worlds_version, &dl.version),
    ))
}

/// The carry-over rules that apply when worlds of version `from` give way to worlds of `to`: a
/// `same_version_only` file is carried only when the versions are the same (a Reset).
fn carry_rules(entry: &MapEntry, from: &str, to: &str) -> Vec<CarryOver> {
    entry
        .carry_over
        .iter()
        .filter(|c| !c.same_version_only || from == to)
        .cloned()
        .collect()
}

/// The update decision, pure: up to date when the stored copy is the catalogue's file;
/// otherwise replace when no world has been played, and ask when one has. `carry_over` is the
/// list the plan shows (the rules that apply to this update).
pub fn decide_update(
    rec: &InstallRecord,
    dl: &MapDownload,
    views: Vec<WorldView>,
    carry_over: &[CarryOver],
) -> UpdatePlan {
    if rec.pristine.sha512 == dl.sha512 {
        return UpdatePlan::UpToDate;
    }
    let from = rec.pristine.version.clone();
    let to = dl.version.clone();
    let played: Vec<WorldView> = views
        .into_iter()
        .filter(|v| v.state == WorldState::Played)
        .collect();
    if played.is_empty() {
        UpdatePlan::Replace { from, to }
    } else {
        UpdatePlan::Ask {
            from,
            to,
            played,
            carry_over: carry_over.to_vec(),
        }
    }
}

/// Deletes a pristine copy no record uses any more.
fn drop_unused_store_file(paths: &Paths, sha512: &str) {
    if record::all(paths)
        .iter()
        .any(|r| r.pristine.sha512 == sha512)
    {
        return;
    }
    let _ = std::fs::remove_file(paths.store_sha512(sha512));
}

#[cfg(test)]
mod tests;
