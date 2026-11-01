//! `resolve` on a fixed snapshot of the manifests (tests/fixtures/meta, trimmed from the live files
//! of 2026-10-26 by tests/fixtures/trim.py).
use std::path::Path;

use srl_core::{Instance, Mode};
use srl_mods::{
    ModMeta, ModSet, ModSource, Note, Os, Reason, ResolveOptions, resolve, resolve_ids,
};

fn meta() -> ModMeta {
    ModMeta::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/meta")).unwrap()
}

fn set(meta: &ModMeta, mode: Mode, os: Os) -> ModSet {
    resolve(meta, mode, os, &ResolveOptions::default()).unwrap()
}

/// The mc.sr recommended 1.16.1 set for Windows and Linux on 2026-10-26. Hermes 0.16.0 no longer
/// needs Hermes Core on 1.16.1 (research §2.2 predates that).
const RSG: [&str; 22] = [
    "antigone",
    "antiresourcereload",
    "atum",
    "chunkumulator",
    "fast_reset",
    "forceport",
    "hermes",
    "krypton",
    "lazydfu",
    "lazystronghold",
    "lithium",
    "mcsrfairplay",
    "seedqueue",
    "sleepbackground",
    "sodium",
    "speedrunapi",
    "speedrunigt",
    "standardsettings",
    "starlight",
    "state-output",
    "voyager",
    "worldpreview",
];

fn with(base: &[&str], add: &[&str], remove: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = base
        .iter()
        .chain(add)
        .filter(|m| !remove.contains(m))
        .map(|s| s.to_string())
        .collect();
    v.sort();
    v
}

fn ids(set: &ModSet) -> Vec<String> {
    set.modids().into_iter().map(String::from).collect()
}

#[test]
fn rsg_is_the_recommended_set_on_windows_and_linux() {
    let meta = meta();
    for os in [Os::Linux, Os::Windows] {
        let s = set(&meta, Mode::Rsg, os);
        assert_eq!(ids(&s), with(&RSG, &[], &[]), "{os:?}");
        assert!(s.notes.is_empty(), "{:?}", s.notes);
        assert_eq!(s.minecraft_version, "1.16.1");
        assert!(s.mods.iter().all(|m| m.source == ModSource::Legal));
        let seedqueue = s.get("seedqueue").unwrap();
        assert_eq!(seedqueue.version, "1.8+1.16.1");
        assert_eq!(seedqueue.file_name, "seedqueue-1.8+1.16.1.jar");
        assert_eq!(seedqueue.dependencies, ["atum", "speedrunapi"]);
    }
}

#[test]
fn every_legal_entry_is_in_legal_builds() {
    let meta = meta();
    for mode in Mode::ALL {
        for os in Os::ALL {
            for m in &set(&meta, mode, os).mods {
                if m.source == ModSource::Legal {
                    assert!(
                        meta.legal.contains(&m.sha512),
                        "{mode:?} {os:?} {}",
                        m.modid
                    );
                }
            }
        }
    }
}

#[test]
fn macos_swaps_sodium_and_adds_mac_mods() {
    let meta = meta();
    let s = set(&meta, Mode::Rsg, Os::MacOs);
    assert_eq!(
        ids(&s),
        with(
            &RSG,
            &["sodiummac", "retino", "boundlesswindow"],
            &["sodium"]
        )
    );
    assert_eq!(
        s.notes,
        [Note::Replaced {
            kept: "sodiummac".into(),
            dropped: "sodium".into()
        }]
    );
}

#[test]
fn ranked_is_rsg_plus_the_newest_ranked_build() {
    let meta = meta();
    let s = set(&meta, Mode::Ranked, Os::Windows);
    assert_eq!(ids(&s), with(&RSG, &["mcsrranked"], &[]));
    let ranked = s.get("mcsrranked").unwrap();
    // Modrinth's newest, not extra.json's stale 5.8.26.
    assert_eq!(ranked.version, "6.0.4");
    assert_eq!(ranked.file_name, "mcsrranked-6.0.4.jar");
    assert!(ranked.sha512.starts_with("df99ce0d0cf60ef2"));
    assert!(matches!(ranked.source, ModSource::Modrinth { .. }));
    assert_eq!(ranked.reason, Reason::ModeExtra);
    for banned in ["fabric-api", "glacier", "practiceseedmod"] {
        assert!(s.get(banned).is_none());
    }
}

#[test]
fn fsg_is_rsg_plus_the_fsg_mod() {
    let meta = meta();
    let s = set(&meta, Mode::Fsg, Os::Linux);
    assert_eq!(ids(&s), with(&RSG, &["fsg-mod"], &[]));
    let fsg = s.get("fsg-mod").unwrap();
    assert_eq!(fsg.version, "5.3.0+MC1.16.1");
    assert_eq!(fsg.file_name, "FSG-Mod-5.3.0+MC1.16.1.jar");
    // Modrinth has no modids for dependencies; they come from extra.json.
    assert_eq!(fsg.dependencies, ["atum"]);
}

#[test]
fn ssg_adds_setspawn_and_drops_rsg_only_mods() {
    let meta = meta();
    let s = set(&meta, Mode::Ssg, Os::Linux);
    assert_eq!(
        ids(&s),
        with(&RSG, &["setspawnmod"], &["worldpreview", "lazystronghold"])
    );
    // chunkcacher is ssg-only too, but not recommended (and its 1.16.1 build was illegalized).
    assert!(s.get("chunkcacher").is_none());
}

#[test]
fn draftout_is_the_legal_26_3_set_draftout_accepts() {
    let meta = meta();
    // The legal 26.3 set also has Planifolia and SpeedRunIGT, but Draftout 1.18.0 refuses to
    // start with them in mods/.
    let base = ["forceport", "hermes", "krypton", "lithium", "sodium"];
    let s = set(&meta, Mode::Draftout, Os::Linux);
    assert_eq!(s.minecraft_version, "26.3");
    assert_eq!(ids(&s), with(&base, &["draftout"], &[]));
    // Every Draftout build is a Modrinth beta; the newest is taken anyway.
    assert_eq!(s.get("draftout").unwrap().version, "1.18.0");
    // Lithium 26.3 depends on mixinextras, which Fabric Loader provides.
    assert_eq!(s.get("lithium").unwrap().dependencies, ["mixinextras"]);
    assert_eq!(
        s.notes,
        [
            Note::NotAllowedInMode {
                modid: "planifolia".into()
            },
            Note::NotAllowedInMode {
                modid: "speedrunigt".into()
            },
        ]
    );
    // A jar of either, found in the Draftout instance, counts as illegal there.
    let rsg = set(&meta, Mode::Rsg, Os::Linux);
    let igt = &meta
        .mods
        .mods
        .iter()
        .find(|m| m.modid == "speedrunigt")
        .unwrap()
        .version_for("26.3")
        .unwrap()
        .sha512;
    assert!(!s.known[&igt.to_ascii_lowercase()].allowed);
    assert!(rsg.get("speedrunigt").is_some());

    let opts = ResolveOptions {
        draftout_extras: true,
        ..Default::default()
    };
    let s = resolve(&meta, Mode::Draftout, Os::Windows, &opts).unwrap();
    assert_eq!(
        ids(&s),
        with(
            &base,
            &["draftout", "nochatrestrictions", "no-telemetry"],
            &[]
        )
    );
    let s = resolve(&meta, Mode::Draftout, Os::MacOs, &opts).unwrap();
    assert_eq!(
        ids(&s),
        with(
            &base,
            &[
                "draftout",
                "nochatrestrictions",
                "no-telemetry",
                "commandqmod",
                "retino",
                "boundlesswindow"
            ],
            &[]
        )
    );
}

#[test]
fn accessibility_is_opt_in() {
    let meta = meta();
    let opts = ResolveOptions {
        accessibility: true,
        ..Default::default()
    };
    let s = resolve(&meta, Mode::Rsg, Os::Linux, &opts).unwrap();
    assert_eq!(ids(&s), with(&RSG, &["extra-options"], &[]));
    assert_eq!(
        s.get("extra-options").unwrap().reason,
        Reason::Accessibility
    );
}

#[test]
fn a_build_missing_from_legal_builds_is_left_out() {
    let mut meta = meta();
    let sha = set(&meta, Mode::Rsg, Os::Linux)
        .get("lazystronghold")
        .unwrap()
        .sha512
        .clone();
    meta.legal.by_sha512.remove(&sha);
    let s = set(&meta, Mode::Rsg, Os::Linux);
    assert_eq!(ids(&s), with(&RSG, &[], &["lazystronghold"]));
    assert_eq!(
        s.notes,
        [Note::NotInLegalBuilds {
            modid: "lazystronghold".into(),
            version: "1.1.4".into()
        }]
    );
    // Its legal build in mods/, if there is one, stays until the CSV lists the new one.
    assert_eq!(s.keep_installed, ["lazystronghold"]);
}

#[test]
fn losing_a_dependency_drops_its_dependents() {
    let mut meta = meta();
    let sha = set(&meta, Mode::Rsg, Os::Linux)
        .get("speedrunapi")
        .unwrap()
        .sha512
        .clone();
    meta.legal.by_sha512.remove(&sha);
    let s = set(&meta, Mode::Rsg, Os::Linux);
    for gone in [
        "speedrunapi",
        "atum",
        "seedqueue",
        "sodium",
        "fast_reset",
        "standardsettings",
    ] {
        assert!(s.get(gone).is_none(), "{gone}");
    }
    assert!(
        s.notes.contains(&Note::MissingDependency {
            modid: "seedqueue".into(),
            dependency: "atum".into()
        }) || s.notes.contains(&Note::MissingDependency {
            modid: "seedqueue".into(),
            dependency: "speedrunapi".into()
        })
    );
    assert!(s.get("lithium").is_some());
}

#[test]
fn a_dependency_is_pulled_in_whatever_its_flags() {
    let mut meta = meta();
    // Make hermes need hermes-core again, as it did before 0.16 (hermes-core is not recommended
    // and has no 1.16.1 build in the snapshot, so give it one).
    let core_version = {
        let core = meta.legal_mod("hermes-core").unwrap();
        let mut v = core.versions[0].clone();
        v.target_version = vec!["1.16.1".into()];
        v
    };
    for m in &mut meta.mods.mods {
        if m.modid == "hermes-core" {
            m.versions.insert(0, core_version.clone());
        }
        if m.modid == "hermes" {
            for v in &mut m.versions {
                v.dependencies = vec!["hermes-core".into()];
            }
        }
    }
    let s = set(&meta, Mode::Rsg, Os::Linux);
    let core = s.get("hermes-core").unwrap();
    assert_eq!(
        core.reason,
        Reason::Dependency {
            of: "hermes".into()
        }
    );
}

#[test]
fn modrinth_missing_falls_back_to_extra_json() {
    let mut meta = meta();
    meta.modrinth.clear();
    let s = set(&meta, Mode::Ranked, Os::Linux);
    let ranked = s.get("mcsrranked").unwrap();
    assert_eq!(ranked.version, "5.8.26");
    assert_eq!(ranked.source, ModSource::Extra);
    // extra.json says 5.8.26 depends on lithium, which RSG has anyway.
    assert_eq!(ranked.dependencies, ["lithium"]);
    assert!(s.notes.contains(&Note::ExtraFromExtraJson {
        modid: "mcsrranked".into(),
        version: "5.8.26".into()
    }));
}

#[test]
fn a_missing_required_extra_is_an_error() {
    let mut meta = meta();
    meta.modrinth.clear();
    meta.extra.mods.retain(|m| m.modid != "draftout");
    assert!(resolve(&meta, Mode::Draftout, Os::Linux, &ResolveOptions::default()).is_err());
}

#[test]
fn unknown_traits_are_left_out() {
    let mut meta = meta();
    for m in &mut meta.mods.mods {
        if m.modid == "voyager" {
            m.traits.push("windows-only".into());
        }
    }
    let s = set(&meta, Mode::Rsg, Os::Windows);
    assert!(s.get("voyager").is_none());
    assert_eq!(
        s.notes,
        [Note::UnknownTrait {
            modid: "voyager".into(),
            traits: vec!["windows-only".into()]
        }]
    );
}

#[test]
fn incompatibilities_keep_the_mode_extra() {
    let mut meta = meta();
    for m in &mut meta.extra.mods {
        if m.modid == "fsg-mod" {
            m.incompatibilities.push("worldpreview".into());
        }
    }
    let s = set(&meta, Mode::Fsg, Os::Linux);
    assert!(s.get("fsg-mod").is_some());
    assert!(s.get("worldpreview").is_none());
    assert!(s.notes.contains(&Note::Incompatible {
        kept: "fsg-mod".into(),
        dropped: "worldpreview".into()
    }));
}

#[test]
fn conditional_mods_add_categories() {
    let mut meta = meta();
    // As if mods.json had lost the trait: conditional-mods.json alone keeps SetSpawn out of RSG.
    for m in &mut meta.mods.mods {
        if m.modid == "setspawnmod" {
            m.traits.clear();
        }
    }
    assert!(
        set(&meta, Mode::Rsg, Os::Linux)
            .get("setspawnmod")
            .is_none()
    );
    assert!(
        set(&meta, Mode::Ssg, Os::Linux)
            .get("setspawnmod")
            .is_some()
    );
}

#[test]
fn known_jars_tell_legal_from_illegal() {
    let meta = meta();
    let sha = |modid: &str, mc: &str| {
        meta.legal_mod(modid)
            .or_else(|| meta.extra_mod(modid))
            .unwrap()
            .version_for(mc)
            .unwrap()
            .sha512
            .clone()
    };
    let allowed = |s: &ModSet, sha: &str| s.known.get(sha).map(|k| k.allowed);
    let rsg = set(&meta, Mode::Rsg, Os::Linux);
    let ssg = set(&meta, Mode::Ssg, Os::Linux);
    let ranked = set(&meta, Mode::Ranked, Os::Linux);
    let fsg = set(&meta, Mode::Fsg, Os::Linux);

    assert_eq!(allowed(&rsg, &sha("setspawnmod", "1.16.1")), Some(false));
    assert_eq!(allowed(&ssg, &sha("setspawnmod", "1.16.1")), Some(true));
    assert_eq!(allowed(&ssg, &sha("worldpreview", "1.16.1")), Some(false));
    assert_eq!(allowed(&rsg, &sha("glacier", "1.16.1")), Some(true));
    assert_eq!(allowed(&ranked, &sha("glacier", "1.16.1")), Some(false));
    assert_eq!(allowed(&rsg, &sha("fsg-mod", "1.16.1")), Some(false));
    assert_eq!(allowed(&fsg, &sha("fsg-mod", "1.16.1")), Some(true));
    // An older Ranked build (extra.json's 5.8.26) is recognised as Ranked, so it is an update.
    let old = ranked.known.get(&sha("mcsrranked", "1.16.1")).unwrap();
    assert_eq!((old.modid.as_str(), old.allowed), ("mcsrranked", true));
    // A 26.3 build in a 1.16.1 instance is not allowed.
    assert_eq!(allowed(&rsg, &sha("sodium", "26.3")), Some(false));
    // An older legal build the manifest dropped is still known through legal-builds.csv.
    let lazy_113 = meta
        .legal
        .by_sha512
        .iter()
        .find(|(_, b)| b.filename == "lazystronghold-1.1.3+1.16.1.jar")
        .map(|(h, _)| h.clone())
        .unwrap();
    assert_eq!(rsg.known[&lazy_113].modid, "lazystronghold");
    assert!(rsg.known[&lazy_113].allowed);
}

/// fsg-mod needs atum, and atum needs speedrunapi. Without speedrunapi, atum can't stay, so the
/// FSG mod can't either: resolve must end (it used to put atum back for fsg-mod forever) and
/// fail, because FSG without the FSG mod is not FSG.
#[test]
fn a_missing_second_level_dependency_ends_and_fails_the_mode() {
    let mut meta = meta();
    let sha = set(&meta, Mode::Rsg, Os::Linux)
        .get("speedrunapi")
        .unwrap()
        .sha512
        .clone();
    meta.legal.by_sha512.remove(&sha);
    let (tx, rx) = std::sync::mpsc::channel();
    let fsg = std::thread::spawn(move || {
        let _ = tx.send(resolve(
            &meta,
            Mode::Fsg,
            Os::Linux,
            &ResolveOptions::default(),
        ));
    });
    let r = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("resolve did not finish in 5 s");
    fsg.join().unwrap();
    let err = r
        .expect_err("FSG without the FSG mod must fail")
        .to_string();
    assert!(err.contains("fsg-mod"), "{err}");
}

/// The Ranked jar bundles Lithium, so a Lithium build missing from legal-builds.csv leaves
/// Lithium out of `mods/` but keeps the Ranked mod (it used to drop it and return Ok).
#[test]
fn ranked_keeps_its_mod_without_a_legal_lithium() {
    let mut meta = meta();
    let sha = set(&meta, Mode::Rsg, Os::Linux)
        .get("lithium")
        .unwrap()
        .sha512
        .clone();
    meta.legal.by_sha512.remove(&sha);
    let s = set(&meta, Mode::Ranked, Os::Linux);
    assert!(s.get("mcsrranked").is_some(), "{:?}", s.notes);
    assert!(s.get("lithium").is_none());
    assert!(
        !s.notes
            .iter()
            .any(|n| matches!(n, Note::MissingDependency { modid, .. } if modid == "mcsrranked")),
        "{:?}",
        s.notes
    );
}

/// A required extra that loses a dependency fails resolve, instead of quietly leaving the mode
/// without its own mod.
#[test]
fn a_required_extra_that_loses_a_dependency_is_an_error() {
    let mut meta = meta();
    let sha = set(&meta, Mode::Rsg, Os::Linux)
        .get("atum")
        .unwrap()
        .sha512
        .clone();
    meta.legal.by_sha512.remove(&sha);
    let err = resolve(&meta, Mode::Fsg, Os::Linux, &ResolveOptions::default())
        .expect_err("FSG without the FSG mod must fail")
        .to_string();
    assert!(err.contains("fsg-mod"), "{err}");
}

/// srl-practice's `PRACTICE.mod_ids(false, false)` (srl-mods doesn't depend on srl-practice;
/// srl-app's practice glue resolves the real list).
const PRACTICE: [&str; 9] = [
    "sodium",
    "speedrunapi",
    "lithium",
    "starlight",
    "lazydfu",
    "voyager",
    "antigone",
    "krypton",
    "speedrunigt",
];

fn practice(meta: &ModMeta, ids: &[&str], os: Os) -> anyhow::Result<ModSet> {
    resolve_ids(meta, Instance::Practice, "1.16.1", ids, os)
}

#[test]
fn an_explicit_list_gets_the_legal_checks_and_nothing_else() {
    let meta = meta();
    let mut want: Vec<String> = PRACTICE.iter().map(|s| s.to_string()).collect();
    want.sort();
    for os in [Os::Linux, Os::Windows] {
        let s = practice(&meta, &PRACTICE, os).unwrap();
        assert_eq!(ids(&s), want, "{os:?}");
        assert!(s.notes.is_empty(), "{:?}", s.notes);
        assert_eq!((s.instance, s.mode()), (Instance::Practice, None));
        assert!(
            s.mods
                .iter()
                .all(|m| m.source == ModSource::Legal && meta.legal.contains(&m.sha512))
        );
        // A run mod copied into the practice instance is recognised and not allowed there.
        let atum = set(&meta, Mode::Rsg, os)
            .get("atum")
            .unwrap()
            .sha512
            .clone();
        assert_eq!(s.known.get(&atum).map(|k| k.allowed), Some(false));
        let sodium = &s.get("sodium").unwrap().sha512;
        assert_eq!(s.known.get(sodium).map(|k| k.allowed), Some(true));
    }
    // The accessibility mod comes in when the list names it, whatever `recommended` says.
    let mut with_access = PRACTICE.to_vec();
    with_access.push("extra-options");
    let s = practice(&meta, &with_access, Os::Linux).unwrap();
    assert!(s.get("extra-options").is_some());
    // macOS: the list names Sodium Mac itself.
    let mut mac = PRACTICE.to_vec();
    mac[0] = "sodiummac";
    let s = practice(&meta, &mac, Os::MacOs).unwrap();
    assert!(s.get("sodiummac").is_some() && s.get("sodium").is_none());
    // Unknown ids, a mod for another system and Fabric API are left out with a note.
    let odd = [
        "sodium",
        "speedrunapi",
        "no-such-mod",
        "sodiummac",
        "fabric-api",
    ];
    let s = practice(&meta, &odd, Os::Linux).unwrap();
    assert_eq!(ids(&s), ["sodium", "speedrunapi"]);
    for id in ["no-such-mod", "sodiummac"] {
        let note = Note::NoBuild {
            modid: id.into(),
            minecraft: "1.16.1".into(),
        };
        assert!(s.notes.contains(&note), "{:?}", s.notes);
    }
    assert!(s.notes.contains(&Note::NotAllowedInMode {
        modid: "fabric-api".into()
    }));
    assert!(practice(&meta, &["nope"], Os::Linux).is_err());
}
