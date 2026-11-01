//! Detection and import on fake launcher trees under a fake home; never the real home folders.
use std::{
    fs,
    path::{Path, PathBuf},
};

use srl_import::{
    Choice, Launcher, Os, Part, SearchRoots, SkipWhy, Source, Target, detect, import, inspect,
};

struct Tmp(PathBuf);

impl Tmp {
    fn new(name: &str) -> Tmp {
        let dir = std::env::temp_dir().join(format!("srl-import-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

const OPTIONS_116: &str = "version:2567\nautoJump:false\nfov:0.25\nmouseSensitivity:0.3\n\
    renderDistance:8\nresourcePacks:[\"vanilla\",\"file/Dark.zip\"]\nincompatibleResourcePacks:[]\n\
    key_key.forward:key.keyboard.e\nkey_Create New World:key.keyboard.f6\n";

/// A MultiMC/Prism instance: `instance.cfg`, `mmc-pack.json` and a game directory.
fn mmc_instance(dir: &Path, name: &str, version: &str, played_ms: u64, game: &str) -> PathBuf {
    write(
        &dir.join("instance.cfg"),
        &format!("InstanceType=OneSix\nname={name}\nlastLaunchTime={played_ms}\n"),
    );
    write(
        &dir.join("mmc-pack.json"),
        &format!(
            r#"{{"components":[{{"uid":"org.lwjgl3","version":"3.2.2"}},{{"uid":"net.minecraft","version":"{version}"}}],"formatVersion":1}}"#
        ),
    );
    let g = dir.join(game);
    write(&g.join("options.txt"), OPTIONS_116);
    g
}

/// A speedrunner's 1.16.1 game directory: options, mod settings, a pack, worlds.
fn full_game_dir(g: &Path) {
    write(&g.join("options.txt"), OPTIONS_116);
    write(&g.join("config/mcsr/standardsettings.json"), "{\"fov\":70}");
    write(&g.join("config/mcsr/atum.json"), "{\"seed\":\"\"}");
    write(&g.join("config/mcsr/seedqueue.json"), "{}");
    write(&g.join("config/mcsr/atum/rsg-attempts.txt"), "4021");
    write(&g.join("speedrunigt/options.txt"), "timer_position:10\n");
    write(&g.join("speedrunigt/latest_world.json"), "{}");
    write(&g.join("resourcepacks/Dark.zip"), "PK zip");
    write(&g.join("saves/Practice Seed/level.dat"), "nbt");
    write(&g.join("saves/Practice Seed/region/r.0.0.mca"), "region");
    write(&g.join("saves/New World/level.dat"), "nbt2");
    fs::create_dir_all(g.join("saves/New World/datapacks")).unwrap();
}

#[test]
fn finds_every_launcher_best_match_first() {
    let t = Tmp::new("detect");
    let home = t.0.join("home");
    // MultiMC on the Desktop: a 1.16.1 instance played last week, a 1.20.1 one played today, one
    // never started (no game directory), and the temp folder it keeps.
    let mmc = home.join("Desktop/MultiMC");
    write(
        &mmc.join("multimc.cfg"),
        "InstanceDir=instances\nLanguage=en\n",
    );
    mmc_instance(
        &mmc.join("instances/RSG"),
        "RSG main",
        "1.16.1",
        1_700_000_000_000,
        ".minecraft",
    );
    mmc_instance(
        &mmc.join("instances/Modern"),
        "Survival",
        "1.20.1",
        1_790_000_000_000,
        ".minecraft",
    );
    write(&mmc.join("instances/Fresh/instance.cfg"), "name=Fresh\n");
    fs::create_dir_all(mmc.join("instances/_MMC_TEMP")).unwrap();
    // Another MultiMC, unpacked from its zip into Downloads, with its instances elsewhere.
    let mmc2 = home.join("Downloads/mmc-develop-win32/MultiMC");
    let elsewhere = t.0.join("D/Instances");
    write(
        &mmc2.join("multimc.cfg"),
        &format!("InstanceDir={}\n", elsewhere.display()),
    );
    mmc_instance(
        &elsewhere.join("FSG"),
        "FSG",
        "1.16.1",
        1_750_000_000_000,
        "minecraft",
    );
    // Prism's data folder (Windows layout), MCSR Launcher's install, the official launcher.
    let prism = home.join("AppData/Roaming/PrismLauncher");
    write(
        &prism.join("prismlauncher.cfg"),
        "[General]\nInstanceDir=instances\n",
    );
    mmc_instance(
        &prism.join("instances/Ranked"),
        "\"Ranked, 1.16.1\"",
        "1.16.1",
        1_760_000_000_000,
        "minecraft",
    );
    let mcsr = home.join("AppData/Local/MCSRLauncher/launcher/instances/ranked-1");
    write(
        &mcsr.join("instance.json"),
        r#"{"id":"ranked-1","displayName":"MCSR Ranked","minecraftVersion":"1.16.1","playTime":3600}"#,
    );
    write(&mcsr.join(".minecraft/options.txt"), OPTIONS_116);
    let official = home.join("AppData/Roaming/.minecraft");
    write(&official.join("options.txt"), "version:3465\nfov:0.0\n");
    let profile_dir = t.0.join("speedrun-profile");
    write(&profile_dir.join("options.txt"), OPTIONS_116);
    write(
        &official.join("launcher_profiles.json"),
        &format!(
            r#"{{"profiles":{{"a":{{"name":"Speedrun","gameDir":{},"lastVersionId":"fabric-loader-0.16.10-1.16.1","lastUsed":"2026-10-20T10:00:00.000Z"}},"b":{{"name":"Bad","gameDir":"relative/path"}},"c":{{"name":"Latest"}}}}}}"#,
            serde_json_str(&profile_dir)
        ),
    );

    let found = detect(&SearchRoots::for_home(&home, Os::Windows));
    let names: Vec<(&str, Launcher, Option<&str>)> = found
        .iter()
        .map(|s| (s.name.as_str(), s.launcher, s.version.as_deref()))
        .collect();
    // 1.16.1 first, the most recently played first among them; then the rest. MCSR Launcher
    // keeps no last-played time, so its options.txt's (written just now) counts.
    assert_eq!(
        names,
        [
            ("MCSR Ranked", Launcher::McsrLauncher, Some("1.16.1")),
            ("Speedrun", Launcher::Official, Some("1.16.1")),
            ("Ranked, 1.16.1", Launcher::Prism, Some("1.16.1")),
            ("FSG", Launcher::MultiMc, Some("1.16.1")),
            ("RSG main", Launcher::MultiMc, Some("1.16.1")),
            (".minecraft", Launcher::Official, Some("1.20.1")),
            ("Survival", Launcher::MultiMc, Some("1.20.1")),
        ]
    );
    // The never-started instance, the temp folder and the relative profile aren't sources.
    assert!(
        !found
            .iter()
            .any(|s| s.name == "Fresh" || s.name == "Bad" || s.name == "Latest")
    );
    let rsg = found.iter().find(|s| s.name == "RSG main").unwrap();
    assert_eq!(rsg.last_played, Some(1_700_000_000));
    assert_eq!(rsg.game_dir, mmc.join("instances/RSG/.minecraft"));
    let sp = found.iter().find(|s| s.name == "Speedrun").unwrap();
    assert_eq!(sp.last_played, Some(1_792_490_400));
    // Linux layout: the same home has none of its folders but the Desktop's MultiMC.
    let linux = detect(&SearchRoots::for_home(&home, Os::Linux));
    let mut l: Vec<_> = linux.iter().map(|s| s.name.as_str()).collect();
    l.sort();
    assert_eq!(l, ["FSG", "RSG main", "Survival"]);
}

fn serde_json_str(p: &Path) -> String {
    format!("\"{}\"", p.display().to_string().replace('\\', "\\\\"))
}

#[test]
fn linux_prism_flatpak_and_dot_minecraft() {
    let t = Tmp::new("linux");
    let home = t.0.join("home");
    let flat = home.join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher");
    write(&flat.join("prismlauncher.cfg"), "[General]\n");
    mmc_instance(
        &flat.join("instances/SSG"),
        "SSG",
        "1.16.1",
        1_000,
        "minecraft",
    );
    write(
        &home.join(".minecraft/options.txt"),
        "version:2567\nfov:0.0\n",
    );
    // MCSR Launcher's jar started from the home folder.
    let mcsr = home.join("launcher/instances/rsg");
    write(
        &mcsr.join("instance.json"),
        r#"{"id":"rsg","displayName":"RSG","minecraftVersion":"1.16.1"}"#,
    );
    write(&mcsr.join("minecraft/config/mcsr/atum.json"), "{}");
    let found = detect(&SearchRoots::for_home(&home, Os::Linux));
    let mut got: Vec<_> = found
        .iter()
        .map(|s| (s.name.as_str(), s.launcher))
        .collect();
    got.sort_by_key(|g| g.0);
    assert_eq!(
        got,
        [
            (".minecraft", Launcher::Official),
            ("RSG", Launcher::McsrLauncher),
            ("SSG", Launcher::Prism)
        ]
    );
    let rsg = found.iter().find(|s| s.name == "RSG").unwrap();
    assert!(!rsg.contents.options);
    assert_eq!(rsg.contents.mods, ["Atum"]);
    assert_eq!(
        found
            .iter()
            .find(|s| s.name == ".minecraft")
            .unwrap()
            .version
            .as_deref(),
        Some("1.16.1")
    );
}

#[test]
fn a_drive_has_launchers_at_its_root_and_one_folder_down() {
    let t = Tmp::new("drives");
    let drive = t.0.join("C");
    // `C:\Games\MultiMC`, `C:\PrismLauncher-Windows`; not two folders down, nor under Windows.
    let at = [
        ("Games/MultiMC", "Nested"),
        ("PrismLauncher-Windows", "Top"),
        ("Games/Old/MultiMC", "Too deep"),
        ("Windows/MultiMC", "System"),
    ];
    for (dir, name) in at {
        write(&drive.join(dir).join("multimc.cfg"), "");
        mmc_instance(
            &drive.join(dir).join("instances").join(name),
            name,
            "1.16.1",
            1_000,
            ".minecraft",
        );
    }
    let roots = SearchRoots {
        drives: vec![drive],
        ..SearchRoots::default()
    };
    let mut names: Vec<_> = detect(&roots).into_iter().map(|s| s.name).collect();
    names.sort();
    assert_eq!(names, ["Nested", "Top"]);
}

#[test]
fn a_chosen_folder_can_be_an_instance_its_game_dir_or_a_launcher() {
    let t = Tmp::new("inspect");
    let inst = t.0.join("MultiMC/instances/RSG");
    let game = mmc_instance(&inst, "RSG", "1.16.1", 5_000, ".minecraft");
    write(&t.0.join("MultiMC/multimc.cfg"), "");
    for dir in [&inst, &game] {
        let got = inspect(dir);
        assert_eq!(got.len(), 1, "{dir:?}");
        assert_eq!(got[0].game_dir, game);
        assert_eq!(got[0].name, "RSG");
    }
    assert_eq!(inspect(&inst)[0].version.as_deref(), Some("1.16.1"));
    // The game directory alone: the version from options.txt, the name from the instance.
    assert_eq!(inspect(&game)[0].version.as_deref(), Some("1.16.1"));
    assert_eq!(inspect(&t.0.join("MultiMC")).len(), 1);
    assert_eq!(inspect(&t.0.join("MultiMC/instances")).len(), 1);
    fs::create_dir_all(t.0.join("empty")).unwrap();
    assert!(inspect(&t.0.join("empty")).is_empty());
    assert!(inspect(&t.0.join("missing")).is_empty());
}

fn targets(root: &Path) -> Vec<Target> {
    let mut v: Vec<Target> = ["ranked", "rsg", "practice"]
        .iter()
        .map(|id| Target::new(id, root.join(id), "1.16.1"))
        .collect();
    v.push(
        Target::new("draftout", root.join("draftout"), "26.3")
            .with_seed("onboardAccessibility", "false"),
    );
    v
}

fn source(game: &Path, version: &str) -> Source {
    let mut s = inspect(game).pop().unwrap();
    s.version = Some(version.into());
    s
}

fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if e.file_type().unwrap().is_dir() {
                stack.push(p);
            } else if e.file_type().unwrap().is_file() {
                out.push((p.clone(), fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn imports_everything_into_1_16_1_and_keys_into_draftout() {
    let t = Tmp::new("full");
    let game = t.0.join("src/.minecraft");
    full_game_dir(&game);
    let src = source(&game, "1.16.1");
    assert_eq!(
        src.contents.mods,
        ["StandardSettings", "SpeedRunIGT", "Atum", "SeedQueue"]
    );
    assert_eq!((src.contents.resource_packs, src.contents.worlds), (1, 2));
    let before = snapshot(&game);
    let data = t.0.join("instances");
    // RSG was played already: its options and one mod file differ; a world has the same name.
    write(
        &data.join("rsg/options.txt"),
        "version:2567\nautoJump:true\nchatScale:0.5\n",
    );
    write(&data.join("rsg/config/mcsr/atum.json"), "{\"old\":true}");
    write(&data.join("rsg/config/mcsr/seedqueue.json"), "{}");
    write(&data.join("rsg/saves/New World/level.dat"), "mine");
    // Draftout has its own options with the seeded line.
    write(
        &data.join("draftout/options.txt"),
        "version:5023\nfov:0.0\nkey_key.forward:key.keyboard.w\nonboardAccessibility:false\nautoJump:true\n",
    );
    let mut steps = Vec::new();
    let report = import(
        &src,
        Choice {
            worlds: true,
            ..Choice::default()
        },
        &targets(&data),
        1_761_480_000,
        &mut |d, n| steps.push((d, n)),
    );

    // The source is untouched.
    assert_eq!(snapshot(&game), before);
    let (last, total) = *steps.last().unwrap();
    assert_eq!(last, total);

    let rsg = report.targets.iter().find(|r| r.id == "rsg").unwrap();
    assert!(rsg.full && rsg.error.is_none() && rsg.left_out.is_empty());
    assert_eq!(rsg.worlds_kept, 1);
    let o = fs::read_to_string(data.join("rsg/options.txt")).unwrap();
    assert!(
        o.contains("autoJump:false\n") && o.contains("chatScale:0.5\n"),
        "{o}"
    );
    assert!(
        o.contains("key_Create New World:key.keyboard.f6")
            && o.contains("resourcePacks:[\"vanilla\",\"file/Dark.zip\"]")
    );
    assert_eq!(
        fs::read_to_string(data.join("rsg/config/mcsr/atum.json")).unwrap(),
        "{\"seed\":\"\"}"
    );
    assert_eq!(
        fs::read_to_string(data.join("rsg/config/mcsr/atum/rsg-attempts.txt")).unwrap(),
        "4021"
    );
    assert_eq!(
        fs::read_to_string(data.join("rsg/speedrunigt/options.txt")).unwrap(),
        "timer_position:10\n"
    );
    assert!(!data.join("rsg/speedrunigt/latest_world.json").exists());
    assert!(data.join("rsg/resourcepacks/Dark.zip").is_file());
    assert_eq!(
        fs::read_to_string(data.join("rsg/saves/New World/level.dat")).unwrap(),
        "mine"
    );
    assert!(
        data.join("rsg/saves/Practice Seed/region/r.0.0.mca")
            .is_file()
    );
    // The replaced files are in the backup, under the same paths; unchanged ones aren't.
    let backup = rsg.backup.clone().unwrap();
    assert_eq!(backup, data.join("rsg/import-backup-1761480000"));
    assert_eq!(
        fs::read_to_string(backup.join("options.txt")).unwrap(),
        "version:2567\nautoJump:true\nchatScale:0.5\n"
    );
    assert_eq!(
        fs::read_to_string(backup.join("config/mcsr/atum.json")).unwrap(),
        "{\"old\":true}"
    );
    assert!(!backup.join("config/mcsr/seedqueue.json").exists());
    // An empty instance gets everything and needs no backup; the world's empty folder too.
    let ranked = report.targets.iter().find(|r| r.id == "ranked").unwrap();
    assert!(ranked.backup.is_none() && ranked.worlds_kept == 0);
    assert!(data.join("ranked/saves/New World/datapacks").is_dir());
    assert!(!data.join("ranked/saves/.New World.srl-import").exists());

    // Draftout: keys, sensitivity and FOV merged; the rest left out and said so.
    let d = report.targets.iter().find(|r| r.id == "draftout").unwrap();
    assert!(!d.full);
    assert_eq!(
        d.left_out,
        [Part::ModSettings, Part::ResourcePacks, Part::Worlds]
    );
    let o = fs::read_to_string(data.join("draftout/options.txt")).unwrap();
    assert_eq!(
        o,
        "version:5023\nfov:0.25\nkey_key.forward:key.keyboard.e\nonboardAccessibility:false\nautoJump:true\nmouseSensitivity:0.3\n"
    );
    assert!(!data.join("draftout/config").exists() && !data.join("draftout/saves").exists());
    assert!(d.backup.as_ref().unwrap().join("options.txt").is_file());

    // Again: nothing changed, so nothing is written and no backup is made.
    let again = import(
        &src,
        Choice::default(),
        &targets(&data),
        1_761_480_100,
        &mut |_, _| {},
    );
    assert!(
        again
            .targets
            .iter()
            .all(|r| r.files == 0 && r.backup.is_none()),
        "{again:#?}"
    );
}

#[test]
fn a_new_draftout_instance_still_gets_onboard_accessibility_off() {
    let t = Tmp::new("draftout-new");
    let game = t.0.join("src");
    full_game_dir(&game);
    let data = t.0.join("instances");
    import(
        &source(&game, "1.16.1"),
        Choice::default(),
        &targets(&data)[3..],
        1,
        &mut |_, _| {},
    );
    let o = fs::read_to_string(data.join("draftout/options.txt")).unwrap();
    assert!(o.starts_with("version:2567\n"), "{o}");
    assert!(
        o.contains("onboardAccessibility:false\n")
            && o.contains("key_key.forward:key.keyboard.e\n")
    );
    assert!(!o.contains("renderDistance") && !o.contains("Create New World"));
}

#[test]
fn the_checkboxes_decide_what_comes_over() {
    let t = Tmp::new("choice");
    let game = t.0.join("src");
    full_game_dir(&game);
    let src = source(&game, "1.16.1");
    let data = t.0.join("instances");
    let rsg = vec![Target::new("rsg", data.join("rsg"), "1.16.1")];
    // Only the resource packs: the packs and their two lines, no other setting.
    let only_packs = Choice {
        options: false,
        mod_settings: false,
        resource_packs: true,
        worlds: false,
    };
    let r = import(&src, only_packs, &rsg, 1, &mut |_, _| {});
    assert_eq!(r.targets[0].settings, 2);
    let o = fs::read_to_string(data.join("rsg/options.txt")).unwrap();
    assert!(
        o.contains("resourcePacks:") && !o.contains("autoJump"),
        "{o}"
    );
    assert!(data.join("rsg/resourcepacks/Dark.zip").is_file());
    assert!(!data.join("rsg/config").exists() && !data.join("rsg/saves").exists());
    // Settings without packs: the instance keeps its own pack list.
    fs::remove_dir_all(&data).unwrap();
    write(
        &data.join("rsg/options.txt"),
        "resourcePacks:[\"vanilla\"]\n",
    );
    let settings_only = Choice {
        options: true,
        mod_settings: false,
        resource_packs: false,
        worlds: false,
    };
    import(&src, settings_only, &rsg, 1, &mut |_, _| {});
    let o = fs::read_to_string(data.join("rsg/options.txt")).unwrap();
    assert!(
        o.contains("resourcePacks:[\"vanilla\"]\n") && o.contains("autoJump:false"),
        "{o}"
    );
    assert!(!data.join("rsg/resourcepacks").exists() && !data.join("rsg/config").exists());
    // Mod settings only.
    let mods_only = Choice {
        options: false,
        mod_settings: true,
        resource_packs: false,
        worlds: false,
    };
    import(
        &src,
        mods_only,
        &[Target::new("fsg", data.join("fsg"), "1.16.1")],
        1,
        &mut |_, _| {},
    );
    assert!(data.join("fsg/config/mcsr/standardsettings.json").is_file());
    assert!(!data.join("fsg/options.txt").exists());
}

#[test]
fn another_version_brings_only_what_is_safe() {
    let t = Tmp::new("other");
    let game = t.0.join("src");
    full_game_dir(&game);
    let src = source(&game, "1.20.1");
    let data = t.0.join("instances");
    let r = import(
        &src,
        Choice {
            worlds: true,
            ..Choice::default()
        },
        &targets(&data),
        1,
        &mut |_, _| {},
    );
    for tr in &r.targets {
        assert!(!tr.full);
        assert_eq!(
            tr.left_out,
            [Part::ModSettings, Part::ResourcePacks, Part::Worlds]
        );
    }
    let o = fs::read_to_string(data.join("rsg/options.txt")).unwrap();
    assert!(
        o.contains("fov:0.25") && !o.contains("renderDistance") && !o.contains("resourcePacks"),
        "{o}"
    );
    assert!(!data.join("rsg/config").exists());
    // An unknown version is another version too.
    let mut unknown = src.clone();
    unknown.version = None;
    assert!(!unknown.runs("1.16.1"));
}

#[test]
fn importing_a_folder_into_itself_is_refused() {
    let t = Tmp::new("self");
    let game = t.0.join("instances/rsg");
    full_game_dir(&game);
    let src = source(&game, "1.16.1");
    let before = snapshot(&game);
    let r = import(
        &src,
        Choice::default(),
        &[Target::new("rsg", &game, "1.16.1")],
        1,
        &mut |_, _| {},
    );
    assert!(r.targets[0].error.is_some());
    assert_eq!(snapshot(&game), before);
}

#[cfg(unix)]
#[test]
fn links_are_never_followed_and_huge_files_stay_out() {
    use std::os::unix::fs::symlink;
    let t = Tmp::new("links");
    let secret = t.0.join("outside/secret.txt");
    write(&secret, "token");
    let game = t.0.join("src");
    full_game_dir(&game);
    symlink(&secret, game.join("config/mcsr/link.json")).unwrap();
    symlink(t.0.join("outside"), game.join("config/outside-dir")).unwrap();
    symlink(t.0.join("outside"), game.join("saves/Linked World")).unwrap();
    symlink(&secret, game.join("speedrunigt/.useglobaloption")).unwrap();
    // A mod "config" bigger than any real one.
    let big = fs::File::create(game.join("config/huge.log")).unwrap();
    big.set_len(9 * 1024 * 1024).unwrap();
    let src = source(&game, "1.16.1");
    let data = t.0.join("instances");
    let r = import(
        &src,
        Choice {
            worlds: true,
            ..Choice::default()
        },
        &targets(&data)[..1],
        1,
        &mut |_, _| {},
    );
    let ranked = data.join("ranked");
    assert!(ranked.join("config/mcsr/atum.json").is_file());
    for p in [
        "config/mcsr/link.json",
        "config/outside-dir",
        "saves/Linked World",
        "speedrunigt/.useglobaloption",
        "config/huge.log",
    ] {
        assert!(fs::symlink_metadata(ranked.join(p)).is_err(), "{p}");
    }
    let why = |p: &str| {
        r.skipped
            .iter()
            .find(|s| s.path == Path::new(p))
            .map(|s| s.why)
    };
    assert_eq!(why("config/mcsr/link.json"), Some(SkipWhy::Link));
    assert_eq!(why("config/outside-dir"), Some(SkipWhy::Link));
    assert_eq!(why("saves/Linked World"), Some(SkipWhy::Link));
    assert_eq!(why("speedrunigt/.useglobaloption"), Some(SkipWhy::Link));
    assert_eq!(why("config/huge.log"), Some(SkipWhy::TooLarge));
    // A linked options.txt isn't read either.
    let game2 = t.0.join("src2");
    fs::create_dir_all(&game2).unwrap();
    symlink(&secret, game2.join("options.txt")).unwrap();
    write(&game2.join("config/a.json"), "{}");
    let src2 = source(&game2, "1.16.1");
    assert!(!src2.contents.options);
    import(
        &src2,
        Choice::default(),
        &[Target::new("x", t.0.join("x"), "1.16.1")],
        1,
        &mut |_, _| {},
    );
    assert!(!t.0.join("x/options.txt").exists());
    // A linked instance folder in a launcher isn't followed by detection.
    let home = t.0.join("home");
    let mmc = home.join("Desktop/MultiMC");
    write(&mmc.join("multimc.cfg"), "");
    fs::create_dir_all(mmc.join("instances")).unwrap();
    symlink(t.0.join("src"), mmc.join("instances/Linked")).unwrap();
    assert!(detect(&SearchRoots::for_home(&home, Os::Linux)).is_empty());
}

#[cfg(unix)]
#[test]
fn recorded_launchers_and_jingle_paths_are_found_anywhere_even_through_links() {
    use std::os::unix::fs::symlink;
    let t = Tmp::new("recorded");
    let home = t.0.join("home");
    fs::create_dir_all(&home).unwrap();
    // A MultiMC renamed and buried where no search looks, that Windows remembers running.
    let buried = t.0.join("E/stuff/old/speedrun/mmc");
    write(&buried.join("multimc.cfg"), "");
    mmc_instance(
        &buried.join("instances/RSG"),
        "RSG",
        "1.16.1",
        2_000,
        ".minecraft",
    );
    // Another one Jingle saw through a link in Downloads (a junction on Windows).
    let real = t.0.join("Games/MultiMC");
    write(&real.join("multimc.cfg"), "");
    mmc_instance(
        &real.join("instances/Ranked"),
        "Ranked",
        "1.16.1",
        3_000,
        ".minecraft",
    );
    fs::create_dir_all(home.join("Downloads")).unwrap();
    symlink(&real, home.join("Downloads/MultiMC")).unwrap();
    // An instance whose launcher keeps its instances elsewhere: found on its own.
    let lone = t.0.join("Instances/FSG");
    mmc_instance(&lone, "FSG", "1.16.1", 1_000, "minecraft");
    let seen = |p: PathBuf| serde_json_str(&p);
    write(
        &home.join(".config/Jingle/options.json"),
        &format!(
            r#"{{"seenPaths":{{{}:1771516766357,{}:1,{}:2,"relative/x":3}}}}"#,
            seen(home.join("Downloads/MultiMC/instances/Ranked/.minecraft")),
            seen(lone.join("minecraft")),
            seen(t.0.join("gone/.minecraft")),
        ),
    );
    let mut roots = SearchRoots::for_home(&home, Os::Windows);
    roots.recorded = vec![buried.clone(), t.0.join("deleted/MultiMC")];
    let found = detect(&roots);
    let mut got: Vec<_> = found
        .iter()
        .map(|s| (s.name.as_str(), s.game_dir.clone()))
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            ("FSG", lone.join("minecraft")),
            ("RSG", buried.join("instances/RSG/.minecraft")),
            // The real folder, not the link the record went through.
            ("Ranked", real.join("instances/Ranked/.minecraft")),
        ]
    );
    // The link alone still isn't followed by the ordinary search.
    fs::remove_file(home.join(".config/Jingle/options.json")).unwrap();
    assert!(detect(&SearchRoots::for_home(&home, Os::Windows)).is_empty());
}
