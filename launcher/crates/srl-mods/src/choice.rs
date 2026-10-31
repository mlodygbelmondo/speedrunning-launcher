//! The player's own changes to a mode's set, the way MultiMC lets them turn mods on and off.
//!
//! A mode's set from `resolve` is the starting point (the legal, recommended set of its
//! category). On the Mods page the player may turn any of its mods off, turn on any other mod the
//! lists know for the Minecraft version (a legal mod of the other category, a practice mod from
//! `extra.json`, the Ranked mod in RSG), and add jars of their own. [`apply`] makes the set Play
//! installs from that; [`concerns`] says what makes a run with it not legal for the mode, and
//! the launcher shows it but still starts the game: the choice is the player's.
//!
//! What never goes in, whatever the choice: the `forbidden` mods (Fabric API is banned on /mc,
//! and the Ranked jar `breaks` Glacier and Practice Seed Mod, so the game would not start).
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use srl_core::Mode;

use crate::{
    ModMeta,
    resolve::{
        Category, Ctx, KnownJar, ModEntry, ModSet, ModSource, Note, Os, Reason, TraitFit,
        add_dependencies, allowed_only, break_incompatibilities, drop_broken_dependents, entry,
        extra_entry, forbidden_in, make_file_names_unique, trait_fit,
    },
};

/// What the player changed in one mode's set.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModChoice {
    /// Mod ids turned on on top of the mode's set.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub on: BTreeSet<String>,
    /// Mod ids of the mode's set turned off. Mods that need one of them go off too.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub off: BTreeSet<String>,
    /// The player's own jars that are on in this mode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<CustomJar>,
}

impl ModChoice {
    pub fn is_empty(&self) -> bool {
        self.on.is_empty() && self.off.is_empty() && self.custom.is_empty()
    }
}

/// A jar the player added. The launcher keeps it in the content store
/// (`Paths::store_sha512`), so `install` links it like any other jar and never downloads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomJar {
    /// Shown on the Mods page: the jar's `fabric.mod.json` name, else its file name.
    pub name: String,
    pub file_name: String,
    /// Lower-case hex.
    pub sha512: String,
    pub size: u64,
}

impl CustomJar {
    /// The id of its entry in a `ModSet`. No mod list uses a `:`, so it can't clash.
    pub fn modid(&self) -> String {
        format!("custom:{}", &self.sha512[..self.sha512.len().min(16)])
    }
}

/// `set` (a mode's set from `resolve`) with the player's changes. Ids the lists don't know for
/// the set's Minecraft version, or that are forbidden there, are left out with a note.
pub fn apply(meta: &ModMeta, set: ModSet, choice: &ModChoice) -> ModSet {
    if choice.is_empty() {
        return set;
    }
    let ModSet {
        instance,
        os,
        minecraft_version: mc,
        mods,
        mut notes,
        mut known,
    } = set;
    let category = Category::of(instance);
    let mut map: BTreeMap<String, ModEntry> =
        mods.into_iter().map(|m| (m.modid.clone(), m)).collect();
    let mut pinned = BTreeSet::new();
    for id in &choice.on {
        if map.contains_key(id) || choice.off.contains(id) {
            continue;
        }
        if forbidden_in(instance).contains(&id.as_str()) {
            notes.push(Note::NotAllowedInMode { modid: id.clone() });
            continue;
        }
        match chosen_entry(meta, id, &mc, os, &mut notes) {
            Some(e) => {
                pinned.insert(e.modid.clone());
                map.insert(e.modid.clone(), e);
            }
            None => notes.push(Note::NoBuild {
                modid: id.clone(),
                minecraft: mc.clone(),
            }),
        }
    }
    for c in &choice.custom {
        let modid = c.modid();
        pinned.insert(modid.clone());
        map.insert(
            modid.clone(),
            ModEntry {
                modid,
                name: c.name.clone(),
                version: String::new(),
                file_name: c.file_name.clone(),
                url: String::new(),
                sha512: c.sha512.to_ascii_lowercase(),
                size: c.size,
                source: ModSource::Custom,
                reason: Reason::Chosen,
                dependencies: Vec::new(),
            },
        );
    }
    let ctx = Ctx {
        meta,
        mc: &mc,
        category,
        os,
        instance,
    };
    add_dependencies(&ctx, &mut map, &mut notes);
    // Off wins over a dependency: a mod that needs a mod turned off goes off with it.
    for id in &choice.off {
        map.remove(id);
    }
    break_incompatibilities(meta, &pinned, &mut map, &mut notes);
    drop_broken_dependents(&mut map, &mut notes);
    let mut mods: Vec<ModEntry> = map.into_values().collect();
    make_file_names_unique(&mut mods);
    // What the player chose is allowed in this instance: `status` doesn't call it illegal (the
    // legality of the run is `concerns`' business).
    for m in &mods {
        known
            .entry(m.sha512.clone())
            .and_modify(|k| k.allowed = true)
            .or_insert_with(|| KnownJar {
                modid: m.modid.clone(),
                version: m.version.clone(),
                allowed: true,
            });
    }
    ModSet {
        instance,
        os,
        minecraft_version: mc,
        mods,
        notes,
        known,
    }
}

/// A build of `id` for `mc`: a live legal build from `mods.json` (any category, not another OS's
/// build), else the mod's build from Modrinth or `extra.json`.
fn chosen_entry(
    meta: &ModMeta,
    id: &str,
    mc: &str,
    os: Os,
    notes: &mut Vec<Note>,
) -> Option<ModEntry> {
    if let Some(m) = meta.legal_mod(id) {
        let v = m.version_for(mc)?;
        let other_os = meta.traits_of(m).iter().any(|t| t == "mac-only") && os != Os::MacOs;
        if other_os || m.obsolete || v.obsolete || !meta.legal.contains(&v.sha512) {
            return None;
        }
        return entry(m, v, ModSource::Legal, Reason::Chosen, notes);
    }
    let mut e = extra_entry(meta, id, mc, notes)?;
    e.reason = Reason::Chosen;
    Some(e)
}

/// Why a run with a set would not count for its mode (or would not start).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Concern {
    /// A legal mod of the other category (SetSpawn in RSG, WorldPreview in SSG).
    OtherCategory { name: String, only: String },
    /// A mod from no legal list: a practice mod, or another mode's own mod (the Ranked mod in
    /// RSG).
    NotLegal { name: String },
    /// One of the player's own jars: nothing says whether it is legal.
    Unknown { name: String },
    /// The mode's own mod is off (Ranked without the Ranked mod can't play a match).
    MissingOwn { name: String },
    /// Draftout refuses to start with a mod outside its own list.
    RefusedByMode { name: String },
}

/// What makes `set` not legal (or not playable) as a run of `mode`, in the order of `set.mods`,
/// then the mode's missing own mod.
pub fn concerns(meta: &ModMeta, set: &ModSet, mode: Mode) -> Vec<Concern> {
    let category = Category::of(mode.into());
    let own = own_mods(mode);
    let mut out = Vec::new();
    for m in &set.mods {
        let name = m.name.clone();
        if let Some(allowed) = allowed_only(mode)
            && !allowed.contains(&m.modid.as_str())
        {
            out.push(Concern::RefusedByMode { name });
            continue;
        }
        match m.source {
            ModSource::Custom => out.push(Concern::Unknown { name }),
            ModSource::Legal => {
                let traits = meta
                    .legal_mod(&m.modid)
                    .map(|lm| meta.traits_of(lm))
                    .unwrap_or_default();
                if let TraitFit::Excluded = trait_fit(&traits, category, set.os) {
                    let only = if traits.iter().any(|t| t == "ssg-only") {
                        "SSG"
                    } else {
                        "RSG"
                    };
                    if only != category.label() {
                        out.push(Concern::OtherCategory {
                            name,
                            only: only.into(),
                        });
                    }
                }
            }
            ModSource::Modrinth { .. } | ModSource::Extra => {
                let is_dependency = matches!(m.reason, Reason::Dependency { .. });
                if !own.contains(&m.modid.as_str()) && !is_dependency {
                    out.push(Concern::NotLegal { name });
                }
            }
        }
    }
    let required: &[&str] = match mode {
        Mode::Ranked => &["mcsrranked"],
        Mode::Fsg => &["fsg-mod"],
        Mode::Draftout => &["draftout"],
        Mode::Rsg | Mode::Ssg => &[],
    };
    for id in required {
        if set.get(id).is_none() {
            let name = meta
                .extra_mod(id)
                .map_or_else(|| (*id).to_string(), |m| m.name.clone());
            out.push(Concern::MissingOwn { name });
        }
    }
    out
}

/// The mode's own non-legal mods: they are legal in that mode.
fn own_mods(mode: Mode) -> &'static [&'static str] {
    match mode {
        Mode::Ranked => &["mcsrranked"],
        Mode::Fsg => &["fsg-mod"],
        Mode::Draftout => &[
            "draftout",
            "nochatrestrictions",
            "no-telemetry",
            "commandqmod",
        ],
        Mode::Rsg | Mode::Ssg => &[],
    }
}

/// One row of the Mods page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Available {
    pub modid: String,
    pub name: String,
    pub description: String,
    /// In the mode's own set (`resolve`), before the player's changes.
    pub in_mode: bool,
    /// In the set Play installs now.
    pub on: bool,
    /// Pulled in because a mod that is on needs it (its switch can still turn it off, and the
    /// mods that need it with it).
    pub needed_by: Option<String>,
    /// `RSG` or `SSG` for a mod legal in one category only.
    pub only: Option<String>,
    /// Not on the legal list (a practice mod, another mode's own mod, or the player's jar).
    pub not_legal: bool,
    pub custom: bool,
}

/// Every mod the player can turn on or off in `mode`: the mods of the mode's set and of `now`
/// (the set with the player's changes), every legal mod with a live build for the version, and
/// every mod of `extra.json` for it. Sorted: the mode's own set first, then the rest, each by
/// name; a switch never moves its row, so the next click lands where the player expects.
pub fn catalogue(meta: &ModMeta, mode_set: &ModSet, now: &ModSet, mode: Mode) -> Vec<Available> {
    let mc = mode_set.minecraft_version.as_str();
    let os = mode_set.os;
    let own = own_mods(mode);
    let mut rows: BTreeMap<String, Available> = BTreeMap::new();
    let only_of = |traits: &[String]| {
        if traits.iter().any(|t| t == "ssg-only") {
            Some("SSG".to_string())
        } else if traits.iter().any(|t| t == "rsg-only") {
            Some("RSG".to_string())
        } else {
            None
        }
    };
    for m in &meta.mods.mods {
        let Some(v) = m.version_for(mc) else { continue };
        let traits = meta.traits_of(m);
        let other_os = traits.iter().any(|t| t == "mac-only") && os != Os::MacOs;
        if other_os || m.obsolete || v.obsolete || !meta.legal.contains(&v.sha512) {
            continue;
        }
        if forbidden_in(mode.into()).contains(&m.modid.as_str()) {
            continue;
        }
        rows.insert(
            m.modid.clone(),
            Available {
                modid: m.modid.clone(),
                name: m.name.clone(),
                description: m.description.clone(),
                in_mode: false,
                on: false,
                needed_by: None,
                only: only_of(&traits),
                not_legal: false,
                custom: false,
            },
        );
    }
    for m in &meta.extra.mods {
        if m.version_for(mc).is_none() || rows.contains_key(&m.modid) {
            continue;
        }
        if forbidden_in(mode.into()).contains(&m.modid.as_str()) {
            continue;
        }
        rows.insert(
            m.modid.clone(),
            Available {
                modid: m.modid.clone(),
                name: m.name.clone(),
                description: m.description.clone(),
                in_mode: false,
                on: false,
                needed_by: None,
                only: None,
                not_legal: !own.contains(&m.modid.as_str()),
                custom: false,
            },
        );
    }
    for m in mode_set.mods.iter().chain(&now.mods) {
        rows.entry(m.modid.clone()).or_insert_with(|| Available {
            modid: m.modid.clone(),
            name: m.name.clone(),
            description: String::new(),
            in_mode: false,
            on: false,
            needed_by: None,
            only: None,
            not_legal: matches!(m.source, ModSource::Custom)
                || (matches!(m.source, ModSource::Modrinth { .. } | ModSource::Extra)
                    && !own.contains(&m.modid.as_str())),
            custom: matches!(m.source, ModSource::Custom),
        });
    }
    for row in rows.values_mut() {
        row.in_mode = mode_set.get(&row.modid).is_some();
        if let Some(e) = now.get(&row.modid) {
            row.on = true;
            if let Reason::Dependency { of } = &e.reason {
                row.needed_by = Some(now.get(of).map_or_else(|| of.clone(), |m| m.name.clone()));
            }
        }
    }
    let mut out: Vec<Available> = rows.into_values().collect();
    out.sort_by(|a, b| {
        b.in_mode
            .cmp(&a.in_mode)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}
