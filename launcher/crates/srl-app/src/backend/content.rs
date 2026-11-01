//! What is inside an instance's game directory, for the Instances page: its worlds, resource
//! packs, config files, screenshots and logs, and the few changes the page can make to them
//! (delete worlds, turn a resource pack on or off, add or remove a pack).
//!
//! Everything here is plain file-system work on one game directory, so it runs on a background
//! thread (`cx.background_spawn`) and never on the UI thread: a speedrunner's `saves/` can hold
//! thousands of reset worlds, and their sizes take a while to add up.
//!
//! Worlds and resource packs are never deleted outright by a single click: they go to
//! `<data>/trash/` (a rename on the same drive), so a mistake can be undone by hand. Only the
//! "Clean up" of reset worlds deletes for good, because moving thousands of throwaway worlds to the
//! trash would only move the problem; the page says so before it does it.
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use anyhow::{Context, Result, bail};
use srl_core::Mode;

/// One world in `saves/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    /// The folder name in `saves/`, the world's id on disk.
    pub dir: String,
    /// The name the game's Singleplayer list shows (`LevelName`, colour codes removed), else the
    /// folder name.
    pub name: String,
    /// When the game last saved it (`LastPlayed`, else `level.dat`'s time), ms since the Unix
    /// epoch.
    pub played_ms: Option<i64>,
    /// The world seed, to copy after a good run.
    pub seed: Option<i64>,
    /// Bytes on disk, the whole folder.
    pub size: u64,
    /// The mode that made it, in the shared 1.16.1 directory (`srl_core::shared::WorldModes`).
    pub mode: Option<Mode>,
    /// A world a reset mod made (Atum's "Random Speedrun #12", "Set Speedrun #3", the practice
    /// mods' "Practice Seed"), which the Clean up offers to delete.
    pub reset: bool,
}

/// A file (or a folder, for unzipped resource packs) in one of the instance's folders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// The path relative to the folder the tab lists (`config/`, `resourcepacks/`, ...), with `/`.
    pub rel: String,
    pub size: u64,
    pub modified_ms: Option<i64>,
    pub folder: bool,
}

/// A resource pack and whether the game loads it (`options.txt`'s `resourcePacks`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pack {
    pub item: Item,
    pub on: bool,
}

/// Everything the Instances page lists for one game directory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Content {
    pub game_dir: PathBuf,
    /// The directory is there (the instance was installed or played once).
    pub exists: bool,
    /// Newest first.
    pub worlds: Vec<World>,
    pub packs: Vec<Pack>,
    /// `options.txt` first when it exists, then `config/` in path order.
    pub config: Vec<Item>,
    /// Newest first.
    pub screenshots: Vec<Item>,
    /// The game's logs and crash reports, newest first (`logs/…`, `crash-reports/…`).
    pub logs: Vec<Item>,
    /// Bytes of the whole game directory.
    pub size: u64,
    /// Bytes of `saves/`.
    pub saves_size: u64,
}

impl Content {
    pub fn reset_worlds(&self) -> impl Iterator<Item = &World> {
        self.worlds.iter().filter(|w| w.reset)
    }
}

/// Reads the game directory. `modes`: for the shared 1.16.1 directory, which mode made each world.
/// A missing directory is an empty instance, not an error (nothing installed yet).
pub fn scan(game_dir: &Path, modes: Option<&srl_core::shared::WorldModes>) -> Content {
    let mut c = Content {
        game_dir: game_dir.to_path_buf(),
        exists: game_dir.is_dir(),
        ..Content::default()
    };
    c.worlds = worlds(&game_dir.join("saves"), modes);
    c.saves_size = c.worlds.iter().map(|w| w.size).sum();
    let enabled = enabled_packs(&game_dir.join("options.txt"));
    c.packs = list(&game_dir.join("resourcepacks"), false)
        .into_iter()
        .filter(|i| i.folder || i.rel.to_ascii_lowercase().ends_with(".zip"))
        .map(|item| Pack {
            on: enabled.iter().any(|e| e == &format!("file/{}", item.rel)),
            item,
        })
        .collect();
    let options = game_dir.join("options.txt");
    if let Some(i) = item(&options, "options.txt") {
        c.config.push(i);
    }
    c.config.extend(
        list(&game_dir.join("config"), true)
            .into_iter()
            .filter(|i| !i.folder)
            .map(|mut i| {
                i.rel = format!("config/{}", i.rel);
                i
            }),
    );
    c.screenshots = list(&game_dir.join("screenshots"), false)
        .into_iter()
        .filter(|i| !i.folder && is_image(&i.rel))
        .collect();
    newest_first(&mut c.screenshots);
    for (dir, prefix) in [("logs", "logs/"), ("crash-reports", "crash-reports/")] {
        c.logs.extend(
            list(&game_dir.join(dir), false)
                .into_iter()
                .filter(|i| !i.folder)
                .map(|mut i| {
                    i.rel = format!("{prefix}{}", i.rel);
                    i
                }),
        );
    }
    newest_first(&mut c.logs);
    c.size = dir_size(game_dir);
    c
}

fn newest_first(items: &mut [Item]) {
    items.sort_by(|a, b| {
        b.modified_ms
            .cmp(&a.modified_ms)
            .then_with(|| a.rel.cmp(&b.rel))
    });
}

fn is_image(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".png") || n.ends_with(".jpg") || n.ends_with(".jpeg")
}

fn worlds(saves: &Path, modes: Option<&srl_core::shared::WorldModes>) -> Vec<World> {
    let Ok(entries) = fs::read_dir(saves) else {
        return Vec::new();
    };
    let mut v: Vec<World> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let dir = e.file_name().to_str()?.to_string();
            let path = e.path();
            let level = path.join("level.dat");
            // A folder without level.dat isn't a world the game lists (a half-deleted one).
            let meta = fs::metadata(&level).ok()?;
            let info = srl_practice::level::read_level_dat_file(&level).ok();
            let name = info
                .as_ref()
                .and_then(|l| l.display_name())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| dir.clone());
            let played_ms = info
                .as_ref()
                .and_then(|l| l.last_played)
                .filter(|t| *t > 0)
                .or_else(|| meta.modified().ok().and_then(ms));
            let seed = info.as_ref().and_then(|l| l.seed);
            let mode = modes.map(|m| m.mode_of(&path, None));
            Some(World {
                reset: is_reset_world(&name) || is_reset_world(&dir),
                size: dir_size(&path),
                dir,
                name,
                played_ms,
                seed,
                mode,
            })
        })
        .collect();
    v.sort_by(|a, b| {
        b.played_ms
            .cmp(&a.played_ms)
            .then_with(|| a.dir.cmp(&b.dir))
    });
    v
}

/// The names reset mods give their worlds: Atum's "Random Speedrun #N" and "Set Speedrun #N"
/// (and its older "Speedrun #N"), SeedQueue's the same, the practice seed mods' "Practice Seed",
/// and the game's own "New World" (a reset without Atum).
pub fn is_reset_world(name: &str) -> bool {
    let n = name.trim();
    let numbered = |prefix: &str| {
        n.strip_prefix(prefix).is_some_and(|rest| {
            let rest = rest.trim();
            !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
        })
    };
    numbered("Random Speedrun #")
        || numbered("Set Speedrun #")
        || numbered("Speedrun #")
        || numbered("Practice Seed #")
        || n == "Practice Seed"
        || n == "New World"
        || n.strip_prefix("New World (")
            .and_then(|r| r.strip_suffix(')'))
            .is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()))
}

/// The entries of `dir` (recursively when `deep`, never following links), sorted by path.
fn list(dir: &Path, deep: bool) -> Vec<Item> {
    let mut out = Vec::new();
    walk(dir, "", deep, &mut out, 0);
    out.sort_by_key(|i| i.rel.to_lowercase());
    out
}

fn walk(dir: &Path, prefix: &str, deep: bool, out: &mut Vec<Item>, depth: usize) {
    // Mod configs nest a few levels at most; a cap keeps a looping junction from running away.
    if depth > 8 || out.len() > 5000 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let Some(name) = e.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let rel = format!("{prefix}{name}");
        let Ok(t) = e.file_type() else { continue };
        if t.is_symlink() {
            continue;
        }
        if t.is_dir() {
            if deep {
                walk(&e.path(), &format!("{rel}/"), deep, out, depth + 1);
            } else if let Some(mut i) = item(&e.path(), &rel) {
                i.folder = true;
                i.size = dir_size(&e.path());
                out.push(i);
            }
        } else if let Some(i) = item(&e.path(), &rel) {
            out.push(i);
        }
    }
}

fn item(path: &Path, rel: &str) -> Option<Item> {
    let meta = fs::symlink_metadata(path).ok()?;
    Some(Item {
        rel: rel.to_string(),
        size: if meta.is_file() { meta.len() } else { 0 },
        modified_ms: meta.modified().ok().and_then(ms),
        folder: meta.is_dir(),
    })
}

fn ms(t: std::time::SystemTime) -> Option<i64> {
    Some(t.duration_since(UNIX_EPOCH).ok()?.as_millis() as i64)
}

/// Bytes under `dir`, links not followed.
pub fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

/// The packs `options.txt` turns on (`resourcePacks:["vanilla","file/x.zip"]`), as written.
pub fn enabled_packs(options: &Path) -> Vec<String> {
    fs::read_to_string(options)
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|l| l.strip_prefix("resourcePacks:"))
                .map(parse_list)
        })
        .unwrap_or_default()
}

/// A JSON-ish string list as options.txt writes it. Malformed input gives what parsed.
fn parse_list(s: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(s.trim()).unwrap_or_default()
}

/// Turns the pack `rel` (a name in `resourcepacks/`) on or off in `options.txt`. On puts it on
/// top (the game lists the highest-priority pack last). Writes atomically. Fails while the game
/// runs is the caller's business: the game rewrites options.txt when it exits.
pub fn set_pack(game_dir: &Path, rel: &str, on: bool) -> Result<()> {
    let path = game_dir.join("options.txt");
    let text = fs::read_to_string(&path).unwrap_or_default();
    let key = format!("file/{rel}");
    let mut found = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| match l.strip_prefix("resourcePacks:") {
            Some(list) => {
                found = true;
                let mut packs = parse_list(list);
                packs.retain(|p| p != &key);
                if on {
                    packs.push(key.clone());
                }
                format!(
                    "resourcePacks:{}",
                    serde_json::to_string(&packs).unwrap_or_else(|_| "[]".into())
                )
            }
            None => l.to_string(),
        })
        .collect();
    if !found && on {
        lines.push(format!(
            "resourcePacks:{}",
            serde_json::to_string(&["vanilla".to_string(), key]).unwrap_or_default()
        ));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    let tmp = path.with_extension("txt.part");
    fs::write(&tmp, out).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, &path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

/// `<data>/trash/<what>-<time>/`: where removed things go.
fn trash_dir(data_dir: &Path, what: &str) -> PathBuf {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    data_dir.join("trash").join(format!("{what}-{stamp}"))
}

/// Moves `rel` (a world folder in `saves/`, or a pack in `resourcepacks/`) of `game_dir` into the
/// trash. A name with a path in it is refused, so a crafted name can't reach outside the folder.
pub fn trash(data_dir: &Path, game_dir: &Path, folder: &str, rel: &str) -> Result<PathBuf> {
    if rel.is_empty() || rel.contains(['/', '\\']) || rel == "." || rel == ".." {
        bail!("not a plain name: {rel}");
    }
    let from = game_dir.join(folder).join(rel);
    if !from.exists() {
        bail!("{} is gone", from.display());
    }
    let to_dir = trash_dir(data_dir, folder);
    fs::create_dir_all(&to_dir).with_context(|| format!("creating {}", to_dir.display()))?;
    let to = to_dir.join(rel);
    fs::rename(&from, &to)
        .with_context(|| format!("moving {} to {}", from.display(), to.display()))?;
    Ok(to)
}

/// Deletes the reset worlds named `dirs` from `saves/` for good. Only folders that still read as
/// reset worlds are deleted (a renamed world is kept). Returns how many went and the bytes freed.
pub fn delete_reset_worlds(game_dir: &Path, dirs: &[String]) -> (usize, u64) {
    let saves = game_dir.join("saves");
    let mut n = 0;
    let mut bytes = 0;
    for d in dirs {
        if d.is_empty() || d.contains(['/', '\\']) || d == "." || d == ".." {
            continue;
        }
        let path = saves.join(d);
        let name = srl_practice::level::read_level_dat_file(&path.join("level.dat"))
            .ok()
            .and_then(|l| l.display_name())
            .unwrap_or_else(|| d.clone());
        if !(is_reset_world(&name) || is_reset_world(d)) {
            continue;
        }
        let size = dir_size(&path);
        if fs::remove_dir_all(&path).is_ok() {
            n += 1;
            bytes += size;
        }
    }
    (n, bytes)
}

/// Copies the player's pack (a `.zip`) into `resourcepacks/`, under a free name.
pub fn add_pack(game_dir: &Path, file: &Path) -> Result<String> {
    let name = file
        .file_name()
        .and_then(|n| n.to_str())
        .context("the file has no name")?;
    if !name.to_ascii_lowercase().ends_with(".zip") {
        bail!("a resource pack is a .zip file");
    }
    let dir = game_dir.join("resourcepacks");
    fs::create_dir_all(&dir)?;
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, "zip"));
    let mut dest = dir.join(name);
    let mut i = 2;
    while dest.exists() {
        dest = dir.join(format!("{stem} ({i}).{ext}"));
        i += 1;
    }
    let tmp = dest.with_extension("part");
    fs::copy(file, &tmp).with_context(|| format!("copying {}", file.display()))?;
    fs::rename(&tmp, &dest)?;
    Ok(dest.file_name().unwrap().to_string_lossy().into_owned())
}

/// The last `n` lines of a text file (a log), lossily decoded. Reads at most the last 256 KB.
pub fn tail(path: &Path, n: usize) -> Vec<String> {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = fs::File::open(path) else {
        return Vec::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(256 * 1024);
    let _ = f.seek(SeekFrom::Start(start));
    let mut buf = Vec::new();
    let _ = f.read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<String> = text
        .lines()
        .rev()
        .take(n)
        .map(|l| l.replace('\t', "    "))
        .collect();
    lines.reverse();
    lines
}

/// Sample content for the fake backend's screenshots (`SRL_FAKE=1`): a shared 1.16.1 directory with
/// a few played worlds, a pile of reset worlds, two resource packs (one on), configs, screenshots
/// and logs. Written only into an empty `game_dir`.
pub fn write_sample(game_dir: &Path) -> Result<()> {
    if game_dir.join("saves").exists() {
        return Ok(());
    }
    let saves = game_dir.join("saves");
    let mut named = vec![
        "Bastion route practice".to_string(),
        "Sub 10 attempt".to_string(),
        "Building test".to_string(),
    ];
    for i in (2360..2398).rev() {
        named.push(format!("Random Speedrun #{i}"));
    }
    for i in (40..46).rev() {
        named.push(format!("Set Speedrun #{i}"));
    }
    let now = std::time::SystemTime::now();
    for (i, name) in named.iter().enumerate() {
        let w = saves.join(name);
        fs::create_dir_all(w.join("region"))?;
        // Played one after another, the named ones first (so newest on top reads like a real
        // instance).
        let age = std::time::Duration::from_secs(if i < 3 {
            3600 * 24 * (i as u64 + 1)
        } else {
            600 * (i as u64 - 2)
        });
        let played = ms(now - age).unwrap_or(0);
        // Seeds that look like real ones: 64-bit, either sign.
        let seed = (i as i64 + 1).wrapping_mul(0x5DEE_CE66_D1F3_A2B7_u64 as i64) ^ 0x2545_F491;
        fs::write(
            w.join("level.dat"),
            srl_practice::level::sample_level_dat(name, seed, played),
        )?;
        fs::File::options()
            .write(true)
            .open(w.join("level.dat"))?
            .set_modified(now - age)?;
        fs::write(w.join("region/r.0.0.mca"), vec![0u8; 4096 * (3 + i % 7)])?;
    }
    let packs = game_dir.join("resourcepacks");
    fs::create_dir_all(&packs)?;
    fs::write(packs.join("Couri's MCSR Pack.zip"), vec![0u8; 182_000])?;
    fs::write(packs.join("Clear Glass.zip"), vec![0u8; 24_000])?;
    fs::write(
        game_dir.join("options.txt"),
        "version:2586\nfov:0.0\nresourcePacks:[\"vanilla\",\"file/Couri's MCSR Pack.zip\"]\nkey_key.drop:key.keyboard.q\n",
    )?;
    for (rel, body) in [
        ("config/mcsr/atum.json", "{\"seed\":\"\",\"worldType\":0}"),
        ("config/mcsr/standardsettings.json", "{\"fov\":70}"),
        ("config/sodium-options.json", "{}"),
        ("config/speedrunigt/options.json", "{}"),
        ("config/seedqueue.json", "{\"maxCapacity\":8}"),
    ] {
        let p = game_dir.join(rel);
        fs::create_dir_all(p.parent().unwrap())?;
        fs::write(p, body)?;
    }
    let shots = game_dir.join("screenshots");
    fs::create_dir_all(&shots)?;
    for (i, n) in ["2026-10-30_21.14.02.png", "2026-10-31_18.40.55.png"]
        .into_iter()
        .enumerate()
    {
        fs::write(shots.join(n), sample_png(i))?;
    }
    let logs = game_dir.join("logs");
    fs::create_dir_all(&logs)?;
    fs::write(
        logs.join("latest.log"),
        "[18:40:01] [main/INFO]: Loading Minecraft 1.16.1 with Fabric Loader 0.16.14\n\
         [18:40:03] [main/INFO]: Loading 23 mods\n\
         [18:40:09] [Render thread/INFO]: Atum: resetting\n\
         [18:40:10] [Server thread/INFO]: Preparing spawn area: 84%\n\
         [18:40:11] [Server thread/INFO]: Random Speedrun #2397 loaded\n\
         [18:52:59] [Render thread/INFO]: Stopping!\n",
    )?;
    fs::write(logs.join("2026-10-30-1.log.gz"), vec![0u8; 12_000])?;
    Ok(())
}

/// A 320×180 violet-to-dusk gradient PNG standing in for a screenshot.
fn sample_png(variant: usize) -> Vec<u8> {
    let (w, h) = (320u32, 180u32);
    let mut data = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let (r, g, b) = if variant == 0 {
                (40. + 120. * fx, 30. + 60. * fy, 90. + 100. * (1. - fy))
            } else {
                (200. * (1. - fy), 120. * fx, 60. + 80. * fy)
            };
            data.extend([r as u8, g as u8, b as u8]);
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().expect("writing to a Vec");
        writer.write_image_data(&data).expect("writing to a Vec");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("srl-content-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn reset_worlds_by_name() {
        for n in [
            "Random Speedrun #1",
            "Random Speedrun #2397",
            "Set Speedrun #4",
            "Speedrun #12",
            "New World",
            "New World (3)",
            "Practice Seed",
        ] {
            assert!(is_reset_world(n), "{n}");
        }
        for n in [
            "Random Speedrun #",
            "Random Speedrun #12 PB",
            "My world",
            "New World Order",
            "Bastion Trainer",
            "New World ()",
        ] {
            assert!(!is_reset_world(n), "{n}");
        }
    }

    #[test]
    fn scan_lists_worlds_packs_config_and_logs() {
        let d = tmp("scan");
        write_sample(&d).unwrap();
        let c = scan(&d, None);
        assert_eq!(c.worlds.len(), 3 + 38 + 6);
        assert_eq!(c.reset_worlds().count(), 44);
        assert!(
            c.worlds
                .iter()
                .any(|w| w.name == "Sub 10 attempt" && !w.reset)
        );
        assert!(c.worlds.iter().all(|w| w.size > 0 && w.seed.is_some()));
        // Newest first, by LastPlayed.
        assert!(
            c.worlds
                .windows(2)
                .all(|p| p[0].played_ms >= p[1].played_ms)
        );
        assert_eq!(c.packs.len(), 2);
        assert!(
            c.packs
                .iter()
                .any(|p| p.item.rel == "Couri's MCSR Pack.zip" && p.on)
        );
        assert!(
            c.packs
                .iter()
                .any(|p| p.item.rel == "Clear Glass.zip" && !p.on)
        );
        assert_eq!(c.config[0].rel, "options.txt");
        assert!(c.config.iter().any(|i| i.rel == "config/mcsr/atum.json"));
        assert_eq!(c.screenshots.len(), 2);
        assert!(c.logs.iter().any(|i| i.rel == "logs/latest.log"));
        assert!(c.size >= c.saves_size && c.saves_size > 0);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn packs_turn_on_and_off_in_options() {
        let d = tmp("packs");
        fs::write(
            d.join("options.txt"),
            "fov:0.0\nresourcePacks:[\"vanilla\"]\nlang:en_us\n",
        )
        .unwrap();
        set_pack(&d, "a.zip", true).unwrap();
        assert_eq!(
            enabled_packs(&d.join("options.txt")),
            vec!["vanilla", "file/a.zip"]
        );
        set_pack(&d, "a.zip", false).unwrap();
        assert_eq!(enabled_packs(&d.join("options.txt")), vec!["vanilla"]);
        let text = fs::read_to_string(d.join("options.txt")).unwrap();
        assert!(text.contains("fov:0.0") && text.contains("lang:en_us"));
        // No options.txt yet (never played): the line is written.
        let e = tmp("packs-new");
        set_pack(&e, "b.zip", true).unwrap();
        assert_eq!(
            enabled_packs(&e.join("options.txt")),
            vec!["vanilla", "file/b.zip"]
        );
        fs::remove_dir_all(&d).unwrap();
        fs::remove_dir_all(&e).unwrap();
    }

    #[test]
    fn cleanup_deletes_only_reset_worlds_and_trash_keeps_the_rest() {
        let d = tmp("cleanup");
        let data = tmp("cleanup-data");
        write_sample(&d).unwrap();
        let c = scan(&d, None);
        let all: Vec<String> = c.worlds.iter().map(|w| w.dir.clone()).collect();
        // Every world asked for, yet only the reset ones go; a path can't escape saves/.
        let mut asked = all.clone();
        asked.push("../options.txt".into());
        let (n, bytes) = delete_reset_worlds(&d, &asked);
        assert_eq!(n, 44);
        assert!(bytes > 0);
        assert!(d.join("options.txt").exists());
        let left = scan(&d, None);
        assert_eq!(left.worlds.len(), 3);
        // One world to the trash, recoverable.
        let to = trash(&data, &d, "saves", "Building test").unwrap();
        assert!(to.join("level.dat").exists());
        assert!(!d.join("saves/Building test").exists());
        assert!(trash(&data, &d, "saves", "../options.txt").is_err());
        fs::remove_dir_all(&d).unwrap();
        fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn added_packs_get_a_free_name() {
        let d = tmp("addpack");
        let src = d.join("pack.zip");
        fs::write(&src, b"zip").unwrap();
        assert_eq!(add_pack(&d, &src).unwrap(), "pack.zip");
        assert_eq!(add_pack(&d, &src).unwrap(), "pack (2).zip");
        assert!(add_pack(&d, &d.join("x.txt")).is_err());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn tail_gives_the_last_lines() {
        let d = tmp("tail");
        let f = d.join("latest.log");
        fs::write(&f, "a\nb\nc\nd\n").unwrap();
        assert_eq!(tail(&f, 2), vec!["c", "d"]);
        assert!(tail(&d.join("none"), 2).is_empty());
        fs::remove_dir_all(&d).unwrap();
    }
}
