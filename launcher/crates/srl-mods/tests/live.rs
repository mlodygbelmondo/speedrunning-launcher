//! Against the live manifests and CDNs. Ignored by default; run it with a throwaway data root:
//!
//! ```text
//! SRL_HOME=$HOME/.cache/srl-dev/mods srl-cpu cargo test -j 4 -p srl-mods --test live -- --ignored --nocapture
//! ```
//!
//! Resolves all five modes for Linux and Windows and prints the sets, cross-checks the manifest
//! against legal-builds.csv, installs RSG and Ranked, and re-hashes every jar in `mods/`.
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use srl_core::{Downloader, Mode, Paths, Progress, download::file_sha512};
use srl_mods::{
    InstanceGuard, ModMeta, ModSet, ModSource, Os, ResolveOptions, Status, install, meta, mods_dir,
    resolve, status,
};

fn print_set(set: &ModSet) {
    println!(
        "\n== {} on {:?} (Minecraft {}, {} jars, {:.1} MB)",
        set.instance.id(),
        set.os,
        set.minecraft_version,
        set.mods.len(),
        set.size() as f64 / 1e6
    );
    for m in &set.mods {
        let source = match &m.source {
            ModSource::Legal => "legal".to_string(),
            ModSource::Modrinth { version_id, .. } => format!("modrinth:{version_id}"),
            ModSource::Extra => "extra.json".to_string(),
            ModSource::Custom => "custom".to_string(),
        };
        println!(
            "  {:<20} {:<30} {:>9} {:<18} {}",
            m.modid,
            m.version,
            m.size,
            source,
            &m.sha512[..16]
        );
    }
    for n in &set.notes {
        println!("  note: {n}");
    }
}

#[tokio::test]
#[ignore = "network: fetches the live manifests and downloads about 90 MB of jars"]
async fn live_resolve_and_install() {
    let root = std::env::var_os("SRL_HOME").expect("set SRL_HOME to a throwaway directory");
    let paths = Paths::new(root);
    let dl = Downloader::new().unwrap();
    let meta = ModMeta::load(&paths, &dl, true).await.unwrap();
    println!("meta notes: {:?}", meta.notes);
    println!(
        "mods.json: {} mods, {} versions; extra.json: {} mods; legal-builds.csv: {} hashes",
        meta.mods.mods.len(),
        meta.mods
            .mods
            .iter()
            .map(|m| m.versions.len())
            .sum::<usize>(),
        meta.extra.mods.len(),
        meta.legal.by_sha512.len()
    );

    // The manifest against the legal authority.
    let mut gaps = Vec::new();
    let mut total = 0;
    for m in &meta.mods.mods {
        for v in &m.versions {
            if !meta.legal.contains(&v.sha512) {
                total += 1;
                if v.target_version
                    .iter()
                    .any(|t| t == "1.16.1" || t == "26.3")
                {
                    gaps.push(format!("{} {} {:?}", m.modid, v.version, v.target_version));
                }
            }
        }
    }
    println!("mods.json builds not in legal-builds.csv: {total} in all, for 1.16.1/26.3: {gaps:?}");

    // Every mode on both desktop OSes; the sets must not differ between them.
    let mut sets = Vec::new();
    for mode in Mode::ALL {
        let linux = resolve(&meta, mode, Os::Linux, &ResolveOptions::default()).unwrap();
        let windows = resolve(&meta, mode, Os::Windows, &ResolveOptions::default()).unwrap();
        print_set(&linux);
        print_set(&windows);
        assert_eq!(linux.mods, windows.mods, "{mode:?}");
        for m in &linux.mods {
            if m.source == ModSource::Legal {
                assert!(meta.legal.contains(&m.sha512), "{}", m.modid);
            }
        }
        sets.push(linux);
    }

    // Install RSG and Ranked; they share most jars through the store.
    let guard = InstanceGuard::new();
    let mut hashes = BTreeSet::new();
    for mode in [Mode::Rsg, Mode::Ranked] {
        let set = sets.iter().find(|s| s.mode() == Some(mode)).unwrap();
        let before = status(&paths, set).await.unwrap();
        println!("\n{} status before: {}", mode.id(), summary(&before));
        let last = Arc::new(Mutex::new(None::<Progress>));
        let seen = last.clone();
        let report = install(
            &paths,
            &dl,
            set,
            &guard,
            Some(Arc::new(move |p| *seen.lock().unwrap() = Some(p))),
        )
        .await
        .unwrap();
        println!(
            "{} install: +{} ~{} -{} ={} (hard links {}, copies {}), last progress {:?}",
            mode.id(),
            report.added.len(),
            report.updated.len(),
            report.removed.len(),
            report.unchanged,
            report.hard_linked,
            report.copied,
            last.lock().unwrap()
        );

        // Re-hash what is in mods/ now: exactly the set, byte for byte.
        let dir = mods_dir(&paths, mode);
        let mut on_disk = BTreeSet::new();
        for e in std::fs::read_dir(&dir).unwrap() {
            let e = e.unwrap();
            assert!(e.file_type().unwrap().is_file(), "{:?}", e.path());
            on_disk.insert((
                e.file_name().to_string_lossy().into_owned(),
                file_sha512(&e.path()).unwrap(),
            ));
        }
        let wanted: BTreeSet<(String, String)> = set
            .mods
            .iter()
            .map(|m| (m.file_name.clone(), m.sha512.clone()))
            .collect();
        assert_eq!(on_disk, wanted, "{mode:?}");
        println!(
            "{} mods/: {} jars, every sha512 verified",
            mode.id(),
            on_disk.len()
        );
        hashes.extend(set.mods.iter().map(|m| m.sha512.clone()));

        assert_eq!(status(&paths, set).await.unwrap(), Status::UpToDate);
        let again = install(&paths, &dl, set, &guard, None).await.unwrap();
        assert!(!again.changed(), "a second install changes nothing");
    }

    // One store file per distinct jar.
    let store = paths.root().join("store/sha512");
    let mut stored = BTreeSet::new();
    for prefix in std::fs::read_dir(&store).unwrap() {
        for f in std::fs::read_dir(prefix.unwrap().path()).unwrap() {
            stored.insert(f.unwrap().file_name().to_string_lossy().into_owned());
        }
    }
    assert_eq!(stored, hashes);
    let jars = |mode: Mode| {
        sets.iter()
            .find(|s| s.mode() == Some(mode))
            .unwrap()
            .mods
            .len()
    };
    println!(
        "store: {} files for RSG ({} jars) + Ranked ({} jars)",
        stored.len(),
        jars(Mode::Rsg),
        jars(Mode::Ranked)
    );

    // The cache alone (no network) gives the same sets.
    let cached = ModMeta::from_dir(&meta::cache_dir(&paths)).unwrap();
    for s in &sets {
        let again = resolve(&cached, s.mode().unwrap(), s.os, &ResolveOptions::default()).unwrap();
        assert_eq!(again.mods, s.mods);
    }
}

fn summary(s: &Status) -> String {
    match s {
        Status::UpToDate => "up to date".into(),
        Status::UpdateAvailable(p) => format!(
            "update available: +{} ~{} -{}",
            p.add.len(),
            p.update.len(),
            p.remove.len()
        ),
        Status::IllegalJarPresent { illegal, .. } => format!(
            "illegal: {:?}",
            illegal.iter().map(|j| &j.file_name).collect::<Vec<_>>()
        ),
    }
}
