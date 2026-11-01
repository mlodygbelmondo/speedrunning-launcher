//! `install` and `status` without network: the content store is filled with small generated
//! "jars" first, so the `Downloader` finds every file already verified and never connects.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha512};
use srl_core::{Downloader, Mode, Paths};
use srl_mods::{
    InstanceBusy, InstanceGuard, KnownJar, ModEntry, ModSet, ModSource, Os, Reason, Status,
    install, mods_dir, status,
};

fn sha512(bytes: &[u8]) -> String {
    hex::encode(Sha512::digest(bytes))
}

/// A fresh SRL_HOME per test under the target directory (not /tmp).
fn home(name: &str) -> Paths {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("srl-mods-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    Paths::new(dir)
}

struct Jar {
    modid: &'static str,
    version: &'static str,
    bytes: Vec<u8>,
}

impl Jar {
    fn new(modid: &'static str, version: &'static str) -> Self {
        Self {
            modid,
            version,
            bytes: format!("fake jar {modid} {version}").into_bytes(),
        }
    }
    fn sha(&self) -> String {
        sha512(&self.bytes)
    }
    fn file_name(&self) -> String {
        format!("{}-{}.jar", self.modid, self.version)
    }
    fn entry(&self) -> ModEntry {
        ModEntry {
            modid: self.modid.into(),
            name: self.modid.into(),
            version: self.version.into(),
            file_name: self.file_name(),
            // Never fetched: the store already has the file.
            url: "http://127.0.0.1:9/never".into(),
            sha512: self.sha(),
            size: self.bytes.len() as u64,
            source: ModSource::Legal,
            reason: Reason::Recommended,
            dependencies: vec![],
        }
    }
    fn known(&self, allowed: bool) -> (String, KnownJar) {
        (
            self.sha(),
            KnownJar {
                modid: self.modid.into(),
                version: self.version.into(),
                allowed,
            },
        )
    }
}

fn put_in_store(paths: &Paths, jars: &[&Jar]) {
    for j in jars {
        let p = paths.store_sha512(&j.sha());
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, &j.bytes).unwrap();
    }
}

fn set_of(mode: Mode, jars: &[&Jar], known: Vec<(String, KnownJar)>) -> ModSet {
    let mut all: HashMap<String, KnownJar> = known.into_iter().collect();
    for j in jars {
        all.insert(j.sha(), j.known(true).1);
    }
    ModSet {
        instance: mode.into(),
        os: Os::Linux,
        minecraft_version: mode.minecraft_version().into(),
        mods: jars.iter().map(|j| j.entry()).collect(),
        notes: vec![],
        known: all,
        keep_installed: vec![],
    }
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[tokio::test]
async fn fresh_install_links_from_the_store_and_is_then_up_to_date() {
    let paths = home("fresh");
    let (a, b) = (Jar::new("atum", "2.7.2"), Jar::new("seedqueue", "1.8"));
    put_in_store(&paths, &[&a, &b]);
    let set = set_of(Mode::Rsg, &[&a, &b], vec![]);
    let dl = Downloader::new().unwrap();
    let guard = InstanceGuard::new();

    match status(&paths, &set).await.unwrap() {
        Status::UpdateAvailable(plan) => assert_eq!(plan.add.len(), 2),
        other => panic!("{other:?}"),
    }
    let report = install(&paths, &dl, &set, &guard, None).await.unwrap();
    assert_eq!(report.added.len(), 2);
    // Windows copies, so one mode's running game never pins another mode's jars.
    if cfg!(windows) {
        assert_eq!(report.copied, 2);
    } else {
        assert_eq!(report.hard_linked, 2);
    }
    let dir = mods_dir(&paths, Mode::Rsg);
    assert_eq!(names(&dir), ["atum-2.7.2.jar", "seedqueue-1.8.jar"]);
    assert_eq!(std::fs::read(dir.join("atum-2.7.2.jar")).unwrap(), a.bytes);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::symlink_metadata(dir.join("atum-2.7.2.jar")).unwrap();
        assert!(m.file_type().is_file(), "never a symlink");
        assert_eq!(m.nlink(), 2, "a hard link to the store");
    }
    assert_eq!(status(&paths, &set).await.unwrap(), Status::UpToDate);

    let again = install(&paths, &dl, &set, &guard, None).await.unwrap();
    assert!(!again.changed());
    assert_eq!(again.unchanged, 2);
}

#[tokio::test]
async fn outdated_builds_are_updated_and_foreign_jars_removed() {
    let paths = home("update");
    let old = Jar::new("seedqueue", "1.7.1");
    let new = Jar::new("seedqueue", "1.8");
    let atum = Jar::new("atum", "2.7.2");
    let glacier = Jar::new("glacier", "1.0.1");
    let foreign = Jar::new("fabric-api", "0.42");
    put_in_store(&paths, &[&new, &atum, &glacier]);
    let set = set_of(
        Mode::Rsg,
        &[&new, &atum],
        // Glacier is legal (just not in the set), the old SeedQueue is still a legal build.
        vec![old.known(true), glacier.known(true)],
    );
    let dir = mods_dir(&paths, Mode::Rsg);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(old.file_name()), &old.bytes).unwrap();
    std::fs::write(dir.join(glacier.file_name()), &glacier.bytes).unwrap();
    std::fs::write(dir.join("fabric-api.jar"), &foreign.bytes).unwrap();
    // The same right build twice under two names: one is removed.
    std::fs::write(dir.join(atum.file_name()), &atum.bytes).unwrap();
    std::fs::write(dir.join("atum-copy.jar"), &atum.bytes).unwrap();
    // Not jars: left alone (Fabric ignores them), except our own leftovers.
    std::fs::write(dir.join("notes.txt"), b"mine").unwrap();
    std::fs::write(dir.join(".srl-x.jar.tmp"), b"crash leftover").unwrap();

    let Status::IllegalJarPresent { illegal, plan } = status(&paths, &set).await.unwrap() else {
        panic!("fabric-api.jar is foreign");
    };
    assert_eq!(illegal.len(), 1);
    assert_eq!(illegal[0].file_name, "fabric-api.jar");
    assert!(illegal[0].known.is_none());
    assert_eq!(plan.update.len(), 1);
    assert_eq!(plan.update[0].from.file_name, "seedqueue-1.7.1.jar");
    assert_eq!(plan.update[0].to.version, "1.8");
    assert_eq!(plan.keep.len(), 1);
    assert_eq!(plan.remove.len(), 3);

    let report = install(
        &paths,
        &Downloader::new().unwrap(),
        &set,
        &InstanceGuard::new(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(report.updated.len(), 1);
    assert_eq!(report.removed.len(), 3);
    assert_eq!(
        names(&dir),
        ["atum-2.7.2.jar", "notes.txt", "seedqueue-1.8.jar"]
    );
    assert_eq!(status(&paths, &set).await.unwrap(), Status::UpToDate);
    // Glacier and the second atum are in the store, so they are simply gone from mods/. The
    // foreign jar and the old SeedQueue have no other copy: they are moved aside, not deleted.
    let mut aside: Vec<String> = report
        .set_aside
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    aside.sort();
    assert_eq!(aside, ["fabric-api.jar", "seedqueue-1.7.1.jar"]);
    let aside_dir = report.set_aside[0].parent().unwrap();
    assert_eq!(
        aside_dir.parent().unwrap(),
        paths.instance(Mode::Rsg).join("mods-removed")
    );
    assert_eq!(
        std::fs::read(aside_dir.join("fabric-api.jar")).unwrap(),
        foreign.bytes
    );
}

#[tokio::test]
async fn a_jar_not_allowed_in_the_mode_is_illegal() {
    let paths = home("illegal");
    let atum = Jar::new("atum", "2.7.2");
    let setspawn = Jar::new("setspawnmod", "4.2.1");
    put_in_store(&paths, &[&atum]);
    let set = set_of(Mode::Rsg, &[&atum], vec![setspawn.known(false)]);
    let dir = mods_dir(&paths, Mode::Rsg);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(atum.file_name()), &atum.bytes).unwrap();
    std::fs::write(dir.join(setspawn.file_name()), &setspawn.bytes).unwrap();
    match status(&paths, &set).await.unwrap() {
        Status::IllegalJarPresent { illegal, .. } => {
            assert_eq!(illegal[0].known.as_ref().unwrap().modid, "setspawnmod")
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_running_game_blocks_install() {
    let paths = home("running");
    let atum = Jar::new("atum", "2.7.2");
    put_in_store(&paths, &[&atum]);
    let set = set_of(Mode::Ranked, &[&atum], vec![]);
    let dl = Downloader::new().unwrap();
    let guard = InstanceGuard::new();

    let token = guard.game_started(Mode::Ranked).unwrap();
    assert!(guard.is_game_running(Mode::Ranked));
    assert_eq!(
        guard.game_started(Mode::Ranked).err(),
        Some(InstanceBusy::GameRunning(Mode::Ranked.into()))
    );
    let err = install(&paths, &dl, &set, &guard, None).await.unwrap_err();
    assert_eq!(
        err.downcast_ref::<InstanceBusy>(),
        Some(&InstanceBusy::GameRunning(Mode::Ranked.into()))
    );
    assert!(!mods_dir(&paths, Mode::Ranked).exists(), "nothing touched");
    // RSG plays in the same directory: blocked too, and told which game runs.
    let err = install(
        &paths,
        &dl,
        &set_of(Mode::Rsg, &[&atum], vec![]),
        &guard,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(
        err.downcast_ref::<InstanceBusy>(),
        Some(&InstanceBusy::GameRunning(Mode::Ranked.into()))
    );
    assert_eq!(
        guard.game_started(Mode::Ssg).err(),
        Some(InstanceBusy::GameRunning(Mode::Ranked.into()))
    );
    assert!(!guard.is_game_running(Mode::Rsg));
    assert_eq!(guard.game_in_dir(Mode::Rsg), Some(Mode::Ranked.into()));
    // Draftout's directory is not affected.
    install(
        &paths,
        &dl,
        &set_of(Mode::Draftout, &[&atum], vec![]),
        &guard,
        None,
    )
    .await
    .unwrap();

    drop(token);
    assert!(!guard.is_game_running(Mode::Ranked));
    let report = install(&paths, &dl, &set, &guard, None).await.unwrap();
    assert_eq!(report.added.len(), 1);
}

#[tokio::test]
async fn a_corrupted_store_file_is_downloaded_again() {
    // The store copy no longer matches its name, and the URL is unreachable: install must fail
    // rather than link a bad jar.
    let paths = home("corrupt");
    let atum = Jar::new("atum", "2.7.2");
    let p = paths.store_sha512(&atum.sha());
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, b"truncated").unwrap();
    let set = set_of(Mode::Rsg, &[&atum], vec![]);
    let res = install(
        &paths,
        &Downloader::new().unwrap(),
        &set,
        &InstanceGuard::new(),
        None,
    )
    .await;
    assert!(res.is_err());
    let dir = mods_dir(&paths, Mode::Rsg);
    assert!(!dir.join(atum.file_name()).exists());
}

#[tokio::test]
async fn modes_take_turns_in_the_shared_mods_folder() {
    let paths = home("turns");
    let atum = Jar::new("atum", "2.7.2");
    let preview = Jar::new("worldpreview", "7.0");
    let setspawn = Jar::new("setspawnmod", "4.1");
    put_in_store(&paths, &[&atum, &preview, &setspawn]);
    let (dl, guard) = (Downloader::new().unwrap(), InstanceGuard::new());
    let rsg = set_of(Mode::Rsg, &[&atum, &preview], vec![]);
    let ssg = set_of(Mode::Ssg, &[&atum, &setspawn], vec![]);
    install(&paths, &dl, &rsg, &guard, None).await.unwrap();
    install(&paths, &dl, &ssg, &guard, None).await.unwrap();
    assert_eq!(
        names(&mods_dir(&paths, Mode::Rsg)),
        vec![atum.file_name(), setspawn.file_name()]
    );
    assert_eq!(srl_mods::owner(&paths, Mode::Rsg), Some(Mode::Ssg.into()));
    // SSG's set is in place, yet RSG is up to date: the switch is no update.
    assert_eq!(status(&paths, &rsg).await.unwrap(), Status::UpToDate);
    assert_eq!(status(&paths, &ssg).await.unwrap(), Status::UpToDate);
    // A newer build for RSG is.
    let atum2 = Jar::new("atum", "2.8");
    let rsg2 = set_of(Mode::Rsg, &[&atum2, &preview], vec![atum.known(true)]);
    match status(&paths, &rsg2).await.unwrap() {
        Status::UpdateAvailable(plan) => assert_eq!(plan.update[0].to.version, "2.8"),
        other => panic!("{other:?}"),
    }
    // FSG never installed here: compared with mods/ as it is.
    let fsg = set_of(Mode::Fsg, &[&atum, &setspawn], vec![]);
    assert_eq!(status(&paths, &fsg).await.unwrap(), Status::UpToDate);
}

#[tokio::test]
async fn the_players_own_jar_comes_from_the_store_only() {
    let paths = home("custom");
    let mine = Jar::new("mymod", "1");
    let (dl, guard) = (Downloader::new().unwrap(), InstanceGuard::new());
    let mut entry = mine.entry();
    entry.source = ModSource::Custom;
    entry.url = String::new();
    let mut set = set_of(Mode::Rsg, &[], vec![mine.known(true)]);
    set.mods.push(entry);
    let err = install(&paths, &dl, &set, &guard, None).await.unwrap_err();
    assert!(err.to_string().contains("add the jar again"), "{err:#}");
    put_in_store(&paths, &[&mine]);
    install(&paths, &dl, &set, &guard, None).await.unwrap();
    assert_eq!(names(&mods_dir(&paths, Mode::Rsg)), vec![mine.file_name()]);
}

/// A jar the player put into `mods/` under the name of a set jar (another build, or their own
/// mod named the same) has no other copy: it goes aside like any foreign jar, never under the
/// set's jar.
#[tokio::test]
async fn a_foreign_jar_under_a_set_jars_name_is_kept_aside() {
    let paths = home("same-name");
    let atum = Jar::new("atum", "2.7.2");
    put_in_store(&paths, &[&atum]);
    let set = set_of(Mode::Rsg, &[&atum], vec![]);
    let dir = mods_dir(&paths, Mode::Rsg);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(atum.file_name()), b"the player's own build").unwrap();
    let report = install(
        &paths,
        &Downloader::new().unwrap(),
        &set,
        &InstanceGuard::new(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read(dir.join(atum.file_name())).unwrap(),
        atum.bytes
    );
    assert_eq!(report.set_aside.len(), 1, "{report:?}");
    assert_eq!(
        std::fs::read(&report.set_aside[0]).unwrap(),
        b"the player's own build"
    );
    assert_eq!(status(&paths, &set).await.unwrap(), Status::UpToDate);
}

/// SeedQueue's newest build isn't in legal-builds.csv yet: the legal 1.7.1 in mods/ stays, an
/// illegal build of it doesn't.
#[tokio::test]
async fn a_mod_waiting_for_legal_builds_keeps_its_installed_build() {
    let paths = home("keep-installed");
    let (atum, old) = (Jar::new("atum", "2.7.2"), Jar::new("seedqueue", "1.7.1"));
    put_in_store(&paths, &[&atum, &old]);
    let dl = Downloader::new().unwrap();
    let guard = InstanceGuard::new();
    install(
        &paths,
        &dl,
        &set_of(Mode::Rsg, &[&atum, &old], vec![]),
        &guard,
        None,
    )
    .await
    .unwrap();

    let mut set = set_of(Mode::Rsg, &[&atum], vec![old.known(true)]);
    set.keep_installed = vec!["seedqueue".into()];
    assert_eq!(status(&paths, &set).await.unwrap(), Status::UpToDate);
    install(&paths, &dl, &set, &guard, None).await.unwrap();
    let dir = mods_dir(&paths, Mode::Rsg);
    assert_eq!(names(&dir), ["atum-2.7.2.jar", "seedqueue-1.7.1.jar"]);

    // An illegalized build is not kept.
    let mut set = set_of(Mode::Rsg, &[&atum], vec![old.known(false)]);
    set.keep_installed = vec!["seedqueue".into()];
    install(&paths, &dl, &set, &guard, None).await.unwrap();
    assert_eq!(names(&dir), ["atum-2.7.2.jar"]);
}
