//! The mod set of a mode on an OS.
//!
//! The base is the mc.sr "Select recommended" set for the mode's Minecraft version: a mod is in it
//! if `mod.recommended && version.recommended && !obsolete`, its traits fit the mode's category
//! and the OS, and its jar is in `legal-builds.csv` by sha512. The mode then adds its non-legal
//! extra (MCSR Ranked, the FSG mod, Draftout), dependencies are pulled in, and incompatible pairs
//! are broken up. See `Mode` deltas in `extras`, `forbidden` and `allowed_only`.
use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use srl_core::{Instance, Mode};

use crate::meta::{MetaMod, MetaVersion, ModMeta, sanitize_file_name};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Os {
    Linux,
    Windows,
    MacOs,
}

impl Os {
    pub const ALL: [Os; 3] = [Os::Linux, Os::Windows, Os::MacOs];

    /// The OS this build runs on. Other Unixes get the Linux set: no trait targets them.
    pub fn current() -> Os {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveOptions {
    /// Adds the mods with the `accessibility` trait (today only Extra Options: FOV and distortion
    /// sliders). mc.sr never selects them by default, and abusing them is banned (A.8.12), so
    /// they are opt-in per player.
    pub accessibility: bool,
    /// Draftout only: adds the extras Draftout allows on top of the legal 26.3 set but does not
    /// require (No Chat Restrictions, No Telemetry, and CommandQMod on macOS).
    pub draftout_extras: bool,
}

/// Why an entry is in the set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reason {
    /// The recommended legal set.
    Recommended,
    /// Opted in with `ResolveOptions::accessibility`.
    Accessibility,
    /// Needed by another entry (e.g. SpeedrunAPI for Atum), whatever its own flags say.
    Dependency { of: String },
    /// The mode's own non-legal mod (MCSR Ranked, the FSG mod, Draftout and its extras).
    ModeExtra,
    /// Turned on by the player on the Mods page (`choice::apply`).
    Chosen,
}

/// Where the entry's metadata and hash come from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModSource {
    /// `mods.json`, and the sha512 is in `legal-builds.csv`.
    Legal,
    /// The Modrinth project of a non-legal extra; the hash is Modrinth's.
    Modrinth {
        project_id: String,
        version_id: String,
    },
    /// `extra.json`; the hash is its own.
    Extra,
    /// A jar the player added (`choice::CustomJar`): already in the content store, no URL, and
    /// no list says whether it is legal.
    Custom,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModEntry {
    pub modid: String,
    pub name: String,
    pub version: String,
    /// The name of the jar in `mods/`.
    pub file_name: String,
    pub url: String,
    /// Lower-case hex.
    pub sha512: String,
    pub size: u64,
    pub source: ModSource,
    pub reason: Reason,
    pub dependencies: Vec<String>,
}

/// Something resolving decided that the player or a developer may want to know.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Note {
    /// The manifest recommends a build that `legal-builds.csv` does not list, so it was left out
    /// (rule A.8.5: an unlisted jar is not legal).
    NotInLegalBuilds { modid: String, version: String },
    /// The mod has a trait this launcher does not know; left out rather than guessed.
    UnknownTrait { modid: String, traits: Vec<String> },
    /// A dependency could not be resolved (no legal build, or it lost a dependency of its own),
    /// so the mod that needs it was left out.
    MissingDependency { modid: String, dependency: String },
    /// Two mods are incompatible; one was left out.
    Incompatible { kept: String, dropped: String },
    /// An OS-specific build replaces the generic one (Sodium Mac for Sodium).
    Replaced { kept: String, dropped: String },
    /// Modrinth was not available for this extra, so `extra.json` (which can lag) was used.
    ExtraFromExtraJson { modid: String, version: String },
    /// An optional extra has no build for this Minecraft version.
    OptionalExtraMissing { modid: String },
    /// The URL of a build has no usable jar file name.
    BadFileName { modid: String, url: String },
    /// Legal, but the mode's own mod refuses to start with it (`allowed_only`).
    NotAllowedInMode { modid: String },
    /// An id of an explicit list (`resolve_ids`) with no live build for the Minecraft version
    /// that fits this OS (or no such mod in mods.json).
    NoBuild { modid: String, minecraft: String },
}

impl std::fmt::Display for Note {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Note::NotInLegalBuilds { modid, version } => {
                write!(f, "{modid} {version} is not in legal-builds.csv: left out")
            }
            Note::UnknownTrait { modid, traits } => {
                write!(f, "{modid} has unknown traits {traits:?}: left out")
            }
            Note::MissingDependency { modid, dependency } => {
                write!(
                    f,
                    "{modid} needs {dependency}, which can't be installed: left out"
                )
            }
            Note::Incompatible { kept, dropped } => {
                write!(f, "{dropped} is incompatible with {kept}: left out")
            }
            Note::Replaced { kept, dropped } => write!(f, "{kept} replaces {dropped}"),
            Note::ExtraFromExtraJson { modid, version } => {
                write!(
                    f,
                    "{modid} {version} from extra.json (Modrinth unavailable)"
                )
            }
            Note::OptionalExtraMissing { modid } => write!(f, "{modid} has no build: left out"),
            Note::BadFileName { modid, url } => write!(f, "{modid}: no jar name in {url}"),
            Note::NotAllowedInMode { modid } => {
                write!(f, "{modid} is not on the mode's own allowlist: left out")
            }
            Note::NoBuild { modid, minecraft } => {
                write!(
                    f,
                    "{modid} has no legal build for Minecraft {minecraft} on this system: left out"
                )
            }
        }
    }
}

/// A jar the launcher can recognise by sha512, to tell an outdated build from a foreign file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownJar {
    pub modid: String,
    /// The version from a manifest, or the file name from `legal-builds.csv` for an older legal
    /// build that the manifest no longer lists.
    pub version: String,
    /// Whether this jar may be in this mode's instance at all: a legal build whose traits fit the
    /// mode, or a build of the mode's own extra. A jar that is not allowed (an illegalized build,
    /// SetSpawn in RSG, the FSG mod in RSG, Glacier in Ranked) makes the instance illegal.
    pub allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModSet {
    /// Whose `mods/` it is for: a mode (`resolve`) or the practice instance (`resolve_ids`).
    pub instance: Instance,
    pub os: Os,
    pub minecraft_version: String,
    /// Sorted by modid.
    pub mods: Vec<ModEntry>,
    pub notes: Vec<Note>,
    /// Every jar the metadata knows for this instance, by sha512. Used by `status` and
    /// `install`.
    pub known: HashMap<String, KnownJar>,
}

impl ModSet {
    /// The mode, `None` for the practice instance.
    pub fn mode(&self) -> Option<Mode> {
        self.instance.mode()
    }

    pub fn get(&self, modid: &str) -> Option<&ModEntry> {
        self.mods.iter().find(|m| m.modid == modid)
    }

    pub fn modids(&self) -> Vec<&str> {
        self.mods.iter().map(|m| m.modid.as_str()).collect()
    }

    /// Total download size of the set.
    pub fn size(&self) -> u64 {
        self.mods.iter().map(|m| m.size).sum()
    }
}

/// The category a mode's runs count as, for the `rsg-only` and `ssg-only` traits. Ranked, FSG and
/// Draftout play random (if filtered) seeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Category {
    Rsg,
    Ssg,
}

impl Category {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Category::Rsg => "RSG",
            Category::Ssg => "SSG",
        }
    }

    /// The practice instance trains for random-seed runs (and its explicit list names no
    /// category mod), so it counts as RSG for the traits of its dependencies.
    pub(crate) fn of(instance: Instance) -> Self {
        match instance {
            Instance::Mode(Mode::Ssg) => Category::Ssg,
            Instance::Mode(Mode::Ranked | Mode::Rsg | Mode::Fsg | Mode::Draftout)
            | Instance::Practice => Category::Rsg,
        }
    }
}

/// Mod ids that may never be in a mode's instance, whatever the manifests say.
///
/// - Fabric API is banned on /mc (A.8.2.c); MCSR Ranked bundles the modules it needs and its
///   guide says never to add it next to the mod.
/// - The Ranked jar (6.0.4, `fabric.mod.json`) declares `breaks` on Glacier and Practice Seed
///   Mod, so the game would not even start with them.
pub fn forbidden(mode: Mode) -> &'static [&'static str] {
    match mode {
        Mode::Ranked => &["fabric-api", "fabric", "glacier", "practiceseedmod"],
        _ => &["fabric-api", "fabric"],
    }
}

/// The only mod ids a mode's own mod accepts in `mods/`, for modes that check. `None`: no check.
///
/// Draftout 1.18.0 refuses to start when `mods/` holds a mod outside its list (the
/// `ALLOWED_MODS` of `DisallowedModsChecker`, a pre-launch entrypoint: "Draftout blocked launch
/// because disallowed mods were found in your mods folder"). Planifolia and SpeedRunIGT are
/// legal on 26.3 but not on that list, so the Draftout set leaves them out. The list also holds
/// the macOS mods and the optional extras.
pub fn allowed_only(mode: Mode) -> Option<&'static [&'static str]> {
    match mode {
        Mode::Draftout => Some(&[
            "draftout",
            "sodium",
            "lithium",
            "krypton",
            "forceport",
            "hermes-core",
            "hermes",
            "boundlesswindow",
            "commandqmod",
            "retino",
            "nochatrestrictions",
            "no-telemetry",
        ]),
        _ => None,
    }
}

/// What no instance may have (Fabric API, banned on /mc), for instances that aren't modes.
const FORBIDDEN_EVERYWHERE: &[&str] = &["fabric-api", "fabric"];

/// [`forbidden`] for any instance.
pub(crate) fn forbidden_in(instance: Instance) -> &'static [&'static str] {
    match instance {
        Instance::Mode(mode) => forbidden(mode),
        Instance::Practice => FORBIDDEN_EVERYWHERE,
    }
}

/// Whether `modid` may never be in the instance: forbidden, or not on its mode's allowlist.
fn excluded(instance: Instance, modid: &str) -> bool {
    forbidden_in(instance).contains(&modid)
        || instance
            .mode()
            .and_then(allowed_only)
            .is_some_and(|a| !a.contains(&modid))
}

/// The non-legal extras of an instance: (modid, required). Only modes have any.
pub(crate) fn extras(
    instance: Instance,
    os: Os,
    opts: &ResolveOptions,
) -> Vec<(&'static str, bool)> {
    let Instance::Mode(mode) = instance else {
        return Vec::new();
    };
    match mode {
        Mode::Ranked => vec![("mcsrranked", true)],
        Mode::Fsg => vec![("fsg-mod", true)],
        Mode::Draftout => {
            let mut v = vec![("draftout", true)];
            if opts.draftout_extras {
                v.push(("nochatrestrictions", false));
                v.push(("no-telemetry", false));
                if os == Os::MacOs {
                    v.push(("commandqmod", false));
                }
            }
            v
        }
        Mode::Rsg | Mode::Ssg => vec![],
    }
}

/// Dependencies Fabric Loader itself provides (it has bundled MixinExtras since 0.15). Lithium
/// for 26.3 lists `mixinextras`, which no manifest has.
const LOADER_PROVIDED: [&str; 4] = ["mixinextras", "fabricloader", "minecraft", "java"];

/// On macOS these builds replace another mod: (mac build, generic mod).
const MAC_REPLACES: [(&str, &str); 1] = [("sodiummac", "sodium")];

/// Dependencies a mod brings inside its own jar (Fabric jar-in-jar), so they hold without a
/// separate jar: (mod, bundled modids). MCSR Ranked 6.0.4 declares `"lithium": ">=1.0"` and
/// carries `META-INF/jars/lithium-1.0+backport-0.6.7+1.16.1.jar`; `extra.json` lists the
/// dependency, so without this a Lithium build missing from `legal-builds.csv` would take the
/// Ranked mod out of Ranked.
const BUNDLED: [(&str, &[&str]); 1] = [("mcsrranked", &["lithium"])];

/// Whether `modid`'s dependency `dep` holds without a jar of its own in `mods/`.
pub(crate) fn provided(modid: &str, dep: &str) -> bool {
    LOADER_PROVIDED.contains(&dep)
        || BUNDLED
            .iter()
            .any(|(m, deps)| *m == modid && deps.contains(&dep))
}

/// What dependency resolution needs to know about the mode.
#[derive(Clone, Copy)]
pub(crate) struct Ctx<'a> {
    pub(crate) meta: &'a ModMeta,
    pub(crate) mc: &'a str,
    pub(crate) category: Category,
    pub(crate) os: Os,
    pub(crate) instance: Instance,
}

pub(crate) enum TraitFit {
    Fits { accessibility: bool },
    Excluded,
    Unknown(Vec<String>),
}

pub(crate) fn trait_fit(traits: &[String], category: Category, os: Os) -> TraitFit {
    let mut accessibility = false;
    let mut unknown = Vec::new();
    for t in traits {
        match t.as_str() {
            "rsg-only" if category != Category::Rsg => return TraitFit::Excluded,
            "ssg-only" if category != Category::Ssg => return TraitFit::Excluded,
            "mac-only" if os != Os::MacOs => return TraitFit::Excluded,
            "rsg-only" | "ssg-only" | "mac-only" => {}
            "accessibility" => accessibility = true,
            other => unknown.push(other.to_string()),
        }
    }
    if unknown.is_empty() {
        TraitFit::Fits { accessibility }
    } else {
        TraitFit::Unknown(unknown)
    }
}

pub(crate) fn entry(
    m: &MetaMod,
    v: &MetaVersion,
    source: ModSource,
    reason: Reason,
    notes: &mut Vec<Note>,
) -> Option<ModEntry> {
    let Some(file_name) = v.file_name() else {
        notes.push(Note::BadFileName {
            modid: m.modid.clone(),
            url: v.url.clone(),
        });
        return None;
    };
    Some(ModEntry {
        modid: m.modid.clone(),
        name: m.name.clone(),
        version: v.version.clone(),
        file_name,
        url: v.url.clone(),
        sha512: v.sha512.to_ascii_lowercase(),
        size: v.size,
        source,
        reason,
        dependencies: v.dependencies.clone(),
    })
}

/// Resolves the mod set of `mode` on `os`. Fails only if the mode's required extra has no build,
/// or loses one of its dependencies (Ranked without the Ranked mod is plain RSG under the wrong
/// name); everything else that goes wrong leaves a mod out and adds a `Note`.
pub fn resolve(meta: &ModMeta, mode: Mode, os: Os, opts: &ResolveOptions) -> Result<ModSet> {
    let mc = mode.minecraft_version();
    let instance = Instance::Mode(mode);
    let category = Category::of(instance);
    let mut notes = Vec::new();
    let mut set: BTreeMap<String, ModEntry> = BTreeMap::new();

    // 1. The recommended legal set.
    for m in &meta.mods.mods {
        let Some(v) = m.version_for(mc) else { continue };
        let accessibility = match trait_fit(&meta.traits_of(m), category, os) {
            TraitFit::Fits { accessibility } => accessibility,
            TraitFit::Excluded => continue,
            TraitFit::Unknown(traits) => {
                notes.push(Note::UnknownTrait {
                    modid: m.modid.clone(),
                    traits,
                });
                continue;
            }
        };
        let live = !m.obsolete && !v.obsolete;
        let (wanted, reason) = if accessibility {
            (opts.accessibility && live, Reason::Accessibility)
        } else {
            (m.recommended && v.recommended && live, Reason::Recommended)
        };
        if !wanted || forbidden(mode).contains(&m.modid.as_str()) {
            continue;
        }
        if excluded(instance, &m.modid) {
            notes.push(Note::NotAllowedInMode {
                modid: m.modid.clone(),
            });
            continue;
        }
        if !meta.legal.contains(&v.sha512) {
            notes.push(Note::NotInLegalBuilds {
                modid: m.modid.clone(),
                version: v.version.clone(),
            });
            continue;
        }
        if let Some(e) = entry(m, v, ModSource::Legal, reason, &mut notes) {
            set.insert(e.modid.clone(), e);
        }
    }
    if os == Os::MacOs {
        for (mac, generic) in MAC_REPLACES {
            if set.contains_key(mac) && set.remove(generic).is_some() {
                notes.push(Note::Replaced {
                    kept: mac.into(),
                    dropped: generic.into(),
                });
            }
        }
    }

    // 2. The mode's extras.
    let mut pinned = BTreeSet::new();
    let extras = extras(instance, os, opts);
    for &(modid, required) in &extras {
        match extra_entry(meta, modid, mc, &mut notes) {
            Some(e) => {
                pinned.insert(e.modid.clone());
                set.insert(e.modid.clone(), e);
            }
            None if required => bail!("{modid} has no build for Minecraft {mc}"),
            None => notes.push(Note::OptionalExtraMissing {
                modid: modid.into(),
            }),
        }
    }

    // 3. Dependencies, 4. incompatibilities, then drop whatever lost a dependency in 4.
    let ctx = Ctx {
        meta,
        mc,
        category,
        os,
        instance,
    };
    add_dependencies(&ctx, &mut set, &mut notes);
    break_incompatibilities(meta, &pinned, &mut set, &mut notes);
    drop_broken_dependents(&mut set, &mut notes);
    for &(modid, required) in &extras {
        if required && pinned.contains(modid) && !set.contains_key(modid) {
            let why = notes
                .iter()
                .rev()
                .find(|n| match n {
                    Note::MissingDependency { modid: m, .. } => m == modid,
                    Note::Incompatible { dropped, .. } => dropped == modid,
                    _ => false,
                })
                .map_or_else(|| "it was left out".to_string(), |n| n.to_string());
            bail!("{modid} can't be installed for Minecraft {mc}: {why}");
        }
    }

    let mut mods: Vec<ModEntry> = set.into_values().collect();
    make_file_names_unique(&mut mods);
    Ok(ModSet {
        instance,
        os,
        minecraft_version: mc.to_string(),
        mods,
        notes,
        known: known_jars(meta, instance, mc, os),
    })
}

/// A set from an explicit list of mod ids instead of a mode's rules: the practice instance's
/// (srl-practice's `PRACTICE.mod_ids`, which already swaps in Sodium Mac on macOS). Every id
/// gets the checks a mode's set gets: a mod in `mods.json` with a live build for `mc` whose
/// traits fit `os`, and that build in `legal-builds.csv` (rule A.8.5), never a forbidden mod.
/// `recommended` and the accessibility trait don't matter here: the list chose the mod. An id
/// that fails a check is left out with a note, as in a mode's set; dependencies are pulled in
/// and incompatible pairs broken up the same way. Only legal builds: no Modrinth or extra.json
/// extras. `known` allows only builds of the mods in the set, so any other jar in `mods/` (Atum
/// copied in from a run setup) makes `status` report the instance as not clean.
pub fn resolve_ids(
    meta: &ModMeta,
    instance: Instance,
    mc: &str,
    ids: &[&str],
    os: Os,
) -> Result<ModSet> {
    let category = Category::of(instance);
    let mut notes = Vec::new();
    let mut set: BTreeMap<String, ModEntry> = BTreeMap::new();
    for &id in ids {
        if excluded(instance, id) {
            notes.push(Note::NotAllowedInMode { modid: id.into() });
            continue;
        }
        let build = meta.legal_mod(id).and_then(|m| {
            let v = m.version_for(mc)?;
            let fits = matches!(
                trait_fit(&meta.traits_of(m), category, os),
                TraitFit::Fits { .. }
            );
            (fits && !m.obsolete && !v.obsolete).then_some((m, v))
        });
        let Some((m, v)) = build else {
            notes.push(Note::NoBuild {
                modid: id.into(),
                minecraft: mc.into(),
            });
            continue;
        };
        if !meta.legal.contains(&v.sha512) {
            notes.push(Note::NotInLegalBuilds {
                modid: m.modid.clone(),
                version: v.version.clone(),
            });
            continue;
        }
        if let Some(e) = entry(m, v, ModSource::Legal, Reason::Recommended, &mut notes) {
            set.insert(e.modid.clone(), e);
        }
    }
    let ctx = Ctx {
        meta,
        mc,
        category,
        os,
        instance,
    };
    add_dependencies(&ctx, &mut set, &mut notes);
    break_incompatibilities(meta, &BTreeSet::new(), &mut set, &mut notes);
    drop_broken_dependents(&mut set, &mut notes);
    if set.is_empty() && !ids.is_empty() {
        bail!(
            "none of {} has a legal build for Minecraft {mc}",
            ids.join(", ")
        );
    }
    let mut mods: Vec<ModEntry> = set.into_values().collect();
    make_file_names_unique(&mut mods);
    let mut known = known_jars(meta, instance, mc, os);
    for k in known.values_mut() {
        k.allowed &= mods.iter().any(|m| m.modid == k.modid);
    }
    Ok(ModSet {
        instance,
        os,
        minecraft_version: mc.to_string(),
        mods,
        notes,
        known,
    })
}

/// A mode extra: the newest Modrinth build if Modrinth is available, else `extra.json`.
pub(crate) fn extra_entry(
    meta: &ModMeta,
    modid: &str,
    mc: &str,
    notes: &mut Vec<Note>,
) -> Option<ModEntry> {
    let listed = meta.extra_mod(modid);
    if let Some(v) = meta.modrinth_latest(modid, mc) {
        let jar = v.jar()?;
        let file_name = sanitize_file_name(&jar.filename)?;
        // Modrinth lists dependencies by project id; extra.json has them by modid.
        let dependencies = listed
            .and_then(|m| m.version_for(mc))
            .map(|lv| lv.dependencies.clone())
            .unwrap_or_default();
        return Some(ModEntry {
            modid: modid.to_string(),
            name: listed.map_or_else(|| modid.to_string(), |m| m.name.clone()),
            version: v.version_number.clone(),
            file_name,
            url: jar.url.clone(),
            sha512: jar.hashes.sha512.to_ascii_lowercase(),
            size: jar.size,
            source: ModSource::Modrinth {
                project_id: v.project_id.clone(),
                version_id: v.id.clone(),
            },
            reason: Reason::ModeExtra,
            dependencies,
        });
    }
    let m = listed?;
    let v = m.version_for(mc)?;
    if crate::meta::MODRINTH_PROJECTS
        .iter()
        .any(|(id, _)| *id == modid)
    {
        notes.push(Note::ExtraFromExtraJson {
            modid: modid.into(),
            version: v.version.clone(),
        });
    }
    entry(m, v, ModSource::Extra, Reason::ModeExtra, notes)
}

/// A dependency's build: from `mods.json` if it has one that is legal and fits the mode
/// (whatever its `recommended` flag: Hermes Core is not recommended on its own), else from
/// `extra.json`.
fn dependency_entry(ctx: &Ctx, modid: &str, of: &str, notes: &mut Vec<Note>) -> Option<ModEntry> {
    let Ctx {
        meta,
        mc,
        category,
        os,
        instance,
    } = *ctx;
    if excluded(instance, modid) {
        return None;
    }
    let reason = Reason::Dependency { of: of.into() };
    if let Some(m) = meta.legal_mod(modid) {
        let v = m.version_for(mc)?;
        let fits = matches!(
            trait_fit(&meta.traits_of(m), category, os),
            TraitFit::Fits { .. }
        );
        if !fits || m.obsolete || v.obsolete || !meta.legal.contains(&v.sha512) {
            return None;
        }
        return entry(m, v, ModSource::Legal, reason, notes);
    }
    let m = meta.extra_mod(modid)?;
    let v = m.version_for(mc)?;
    entry(m, v, ModSource::Extra, reason, notes)
}

/// Pulls in every dependency, transitively. A mod that can't have one of its dependencies is
/// removed and marked as failed, and a failed mod is never added again: otherwise a missing
/// second-level dependency (fsg-mod needs atum, atum needs a speedrunapi that isn't legal) would
/// remove atum and add it back for fsg-mod forever. Every pass either adds a mod that never
/// failed or fails one more, so the loop ends.
pub(crate) fn add_dependencies(
    ctx: &Ctx,
    set: &mut BTreeMap<String, ModEntry>,
    notes: &mut Vec<Note>,
) {
    let mut failed: BTreeSet<String> = BTreeSet::new();
    loop {
        let mut changed = false;
        let needs: Vec<(String, String)> = set
            .values()
            .flat_map(|e| e.dependencies.iter().map(|d| (e.modid.clone(), d.clone())))
            .collect();
        for (modid, dep) in needs {
            if !set.contains_key(&modid) || set.contains_key(&dep) || provided(&modid, &dep) {
                continue;
            }
            changed = true;
            let found = if failed.contains(&dep) {
                None
            } else {
                dependency_entry(ctx, &dep, &modid, notes)
            };
            match found {
                Some(e) => {
                    set.insert(dep, e);
                }
                None => {
                    set.remove(&modid);
                    failed.insert(modid.clone());
                    failed.insert(dep.clone());
                    notes.push(Note::MissingDependency {
                        modid,
                        dependency: dep,
                    });
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// Incompatibility is symmetric: either side may list it, in `mods.json` or `extra.json`. The
/// mode's extras win; between two others, a dependency loses to a chosen mod, then the later
/// modid loses, so the outcome never depends on manifest order.
pub(crate) fn break_incompatibilities(
    meta: &ModMeta,
    pinned: &BTreeSet<String>,
    set: &mut BTreeMap<String, ModEntry>,
    notes: &mut Vec<Note>,
) {
    let mut pairs = BTreeSet::new();
    for m in meta.mods.mods.iter().chain(&meta.extra.mods) {
        for other in &m.incompatibilities {
            let (a, b) = if &m.modid < other {
                (m.modid.clone(), other.clone())
            } else {
                (other.clone(), m.modid.clone())
            };
            pairs.insert((a, b));
        }
    }
    for (a, b) in pairs {
        let (Some(ea), Some(eb)) = (set.get(&a), set.get(&b)) else {
            continue;
        };
        let rank = |e: &ModEntry| {
            (
                pinned.contains(&e.modid),
                !matches!(e.reason, Reason::Dependency { .. }),
            )
        };
        let (kept, dropped) = if rank(eb) > rank(ea) { (b, a) } else { (a, b) };
        set.remove(&dropped);
        notes.push(Note::Incompatible { kept, dropped });
    }
}

pub(crate) fn drop_broken_dependents(set: &mut BTreeMap<String, ModEntry>, notes: &mut Vec<Note>) {
    loop {
        let broken = set.values().find_map(|e| {
            e.dependencies
                .iter()
                .find(|d| !set.contains_key(*d) && !provided(&e.modid, d))
                .map(|d| (e.modid.clone(), d.clone()))
        });
        let Some((modid, dependency)) = broken else {
            break;
        };
        set.remove(&modid);
        notes.push(Note::MissingDependency { modid, dependency });
    }
}

/// Two builds with one file name (it has not happened, but nothing forbids it) would overwrite
/// each other in `mods/`; prefix the later ones with their modid (an own jar's `custom:<hash>`
/// with `-` for the `:`, which Windows file names can't hold).
pub(crate) fn make_file_names_unique(mods: &mut [ModEntry]) {
    let mut seen = BTreeSet::new();
    for m in mods.iter_mut() {
        if !seen.insert(m.file_name.to_ascii_lowercase()) {
            m.file_name = format!("{}-{}", m.modid.replace(':', "-"), m.file_name);
            seen.insert(m.file_name.to_ascii_lowercase());
        }
    }
}

/// Every jar the metadata can recognise, with whether it is allowed in this instance (on `mc`).
pub(crate) fn known_jars(
    meta: &ModMeta,
    instance: Instance,
    mc: &str,
    os: Os,
) -> HashMap<String, KnownJar> {
    let category = Category::of(instance);
    let fits = |m: &MetaMod| {
        !excluded(instance, &m.modid)
            && matches!(
                trait_fit(&meta.traits_of(m), category, os),
                TraitFit::Fits { .. }
            )
    };
    let mut known = HashMap::new();
    // Older legal builds first, so the richer manifest entries below overwrite them.
    for (sha512, b) in &meta.legal.by_sha512 {
        let allowed = meta
            .legal_mod(&b.modid)
            .is_none_or(|m| fits(m) && !m.obsolete)
            && !excluded(instance, &b.modid);
        known.insert(
            sha512.clone(),
            KnownJar {
                modid: b.modid.clone(),
                version: b.filename.clone(),
                allowed,
            },
        );
    }
    for m in &meta.mods.mods {
        for v in &m.versions {
            let allowed = meta.legal.contains(&v.sha512)
                && v.target_version.iter().any(|t| t == mc)
                && fits(m);
            known.insert(
                v.sha512.to_ascii_lowercase(),
                KnownJar {
                    modid: m.modid.clone(),
                    version: v.version.clone(),
                    allowed,
                },
            );
        }
    }
    let own: Vec<&str> = extras(
        instance,
        os,
        &ResolveOptions {
            draftout_extras: true,
            ..Default::default()
        },
    )
    .into_iter()
    .map(|(id, _)| id)
    .collect();
    for m in &meta.extra.mods {
        for v in &m.versions {
            let allowed =
                own.contains(&m.modid.as_str()) && v.target_version.iter().any(|t| t == mc);
            known
                .entry(v.sha512.to_ascii_lowercase())
                .or_insert(KnownJar {
                    modid: m.modid.clone(),
                    version: v.version.clone(),
                    allowed,
                });
        }
    }
    for (modid, versions) in &meta.modrinth {
        for v in versions {
            let allowed = own.contains(&modid.as_str()) && v.game_versions.iter().any(|g| g == mc);
            for f in &v.files {
                known.insert(
                    f.hashes.sha512.to_ascii_lowercase(),
                    KnownJar {
                        modid: modid.clone(),
                        version: v.version_number.clone(),
                        allowed,
                    },
                );
            }
        }
    }
    known
}
