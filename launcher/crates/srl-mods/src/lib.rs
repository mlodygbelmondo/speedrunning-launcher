//! Legal speedrun mods: mcsr-meta schema 7, the legal-builds.csv cross-check, the per-mode sets and
//! installing them into an instance. See launcher/README.md, stream "mods".
//!
//! # API
//!
//! ```ignore
//! use srl_mods::{InstanceGuard, ModMeta, Os, ResolveOptions, Status};
//!
//! // Once per launcher process, shared with the game side (see below).
//! let guard = InstanceGuard::new();
//!
//! // On start and before Play: fetch the manifests (refresh = true), falling back to the last
//! // good copies under Paths::meta()/mcsr when offline. `meta.notes` says what came from cache.
//! let meta = ModMeta::load(&paths, &downloader, true).await?;
//!
//! // The mode's set for this OS. Pure: no I/O. `set.notes` lists what was left out and why.
//! let set = srl_mods::resolve(&meta, Mode::Ranked, Os::current(), &ResolveOptions::default())?;
//!
//! // Without downloading: UpToDate, UpdateAvailable(plan) or IllegalJarPresent { illegal, plan }.
//! let status = srl_mods::status(&paths, &set).await?;
//!
//! // Make instances/<mode>/mods exactly the set. Reports Stage::Mods progress. Fails with
//! // srl_mods::InstanceBusy (downcast the anyhow error) if that mode's game is running.
//! let report = srl_mods::install(&paths, &downloader, &set, &guard, Some(progress)).await?;
//!
//! // The game side: hold the token for as long as the game process lives. It fails while an
//! // install of that mode is running, so the two can never overlap.
//! let token = guard.game_started(Mode::Ranked)?;
//! ```
//!
//! # Sets per mode (all from `resolve`)
//!
//! - **RSG**: the mc.sr recommended 1.16.1 set: `mod.recommended && version.recommended &&
//!   !obsolete`, traits `rsg-only` in, `ssg-only` out, `mac-only` only on macOS (where Sodium Mac
//!   replaces Sodium), `accessibility` only with `ResolveOptions::accessibility`, every jar in
//!   `legal-builds.csv`, plus dependencies.
//! - **Ranked**: RSG + `mcsrranked` from Modrinth `mcsr-ranked` (its newest build: only the newest
//!   can connect). Never Fabric API (bundled in the Ranked jar), Glacier or Practice Seed Mod (the
//!   Ranked jar `breaks` them). The Ranked anti-cheat whitelists exactly `mods.json`.
//! - **FSG**: RSG + `fsg-mod` (Modrinth `fsg-mod`, needs Atum).
//! - **SSG**: the `ssg-only` category: + SetSpawn, − WorldPreview, − LazyStronghold (traits).
//! - **The player's changes** (`choice`): any mod of a set off, any mod the lists know for the
//!   version on, and jars of their own; `concerns` says what makes the run not legal.
//! - **Draftout**: the recommended legal 26.3 set + `draftout` (Modrinth, newest; it self-updates
//!   in game anyway), cut down to the mods Draftout itself accepts (`allowed_only`: it refuses to
//!   start with Planifolia or SpeedRunIGT); its optional extras (No Chat Restrictions, No
//!   Telemetry, CommandQMod on mac) with `ResolveOptions::draftout_extras`.
//!
//! # Legality policy
//!
//! - A legal-board jar is installed only if its sha512 is in `legal-builds.csv` (rule A.8.5). A
//!   recommended build missing from the CSV is left out with `Note::NotInLegalBuilds` rather than
//!   installed: the CSV is the authority, and gaps close when the next legal-mods commit lands.
//!   Schema 7 has one build per Minecraft version, so the only older build is the one already in
//!   `mods/`: a legal one stays there until the CSV lists the new build (`ModSet::keep_installed`).
//! - The non-legal extras are verified against their own source's sha512: Modrinth for
//!   `mcsrranked`, `fsg-mod` and `draftout` (extra.json lags behind it and is only the offline
//!   fallback), `extra.json` for Draftout's optional extras.
//! - `mods/` is owned by the launcher: every jar not in the set leaves it (deleted when the store
//!   has a copy, else moved to `mods-removed/<date>/`, so a jar the player added is never lost),
//!   and a jar that is not allowed in the mode at all (unknown, illegalized, another category's,
//!   forbidden) is reported as illegal by `status`.
pub mod choice;
pub mod install;
pub mod meta;
pub mod resolve;

pub use choice::{Available, Concern, CustomJar, ModChoice, apply, catalogue, concerns};
pub use install::{
    GameToken, InstallReport, InstalledJar, InstanceBusy, InstanceGuard, Plan, Status, Update,
    install, mods_dir, owner, plan, removed_dir, status,
};
pub use meta::{LegalBuilds, Manifest, MetaMod, MetaVersion, ModMeta, ModrinthVersion};
pub use resolve::{
    KnownJar, ModEntry, ModSet, ModSource, Note, Os, Reason, ResolveOptions, allowed_only,
    forbidden, resolve, resolve_ids,
};
