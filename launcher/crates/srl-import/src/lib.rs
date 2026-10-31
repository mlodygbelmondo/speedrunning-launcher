//! "Import my current settings": finds the player's instances in MultiMC, Prism Launcher (and
//! PolyMC), MCSR Launcher and the official launcher, and copies what the player ticks into the
//! launcher's instances. No UI and no async: the app runs it on its blocking pool.
//!
//! ```no_run
//! use srl_import::{Choice, SearchRoots, Target, detect, import};
//!
//! let sources = detect(&SearchRoots::from_env()); // best match first: 1.16.1, latest played
//! let targets = vec![Target::new("rsg", "/data/instances/rsg", "1.16.1")];
//! let report = import(&sources[0], Choice::default(), &targets, 1_761_480_000, &mut |_, _| {});
//! ```
//!
//! - **Detection** (`detect.rs`) takes its search roots as a parameter (`SearchRoots::for_home`
//!   builds them from any home folder, so tests use fake trees); `SearchRoots::from_env` is this
//!   computer, or `SRL_IMPORT_HOME` for tests. `inspect` reads a folder the player picked.
//! - **What comes over** (`Choice`): the game's settings (`options.txt`), the mods' settings
//!   (`config/` and SpeedRunIGT's options), resource packs (and which are on), worlds. A target
//!   on the source's Minecraft version gets everything ticked; any other gets only the key
//!   bindings both versions have, mouse sensitivity and FOV, merged into its options.txt
//!   (`options.rs`).
//! - **Safety** (`apply.rs`): the source is only read, links in it are never followed, oversized
//!   files are left out, and whatever an instance had is moved to `import-backup-<unix time>/`
//!   inside it before it is replaced. Worlds that exist already are kept, and so are the files a
//!   target keeps for itself (`Target::keeping`: the shared 1.16.1 instance's Atum settings).
//! - Shared on purpose, so not imported: Toolscreen's settings (`%USERPROFILE%\.config\
//!   toolscreen`), Ninjabrain Bot's (Java Preferences), SpeedRunIGT's and SpeedrunAPI's global
//!   files in the home folder. They already are the player's.
use std::path::PathBuf;

mod apply;
mod detect;
pub mod options;

pub use apply::{MAX_FILE, Part, Report, SkipWhy, Skipped, TargetReport, import};
pub use detect::{
    MOD_SETTINGS, Os, SPEEDRUNIGT_FILES, SearchRoots, contents, detect, inspect, sort,
    version_of_data,
};

/// The Minecraft version of the speedrun modes and the practice instance.
pub const VERSION_1_16_1: &str = "1.16.1";

/// The launcher an instance comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Launcher {
    MultiMc,
    Prism,
    PolyMc,
    McsrLauncher,
    /// The official Minecraft Launcher.
    Official,
    /// A folder the player picked that no launcher claims.
    Folder,
}

impl Launcher {
    /// Its name as the launcher itself writes it (the same in every language).
    pub fn name(self) -> &'static str {
        match self {
            Launcher::MultiMc => "MultiMC",
            Launcher::Prism => "Prism Launcher",
            Launcher::PolyMc => "PolyMC",
            Launcher::McsrLauncher => "MCSR Launcher",
            Launcher::Official => "Minecraft Launcher",
            Launcher::Folder => "",
        }
    }
}

/// An instance (or game folder) to import from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub launcher: Launcher,
    /// Its name in its launcher, else its folder's.
    pub name: String,
    /// The game directory: `options.txt`, `config/`, `resourcepacks/`, `saves/`.
    pub game_dir: PathBuf,
    /// The Minecraft version, from the launcher's metadata, else options.txt's data version.
    /// `None`: unknown, so treated as another version than any target's.
    pub version: Option<String>,
    /// When it was last played (Unix seconds): the launcher's record, else the newest of
    /// options.txt and the game log.
    pub last_played: Option<u64>,
    pub contents: Contents,
}

impl Source {
    /// Whether the source runs Minecraft `version` (then everything ticked can come over).
    pub fn runs(&self, version: &str) -> bool {
        self.version.as_deref() == Some(version)
    }
}

/// What a source has to import.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contents {
    pub options: bool,
    /// Files in `config/` (up to 5000) and SpeedRunIGT's options.
    pub mod_files: u32,
    /// The speedrun mods whose settings are there (`MOD_SETTINGS` names), in that order.
    pub mods: Vec<&'static str>,
    pub resource_packs: u32,
    pub worlds: u32,
}

impl Contents {
    pub fn is_empty(&self) -> bool {
        !self.options && self.mod_files == 0 && self.resource_packs == 0 && self.worlds == 0
    }
}

/// What the player ticked. The default is the screen's: everything but the worlds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    /// options.txt: keys, mouse, FOV, video, sound.
    pub options: bool,
    /// `config/` (StandardSettings, Atum, SeedQueue, Sodium...) and SpeedRunIGT's options.
    pub mod_settings: bool,
    /// `resourcepacks/` and the list of the ones that are on.
    pub resource_packs: bool,
    /// `saves/`.
    pub worlds: bool,
}

impl Default for Choice {
    fn default() -> Self {
        Choice {
            options: true,
            mod_settings: true,
            resource_packs: true,
            worlds: false,
        }
    }
}

impl Choice {
    pub fn any(&self) -> bool {
        self.options || self.mod_settings || self.resource_packs || self.worlds
    }
}

/// An instance of this launcher to import into.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// Its id (`rsg`, `practice`), for the report.
    pub id: String,
    /// Its game directory.
    pub dir: PathBuf,
    /// The Minecraft version it runs.
    pub version: String,
    /// Lines its options.txt must have; added when missing (Draftout's
    /// `onboardAccessibility:false`, which the launcher otherwise writes only into a new file).
    pub seed: Vec<(String, String)>,
    /// Files (relative to the game directory, `/`-separated) the import never writes here: the
    /// shared 1.16.1 instance keeps Atum's settings per mode, and a source's could carry an SSG
    /// seed into RSG.
    pub keep: Vec<String>,
}

impl Target {
    pub fn new(id: &str, dir: impl Into<PathBuf>, version: &str) -> Target {
        Target {
            id: id.into(),
            dir: dir.into(),
            version: version.into(),
            seed: Vec::new(),
            keep: Vec::new(),
        }
    }

    pub fn with_seed(mut self, key: &str, value: &str) -> Target {
        self.seed.push((key.into(), value.into()));
        self
    }

    /// Leaves the instance's `rel` (`config/mcsr/atum.json`) as it is.
    pub fn keeping(mut self, rel: &str) -> Target {
        self.keep.push(rel.into());
        self
    }

    /// Whether the import leaves `rel` alone here. File names are compared without case, as
    /// Windows does.
    pub fn keeps(&self, rel: &std::path::Path) -> bool {
        let rel = rel.to_string_lossy().replace('\\', "/");
        self.keep.iter().any(|k| k.eq_ignore_ascii_case(&rel))
    }
}
