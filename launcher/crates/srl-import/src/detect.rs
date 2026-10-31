//! Finding the player's instances in other launchers. Everything here only reads, never follows
//! a link it doesn't have to, and caps how much it reads: the folders are the player's and may
//! hold anything.
//!
//! Where the launchers keep their instances (read from their public source and docs):
//!
//! - **MultiMC** is portable: wherever the zip was unpacked (the Desktop, Documents, Downloads,
//!   `C:\MultiMC`...). Its folder has `multimc.cfg`; `InstanceDir` there (default `instances`,
//!   relative to the folder) holds one folder per instance with `instance.cfg` (`name`,
//!   `lastLaunchTime` in ms) and `mmc-pack.json` (the `net.minecraft` component's version). The
//!   game directory is `<instance>/.minecraft` (or `minecraft`).
//! - **Prism Launcher** (and PolyMC, its predecessor) uses the same instance layout, with
//!   `prismlauncher.cfg` (`polymc.cfg`) in `%APPDATA%\PrismLauncher`,
//!   `~/.local/share/PrismLauncher`, the Flatpak's `~/.var/app/org.prismlauncher.PrismLauncher/
//!   data/PrismLauncher`, `~/Library/Application Support/PrismLauncher`, or a portable folder.
//! - **MCSR Launcher** keeps everything in `launcher/` under its working folder: the Windows
//!   installer puts it in `%LOCALAPPDATA%\MCSRLauncher` (so `...\MCSRLauncher\launcher\
//!   instances`); the jar works from wherever it was started. An instance is `instances/<id>/`
//!   with `instance.json` (`displayName`, `minecraftVersion`) and its game in `.minecraft`
//!   (`minecraft` on Linux and macOS). It doesn't save when an instance was last played.
//! - **The official launcher**: `%APPDATA%\.minecraft`, `~/.minecraft`, `~/Library/Application
//!   Support/minecraft`, plus every profile in its `launcher_profiles.json` with a game
//!   directory of its own.
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use crate::{Contents, Launcher, Source, VERSION_1_16_1};

/// Files bigger than this aren't read as text (a cfg, a json, options.txt).
pub(crate) const MAX_TEXT: u64 = 8 * 1024 * 1024;
/// At most this many entries of one folder are looked at.
const MAX_ENTRIES: usize = 2000;
/// At most this many folders are checked for a launcher in one search.
const MAX_DIRS: usize = 6000;
/// Folders not worth looking into for a portable launcher.
const SKIP: [&str; 10] = [
    "AppData",
    "Library",
    "node_modules",
    "Windows",
    "Program Files",
    "Program Files (x86)",
    "ProgramData",
    "$Recycle.Bin",
    "snap",
    "go",
];

/// The system the launchers lay their folders out for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
    MacOs,
}

impl Os {
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

/// Where to look. Tests build it from a fake home (`for_home`); the app uses `from_env`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchRoots {
    /// Folders a portable launcher is often unpacked in (home, Desktop, Documents, Downloads):
    /// each one, its subfolders and theirs.
    pub portable: Vec<PathBuf>,
    /// Drive roots (`C:\`): only their subfolders named like a launcher (`MultiMC*`, `Prism*`,
    /// `MCSR*`, `PolyMC*`).
    pub drives: Vec<PathBuf>,
    /// Data folders of installed launchers, checked as they are.
    pub launchers: Vec<PathBuf>,
    /// The official launcher's game folders.
    pub official: Vec<PathBuf>,
}

impl SearchRoots {
    /// The usual places for a player whose home folder is `home` on `os`.
    pub fn for_home(home: &Path, os: Os) -> SearchRoots {
        let h = |p: &str| home.join(p);
        let mut portable = vec![
            home.to_path_buf(),
            h("Desktop"),
            h("Documents"),
            h("Downloads"),
        ];
        let (launchers, official) = match os {
            Os::Windows => {
                portable.extend([h("OneDrive/Desktop"), h("OneDrive/Documents")]);
                let roaming = h("AppData/Roaming");
                let local = h("AppData/Local");
                (
                    vec![
                        roaming.join("PrismLauncher"),
                        roaming.join("PolyMC"),
                        local.join("MCSRLauncher"),
                    ],
                    vec![roaming.join(".minecraft")],
                )
            }
            Os::Linux => {
                let share = h(".local/share");
                (
                    vec![
                        share.join("PrismLauncher"),
                        share.join("PolyMC"),
                        share.join("multimc"),
                        share.join("MultiMC"),
                        h(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher"),
                        h(".var/app/org.polymc.PolyMC/data/PolyMC"),
                    ],
                    vec![
                        h(".minecraft"),
                        h(".var/app/com.mojang.Minecraft/.minecraft"),
                    ],
                )
            }
            Os::MacOs => {
                let support = h("Library/Application Support");
                (
                    vec![support.join("PrismLauncher"), support.join("PolyMC")],
                    vec![support.join("minecraft")],
                )
            }
        };
        SearchRoots {
            portable,
            drives: Vec::new(),
            launchers,
            official,
        }
    }

    /// This computer. `SRL_IMPORT_HOME` (tests) stands in for the home folder and nothing else
    /// is searched; otherwise the home folder's places, the user's own Desktop, Documents and
    /// Downloads (they may be named in the system's language or moved), `%APPDATA%`,
    /// `%LOCALAPPDATA%`, `$XDG_DATA_HOME` and the system drive.
    pub fn from_env() -> SearchRoots {
        let os = Os::current();
        if let Some(home) = std::env::var_os("SRL_IMPORT_HOME").filter(|v| !v.is_empty()) {
            return SearchRoots::for_home(Path::new(&home), os);
        }
        let Some(base) = directories::BaseDirs::new() else {
            return SearchRoots::default();
        };
        let mut roots = SearchRoots::for_home(base.home_dir(), os);
        if let Some(user) = directories::UserDirs::new() {
            for dir in [user.desktop_dir(), user.document_dir(), user.download_dir()]
                .into_iter()
                .flatten()
            {
                roots.portable.push(dir.to_path_buf());
            }
        }
        let env_dir = |name: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        match os {
            Os::Windows => {
                if let Some(roaming) = env_dir("APPDATA") {
                    roots.launchers.push(roaming.join("PrismLauncher"));
                    roots.launchers.push(roaming.join("PolyMC"));
                    roots.official.push(roaming.join(".minecraft"));
                }
                if let Some(local) = env_dir("LOCALAPPDATA") {
                    roots.launchers.push(local.join("MCSRLauncher"));
                }
                let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
                roots.drives.push(PathBuf::from(format!("{drive}\\")));
            }
            Os::Linux => {
                if let Some(data) = env_dir("XDG_DATA_HOME") {
                    roots.launchers.push(data.join("PrismLauncher"));
                    roots.launchers.push(data.join("PolyMC"));
                }
            }
            Os::MacOs => {}
        }
        roots
    }
}

/// Every instance found under `roots` that has something to import, best match first
/// (`sort`).
pub fn detect(roots: &SearchRoots) -> Vec<Source> {
    let mut found = Vec::new();
    let mut budget = MAX_DIRS;
    let mut launchers = Vec::new();
    for dir in &roots.launchers {
        if let Some(l) = launcher_at(dir) {
            launchers.push(l);
        }
    }
    for root in &roots.portable {
        walk_portable(root, 2, &mut budget, &mut launchers);
    }
    for drive in &roots.drives {
        for child in subdirs(drive) {
            let name = file_name(&child).to_ascii_lowercase();
            if ["multimc", "prism", "polymc", "mcsr"]
                .iter()
                .any(|p| name.starts_with(p))
            {
                walk_portable(&child, 1, &mut budget, &mut launchers);
            }
        }
    }
    let mut seen_launchers = HashSet::new();
    for (launcher, instances) in launchers {
        if seen_launchers.insert(identity(&instances)) {
            found.extend(instances_in(launcher, &instances));
        }
    }
    for dir in &roots.official {
        found.extend(official(dir));
    }
    finish(found)
}

/// A folder the player picked ("Choose folder…"): a game directory, an instance folder, or a
/// launcher's folder (all its instances). Empty when there is nothing to import there.
pub fn inspect(dir: &Path) -> Vec<Source> {
    if !is_dir(dir) {
        return Vec::new();
    }
    if let Some((launcher, instances)) = launcher_at(dir) {
        return finish(instances_in(launcher, &instances));
    }
    // MCSR Launcher's own `launcher` folder, or any launcher's `instances` folder.
    for instances in [dir.join("instances"), dir.to_path_buf()] {
        let found = instances_in(Launcher::Folder, &instances);
        if !found.is_empty() {
            let kind = if dir.join("instances").is_dir() && dir.join("options.json").is_file() {
                Launcher::McsrLauncher
            } else {
                Launcher::Folder
            };
            return finish(
                found
                    .into_iter()
                    .map(|s| Source {
                        launcher: kind,
                        ..s
                    })
                    .collect(),
            );
        }
    }
    if let Some(source) = instance(Launcher::Folder, dir) {
        return finish(vec![source]);
    }
    // The game directory itself: name it after the instance when it is `.minecraft` in one.
    let name = match file_name(dir).as_str() {
        ".minecraft" | "minecraft" => dir
            .parent()
            .map(file_name)
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| file_name(dir)),
        n => n.to_string(),
    };
    let official = dir.join("launcher_profiles.json").is_file();
    let kind = if official {
        Launcher::Official
    } else {
        Launcher::Folder
    };
    finish(
        game_dir_source(kind, name, dir, None, None)
            .into_iter()
            .collect(),
    )
}

/// Drops duplicates (one instance found through two roots) and sorts: 1.16.1 first, then the
/// most recently played, then by name.
fn finish(mut found: Vec<Source>) -> Vec<Source> {
    let mut seen = HashSet::new();
    found.retain(|s| seen.insert(identity(&s.game_dir)));
    sort(&mut found);
    found
}

/// Best match first: Minecraft 1.16.1, then the most recently played, then by name.
pub fn sort(sources: &mut [Source]) {
    sources.sort_by(|a, b| {
        let speedrun = |s: &Source| s.version.as_deref() == Some(VERSION_1_16_1);
        speedrun(b)
            .cmp(&speedrun(a))
            .then(b.last_played.cmp(&a.last_played))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

fn identity(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A real folder, not a link to one.
fn is_dir(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok_and(|m| m.is_dir())
}

fn is_file(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok_and(|m| m.is_file())
}

/// The subfolders of `dir` (not links), at most `MAX_ENTRIES`, sorted by name.
fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .take(MAX_ENTRIES)
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    out.sort();
    out
}

/// Looks for portable launchers in `dir` and up to `depth` levels below it.
fn walk_portable(dir: &Path, depth: u32, budget: &mut usize, out: &mut Vec<(Launcher, PathBuf)>) {
    if *budget == 0 {
        return;
    }
    *budget -= 1;
    if let Some(found) = launcher_at(dir) {
        out.push(found);
        return;
    }
    if depth == 0 {
        return;
    }
    for child in subdirs(dir) {
        let name = file_name(&child);
        if name.starts_with('.') || SKIP.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
            continue;
        }
        walk_portable(&child, depth - 1, budget, out);
    }
}

/// The launcher whose folder `dir` is, with its instances folder.
fn launcher_at(dir: &Path) -> Option<(Launcher, PathBuf)> {
    for (cfg, launcher) in [
        ("multimc.cfg", Launcher::MultiMc),
        ("prismlauncher.cfg", Launcher::Prism),
        ("polymc.cfg", Launcher::PolyMc),
    ] {
        let path = dir.join(cfg);
        if is_file(&path) {
            let instances = read_text(&path)
                .and_then(|t| ini_value(&t, "InstanceDir"))
                .filter(|v| !v.trim().is_empty())
                .map(|v| dir.join(v.trim()))
                .unwrap_or_else(|| dir.join("instances"));
            return Some((launcher, instances));
        }
    }
    let mcsr = dir.join("launcher").join("instances");
    if is_dir(&mcsr)
        && fs::read_dir(&mcsr)
            .is_ok_and(|mut d| d.any(|e| e.is_ok_and(|e| e.path().join("instance.json").is_file())))
    {
        return Some((Launcher::McsrLauncher, mcsr));
    }
    None
}

/// The instances in a launcher's instances folder.
fn instances_in(launcher: Launcher, dir: &Path) -> Vec<Source> {
    subdirs(dir)
        .iter()
        .filter_map(|inst| instance(launcher, inst))
        .collect()
}

/// One instance folder (MultiMC, Prism or PolyMC's `instance.cfg`, MCSR Launcher's
/// `instance.json`, or none: a folder with a game directory in it).
fn instance(launcher: Launcher, inst: &Path) -> Option<Source> {
    let game = [".minecraft", "minecraft"]
        .map(|n| inst.join(n))
        .into_iter()
        .filter(|g| is_dir(g))
        .max_by_key(|g| g.join("options.txt").is_file())?;
    let folder = file_name(inst);
    let cfg = inst.join("instance.cfg");
    let json = inst.join("instance.json");
    let (name, version, played, launcher) = if is_file(&cfg) {
        let text = read_text(&cfg).unwrap_or_default();
        let name = ini_value(&text, "name").unwrap_or_else(|| folder.clone());
        let played = ini_value(&text, "lastLaunchTime")
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|ms| *ms > 0)
            .map(|ms| ms / 1000);
        let version = read_text(&inst.join("mmc-pack.json"))
            .and_then(|t| pack_version(&t))
            .or_else(|| ini_value(&text, "IntendedVersion"));
        let launcher = match launcher {
            Launcher::Folder => Launcher::MultiMc,
            l => l,
        };
        (name, version, played, launcher)
    } else if is_file(&json) {
        let v: serde_json::Value = read_text(&json)
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let s = |k: &str| v[k].as_str().map(str::to_string).filter(|s| !s.is_empty());
        let launcher = match launcher {
            Launcher::Folder => Launcher::McsrLauncher,
            l => l,
        };
        (
            s("displayName").or_else(|| s("id")).unwrap_or(folder),
            s("minecraftVersion"),
            None,
            launcher,
        )
    } else {
        (folder, None, None, launcher)
    };
    game_dir_source(launcher, name, &game, version, played)
}

/// A source for a game directory, if it has anything to import. A version the launcher didn't
/// say comes from options.txt's data version; a last-played time it didn't record from the
/// game's own files.
fn game_dir_source(
    launcher: Launcher,
    name: String,
    game: &Path,
    version: Option<String>,
    played: Option<u64>,
) -> Option<Source> {
    let contents = contents(game);
    if contents.is_empty() {
        return None;
    }
    let version = version
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| options_version(game));
    let played = played.or_else(|| {
        ["options.txt", "logs/latest.log"]
            .iter()
            .filter_map(|f| mtime(&game.join(f)))
            .max()
    });
    Some(Source {
        launcher,
        name: clean_name(&name),
        game_dir: game.to_path_buf(),
        version,
        last_played: played,
        contents,
    })
}

/// A name for one line of the UI: no control characters, at most 60 characters.
fn clean_name(name: &str) -> String {
    let s: String = name.chars().filter(|c| !c.is_control()).take(60).collect();
    let s = s.trim().to_string();
    if s.is_empty() { "Minecraft".into() } else { s }
}

/// The official launcher's game folder and the profiles with their own game folder.
fn official(dir: &Path) -> Vec<Source> {
    let mut out = Vec::new();
    if !is_dir(dir) {
        return out;
    }
    if let Some(s) = game_dir_source(Launcher::Official, ".minecraft".into(), dir, None, None) {
        out.push(s);
    }
    let Some(profiles) = read_text(&dir.join("launcher_profiles.json"))
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    else {
        return out;
    };
    let Some(map) = profiles["profiles"].as_object() else {
        return out;
    };
    for p in map.values().take(200) {
        let Some(game) = p["gameDir"].as_str().filter(|g| !g.trim().is_empty()) else {
            continue;
        };
        let game = PathBuf::from(game.trim());
        if !game.is_absolute() || !is_dir(&game) {
            continue;
        }
        let version = p["lastVersionId"].as_str().and_then(version_from_id);
        let name = p["name"]
            .as_str()
            .filter(|n| !n.trim().is_empty())
            .or(p["lastVersionId"].as_str())
            .unwrap_or("Minecraft")
            .to_string();
        let played = p["lastUsed"].as_str().and_then(iso_seconds);
        out.extend(game_dir_source(
            Launcher::Official,
            name,
            &game,
            version,
            played,
        ));
    }
    out
}

/// The Minecraft version in an official launcher version id: `1.16.1`, `fabric-loader-0.16.10-1.16.1`.
fn version_from_id(id: &str) -> Option<String> {
    let last = id.rsplit('-').next()?;
    let looks = last.split('.').count() >= 2
        && last
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    looks.then(|| last.to_string())
}

/// `2026-10-24T18:03:11.123Z` (the official launcher's `lastUsed`) in Unix seconds.
fn iso_seconds(s: &str) -> Option<u64> {
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + h * 3600 + mi * 60 + sec).ok()
}

fn mtime(p: &Path) -> Option<u64> {
    let m = fs::symlink_metadata(p).ok()?;
    m.modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

/// A small text file, or `None` if it's missing, a link, too big or not text.
pub(crate) fn read_text(p: &Path) -> Option<String> {
    let m = fs::symlink_metadata(p).ok()?;
    if !m.is_file() || m.len() > MAX_TEXT {
        return None;
    }
    let bytes = fs::read(p).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// A value from a Qt-style ini file (`key=value`, any section), unquoted.
fn ini_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == key).then(|| {
            let v = v.trim();
            v.strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(v)
                .to_string()
        })
    })
}

/// The Minecraft version in `mmc-pack.json`.
fn pack_version(text: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    v["components"]
        .as_array()?
        .iter()
        .find(|c| c["uid"] == "net.minecraft")?["version"]
        .as_str()
        .map(str::to_string)
}

/// The version that last wrote the game directory's options.txt, by its data version.
fn options_version(game: &Path) -> Option<String> {
    let text = read_text(&game.join("options.txt"))?;
    let dv = crate::options::Options::parse(&text).data_version()?;
    version_of_data(dv).map(str::to_string)
}

/// Minecraft releases by the data version their options.txt carries (minecraft.wiki "Data
/// version"). Only exact matches: an unknown number is an unknown version.
pub fn version_of_data(data_version: u32) -> Option<&'static str> {
    const TABLE: [(u32, &str); 27] = [
        (1976, "1.14.4"),
        (2230, "1.15.2"),
        (2566, "1.16"),
        (2567, "1.16.1"),
        (2578, "1.16.2"),
        (2580, "1.16.3"),
        (2584, "1.16.4"),
        (2586, "1.16.5"),
        (2724, "1.17"),
        (2730, "1.17.1"),
        (2860, "1.18"),
        (2865, "1.18.1"),
        (2975, "1.18.2"),
        (3105, "1.19"),
        (3117, "1.19.1"),
        (3120, "1.19.2"),
        (3218, "1.19.3"),
        (3337, "1.19.4"),
        (3463, "1.20"),
        (3465, "1.20.1"),
        (3578, "1.20.2"),
        (3700, "1.20.4"),
        (3839, "1.20.6"),
        (3953, "1.21"),
        (3955, "1.21.1"),
        (4189, "1.21.4"),
        (4325, "1.21.5"),
    ];
    TABLE
        .iter()
        .find(|(dv, _)| *dv == data_version)
        .map(|(_, v)| *v)
}

/// Mod settings the import names, by the files those mods keep in the game directory (1.16.1
/// builds, read from their source): SpeedrunAPI-based mods use `config/mcsr/<mod>.json` (a
/// `<mod>.global` file there means the shared `~/.config/mcsr/config/<mod>.json`); older builds
/// had their own files.
pub const MOD_SETTINGS: [(&str, &[&str]); 7] = [
    (
        "StandardSettings",
        &[
            "config/mcsr/standardsettings.json",
            "config/standardoptions.txt",
        ],
    ),
    (
        "SpeedRunIGT",
        &["speedrunigt/options.txt", "config/speedrunigt/options.txt"],
    ),
    (
        "Atum",
        &["config/mcsr/atum.json", "config/atum/atum.properties"],
    ),
    ("SeedQueue", &["config/mcsr/seedqueue.json"]),
    (
        "Sodium",
        &["config/mcsr/sodium.json", "config/sodium-options.json"],
    ),
    ("WorldPreview", &["config/mcsr/worldpreview.json"]),
    ("SleepBackground", &["config/mcsr/sleepbackground.json"]),
];

/// SpeedRunIGT's per-instance settings outside `config/`: its options and the marker that says
/// to use the shared ones in the home folder.
pub const SPEEDRUNIGT_FILES: [&str; 2] =
    ["speedrunigt/options.txt", "speedrunigt/.useglobaloption"];

/// What a game directory has to import. Counts only: worlds can be thousands.
pub fn contents(game: &Path) -> Contents {
    let count = |dir: &str, keep: &dyn Fn(&fs::DirEntry) -> bool| {
        fs::read_dir(game.join(dir))
            .map(|d| {
                d.take(100_000)
                    .flatten()
                    .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                    .filter(|e| keep(e))
                    .count() as u32
            })
            .unwrap_or(0)
    };
    let mut mod_files = crate::apply::count_files(&game.join("config"), 5000);
    mod_files += SPEEDRUNIGT_FILES
        .iter()
        .filter(|f| is_file(&game.join(f)))
        .count() as u32;
    let mods = MOD_SETTINGS
        .iter()
        .filter(|(_, files)| files.iter().any(|f| is_file(&game.join(f))))
        .map(|(name, _)| *name)
        .collect();
    Contents {
        options: is_file(&game.join("options.txt")),
        mod_files,
        mods,
        resource_packs: count("resourcepacks", &|e| {
            e.file_type().is_ok_and(|t| t.is_dir())
                || e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("zip"))
        }),
        worlds: count("saves", &|e| e.file_type().is_ok_and(|t| t.is_dir())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_parsers() {
        assert_eq!(
            ini_value("[General]\nname=\"RSG, main\"\nlastLaunchTime=1\n", "name").as_deref(),
            Some("RSG, main")
        );
        assert_eq!(
            pack_version(r#"{"components":[{"uid":"org.lwjgl3","version":"3.2.2"},{"uid":"net.minecraft","version":"1.16.1"}]}"#)
                .as_deref(),
            Some("1.16.1")
        );
        assert_eq!(pack_version("not json"), None);
        assert_eq!(
            version_from_id("fabric-loader-0.16.10-1.16.1").as_deref(),
            Some("1.16.1")
        );
        assert_eq!(version_from_id("1.21.4").as_deref(), Some("1.21.4"));
        assert_eq!(version_from_id("latest-release"), None);
        assert_eq!(iso_seconds("1970-01-02T00:00:01.000Z"), Some(86_401));
        assert_eq!(iso_seconds("2026-10-26T12:00:00Z"), Some(1_793_016_000));
        assert_eq!(iso_seconds("garbage"), None);
        assert_eq!(version_of_data(2567), Some("1.16.1"));
        assert_eq!(version_of_data(9999), None);
        assert_eq!(clean_name("  a\u{7}b "), "ab");
        assert_eq!(clean_name(""), "Minecraft");
    }
}
