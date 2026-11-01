//! What the launcher offers, independent of any one install: the five modes with their art, and
//! the practice catalogue. This is product data, true by construction, so it shows without
//! `SRL_DEMO`. Translatable text is in copy.json
//! (`mode.<id>.*`, `practice.cat.<id>`, `map.<id>`, `practice.credit.<id>` for a
//! map with more credit than its authors, `practice.req.<id>`, `ptool.<id>`, `learn.topic.<id>`,
//! `learn.g.<id>`).
//!
//! The practice maps have one source: srl-practice's catalogue (`maps()`), which also installs
//! them, so a card can't name a map, version or author the install doesn't have.
//! `assets/practice.json` holds only what the page adds: the categories' art and the practice
//! tools.
use std::sync::OnceLock;

use serde::Deserialize;
use srl_core::Mode;

use crate::assets::{LEARN_JSON, PRACTICE_JSON};

pub struct ModeDef {
    pub mode: Mode,
    /// Display name, the same in every language ("Ranked").
    pub short: &'static str,
    /// Art id: `art/hero/<art>.jpg` and `art/card/<art>.jpg`.
    pub art: &'static str,
}

/// In `Mode::ALL` order.
pub const MODES: [ModeDef; 5] = [
    ModeDef {
        mode: Mode::Ranked,
        short: "Ranked",
        art: "ranked",
    },
    ModeDef {
        mode: Mode::Rsg,
        short: "RSG",
        art: "rsg",
    },
    ModeDef {
        mode: Mode::Fsg,
        short: "FSG",
        art: "fsg",
    },
    ModeDef {
        mode: Mode::Ssg,
        short: "SSG",
        art: "ssg",
    },
    ModeDef {
        mode: Mode::Draftout,
        short: "Draftout",
        art: "draftout",
    },
];

pub fn def(mode: Mode) -> &'static ModeDef {
    &MODES[crate::backend::index(mode)]
}

/// About what a first install of `mode` downloads, for the Download button before anything is
/// installed (the backend knows the real size only for what an installed mode is missing).
/// Measured from an empty data folder on Linux, 2026-10-26: RSG with Ninjabrain Bot 537 MB,
/// Draftout 743 MB on disk. Rounded down, and shown as "about".
pub fn approx_download(mode: Mode) -> u64 {
    const MIB: u64 = 1024 * 1024;
    if mode == Mode::Draftout {
        700 * MIB
    } else {
        500 * MIB
    }
}

// ---------------------------------------------------------------------------------------------
// Practice
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct Category {
    pub id: String,
    /// Card art id.
    pub art: String,
}

#[derive(Deserialize)]
pub struct PracticeTool {
    pub id: String,
    pub name: String,
    pub version: String,
    pub authors: String,
    /// "<authors> and contributors".
    #[serde(default)]
    pub contributors: bool,
}

#[derive(Deserialize)]
pub struct Practice {
    /// The first one is the big tile.
    pub categories: Vec<Category>,
    pub tools: Vec<PracticeTool>,
}

pub fn practice() -> &'static Practice {
    static P: OnceLock<Practice> = OnceLock::new();
    P.get_or_init(|| serde_json::from_str(PRACTICE_JSON).expect("practice.json"))
}

/// The practice maps, most popular first, then the ones only the author's page has: credit,
/// links, requirements, licence and the pinned download (srl-practice, built in).
pub fn maps() -> &'static srl_practice::Catalogue {
    static C: OnceLock<srl_practice::Catalogue> = OnceLock::new();
    C.get_or_init(|| srl_practice::Catalogue::builtin().expect("srl-practice's catalogue"))
}

/// The maps of category `id`, in catalogue order.
pub fn maps_in(id: &str) -> impl Iterator<Item = &'static srl_practice::MapEntry> {
    maps().maps.iter().filter(move |m| m.category == id)
}

// ---------------------------------------------------------------------------------------------
// Learn
// ---------------------------------------------------------------------------------------------

/// A guide by someone else, linked and credited, never embedded (`assets/learn.json`).
#[derive(Deserialize)]
pub struct Guide {
    /// Also the copy key of what it covers: `learn.g.<id>`.
    pub id: String,
    /// `video`, `series`, `text` or `tool` (copy `learn.kind.<kind>`).
    pub kind: String,
    /// The author's own title, untranslated.
    pub title: String,
    pub author: String,
    pub url: String,
    /// `YYYY-MM`, when the source has a date.
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub minutes: Option<u32>,
    /// Videos in a series.
    #[serde(default)]
    pub parts: Option<u32>,
    /// Modes, when the guide is for fewer than its topic.
    #[serde(default)]
    pub modes: Vec<String>,
    /// `windows` or `linux`, when it is for one system only.
    #[serde(default)]
    pub platform: Option<String>,
}

#[derive(Deserialize)]
pub struct Topic {
    /// Copy keys `learn.topic.<id>` and `learn.topic.<id>.desc`.
    pub id: String,
    /// `runs` (every 1.16.1 run) or `modes` (a mode with its own rules); copy `learn.group.<group>`.
    pub group: String,
    /// Card art id.
    pub art: String,
    /// The modes it is for (`Mode` ids).
    pub modes: Vec<String>,
    pub guides: Vec<Guide>,
}

#[derive(Deserialize)]
pub struct Learn {
    pub topics: Vec<Topic>,
}

/// The topic groups, in the order the list shows them.
pub const LEARN_GROUPS: [&str; 2] = ["runs", "modes"];

pub fn learn() -> &'static Learn {
    static L: OnceLock<Learn> = OnceLock::new();
    L.get_or_init(|| serde_json::from_str(LEARN_JSON).expect("learn.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_match_core_order_and_practice_parses() {
        for (d, m) in MODES.iter().zip(Mode::ALL) {
            assert_eq!(d.mode, m);
        }
        let p = practice();
        assert_eq!(p.categories.len(), 6, "the grid has 6 category cells");
        let has = |k: String| crate::copy::t_opt(&k).is_some();
        for m in &maps().maps {
            assert!(p.categories.iter().any(|c| c.id == m.category), "{}", m.id);
            // Every map, a link or not, has its words (copy.rs checks that Polish has them too).
            // A credit line (`practice.credit.<id>`) is there only when it says more than the
            // authors the row already names.
            assert!(has(format!("map.{}", m.id)), "map.{}", m.id);
            for r in &m.requirements {
                assert!(has(format!("practice.req.{}", r.id)), "req {}", r.id);
            }
            if !m.carry_over.is_empty() {
                assert!(has(format!("practice.keep.{}", m.id)), "keep {}", m.id);
            }
        }
    }

    #[test]
    fn learn_is_complete() {
        let l = learn();
        let mut ids = std::collections::HashSet::new();
        for (i, topic) in l.topics.iter().enumerate() {
            assert!(LEARN_GROUPS.contains(&topic.group.as_str()), "{}", topic.id);
            // Topics of a group are listed together.
            if i > 0 {
                let prev = LEARN_GROUPS
                    .iter()
                    .position(|g| *g == l.topics[i - 1].group);
                let this = LEARN_GROUPS.iter().position(|g| *g == topic.group);
                assert!(prev <= this, "{} is out of its group", topic.id);
            }
            assert!(
                !topic.id.contains('-'),
                "{}: deep links split at '-'",
                topic.id
            );
            for m in &topic.modes {
                assert!(Mode::from_id(m).is_some(), "{}: mode {m}", topic.id);
            }
            assert!(!topic.guides.is_empty(), "{}", topic.id);
            // Copy for every topic (copy.rs checks that both languages have it). A guide's
            // description is optional: only where it says more than the title.
            let has = |k: String| crate::copy::t_opt(&k).is_some();
            assert!(has(format!("learn.group.{}", topic.group)), "{}", topic.id);
            assert!(has(format!("learn.topic.{}", topic.id)), "{}", topic.id);
            assert!(
                has(format!("learn.topic.{}.desc", topic.id)),
                "{}",
                topic.id
            );
            for g in &topic.guides {
                assert!(ids.insert(g.id.as_str()), "duplicate guide {}", g.id);
                assert!(
                    ["video", "series", "text", "tool"].contains(&g.kind.as_str()),
                    "{}",
                    g.id
                );
                assert!(g.url.starts_with("https://"), "{}: {}", g.id, g.url);
                assert!(!g.title.is_empty() && !g.author.is_empty(), "{}", g.id);
                for m in &g.modes {
                    assert!(topic.modes.contains(m), "{}: mode {m}", g.id);
                }
                if let Some(d) = &g.date {
                    let ok = d.len() == 7
                        && d.as_bytes()[4] == b'-'
                        && d[..4].parse::<u32>().is_ok()
                        && d[5..].parse::<u32>().is_ok_and(|m| (1..=12).contains(&m));
                    assert!(ok, "{}: date {d}", g.id);
                }
                assert!(
                    g.platform
                        .as_deref()
                        .is_none_or(|p| p == "windows" || p == "linux"),
                    "{}",
                    g.id
                );
            }
        }
    }
}
