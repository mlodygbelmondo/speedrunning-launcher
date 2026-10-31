//! Sample data: `data.json` (the Library leaderboard, SPEC.md) and `screens.json` (prototype screens,
//! SCREENS.md, one copy per language). Parsed once and kept in statics so render closures can
//! borrow it freely.
use std::{collections::HashMap, sync::OnceLock};

use gpui::{ImageSource, Resource, SharedString};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    assets::{DATA_JSON, SCREENS_JSON, SCREENS_PL_JSON},
    copy::lang,
};

// ---------------------------------------------------------------------------------------------
// data.json
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawRow {
    rank: u32,
    name: String,
    category: String,
    time: String,
    delta: String,
    ahead: bool,
    elo: u32,
    avatar: String,
}

/// The v2 Home's pace list and tiles are in the file too; scene v3 Home reads `screens.json`.
#[derive(Deserialize)]
struct RawData {
    rows: Vec<RawRow>,
}

pub struct Row {
    pub rank: SharedString,
    pub name: SharedString,
    pub category: SharedString,
    pub time: SharedString,
    pub delta: SharedString,
    pub ahead: bool,
    pub elo: SharedString,
    avatar: SharedString,
}

pub struct Data {
    pub rows: Vec<Row>,
}

/// `ImageSource` isn't `Sync`, so the statics keep the asset path and build the (cheap,
/// Arc-backed) source when rendering.
fn source(path: &SharedString) -> ImageSource {
    ImageSource::Resource(Resource::Embedded(path.clone()))
}

impl Row {
    pub fn avatar(&self) -> ImageSource {
        source(&self.avatar)
    }
}

pub fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        let raw: RawData = serde_json::from_str(DATA_JSON).expect("data.json");
        let s = |v: String| SharedString::from(v);
        Data {
            rows: raw
                .rows
                .into_iter()
                .map(|r| Row {
                    rank: r.rank.to_string().into(),
                    name: s(r.name),
                    category: s(r.category),
                    time: s(r.time),
                    delta: s(r.delta),
                    ahead: r.ahead,
                    elo: r.elo.to_string().into(),
                    avatar: s(r.avatar),
                })
                .collect(),
        }
    })
}

// ---------------------------------------------------------------------------------------------
// screens.json
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct Account {
    pub name: String,
    pub avatar: String,
    pub active: bool,
    pub expired: bool,
    pub ago: String,
}

#[derive(Deserialize)]
pub struct Accounts {
    pub store: HashMap<String, String>,
    pub device_code: String,
    pub device_url: String,
    pub device_qr: Vec<String>,
    pub list: Vec<Account>,
}

#[derive(Deserialize)]
pub struct Mode {
    pub id: String,
    pub name: String,
    pub full: String,
    pub icon: String,
    pub mc: String,
    pub status: String,
    pub recommended: bool,
    pub desc: String,
    pub leaderboard: String,
    pub layers: Vec<String>,
}

#[derive(Deserialize)]
pub struct Layer {
    pub name: String,
    #[serde(default)]
    pub detail: Option<String>,
    pub mods: Vec<(String, String)>,
}

#[derive(Deserialize)]
pub struct Check {
    pub id: String,
    pub status: String,
    pub args: HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct MapItem {
    pub name: String,
    pub version: String,
    pub authors: String,
    pub category: String,
    pub downloads: String,
    pub status: String,
    pub desc: String,
}

#[derive(Deserialize)]
pub struct PracticeTool {
    pub name: String,
    pub version: String,
    pub authors: String,
    pub status: String,
    pub desc: String,
}

#[derive(Deserialize)]
pub struct Tutorial {
    pub title: String,
    pub creator: String,
    pub kind: String,
    pub length: String,
    pub level: String,
    pub date: String,
    pub current: bool,
    #[allow(dead_code)] // never opened: the prototype only shows a toast
    pub url: String,
}

#[derive(Deserialize)]
pub struct TutorialGroup {
    pub title: String,
    pub icon: String,
    pub items: Vec<Tutorial>,
}

#[derive(Deserialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub version: String,
    pub icon: String,
    pub platform: String,
    pub desc: String,
    pub installed: bool,
    pub pinned: bool,
    #[serde(default)]
    pub external: bool,
    pub start_with_game: bool,
    pub auto_update: bool,
    #[serde(default)]
    pub newer: Option<String>,
}

#[derive(Deserialize)]
pub struct StatTiles {
    pub pb: String,
    pub pb_category: String,
    pub runs_month: String,
    pub runs_month_sub: String,
    pub finished: String,
    pub finished_sub: String,
    pub avg_enter: String,
    pub avg_enter_sub: String,
}

#[derive(Deserialize)]
pub struct Run {
    pub date: String,
    pub mode: String,
    pub finished: bool,
    pub pb: bool,
    pub result: String,
    pub splits: Vec<(String, String, bool)>,
}

#[derive(Deserialize)]
pub struct Stats {
    pub total_runs: String,
    pub tiles: StatTiles,
    pub splits: Vec<String>,
    pub runs: Vec<Run>,
}

#[derive(Deserialize)]
pub struct Submit {
    pub time: String,
    pub category: String,
    pub date: String,
    pub igt: String,
    pub rta: String,
    pub files: Vec<(String, String)>,
    pub zip: String,
    pub zip_size: String,
    pub sample_link: String,
}

#[derive(Deserialize)]
pub struct Settings {
    pub languages: Vec<String>,
    pub memory: Vec<String>,
    pub memory_default: usize,
    pub keybinds: Vec<String>,
    pub keybinds_default: usize,
    pub sensitivity: String,
    pub channels: Vec<String>,
    pub data_folder: HashMap<String, String>,
    pub close_on_launch: bool,
    pub start_with_system: bool,
    pub standard_settings: bool,
    pub auto_update: bool,
}

#[derive(Deserialize)]
pub struct Game {
    pub mode: String,
    pub started: String,
    pub resets: String,
    pub best: String,
    pub nether_entries: String,
}

#[derive(Deserialize)]
pub struct Onboarding {
    pub obs_version: String,
}

// ---------------------------------------------------------------------------------------------
// screens.json `v3` block (REDESIGN.md)
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct V3Mode {
    pub short: String,
    pub art: String,
    pub mods_total: u32,
    pub disk: String,
    #[serde(default)]
    pub update_mod: Option<String>,
    /// Download size; present only for modes that ship as a download.
    #[serde(default)]
    pub download: Option<String>,
}

#[derive(Deserialize)]
pub struct V3Match {
    pub opponent: String,
    pub avatar: String,
    pub won: bool,
    pub time: String,
    pub elo: String,
}

#[derive(Deserialize)]
pub struct V3Weekly {
    pub n: u32,
    pub place: u32,
    pub total: String,
    pub time: String,
}

#[derive(Deserialize)]
pub struct V3Ranked {
    pub elo: u32,
    pub elo_delta: String,
    pub tier: String,
    pub season_w: u32,
    pub season_l: u32,
    pub best: String,
    pub matches: Vec<V3Match>,
    pub weekly: V3Weekly,
}

#[derive(Deserialize)]
pub struct V3Split {
    pub name: String,
    pub art: String,
    pub secs: u32,
    pub time: String,
    pub best: String,
}

#[derive(Deserialize)]
pub struct V3Runs {
    pub pb: String,
    pub pb_date: String,
    pub sob: String,
    pub runs_month: u32,
    pub splits: Vec<V3Split>,
}

#[derive(Deserialize)]
pub struct V3Install {
    pub new: Vec<String>,
    pub have: Vec<String>,
}

#[derive(Deserialize)]
pub struct V3 {
    pub season: u32,
    pub online: String,
    pub default_mode: String,
    pub modes: HashMap<String, V3Mode>,
    pub ranked: V3Ranked,
    pub runs: HashMap<String, V3Runs>,
    pub install: HashMap<String, V3Install>,
    pub practice_art: HashMap<String, String>,
    pub practice_default: String,
}

#[derive(Deserialize)]
pub struct Screens {
    pub accounts: Accounts,
    pub modes: Vec<Mode>,
    pub layers: HashMap<String, Layer>,
    pub disk_saved: String,
    pub checks: Vec<Check>,
    pub practice_categories: Vec<Category>,
    pub maps: Vec<MapItem>,
    pub practice_tools: Vec<PracticeTool>,
    pub tutorial_groups: Vec<TutorialGroup>,
    pub tools: Vec<Tool>,
    pub tools_checked_ago: String,
    pub stats: Stats,
    pub submit: Submit,
    pub settings: Settings,
    pub game: Game,
    pub onboarding: Onboarding,
    pub v3: V3,
}

/// Sample content in the current language (SCREENS.md "Language").
pub fn screens() -> &'static Screens {
    &all_screens()[lang().index()]
}

/// One `Screens` per language, built once: screens.json for English, screens.json merged with the
/// screens.pl.json overlay for Polish.
fn all_screens() -> &'static [Screens; 2] {
    static SCREENS: OnceLock<[Screens; 2]> = OnceLock::new();
    SCREENS.get_or_init(|| {
        let base: Value = serde_json::from_str(SCREENS_JSON).expect("screens.json");
        let overlay: Value = serde_json::from_str(SCREENS_PL_JSON).expect("screens.pl.json");
        let mut pl = base.clone();
        merge(&mut pl, overlay);
        let en = serde_json::from_value(base).expect("screens.json");
        let pl = serde_json::from_value(pl).expect("screens.json + screens.pl.json");
        [en, pl]
    })
}

/// Overlay merge: objects merge by key, arrays merge by index, `null` keeps the base value, any
/// other value replaces it. Overlay entries past the end of a base array are appended.
fn merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (_, Value::Null) => {}
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(slot) => merge(slot, v),
                    None if !v.is_null() => {
                        b.insert(k, v);
                    }
                    None => {}
                }
            }
        }
        (Value::Array(b), Value::Array(o)) => {
            for (i, v) in o.into_iter().enumerate() {
                match b.get_mut(i) {
                    Some(slot) => merge(slot, v),
                    None => b.push(v),
                }
            }
        }
        (slot, v) => *slot = v,
    }
}

/// Parse every language now, so a broken file fails at startup, not mid-render.
pub fn init() {
    data();
    all_screens();
}

impl Screens {
    pub fn layer(&self, id: &str) -> &Layer {
        self.layers
            .get(id)
            .unwrap_or_else(|| panic!("screens.json: unknown layer {id}"))
    }

    /// The v3 extras of `modes[i]`.
    pub fn v3_mode(&self, i: usize) -> &V3Mode {
        let id = &self.modes[i].id;
        self.v3
            .modes
            .get(id)
            .unwrap_or_else(|| panic!("screens.json: v3.modes has no {id}"))
    }

    pub fn mode_index(&self, id: &str) -> Option<usize> {
        self.modes.iter().position(|m| m.id == id)
    }
}

/// The OS key used by per-OS data (`windows`, `linux`, `macos`).
pub fn os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// Look up a per-OS value, falling back to the Linux one.
pub fn per_os(map: &HashMap<String, String>) -> &str {
    map.get(os())
        .or_else(|| map.get("linux"))
        .map(String::as_str)
        .unwrap_or("")
}
