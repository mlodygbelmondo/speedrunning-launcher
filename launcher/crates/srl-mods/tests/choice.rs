//! The player's changes to a mode's set (`choice`), on the fixed manifests of tests/fixtures/meta.
use std::path::Path;

use srl_core::Mode;
use srl_mods::{
    Concern, CustomJar, ModChoice, ModMeta, ModSet, ModSource, Os, Reason, ResolveOptions, apply,
    catalogue, concerns, resolve,
};

fn meta() -> ModMeta {
    ModMeta::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/meta")).unwrap()
}

fn set(meta: &ModMeta, mode: Mode) -> ModSet {
    resolve(meta, mode, Os::Windows, &ResolveOptions::default()).unwrap()
}

fn choice(on: &[&str], off: &[&str]) -> ModChoice {
    ModChoice {
        on: on.iter().map(|s| s.to_string()).collect(),
        off: off.iter().map(|s| s.to_string()).collect(),
        custom: vec![],
    }
}

#[test]
fn no_changes_is_the_modes_own_set() {
    let meta = meta();
    let rsg = set(&meta, Mode::Rsg);
    assert_eq!(apply(&meta, rsg.clone(), &ModChoice::default()), rsg);
    assert!(concerns(&meta, &rsg, Mode::Rsg).is_empty());
    for mode in Mode::ALL {
        assert!(
            concerns(&meta, &set(&meta, mode), mode).is_empty(),
            "{mode:?}"
        );
    }
}

#[test]
fn setspawn_in_rsg_is_a_concern_but_goes_in() {
    let meta = meta();
    let mine = apply(&meta, set(&meta, Mode::Rsg), &choice(&["setspawnmod"], &[]));
    let e = mine.get("setspawnmod").expect("on");
    assert_eq!(e.reason, Reason::Chosen);
    assert!(mine.known[&e.sha512].allowed);
    assert_eq!(
        concerns(&meta, &mine, Mode::Rsg),
        vec![Concern::OtherCategory {
            name: e.name.clone(),
            only: "SSG".into()
        }]
    );
    // WorldPreview turned on in SSG, the other way round.
    let ssg = apply(
        &meta,
        set(&meta, Mode::Ssg),
        &choice(&["worldpreview"], &[]),
    );
    assert!(matches!(
        concerns(&meta, &ssg, Mode::Ssg).as_slice(),
        [Concern::OtherCategory { only, .. }] if only == "RSG"
    ));
}

#[test]
fn off_takes_the_mods_that_need_it_along() {
    let meta = meta();
    let rsg = set(&meta, Mode::Rsg);
    assert!(
        rsg.get("atum")
            .unwrap()
            .dependencies
            .contains(&"speedrunapi".to_string())
    );
    let mine = apply(&meta, rsg, &choice(&[], &["speedrunapi"]));
    assert!(mine.get("speedrunapi").is_none());
    assert!(mine.get("atum").is_none(), "Atum needs SpeedrunAPI");
    // Turning legal mods off never makes a run illegal.
    assert!(concerns(&meta, &mine, Mode::Rsg).is_empty());
}

#[test]
fn the_ranked_mod_outside_ranked_and_ranked_without_it() {
    let meta = meta();
    let rsg = apply(&meta, set(&meta, Mode::Rsg), &choice(&["mcsrranked"], &[]));
    assert!(rsg.get("mcsrranked").is_some());
    assert!(matches!(
        concerns(&meta, &rsg, Mode::Rsg).as_slice(),
        [Concern::NotLegal { .. }]
    ));
    let ranked = apply(
        &meta,
        set(&meta, Mode::Ranked),
        &choice(&[], &["mcsrranked"]),
    );
    assert!(matches!(
        concerns(&meta, &ranked, Mode::Ranked).as_slice(),
        [Concern::MissingOwn { .. }]
    ));
}

#[test]
fn forbidden_mods_never_go_in() {
    let meta = meta();
    let ranked = apply(&meta, set(&meta, Mode::Ranked), &choice(&["glacier"], &[]));
    assert!(
        ranked.get("glacier").is_none(),
        "the Ranked jar breaks Glacier"
    );
    let rsg = apply(&meta, set(&meta, Mode::Rsg), &choice(&["glacier"], &[]));
    assert!(rsg.get("glacier").is_some());
}

#[test]
fn the_players_own_jar() {
    let meta = meta();
    let jar = CustomJar {
        name: "My Mod".into(),
        file_name: "mymod-1.0.jar".into(),
        sha512: "ab".repeat(64),
        size: 10,
    };
    let mine = apply(
        &meta,
        set(&meta, Mode::Ssg),
        &ModChoice {
            custom: vec![jar.clone()],
            ..Default::default()
        },
    );
    let e = mine.get(&jar.modid()).unwrap();
    assert_eq!(e.source, ModSource::Custom);
    assert_eq!(
        concerns(&meta, &mine, Mode::Ssg),
        vec![Concern::Unknown {
            name: "My Mod".into()
        }]
    );
    let rows = catalogue(&meta, &set(&meta, Mode::Ssg), &mine, Mode::Ssg);
    let row = rows.iter().find(|r| r.custom).unwrap();
    assert!(row.on && row.not_legal && !row.in_mode);
}

/// The player's jar named like a jar of the set: one of them is renamed, to a name Windows can
/// create (a `:` there opens an alternate data stream, and the install fails).
#[test]
fn an_own_jar_with_a_taken_name_gets_a_windows_name() {
    let meta = meta();
    let ssg = set(&meta, Mode::Ssg);
    let atum = ssg.get("atum").unwrap().file_name.clone();
    let jar = CustomJar {
        name: "Atum".into(),
        file_name: atum.to_uppercase(),
        sha512: "ab".repeat(64),
        size: 10,
    };
    let mine = apply(
        &meta,
        ssg,
        &ModChoice {
            custom: vec![jar.clone()],
            ..Default::default()
        },
    );
    let names: Vec<&str> = mine.mods.iter().map(|m| m.file_name.as_str()).collect();
    assert!(names.contains(&atum.as_str()), "{names:?}");
    let own = &mine.get(&jar.modid()).unwrap().file_name;
    assert!(!own.eq_ignore_ascii_case(&atum), "{own}");
    for name in names {
        assert!(
            !name.contains(['<', '>', ':', '"', '/', '\\', '|', '?', '*']),
            "{name}"
        );
    }
}

#[test]
fn the_catalogue_lists_every_mod_for_the_version() {
    let meta = meta();
    let rsg = set(&meta, Mode::Rsg);
    let mine = apply(
        &meta,
        rsg.clone(),
        &choice(&["setspawnmod"], &["seedqueue"]),
    );
    let rows = catalogue(&meta, &rsg, &mine, Mode::Rsg);
    let row = |id: &str| {
        rows.iter()
            .find(|r| r.modid == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    assert!(row("setspawnmod").on && !row("setspawnmod").in_mode);
    assert_eq!(row("setspawnmod").only.as_deref(), Some("SSG"));
    assert!(!row("seedqueue").on && row("seedqueue").in_mode);
    assert!(row("mcsrranked").not_legal && !row("mcsrranked").on);
    assert!(row("peepopractice").not_legal);
    // Mods of other versions and other systems are not offered.
    assert!(
        rows.iter()
            .all(|r| r.modid != "draftout" && r.modid != "sodiummac")
    );
    // The mode's own set first, whatever is on or off: a switch never moves its row.
    let first_other = rows.iter().position(|r| !r.in_mode).unwrap();
    assert!(rows[first_other..].iter().all(|r| !r.in_mode));
    assert!(rows[..first_other].iter().any(|r| r.modid == "seedqueue"));
    // Ranked never offers what its jar breaks.
    let ranked = set(&meta, Mode::Ranked);
    let rows = catalogue(&meta, &ranked, &ranked, Mode::Ranked);
    assert!(rows.iter().all(|r| r.modid != "glacier"));
}
