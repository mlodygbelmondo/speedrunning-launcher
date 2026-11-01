//! The facade between the UI and the core. It is the only way the UI talks to `srl-core`,
//! `srl-game` and `srl-mods`: the views read a `Snapshot` once, then apply `Event`s, and ask for
//! work through the `Backend` methods.
//!
//! Rules every implementation keeps (see `crates/srl-app/README.md`):
//! - **Never block the UI thread.** Every method returns at once; the work runs elsewhere (the
//!   real backend on its tokio runtime, `FakeBackend` on its own thread).
//! - **Changes arrive as events** on the channel given at creation. The app drains it on the GPUI
//!   executor and redraws once per batch, so the backend may send progress as often as it likes.
//! - **Idle sends nothing.** With nothing installing or running there are no events, so the app
//!   renders no frames at all.
//!
//! `RealBackend` (`real.rs`) is the one the app runs: it drives `pipeline.rs` (what Play does, over
//! `srl-game` and `srl-mods`) on its own tokio runtime. `FakeBackend` simulates installs and a
//! running game for screenshots and UI work (`SRL_FAKE=1`).
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use srl_core::{CustomId, Instance, Mode, Progress};

pub mod account;
pub mod content;
mod fake;
pub mod importer;
pub mod instances;
pub mod pipeline;
pub mod practice;
mod real;
pub mod stats;
mod stats_sample;
pub mod tools;
pub mod update;

pub use account::{AccountProblem, AccountSnapshot, Accounts, AccountsPreset};
pub use fake::{
    FakeBackend, FakeOptions, fake_failure, fake_launch_failure, fake_sign_in_failure,
    fake_sign_in_first,
};
pub use importer::{ImportEvent, ImportPreset, ImportResult, Importer};
pub use instances::{COPY_CANCELLED, CopyProgress, Created, NewInstance};
pub use practice::{PracticeEvent, PracticeMaps, PracticePreset, PracticeSnapshot};
pub use real::RealBackend;
pub use stats::{StatsEvent, StatsService, StatsSettings, StatsView};
pub use stats_sample::StatsPreset;
pub use tools::{ToolId, ToolSnapshot, ToolState, Tools, ToolsPreset};
pub use update::{UpdatePreset, UpdateState, Updates};

/// What a mode is doing, as the Play button shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum ModeState {
    NotInstalled,
    /// `play` is installing or updating what's missing, or starting the game. `Progress::unit`
    /// says what the numbers count.
    Installing(Progress),
    Ready,
    /// Installed, but something in it (usually a mod) has a newer legal version. `play` updates
    /// it first; `ModeInfo::update` names it when known.
    UpdateAvailable,
    Running,
    /// The last `play` stopped. The UI says why in the player's language and offers a retry,
    /// which is `play` again.
    Failed(Failure),
}

/// Why a `play` stopped, so the UI can say it in the player's language instead of showing the
/// core's English error chain as the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailKind {
    /// Installing needed the network and it could not be reached (refused, timed out, no DNS).
    Offline,
    /// Installing failed for another reason: the disk, a bad download, the mod lists.
    Install,
    /// The game process started, then closed or was stopped by Fabric before its title screen,
    /// or could not be started at all.
    Launch,
    /// The active Microsoft account couldn't sign in to Minecraft before the game started (a
    /// sign-in to do again, a locked keyring, an expired sign-in with no network). Nothing was
    /// started; the detail is `FailDetail::Account`.
    SignIn,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub kind: FailKind,
    /// What went wrong, in parts the UI can say in the player's language. `Display` gives the
    /// English sentence for the launcher log and the command line.
    pub detail: FailDetail,
    /// The mode's files were in place when it failed (a launch failure, or an update of an
    /// installed mode), so the mode still counts as installed.
    pub installed: bool,
    /// The game's log, when the game ran.
    pub log: Option<PathBuf>,
}

/// The detail of a failed `play`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailDetail {
    /// The core's error chain, in English (a failed install, a game that could not be started
    /// at all). The UI shows it as technical detail under a translated label.
    Core(String),
    /// The game exited before its title screen: the exit code (`None`: ended by a signal) and
    /// the last meaningful message in its log, as the game wrote it (never log XML).
    Exited {
        code: Option<i32>,
        said: Option<String>,
    },
    /// Fabric Loader stopped the start (a mod refused to load), with the reason it gave.
    Stopped(String),
    /// srl-auth's reason, worded by the UI through its copy key.
    Account(AccountProblem),
}

impl fmt::Display for FailDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FailDetail::Core(s) => f.write_str(s),
            FailDetail::Exited { code, said } => {
                match code {
                    Some(c) => write!(f, "Minecraft exited with code {c} while starting.")?,
                    None => f.write_str("Minecraft was ended by a signal while starting.")?,
                }
                match said {
                    Some(s) => write!(f, " Its last message: {s}"),
                    None => Ok(()),
                }
            }
            FailDetail::Stopped(reason) => write!(f, "Minecraft stopped while starting: {reason}"),
            FailDetail::Account(p) => write!(f, "The Microsoft sign-in failed: {p}"),
        }
    }
}

/// Facts about a mode the UI shows when the backend knows them. `None` hides the item: the UI
/// never shows a number it can't back up.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModeInfo {
    /// Legal mods in the mode's set (srl-mods), once the mod list is known.
    pub mods: Option<u32>,
    /// Bytes the instance and its shared files take on disk, once installed.
    pub disk: Option<u64>,
    /// Bytes `play` still has to download, when known.
    pub download: Option<u64>,
    /// What an update would update ("MCSR Ranked"), for `ModeState::UpdateAvailable`.
    pub update: Option<String>,
    /// What the running game was started with, while it runs. The settings can change in the
    /// meantime; the game screen shows what the game really has.
    pub launched: Option<Launched>,
    /// The version of the Java runtime the installed mode runs on ("21.0.7"), from its install
    /// record. `None` until the mode is installed.
    pub java: Option<String>,
    /// The Mods page: every mod the player can turn on or off in this mode, once the mod lists
    /// are known.
    pub catalogue: Option<Vec<srl_mods::Available>>,
    /// What makes a run with the mods Play would install not legal for the mode (SetSpawn in
    /// RSG). The game still starts; the Mods page and Play say it.
    pub concerns: Vec<srl_mods::Concern>,
}

/// What a mode needs from the game side, the same on every machine (srl-game's profile).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeNeeds {
    /// The Java feature version the launcher installs for the mode (21, 25).
    pub java_major: u32,
    /// The least `-Xmx` the mode runs with, in MiB, whatever the setting says.
    pub min_memory_mb: u32,
}

pub fn mode_needs(mode: Mode) -> ModeNeeds {
    let p = srl_game::profile(mode);
    ModeNeeds {
        java_major: p.java_major,
        min_memory_mb: p.min_memory_mb,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Launched {
    pub player: String,
    pub memory_mb: u32,
    /// The tools running with this game: started for it, injected into it (Toolscreen), wrapping
    /// it (waywall), or a Ninjabrain Bot the player had already started.
    pub tools: Vec<ToolId>,
    /// What the player should know about the tools of this launch: one left out and why, one
    /// that was already running. Worded with `ToolNote::copy`.
    pub tool_notes: Vec<srl_tools::ToolNote>,
    /// The game plays as a Microsoft account (`player` is its name), not the offline name.
    pub microsoft: bool,
    /// The Microsoft sign-in couldn't be renewed before the start (no network); the token the
    /// game got works until then. The game screen says so.
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    En,
    Pl,
}

impl Language {
    /// In the order the language switch shows them.
    pub const ALL: [Language; 2] = [Language::En, Language::Pl];

    pub fn code(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Pl => "pl",
        }
    }

    pub fn from_code(code: &str) -> Option<Language> {
        Language::ALL
            .into_iter()
            .find(|l| l.code() == code.trim().to_ascii_lowercase())
    }
}

/// The user's settings. The backend owns and persists them; the UI changes them only through
/// `Backend::set_settings`. Every field has a default, so a file from an older launcher (the
/// name, the language, `max_memory_mb` and the last mode) loads unchanged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The offline player name (`OfflineAccount::is_valid_name`). `None` until the first-run
    /// prompt has asked for it.
    pub player_name: Option<String>,
    pub language: Language,
    /// The game's `-Xmx` in MiB for every mode without its own value in `mode_memory_mb`. It was
    /// the only memory setting before memory per mode, so older files keep their meaning.
    pub max_memory_mb: u32,
    /// `-Xmx` in MiB for the modes the player set one for. Read it through `memory_mb`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub mode_memory_mb: BTreeMap<Mode, u32>,
    /// The mode selected when the launcher was last used, selected again on start.
    pub last_mode: Option<Mode>,
    /// Opt-in accessibility mods (`srl_mods::ResolveOptions::accessibility`: Extra Options).
    pub accessibility_mods: bool,
    /// Draftout's optional extras (`ResolveOptions::draftout_extras`: No Chat Restrictions and
    /// No Telemetry). Off by default (Piotr, 2026-10-26).
    pub draftout_extras: bool,
    /// The modes each tool starts with, for the tools the player changed. Read it through
    /// `starts_with`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub tool_autostart: BTreeMap<ToolId, BTreeSet<Mode>>,
    /// The names the Stats page looks the player up by on MCSR Ranked and Draftout (not the
    /// offline name). Read them through `stats::lookup_name`.
    #[serde(skip_serializing_if = "stats_unset")]
    pub stats: StatsSettings,
    /// The first start offered "Import my current settings" and the player imported or skipped
    /// it: it isn't offered again (Settings still has it).
    pub import_offered: bool,
    /// The player's changes to each mode's mods (the Mods page). `on` may name a jar of
    /// `custom_jars` by its `CustomJar::modid`. Read them through `mod_rules`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub mod_choices: BTreeMap<Mode, srl_mods::ModChoice>,
    /// The jars the player added (copied into the content store), for any mode to turn on.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub custom_jars: Vec<srl_mods::CustomJar>,
    /// Temporary, for development: hide every tagline and subtitle (`copy::is_tagline`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dev_hide_taglines: bool,
    /// Look for a new version of the launcher on start, and install it (`update.rs`). On by
    /// default.
    pub auto_update: bool,
    /// The instances the player created (the Instances page), in the order they were created.
    /// The backend owns which exist: `Backend::create_instance`, `duplicate_instance` and
    /// `delete_instance` add and remove them, and `set_settings` only changes the ones that
    /// exist (their name, memory, mods and JVM arguments; never their base mode).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub custom_instances: Vec<CustomInstance>,
}

/// A custom instance: a game directory of its own (`instances/custom-<n>`) that plays like its
/// base mode. The base gives it its Minecraft version, Java, Fabric Loader floor, legal mod set
/// and tool rules; the instance adds its own mod changes, memory and JVM arguments. It never
/// touches the shared 1.16.1 directory or its per-mode Atum settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomInstance {
    pub id: CustomId,
    /// What the player calls it: 1 to `MAX_INSTANCE_NAME` characters after trimming, unique
    /// among the custom instances ignoring case (`Settings::validate_instance_name`).
    pub name: String,
    /// The mode it plays like. Fixed once created: a world opened in another Minecraft version
    /// is upgraded for good, so changing the base would risk the instance's worlds.
    pub base: Mode,
    /// Its own `-Xmx` in MiB; `None` follows the base mode's setting (`Settings::memory_mb`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mb: Option<u32>,
    /// The player's changes to the base mode's set, as in `Settings::mod_choices` (`on` may
    /// name a jar of `custom_jars` by its `CustomJar::modid`).
    #[serde(default, skip_serializing_if = "srl_mods::ModChoice::is_empty")]
    pub mods: srl_mods::ModChoice,
    /// JVM arguments added at launch, right before the main class (after the version's own,
    /// so they win). Checked by `validate_jvm_args`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jvm_args: Vec<String>,
    /// When it was created, in milliseconds since the Unix epoch.
    #[serde(default)]
    pub created_ms: i64,
}

impl CustomInstance {
    /// The instance it is, for the pipeline and `Paths::game_dir`.
    pub fn instance(&self) -> Instance {
        Instance::Custom(self.id)
    }
}

/// The longest custom instance name, in characters.
pub const MAX_INSTANCE_NAME: usize = 40;

/// Why a custom instance name can't be used. `Display` gives the English sentence for the log;
/// the UI words it in the player's language by the variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameProblem {
    /// Nothing but spaces.
    Empty,
    /// More than `MAX_INSTANCE_NAME` characters.
    TooLong,
    /// A line break, tab or other control character.
    Control,
    /// Another custom instance has it (ignoring case).
    Taken,
}

impl fmt::Display for NameProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            NameProblem::Empty => "the name is empty",
            NameProblem::TooLong => "the name is longer than 40 characters",
            NameProblem::Control => "the name has a control character",
            NameProblem::Taken => "another instance has that name",
        })
    }
}

/// Why a custom instance's JVM argument is refused. `Display` gives the English sentence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JvmArgProblem {
    /// An empty (or all-space) argument.
    Empty,
    /// Not an option (no leading `-`): before the main class it would be taken as the class.
    NotAnOption(String),
    /// `-Xmx` or `-Xms`: memory comes from the instance's memory setting.
    Memory(String),
    /// `-cp`, `-classpath`, `-jar` and the like: they would break the launch command.
    Command(String),
    /// A Java agent changes the game like a mod, outside srl-mods' legal checks.
    Agent(String),
}

impl fmt::Display for JvmArgProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JvmArgProblem::Empty => f.write_str("an argument is empty"),
            JvmArgProblem::NotAnOption(a) => write!(f, "{a:?} is not a JVM option (no leading -)"),
            JvmArgProblem::Memory(a) => {
                write!(f, "{a:?} sets memory, which the memory setting does")
            }
            JvmArgProblem::Command(a) => write!(f, "{a:?} would break the launch command"),
            JvmArgProblem::Agent(a) => write!(f, "{a:?} loads a Java agent, which is not allowed"),
        }
    }
}

/// Checks a custom instance's JVM arguments: each one a single non-empty option. Memory comes
/// from the memory setting; options that change the command's shape (`-cp`, `-jar`) or load an
/// agent (which would change the game outside srl-mods' legal checks) are refused.
pub fn validate_jvm_args(args: &[String]) -> Result<(), JvmArgProblem> {
    for a in args {
        let t = a.trim();
        if t.is_empty() {
            return Err(JvmArgProblem::Empty);
        }
        if !t.starts_with('-') || t != a || t.chars().any(char::is_control) {
            return Err(JvmArgProblem::NotAnOption(a.clone()));
        }
        let lower = t.to_ascii_lowercase();
        if lower.starts_with("-xmx") || lower.starts_with("-xms") {
            return Err(JvmArgProblem::Memory(a.clone()));
        }
        let command = [
            "-cp",
            "-classpath",
            "--class-path",
            "-jar",
            "-version",
            "--version",
        ];
        if command.contains(&lower.as_str())
            || lower.starts_with("--class-path=")
            || lower.starts_with("-d64")
        {
            return Err(JvmArgProblem::Command(a.clone()));
        }
        if lower.starts_with("-javaagent") || lower.starts_with("-agent") {
            return Err(JvmArgProblem::Agent(a.clone()));
        }
    }
    Ok(())
}

/// Everything that decides a mode's mods besides the mod lists: the optional mods of Settings
/// and the player's changes on the Mods page. Play and the update check resolve with it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModRules {
    pub opts: srl_mods::ResolveOptions,
    choices: BTreeMap<Mode, srl_mods::ModChoice>,
    /// Each custom instance's base mode and its changes, their own jars filled in.
    customs: BTreeMap<CustomId, (Mode, srl_mods::ModChoice)>,
}

impl ModRules {
    /// The player's changes to `mode`'s set, their own jars filled in.
    pub fn choice(&self, mode: Mode) -> srl_mods::ModChoice {
        self.choices.get(&mode).cloned().unwrap_or_default()
    }

    /// The changes to any instance's set: a mode's (`choice`), a custom instance's own; the
    /// practice instance has none (its list is fixed).
    pub fn choice_of(&self, instance: Instance) -> srl_mods::ModChoice {
        match instance {
            Instance::Mode(mode) => self.choice(mode),
            Instance::Practice => srl_mods::ModChoice::default(),
            Instance::Custom(id) => self
                .customs
                .get(&id)
                .map(|(_, c)| c.clone())
                .unwrap_or_default(),
        }
    }

    /// The mode whose set an instance starts from: a mode's own, a custom instance's base.
    /// `None` for the practice instance (an explicit list) and for a custom instance that no
    /// longer exists.
    pub fn base(&self, instance: Instance) -> Option<Mode> {
        match instance {
            Instance::Mode(mode) => Some(mode),
            Instance::Practice => None,
            Instance::Custom(id) => self.customs.get(&id).map(|(base, _)| *base),
        }
    }
}

/// Turns `modid` on or off in a set of changes. `in_set`: the mod is in the set the changes
/// apply to (so off is the change), else on is.
fn toggle(c: &mut srl_mods::ModChoice, modid: &str, on: bool, in_set: bool) {
    if in_set {
        c.on.remove(modid);
        if on {
            c.off.remove(modid);
        } else {
            c.off.insert(modid.to_string());
        }
    } else {
        c.off.remove(modid);
        if on {
            c.on.insert(modid.to_string());
        } else {
            c.on.remove(modid);
        }
    }
}

fn stats_unset(s: &StatsSettings) -> bool {
    *s == StatsSettings::default()
}

/// The `-Xmx` choices the Settings page offers, in MiB.
pub const MEMORY_CHOICES_MB: [u32; 4] = [2048, 3072, 4096, 6144];

/// The Settings choice closest to `mb` (the smaller one on a tie).
pub fn nearest_memory_choice(mb: u32) -> u32 {
    MEMORY_CHOICES_MB
        .into_iter()
        .min_by_key(|c| c.abs_diff(mb))
        .unwrap_or(MEMORY_CHOICES_MB[0])
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            player_name: None,
            language: Language::En,
            // REDESIGN/SCREENS: "3 GB is recommended".
            max_memory_mb: 3072,
            mode_memory_mb: BTreeMap::new(),
            last_mode: None,
            accessibility_mods: false,
            draftout_extras: false,
            tool_autostart: BTreeMap::new(),
            stats: StatsSettings::default(),
            import_offered: false,
            mod_choices: BTreeMap::new(),
            custom_jars: Vec::new(),
            dev_hide_taglines: false,
            auto_update: true,
            custom_instances: Vec::new(),
        }
    }
}

impl Settings {
    /// The `-Xmx` the mode's game starts with, in MiB: its own value, else the common one, and
    /// never below what the mode needs (Draftout: 3 GB).
    pub fn memory_mb(&self, mode: Mode) -> u32 {
        self.mode_memory_mb
            .get(&mode)
            .copied()
            .unwrap_or(self.max_memory_mb)
            .max(mode_needs(mode).min_memory_mb)
    }

    /// Gives the mode its own `-Xmx`.
    pub fn set_memory_mb(&mut self, mode: Mode, mb: u32) {
        self.mode_memory_mb.insert(mode, mb);
    }

    /// What srl-mods adds on top of every mode's legal set.
    pub fn resolve_options(&self) -> srl_mods::ResolveOptions {
        srl_mods::ResolveOptions {
            accessibility: self.accessibility_mods,
            draftout_extras: self.draftout_extras,
        }
    }

    /// The optional mods and every mode's and custom instance's changes from the Mods page. A
    /// custom id in `on` whose jar is gone from `custom_jars` is ignored.
    pub fn mod_rules(&self) -> ModRules {
        let choices = self
            .mod_choices
            .iter()
            .map(|(mode, c)| (*mode, self.with_jars(c)))
            .filter(|(_, c)| !c.is_empty())
            .collect();
        let customs = self
            .custom_instances
            .iter()
            .map(|c| (c.id, (c.base, self.with_jars(&c.mods))))
            .collect();
        ModRules {
            opts: self.resolve_options(),
            choices,
            customs,
        }
    }

    /// `c` as srl-mods takes it: the `custom:` ids in `on` replaced by their jars.
    fn with_jars(&self, c: &srl_mods::ModChoice) -> srl_mods::ModChoice {
        let custom = self
            .custom_jars
            .iter()
            .filter(|j| c.on.contains(&j.modid()))
            .cloned()
            .collect();
        let on =
            c.on.iter()
                .filter(|id| !id.starts_with("custom:"))
                .cloned()
                .collect();
        srl_mods::ModChoice {
            on,
            off: c.off.clone(),
            custom,
        }
    }

    /// Turns a mod of the Mods page on or off in `mode`. `in_mode`: the mod is in the mode's own
    /// set (so off is the change), else on is.
    pub fn set_mod(&mut self, mode: Mode, modid: &str, on: bool, in_mode: bool) {
        let c = self.mod_choices.entry(mode).or_default();
        toggle(c, modid, on, in_mode);
        if c.is_empty() {
            self.mod_choices.remove(&mode);
        }
    }

    /// [`Settings::set_mod`] for a custom instance's Mods tab. `in_base`: the mod is in the base
    /// mode's own set (`Available::in_mode` of its catalogue). Nothing for an unknown id.
    pub fn set_custom_mod(&mut self, id: CustomId, modid: &str, on: bool, in_base: bool) {
        if let Some(c) = self.custom_mut(id) {
            toggle(&mut c.mods, modid, on, in_base);
        }
    }

    /// Adds the player's jar to the library (once) and turns it on in a custom instance.
    pub fn add_custom_jar_to(&mut self, id: CustomId, jar: srl_mods::CustomJar) {
        let modid = jar.modid();
        if !self.custom_jars.iter().any(|j| j.modid() == modid) {
            self.custom_jars.push(jar);
        }
        self.set_custom_mod(id, &modid, true, false);
    }

    /// The custom instance with this id.
    pub fn custom(&self, id: CustomId) -> Option<&CustomInstance> {
        self.custom_instances.iter().find(|c| c.id == id)
    }

    /// The custom instance with this id, to change it (then `Backend::set_settings`).
    pub fn custom_mut(&mut self, id: CustomId) -> Option<&mut CustomInstance> {
        self.custom_instances.iter_mut().find(|c| c.id == id)
    }

    /// Renames a custom instance, if the name is valid (trimmed first).
    pub fn rename_custom(&mut self, id: CustomId, name: &str) -> Result<(), NameProblem> {
        let name = self.validate_instance_name(name, Some(id))?;
        if let Some(c) = self.custom_mut(id) {
            c.name = name;
        }
        Ok(())
    }

    /// The number a new custom instance gets: one more than the highest in use (1 for the
    /// first). The backend also skips numbers whose directory is still on disk.
    pub fn next_custom_id(&self) -> CustomId {
        CustomId(
            self.custom_instances
                .iter()
                .map(|c| c.id.0)
                .max()
                .map_or(1, |n| n.saturating_add(1)),
        )
    }

    /// `name` trimmed, if it can name a custom instance: 1 to `MAX_INSTANCE_NAME` characters, no
    /// control characters, and no other custom instance (`except` is the one being renamed)
    /// with the same name ignoring case.
    pub fn validate_instance_name(
        &self,
        name: &str,
        except: Option<CustomId>,
    ) -> Result<String, NameProblem> {
        let name = name.trim();
        if name.is_empty() {
            return Err(NameProblem::Empty);
        }
        if name.chars().count() > MAX_INSTANCE_NAME {
            return Err(NameProblem::TooLong);
        }
        if name.chars().any(char::is_control) {
            return Err(NameProblem::Control);
        }
        let lower = name.to_lowercase();
        if self
            .custom_instances
            .iter()
            .any(|c| Some(c.id) != except && c.name.trim().to_lowercase() == lower)
        {
            return Err(NameProblem::Taken);
        }
        Ok(name.to_string())
    }

    /// `wanted` if no custom instance has it, else `wanted 2`, `wanted 3`… (cut to fit
    /// `MAX_INSTANCE_NAME`). For a duplicate whose name was taken while it copied.
    pub fn unique_instance_name(&self, wanted: &str) -> String {
        let wanted: String = wanted.trim().chars().take(MAX_INSTANCE_NAME).collect();
        if self.validate_instance_name(&wanted, None).is_ok() {
            return wanted;
        }
        (2u32..)
            .map(|n| {
                let tail = format!(" {n}");
                let head: String = wanted
                    .chars()
                    .take(MAX_INSTANCE_NAME - tail.chars().count())
                    .collect();
                format!("{}{tail}", head.trim_end())
            })
            .find(|n| self.validate_instance_name(n, None).is_ok())
            .expect("an unused name")
    }

    /// The mode whose rules an instance's game follows (its tools, its memory fallback, its
    /// game): a mode's own, practice's `practice::TOOLS_LIKE`, a custom instance's base (RSG for
    /// one that no longer exists).
    pub fn rules_mode(&self, instance: Instance) -> Mode {
        match instance {
            Instance::Mode(mode) => mode,
            Instance::Practice => practice::TOOLS_LIKE,
            Instance::Custom(id) => self.custom(id).map_or(practice::TOOLS_LIKE, |c| c.base),
        }
    }

    /// The `-Xmx` any instance's game starts with, in MiB: a mode's `memory_mb`, practice's
    /// like RSG's, a custom instance's own value else its base mode's; never below what the
    /// game needs.
    pub fn memory_mb_of(&self, instance: Instance) -> u32 {
        let base = self.rules_mode(instance);
        match instance.custom().and_then(|id| self.custom(id)) {
            Some(CustomInstance {
                memory_mb: Some(mb),
                ..
            }) => (*mb).max(mode_needs(base).min_memory_mb),
            _ => self.memory_mb(base),
        }
    }

    /// The JVM arguments the instance's game gets on top of the version's: a custom instance's
    /// own, none for the others.
    pub fn jvm_args_of(&self, instance: Instance) -> Vec<String> {
        instance
            .custom()
            .and_then(|id| self.custom(id))
            .map(|c| c.jvm_args.clone())
            .unwrap_or_default()
    }

    /// The tools any instance's game starts with: those of the mode whose rules it follows
    /// (`rules_mode`).
    pub fn tool_selection_of(&self, instance: Instance) -> srl_tools::ToolSelection {
        self.tool_selection(self.rules_mode(instance))
    }

    /// `incoming`'s custom instances, made to fit `self`'s (the backend's) list: the same
    /// instances in the same order (none added, none removed: only the backend's lifecycle
    /// methods do that), each with `incoming`'s name, memory, mods and JVM arguments where they
    /// are valid, and its own base. Returns whether anything of `incoming` had to change, so
    /// the backend can send the UI the list it kept (`Event::CustomInstances`).
    pub fn adopt_customs(&self, incoming: &mut Settings) -> bool {
        let mut kept = Settings::default();
        for current in &self.custom_instances {
            let mut next = current.clone();
            if let Some(new) = incoming.custom(current.id) {
                if let Ok(name) = kept.validate_instance_name(&new.name, None) {
                    next.name = name;
                }
                next.memory_mb = new.memory_mb.map(nearest_memory_choice);
                next.mods = new.mods.clone();
                if validate_jvm_args(&new.jvm_args).is_ok() {
                    next.jvm_args = new.jvm_args.clone();
                }
            }
            kept.custom_instances.push(next);
        }
        let changed = incoming.custom_instances != kept.custom_instances;
        incoming.custom_instances = kept.custom_instances;
        changed
    }

    /// Whether the player changed anything in `mode`'s mods.
    pub fn mods_changed(&self, mode: Mode) -> bool {
        self.mod_choices.get(&mode).is_some_and(|c| !c.is_empty())
    }

    /// Back to the mode's own set.
    pub fn reset_mods(&mut self, mode: Mode) {
        self.mod_choices.remove(&mode);
    }

    /// Adds the player's jar to the library (once) and turns it on in `mode`.
    pub fn add_custom_jar(&mut self, mode: Mode, jar: srl_mods::CustomJar) {
        let id = jar.modid();
        if !self.custom_jars.iter().any(|j| j.modid() == id) {
            self.custom_jars.push(jar);
        }
        self.set_mod(mode, &id, true, false);
    }

    /// Whether `tool` starts with `mode`'s game (once it is installed). Until the player changes
    /// it, srl-tools' defaults apply (`ToolSelection::defaults`: Ninjabrain Bot with the 1.16.1
    /// modes, Toolscreen with them on Windows, waywall off). A tool the mode's rules ban, or one
    /// this system can't run, never starts (Ninjabrain Bot in Draftout).
    pub fn starts_with(&self, tool: ToolId, mode: Mode) -> bool {
        let os = srl_tools::current_os();
        if srl_tools::availability(tool, mode, os).is_err() {
            return false;
        }
        match self.tool_autostart.get(&tool) {
            Some(modes) => modes.contains(&mode),
            None => srl_tools::ToolSelection::defaults(mode, os).has(tool),
        }
    }

    /// Turns `tool` on or off for `mode`.
    pub fn set_starts_with(&mut self, tool: ToolId, mode: Mode, on: bool) {
        let mut modes: BTreeSet<Mode> = Mode::ALL
            .into_iter()
            .filter(|m| self.starts_with(tool, *m))
            .collect();
        let allowed = srl_tools::availability(tool, mode, srl_tools::current_os()).is_ok();
        if on && allowed {
            modes.insert(mode);
        } else {
            modes.remove(&mode);
        }
        self.tool_autostart.insert(tool, modes);
    }

    /// The tools `mode`'s game starts with, for srl-tools' launch hook.
    pub fn tool_selection(&self, mode: Mode) -> srl_tools::ToolSelection {
        srl_tools::ToolSelection(
            ToolId::ALL
                .into_iter()
                .filter(|t| self.starts_with(*t, mode))
                .collect(),
        )
    }

    /// `<Paths root>/settings.json`, the one place both backends keep the settings.
    pub fn file(paths: &srl_core::Paths) -> PathBuf {
        paths.root().join("settings.json")
    }

    /// The saved settings, or `None` when there are none (or they don't parse, which is logged:
    /// a broken file must not keep the launcher from starting). An invalid saved name is dropped,
    /// so the first-run prompt asks again, and a memory value Settings doesn't offer becomes the
    /// nearest one it does (an edited `"max_memory_mb": 1` would stop the JVM at once, and
    /// Settings would show no choice selected).
    pub fn load(path: &Path) -> Option<Settings> {
        let text = std::fs::read_to_string(path).ok()?;
        match serde_json::from_str::<Settings>(&text) {
            Ok(mut s) => {
                if s.player_name
                    .as_deref()
                    .is_some_and(|n| !srl_core::OfflineAccount::is_valid_name(n))
                {
                    s.player_name = None;
                }
                s.max_memory_mb = nearest_memory_choice(s.max_memory_mb);
                for mb in s.mode_memory_mb.values_mut() {
                    *mb = nearest_memory_choice(*mb);
                }
                // An edited file with a name that can't be looked up: ask again instead of
                // sending it to the APIs (they would refuse it anyway).
                for name in [&mut s.stats.ranked_nickname, &mut s.stats.draftout_username] {
                    if name
                        .as_deref()
                        .is_some_and(|n| !srl_stats::is_lookup_name(n))
                    {
                        *name = None;
                    }
                }
                s.custom_instances = sane_customs(std::mem::take(&mut s.custom_instances));
                Some(s)
            }
            Err(e) => {
                eprintln!("ignoring {}: {e}", path.display());
                None
            }
        }
    }

    /// Writes the settings atomically (a temporary file, then a rename), so a crash mid-write
    /// leaves the previous file intact.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.part");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, path)
    }
}

/// An edited settings file's custom instances made usable: one definition per id (the first),
/// every name valid and unique (a bad one becomes `Instance <n>`), memory on a Settings choice,
/// and JVM arguments that pass `validate_jvm_args` (else none, logged).
fn sane_customs(list: Vec<CustomInstance>) -> Vec<CustomInstance> {
    let mut kept = Settings::default();
    for mut c in list {
        if kept.custom(c.id).is_some() {
            eprintln!("settings: dropping a second custom instance {}", c.id);
            continue;
        }
        c.name = match kept.validate_instance_name(&c.name, None) {
            Ok(name) => name,
            Err(_) => kept.unique_instance_name(&format!("Instance {}", c.id.0)),
        };
        c.memory_mb = c.memory_mb.map(nearest_memory_choice);
        if let Err(e) = validate_jvm_args(&c.jvm_args) {
            eprintln!("settings: {}: dropping its JVM arguments: {e}", c.id);
            c.jvm_args.clear();
        }
        kept.custom_instances.push(c);
    }
    kept.custom_instances
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModeSnapshot {
    pub state: ModeState,
    pub info: ModeInfo,
}

/// Everything the UI needs at start, read once.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// One entry per mode, in `Mode::ALL` order.
    pub modes: [ModeSnapshot; 5],
    /// One entry per custom instance of `settings.custom_instances`: its state and facts, as
    /// for a mode (the catalogue and concerns against its base mode's set).
    pub customs: BTreeMap<CustomId, ModeSnapshot>,
    pub settings: Settings,
    /// The data folder (`Paths::root`): games, mods, Java, logs and the settings file.
    pub data_dir: PathBuf,
    /// The data folder can't be written (a read-only or missing drive, a bad `SRL_HOME`): the
    /// folder and the error. Nothing can be installed or saved, so the UI shows only this.
    pub data_error: Option<String>,
}

impl Snapshot {
    /// The state and facts of a mode or a custom instance (`None` for practice, which has its
    /// own snapshot, and for an unknown custom id).
    pub fn instance(&self, instance: Instance) -> Option<&ModeSnapshot> {
        match instance {
            Instance::Mode(mode) => Some(&self.modes[index(mode)]),
            Instance::Practice => None,
            Instance::Custom(id) => self.customs.get(&id),
        }
    }

    pub(crate) fn instance_mut(&mut self, instance: Instance) -> Option<&mut ModeSnapshot> {
        match instance {
            Instance::Mode(mode) => Some(&mut self.modes[index(mode)]),
            Instance::Practice => None,
            Instance::Custom(id) => self.customs.get_mut(&id),
        }
    }
}

/// The event that tells the UI an instance's new state: `State` for a mode, `CustomState` for a
/// custom instance (practice reports through `PracticeEvent`).
pub(crate) fn state_event(instance: Instance, state: ModeState) -> Option<Event> {
    match instance {
        Instance::Mode(mode) => Some(Event::State(mode, state)),
        Instance::Custom(id) => Some(Event::CustomState(id, state)),
        Instance::Practice => None,
    }
}

/// [`state_event`] for an instance's facts.
pub(crate) fn info_event(instance: Instance, info: ModeInfo) -> Option<Event> {
    match instance {
        Instance::Mode(mode) => Some(Event::Info(mode, info)),
        Instance::Custom(id) => Some(Event::CustomInfo(id, info)),
        Instance::Practice => None,
    }
}

/// A change, sent by the backend from any thread.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    State(Mode, ModeState),
    Info(Mode, ModeInfo),
    /// A tool changed (`Tools`): its whole new snapshot.
    Tool(ToolSnapshot),
    /// A stats answer (`StatsService`).
    Stats(StatsEvent),
    /// The Microsoft accounts changed (`Accounts`): the whole new snapshot (boxed: it is the
    /// biggest event by far, and rare).
    Account(Box<AccountSnapshot>),
    /// A practice map, the practice game, or a question to ask (`PracticeMaps`).
    Practice(PracticeEvent),
    /// The settings import: instances found, a picked folder, progress, the result.
    Import(ImportEvent),
    /// A jar the player picked on the Mods page is in the store (`Backend::add_jar`), or why
    /// not.
    JarAdded(Mode, Result<srl_mods::CustomJar, String>),
    /// The launcher's own update (`Updates`): its new state.
    Update(UpdateState),
    /// A custom instance's state, as `State` is a mode's.
    CustomState(CustomId, ModeState),
    /// A custom instance's facts, as `Info` is a mode's.
    CustomInfo(CustomId, ModeInfo),
    /// The backend's list of custom instances changed (one created, duplicated or deleted), or
    /// `set_settings` had to keep something of it (an invalid name, a changed base): the UI
    /// puts this list into its copy of the settings as is.
    CustomInstances(Vec<CustomInstance>),
    /// A `create_instance` or `duplicate_instance` ended: the new instance (already in the list
    /// sent just before, with its `CustomState`) and the files a duplicate had to leave out, or
    /// why it failed (nothing is left behind then).
    InstanceCreated(Result<Created, String>),
    /// A `duplicate_instance` copying, at most every 50 ms.
    InstanceCopy(CopyProgress),
    /// A `delete_instance` ended: the instance is gone from the list (sent just before) and its
    /// directory is in `trash/`, or why not (it is running or installing, or the directory
    /// couldn't be moved; nothing changed then).
    InstanceDeleted(Result<CustomId, String>),
    /// A jar the player picked on a custom instance's Mods tab is in the store
    /// (`Backend::add_instance_jar`), or why not; the UI then turns it on with
    /// `Settings::add_custom_jar_to`.
    InstanceJarAdded(CustomId, Result<srl_mods::CustomJar, String>),
}

/// Where a backend sends its events. Unbounded: the UI drains it every frame it wakes up for.
pub type Events = futures::channel::mpsc::UnboundedSender<Event>;

pub trait Backend {
    /// The current state of every mode and the settings. Cheap: no I/O.
    fn snapshot(&self) -> Snapshot;
    /// Installs or updates whatever the mode is missing, then starts the game. Progress arrives
    /// as `ModeState::Installing`, then `Running`, then `Ready` once the game exits, or `Failed`.
    /// Does nothing while the mode is already installing or running.
    fn play(&self, mode: Mode);
    /// Stops an install (the mode goes back to what it was) or the running game.
    fn cancel(&self, mode: Mode);
    /// Stores new settings. They apply to the next `play`: the name, each mode's memory, the
    /// optional mods (`Settings::resolve_options`) and the tools that start with the game.
    fn set_settings(&self, settings: Settings);
    /// Opens the data folder (`Snapshot::data_dir`) in the system's file manager.
    fn open_data_folder(&self);
    /// Opens the folder `path` is in (a game log's, an import backup's) in the file manager.
    fn open_containing_folder(&self, path: &Path);
    /// The tools extension: Ninjabrain Bot, Toolscreen and waywall, their state and installs.
    fn tools(&self) -> &dyn Tools;
    /// The stats extension: SpeedRunIGT records, MCSR Ranked and Draftout.
    fn stats(&self) -> &dyn StatsService;
    /// The Microsoft accounts: the list, the login dialog, who Play uses.
    fn account(&self) -> &dyn Accounts;
    /// The practice extension: the maps and the practice instance's game.
    fn practice(&self) -> &dyn PracticeMaps;
    /// The settings import from other launchers (srl-import).
    fn import(&self) -> &dyn Importer;
    /// The launcher's own updates (srl-update).
    fn updates(&self) -> &dyn Updates;
    /// Copies the player's jar at `path` into the content store for `mode`'s Mods page; the
    /// answer comes as `Event::JarAdded` (the UI then turns it on in its settings).
    fn add_jar(&self, mode: Mode, path: PathBuf);
    /// `play` for a custom instance: installs its base mode's game and its own mod set into its
    /// directory, then starts it. Progress comes as `Event::CustomState`.
    fn play_custom(&self, id: CustomId);
    /// `cancel` for a custom instance.
    fn cancel_custom(&self, id: CustomId);
    /// Creates a custom instance: its definition in the settings and its empty directory,
    /// optionally with another instance's settings (options, `config/`, resource packs; never
    /// worlds or mods). Answers with `Event::CustomInstances`, `CustomState` (NotInstalled) and
    /// `InstanceCreated`. A name `Settings::validate_instance_name` refuses fails at once.
    fn create_instance(&self, spec: NewInstance);
    /// Copies an instance's whole game directory (but `mods/`, logs and the launcher's own
    /// files; `saves/` only `with_worlds`) into a new custom instance with the same base mode,
    /// memory, mods and JVM arguments, in the background with `Event::InstanceCopy` progress.
    /// A file that can't be read is left out and named in the result. Ends with
    /// `InstanceCreated`, as `create_instance` does.
    fn duplicate_instance(&self, from: Instance, name: String, with_worlds: bool);
    /// Removes a custom instance: refused while it installs, runs or is being copied; else its
    /// definition goes and its directory moves to `<data>/trash/custom-<n>-<unix time>` (a
    /// rename, nothing deleted). Answers with `Event::CustomInstances` and `InstanceDeleted`.
    fn delete_instance(&self, id: CustomId);
    /// Stops the running `duplicate_instance`s: what they copied is removed and each ends with
    /// `InstanceCreated(Err(COPY_CANCELLED))`.
    fn cancel_copy(&self);
    /// `add_jar` for a custom instance's Mods tab: `Event::InstanceJarAdded`.
    fn add_instance_jar(&self, id: CustomId, path: PathBuf);
    /// Opens an instance's game directory (`Paths::game_dir`) in the file manager.
    fn open_game_folder(&self, instance: Instance);
}

/// Opens `dir` in the file manager (Explorer, Finder, or what `xdg-open` picks), without waiting
/// for it. A failure is only logged.
pub fn open_folder(dir: &Path) {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::fs::create_dir_all(dir);
    match std::process::Command::new(program)
        .arg(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        // Reaped on a thread of its own, so it doesn't stay behind as a zombie.
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(e) => eprintln!("could not open {} with {program}: {e}", dir.display()),
    }
}

/// Opens `path` (a file: a config, a log, a screenshot) in the app the system picks for it,
/// without waiting. Unlike `open_folder`, creates nothing. A failure is only logged.
pub fn open_path(path: &Path) {
    let (program, args): (&str, &[&str]) = if cfg!(windows) {
        // `start` is a cmd built-in; the empty title keeps a quoted path from becoming the title.
        ("cmd", &["/C", "start", ""])
    } else if cfg!(target_os = "macos") {
        ("open", &[])
    } else {
        ("xdg-open", &[])
    };
    let mut cmd = std::process::Command::new(program);
    cmd.args(args)
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashes up for cmd.
        cmd.creation_flags(0x0800_0000);
    }
    match cmd.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(e) => eprintln!("could not open {} with {program}: {e}", path.display()),
    }
}

/// Index of `mode` in `Mode::ALL` (and in `Snapshot::modes`).
pub fn index(mode: Mode) -> usize {
    Mode::ALL.iter().position(|m| *m == mode).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_memory_snaps_to_a_choice() {
        assert_eq!(nearest_memory_choice(1), 2048);
        assert_eq!(nearest_memory_choice(3000), 3072);
        assert_eq!(nearest_memory_choice(2560), 2048);
        assert_eq!(nearest_memory_choice(100_000), 6144);
        let dir = std::env::temp_dir().join(format!("srl-app-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        std::fs::write(
            &file,
            r#"{"player_name": "Violet_Runner", "max_memory_mb": 1}"#,
        )
        .unwrap();
        let s = Settings::load(&file).unwrap();
        assert_eq!(s.max_memory_mb, 2048);
        assert_eq!(s.player_name.as_deref(), Some("Violet_Runner"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A milestone 1 file loads with the new settings at their defaults; memory per mode falls
    /// back to its one common value, raised to the mode's floor.
    #[test]
    fn older_settings_files_still_load() {
        let s: Settings = serde_json::from_str(
            r#"{"player_name":"VioletRunner","language":"pl","max_memory_mb":2048,"last_mode":"fsg"}"#,
        )
        .unwrap();
        assert_eq!(s.language, Language::Pl);
        assert_eq!(s.last_mode, Some(Mode::Fsg));
        assert!(!s.accessibility_mods && !s.draftout_extras);
        assert_eq!(s.memory_mb(Mode::Rsg), 2048);
        assert_eq!(s.memory_mb(Mode::Draftout), 3072);
        // Written and read back unchanged; the empty maps stay out of the file.
        let text = serde_json::to_string(&s).unwrap();
        assert!(
            !text.contains("mode_memory_mb")
                && !text.contains("tool_autostart")
                && !text.contains("stats")
        );
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), s);
    }

    fn custom(id: u32, name: &str, base: Mode) -> CustomInstance {
        CustomInstance {
            id: CustomId(id),
            name: name.into(),
            base,
            memory_mb: None,
            mods: srl_mods::ModChoice::default(),
            jvm_args: Vec::new(),
            created_ms: 0,
        }
    }

    /// Files without custom instances load and save as before; a custom instance round-trips
    /// with only the fields it uses.
    #[test]
    fn custom_instances_in_the_settings_file() {
        let s: Settings = serde_json::from_str(r#"{"player_name":"VioletRunner"}"#).unwrap();
        assert!(s.custom_instances.is_empty());
        assert!(
            !serde_json::to_string(&s)
                .unwrap()
                .contains("custom_instances")
        );
        let mut s = Settings::default();
        s.custom_instances.push(custom(1, "Seeds", Mode::Ssg));
        let text = serde_json::to_string(&s).unwrap();
        assert!(
            text.contains(
                r#""custom_instances":[{"id":1,"name":"Seeds","base":"ssg","created_ms":0}]"#
            ),
            "{text}"
        );
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), s);
    }

    #[test]
    fn instance_names_are_checked() {
        let mut s = Settings::default();
        s.custom_instances.push(custom(1, "Seeds", Mode::Ssg));
        assert_eq!(
            s.validate_instance_name("  Bastions  ", None),
            Ok("Bastions".into())
        );
        assert_eq!(
            s.validate_instance_name("   ", None),
            Err(NameProblem::Empty)
        );
        assert_eq!(
            s.validate_instance_name("seeds", None),
            Err(NameProblem::Taken)
        );
        // Renaming to its own name (another case) is fine.
        assert_eq!(
            s.validate_instance_name("SEEDS", Some(CustomId(1))),
            Ok("SEEDS".into())
        );
        assert_eq!(
            s.validate_instance_name("a\nb", None),
            Err(NameProblem::Control)
        );
        assert_eq!(
            s.validate_instance_name(&"ż".repeat(40), None),
            Ok("ż".repeat(40))
        );
        assert_eq!(
            s.validate_instance_name(&"x".repeat(41), None),
            Err(NameProblem::TooLong)
        );
        assert_eq!(s.unique_instance_name("Seeds"), "Seeds 2");
        assert_eq!(s.unique_instance_name("New"), "New");
        let long = "y".repeat(40);
        s.custom_instances.push(custom(2, &long, Mode::Rsg));
        let next = s.unique_instance_name(&long);
        assert_eq!(next.chars().count(), 40);
        assert!(next.ends_with(" 2"));
        assert_eq!(s.next_custom_id(), CustomId(3));
        assert_eq!(Settings::default().next_custom_id(), CustomId(1));
        assert!(s.rename_custom(CustomId(2), "seeds").is_err());
        s.rename_custom(CustomId(2), " Long one ").unwrap();
        assert_eq!(s.custom(CustomId(2)).unwrap().name, "Long one");
    }

    #[test]
    fn jvm_arguments_are_checked() {
        let ok =
            |a: &[&str]| validate_jvm_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(ok(&[]).is_ok());
        assert!(
            ok(&[
                "-XX:+UseG1GC",
                "-Dfoo=bar baz",
                "--add-opens=java.base/java.lang=ALL-UNNAMED"
            ])
            .is_ok()
        );
        assert_eq!(ok(&[""]), Err(JvmArgProblem::Empty));
        assert_eq!(ok(&["  "]), Err(JvmArgProblem::Empty));
        assert!(matches!(ok(&["-Xmx8G"]), Err(JvmArgProblem::Memory(_))));
        assert!(matches!(ok(&["-xms1g"]), Err(JvmArgProblem::Memory(_))));
        assert!(matches!(
            ok(&["net.Main"]),
            Err(JvmArgProblem::NotAnOption(_))
        ));
        assert!(matches!(
            ok(&[" -Dx=1"]),
            Err(JvmArgProblem::NotAnOption(_))
        ));
        assert!(matches!(ok(&["-cp"]), Err(JvmArgProblem::Command(_))));
        assert!(matches!(ok(&["-jar"]), Err(JvmArgProblem::Command(_))));
        assert!(matches!(
            ok(&["-javaagent:x.jar"]),
            Err(JvmArgProblem::Agent(_))
        ));
        assert!(matches!(
            ok(&["-agentlib:jdwp=x"]),
            Err(JvmArgProblem::Agent(_))
        ));
    }

    /// A custom instance plays like its base mode: its memory falls back to the base's, its
    /// tools are the base's, and its mods are its own changes on the base's set.
    #[test]
    fn a_custom_instance_follows_its_base() {
        let mut s = Settings::default();
        s.set_memory_mb(Mode::Ssg, 4096);
        s.custom_instances.push(custom(1, "Seeds", Mode::Ssg));
        s.custom_instances.push(custom(2, "Draft", Mode::Draftout));
        let (one, two) = (Instance::Custom(CustomId(1)), Instance::Custom(CustomId(2)));
        assert_eq!(s.rules_mode(one), Mode::Ssg);
        assert_eq!(s.rules_mode(Instance::Custom(CustomId(9))), Mode::Rsg);
        assert_eq!(s.memory_mb_of(one), 4096);
        s.custom_mut(CustomId(1)).unwrap().memory_mb = Some(2048);
        assert_eq!(s.memory_mb_of(one), 2048);
        s.custom_mut(CustomId(2)).unwrap().memory_mb = Some(2048);
        assert_eq!(s.memory_mb_of(two), 3072, "Draftout's floor");
        assert_eq!(s.memory_mb_of(Instance::Practice), s.memory_mb(Mode::Rsg));
        assert!(
            !s.tool_selection_of(two).has(ToolId::NinjabrainBot),
            "Draftout bans Ninjabrain Bot"
        );
        assert_eq!(s.tool_selection_of(one), s.tool_selection(Mode::Ssg));

        let jar = srl_mods::CustomJar {
            name: "Mine".into(),
            file_name: "mine.jar".into(),
            sha512: "ab".repeat(64),
            size: 3,
        };
        s.set_custom_mod(CustomId(1), "setspawnmod", false, true);
        s.set_custom_mod(CustomId(1), "worldpreview", true, false);
        s.add_custom_jar_to(CustomId(1), jar.clone());
        // The mode's own choices stay apart.
        assert!(!s.mods_changed(Mode::Ssg));
        let rules = s.mod_rules();
        let c = rules.choice_of(one);
        assert!(c.off.contains("setspawnmod") && c.on.contains("worldpreview"));
        assert_eq!(c.custom, vec![jar]);
        assert!(!c.on.iter().any(|id| id.starts_with("custom:")));
        assert_eq!(rules.base(one), Some(Mode::Ssg));
        assert_eq!(rules.base(Instance::Practice), None);
        assert_eq!(rules.choice_of(two), srl_mods::ModChoice::default());
        assert_eq!(s.jvm_args_of(one), Vec::<String>::new());
    }

    /// The UI can change a custom instance through `set_settings`, but not add, remove or rebase
    /// one: the backend's list wins.
    #[test]
    fn set_settings_keeps_the_backends_list() {
        let mut backend = Settings::default();
        backend.custom_instances.push(custom(1, "Seeds", Mode::Ssg));
        backend
            .custom_instances
            .push(custom(2, "Bastions", Mode::Rsg));
        // An older copy from the UI: instance 2 is missing, 1 is renamed and rebased, a made-up
        // 7 is there.
        let mut ui = Settings::default();
        let mut one = custom(1, " Seeds v2 ", Mode::Draftout);
        one.memory_mb = Some(5000);
        one.jvm_args = vec!["-XX:+UseZGC".into()];
        ui.custom_instances.push(one);
        ui.custom_instances.push(custom(7, "Made up", Mode::Rsg));
        assert!(backend.adopt_customs(&mut ui));
        let ids: Vec<u32> = ui.custom_instances.iter().map(|c| c.id.0).collect();
        assert_eq!(ids, [1, 2]);
        let one = &ui.custom_instances[0];
        assert_eq!(
            (one.name.as_str(), one.base, one.memory_mb),
            ("Seeds v2", Mode::Ssg, Some(4096))
        );
        assert_eq!(one.jvm_args, ["-XX:+UseZGC"]);
        // An invalid change keeps the old value.
        let mut bad = ui.clone();
        bad.custom_mut(CustomId(2)).unwrap().name = "seeds v2".into();
        bad.custom_mut(CustomId(1)).unwrap().jvm_args = vec!["-Xmx1G".into()];
        let before = ui.clone();
        assert!(ui.adopt_customs(&mut bad));
        assert_eq!(bad.custom_instances, before.custom_instances);
        // The same list: nothing to change.
        let mut same = ui.clone();
        assert!(!ui.adopt_customs(&mut same));
    }

    /// An edited file: a second definition with the same id goes, a bad name is replaced, bad
    /// JVM arguments are dropped.
    #[test]
    fn edited_custom_instances_load_usable() {
        let dir = std::env::temp_dir().join(format!("srl-app-customs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        std::fs::write(
            &file,
            r#"{"custom_instances":[
                {"id":1,"name":"","base":"rsg","memory_mb":1},
                {"id":1,"name":"Again","base":"fsg"},
                {"id":2,"name":"Ok","base":"draftout","jvm_args":["-Xmx9G"]}
            ]}"#,
        )
        .unwrap();
        let s = Settings::load(&file).unwrap();
        let ids: Vec<u32> = s.custom_instances.iter().map(|c| c.id.0).collect();
        assert_eq!(ids, [1, 2]);
        assert_eq!(s.custom_instances[0].name, "Instance 1");
        assert_eq!(s.custom_instances[0].memory_mb, Some(2048));
        assert!(s.custom_instances[1].jvm_args.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn memory_and_tools_per_mode() {
        let mut s = Settings::default();
        s.set_memory_mb(Mode::Ranked, 4096);
        assert_eq!(s.memory_mb(Mode::Ranked), 4096);
        assert_eq!(s.memory_mb(Mode::Rsg), 3072);
        s.set_memory_mb(Mode::Draftout, 2048);
        assert_eq!(s.memory_mb(Mode::Draftout), 3072);

        assert!(s.starts_with(ToolId::NinjabrainBot, Mode::Rsg));
        assert!(!s.starts_with(ToolId::NinjabrainBot, Mode::Draftout));
        s.set_starts_with(ToolId::NinjabrainBot, Mode::Rsg, false);
        s.set_starts_with(ToolId::NinjabrainBot, Mode::Draftout, true);
        assert!(!s.starts_with(ToolId::NinjabrainBot, Mode::Rsg));
        assert!(s.starts_with(ToolId::NinjabrainBot, Mode::Ranked));
        assert!(!s.starts_with(ToolId::NinjabrainBot, Mode::Draftout));
        // waywall needs the player's own setup, so it starts with nothing until turned on.
        assert!(!s.starts_with(ToolId::Waywall, Mode::Draftout));
        // Toolscreen: on with the 1.16.1 modes where it runs (Windows), never with Draftout yet.
        assert_eq!(s.starts_with(ToolId::Toolscreen, Mode::Rsg), cfg!(windows));
        assert!(!s.starts_with(ToolId::Toolscreen, Mode::Draftout));
        assert_eq!(
            s.tool_selection(Mode::Ranked)
                .0
                .into_iter()
                .collect::<Vec<_>>(),
            if cfg!(windows) {
                vec![ToolId::NinjabrainBot, ToolId::Toolscreen]
            } else {
                vec![ToolId::NinjabrainBot]
            }
        );
        assert!(!s.tool_selection(Mode::Rsg).has(ToolId::NinjabrainBot));
        if cfg!(target_os = "linux") {
            s.set_starts_with(ToolId::Waywall, Mode::Draftout, true);
            assert!(s.starts_with(ToolId::Waywall, Mode::Draftout));
            assert!(!s.starts_with(ToolId::Waywall, Mode::Rsg));
        }

        let text = serde_json::to_string(&s).unwrap();
        assert!(
            text.contains(r#""ninjabrain-bot":["ranked","fsg","ssg"]"#),
            "{text}"
        );
        assert!(text.contains(r#""draftout":2048"#), "{text}");
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), s);
        let opts = Settings {
            draftout_extras: true,
            ..Settings::default()
        }
        .resolve_options();
        assert!(opts.draftout_extras && !opts.accessibility);
    }
}
