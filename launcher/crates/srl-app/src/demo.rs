//! `SRL_DEMO=1`: the prototype's sample data (elo, matches, PBs, splits, player counts, sizes), so
//! the app can be compared with the prototype's screenshots (`bakeoff/shots/v3`). It's made up, so
//! the app shows none of it unless the flag is set: without it every item backed by this file is
//! hidden.
use std::{
    collections::HashMap,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use serde::Deserialize;
use serde_json::Value;
use srl_core::Mode;

use crate::{
    assets::{DEMO_JSON, DEMO_PL_JSON},
    backend::{Language, ModeInfo, ModeSnapshot, ModeState},
    copy::lang,
};

#[derive(Deserialize)]
pub struct Account {
    pub name: String,
    /// `demo/avatars/<avatar>.png`.
    pub avatar: String,
}

#[derive(Deserialize)]
pub struct DemoMode {
    pub mods_total: u32,
    pub disk: String,
    #[serde(default)]
    pub update_mod: Option<String>,
    #[serde(default)]
    pub download: Option<String>,
}

#[derive(Deserialize)]
pub struct Match {
    pub opponent: String,
    pub avatar: String,
    pub won: bool,
    pub time: String,
    pub elo: String,
}

#[derive(Deserialize)]
pub struct Weekly {
    pub n: u32,
    pub place: u32,
    pub total: String,
    pub time: String,
}

#[derive(Deserialize)]
pub struct Ranked {
    pub elo: u32,
    pub elo_delta: String,
    pub tier: String,
    pub season_w: u32,
    pub season_l: u32,
    pub best: String,
    pub matches: Vec<Match>,
    pub weekly: Weekly,
}

#[derive(Deserialize)]
pub struct Split {
    pub name: String,
    pub art: String,
    pub secs: u32,
    pub time: String,
    pub best: String,
}

#[derive(Deserialize)]
pub struct Runs {
    pub pb: String,
    pub pb_date: String,
    pub sob: String,
    pub runs_month: u32,
    pub splits: Vec<Split>,
}

#[derive(Deserialize)]
pub struct Layer {
    #[serde(default)]
    pub detail: Option<String>,
    pub mods: Vec<(String, String)>,
}

#[derive(Deserialize)]
pub struct MapDemo {
    pub status: String,
    pub downloads: String,
}

#[derive(Deserialize)]
pub struct Demo {
    pub account: Account,
    pub season: u32,
    pub online: String,
    pub disk_saved: String,
    status: HashMap<String, String>,
    modes: HashMap<String, DemoMode>,
    pub ranked: Ranked,
    pub runs: HashMap<String, Runs>,
    pub key_tools: HashMap<String, String>,
    pub layers: HashMap<String, Layer>,
    pub maps: HashMap<String, MapDemo>,
    pub tools: HashMap<String, String>,
}

static ON: AtomicBool = AtomicBool::new(false);

/// Turns the demo on for this run (startup only).
pub fn enable() {
    ON.store(true, Ordering::Relaxed);
    all();
}

/// The sample data in the current language, or `None` without `SRL_DEMO`.
pub fn get() -> Option<&'static Demo> {
    ON.load(Ordering::Relaxed).then(|| {
        let i = Language::ALL.iter().position(|l| *l == lang()).unwrap_or(0);
        &all()[i]
    })
}

/// One `Demo` per language: demo.json for English, demo.json merged with the demo.pl.json overlay
/// for Polish.
fn all() -> &'static [Demo; 2] {
    static DEMO: OnceLock<[Demo; 2]> = OnceLock::new();
    DEMO.get_or_init(|| {
        let base: Value = serde_json::from_str(DEMO_JSON).expect("demo.json");
        let overlay: Value = serde_json::from_str(DEMO_PL_JSON).expect("demo.pl.json");
        let mut pl = base.clone();
        merge(&mut pl, overlay);
        [
            serde_json::from_value(base).expect("demo.json"),
            serde_json::from_value(pl).expect("demo.json + demo.pl.json"),
        ]
    })
}

/// Overlay merge: objects merge by key, arrays merge by index, `null` keeps the base value, any
/// other value replaces it.
fn merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (_, Value::Null) => {}
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(slot) => merge(slot, v),
                    None => {
                        b.insert(k, v);
                    }
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

/// "94 MB" → bytes, the way `copy::size` prints them back.
fn parse_size(s: &str) -> Option<u64> {
    let (n, unit) = s.split_once(' ')?;
    let n: f64 = n.parse().ok()?;
    let mul = match unit {
        "MB" => 1024. * 1024.,
        "GB" => 1024. * 1024. * 1024.,
        _ => return None,
    };
    Some((n * mul) as u64)
}

impl Demo {
    /// The prototype's mode states and facts, as the fake backend's starting point.
    pub fn modes(&self) -> [ModeSnapshot; 5] {
        std::array::from_fn(|i| {
            let id = Mode::ALL[i].id();
            let m = &self.modes[id];
            let state = match self.status.get(id).map(String::as_str) {
                Some("ready") => ModeState::Ready,
                Some("update") => ModeState::UpdateAvailable,
                _ => ModeState::NotInstalled,
            };
            ModeSnapshot {
                state,
                info: ModeInfo {
                    mods: Some(m.mods_total),
                    disk: parse_size(&m.disk),
                    download: m.download.as_deref().and_then(parse_size),
                    update: m.update_mod.clone(),
                    launched: None,
                    // The fake fills these in (Java for the installed modes).
                    java: None,
                    catalogue: None,
                    concerns: Vec::new(),
                },
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages_parse_and_sizes_round_trip() {
        let [en, pl] = all();
        assert_eq!(en.runs["rsg"].pb_date, "Oct 24");
        assert_eq!(pl.runs["rsg"].pb_date, "24 paź");
        crate::copy::set_lang(Language::En);
        for s in ["94 MB", "612 MB", "38 MB"] {
            assert_eq!(crate::copy::size(parse_size(s).unwrap()), s);
        }
        assert_eq!(en.modes()[0].state, ModeState::UpdateAvailable);
    }
}
