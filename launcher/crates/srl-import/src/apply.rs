//! Copying a source into the launcher's instances. The source is only read. In each instance a
//! file that would change is first moved into `import-backup-<unix time>/` (same relative path),
//! so nothing of the instance is lost; a world that exists already is left alone. Links in the
//! source are never followed, files over a sane size are left out, and every file is written to
//! a temporary name first, so a failure never leaves half a file.
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use crate::{
    Choice, Source, Target,
    detect::{MAX_TEXT, SPEEDRUNIGT_FILES, read_text},
    options::{self, Options, Pick},
};

/// Resource packs and world files bigger than this are left out.
pub const MAX_FILE: u64 = 512 * 1024 * 1024;
/// At most this many mod settings files are copied.
const MAX_CONFIG_FILES: usize = 5000;
/// At most this many resource pack files (a pack can be a folder).
const MAX_PACK_FILES: usize = 50_000;
/// At most this many files and bytes of worlds, the most recently played worlds first.
const MAX_WORLD_FILES: usize = 100_000;
const MAX_WORLD_BYTES: u64 = 8 * 1024 * 1024 * 1024;
/// At most this many skipped files are listed one by one.
const MAX_LISTED: usize = 50;

/// A part the player can tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    /// options.txt: keys, mouse, FOV, video, sound.
    Settings,
    /// `config/` and SpeedRunIGT's options.
    ModSettings,
    /// `resourcepacks/` and which of them are on.
    ResourcePacks,
    /// `saves/`.
    Worlds,
}

/// Why a file of the source was left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipWhy {
    /// A link (it may point outside the instance).
    Link,
    TooLarge,
    Unreadable,
    /// Past the limit for its part (files or bytes).
    TooMany,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    /// Relative to the source's game directory.
    pub path: PathBuf,
    pub why: SkipWhy,
}

/// What happened in one instance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TargetReport {
    pub id: String,
    /// The source runs the instance's Minecraft version: everything ticked came over. Otherwise
    /// only the key bindings, mouse sensitivity and FOV did (`left_out` says what didn't).
    pub full: bool,
    /// Settings taken from the source's options.txt.
    pub settings: usize,
    /// Files written (options.txt counts as one).
    pub files: u32,
    /// Where the instance's earlier files went, if any were replaced.
    pub backup: Option<PathBuf>,
    /// Ticked parts this instance didn't get because its Minecraft version differs.
    pub left_out: Vec<Part>,
    /// Worlds with the same folder name already in the instance, left as they were.
    pub worlds_kept: u32,
    /// Why the import into this instance stopped (in English, for the log and as detail).
    pub error: Option<String>,
}

/// What the import did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The Unix time in the backup folders' names (`import-backup-<stamp>`).
    pub stamp: u64,
    pub targets: Vec<TargetReport>,
    /// Files of the source left out, the first `MAX_LISTED`.
    pub skipped: Vec<Skipped>,
    /// How many more were left out.
    pub skipped_more: u32,
}

impl Report {
    pub fn files(&self) -> u32 {
        self.targets.iter().map(|t| t.files).sum()
    }
}

#[derive(Clone, Debug)]
struct Entry {
    rel: PathBuf,
}

/// The source's files per part, gathered once for every target.
#[derive(Default)]
struct Files {
    config: Vec<Entry>,
    packs: Vec<Entry>,
    /// Per world: its folder name and files (folders included, so empty ones come too).
    worlds: Vec<(String, Vec<Entry>, Vec<PathBuf>)>,
    skipped: Vec<Skipped>,
    skipped_more: u32,
}

impl Files {
    fn skip(&mut self, path: PathBuf, why: SkipWhy) {
        if self.skipped.len() < MAX_LISTED {
            self.skipped.push(Skipped { path, why });
        } else {
            self.skipped_more += 1;
        }
    }
}

/// Counts regular files under `dir` (no links), up to `max`.
pub(crate) fn count_files(dir: &Path, max: usize) -> u32 {
    let mut n = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => {
                    n += 1;
                    if n >= max {
                        return n as u32;
                    }
                }
                _ => {}
            }
        }
    }
    n as u32
}

/// Walks `base/rel` (no links followed) and adds its files to `out`, its folders to `dirs`.
/// Returns the bytes added; stops at `max_files` or `max_bytes`.
#[allow(clippy::too_many_arguments)]
fn walk(
    base: &Path,
    rel: &Path,
    max_file: u64,
    max_files: usize,
    max_bytes: u64,
    bytes: &mut u64,
    out: &mut Vec<Entry>,
    dirs: &mut Vec<PathBuf>,
    files: &mut Files,
) -> bool {
    let mut stack = vec![rel.to_path_buf()];
    while let Some(r) = stack.pop() {
        let Ok(entries) = fs::read_dir(base.join(&r)) else {
            files.skip(r, SkipWhy::Unreadable);
            continue;
        };
        dirs.push(r.clone());
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let rel = r.join(e.file_name());
            let Ok(t) = e.file_type() else {
                files.skip(rel, SkipWhy::Unreadable);
                continue;
            };
            if t.is_symlink() {
                files.skip(rel, SkipWhy::Link);
            } else if t.is_dir() {
                stack.push(rel);
            } else if t.is_file() {
                let len = e.metadata().map(|m| m.len()).unwrap_or(u64::MAX);
                if len > max_file {
                    files.skip(rel, SkipWhy::TooLarge);
                } else if out.len() >= max_files || *bytes + len > max_bytes {
                    files.skip(rel, SkipWhy::TooMany);
                    return false;
                } else {
                    *bytes += len;
                    out.push(Entry { rel });
                }
            } else {
                // A socket, a device: nothing a game wrote.
                files.skip(rel, SkipWhy::Unreadable);
            }
        }
    }
    true
}

fn gather(source: &Source, choice: Choice) -> Files {
    let base = &source.game_dir;
    let mut files = Files::default();
    if choice.mod_settings {
        let (mut out, mut dirs, mut bytes) = (Vec::new(), Vec::new(), 0);
        if is_real_dir(&base.join("config")) {
            walk(
                base,
                Path::new("config"),
                MAX_TEXT,
                MAX_CONFIG_FILES,
                u64::MAX,
                &mut bytes,
                &mut out,
                &mut dirs,
                &mut files,
            );
        } else if is_link(&base.join("config")) {
            files.skip("config".into(), SkipWhy::Link);
        }
        for f in SPEEDRUNIGT_FILES {
            match fs::symlink_metadata(base.join(f)) {
                Ok(m) if m.is_file() && m.len() <= MAX_TEXT => out.push(Entry { rel: f.into() }),
                Ok(m) if m.is_symlink() => files.skip(f.into(), SkipWhy::Link),
                Ok(m) if m.is_file() => files.skip(f.into(), SkipWhy::TooLarge),
                _ => {}
            }
        }
        files.config = out;
    }
    if choice.resource_packs && is_real_dir(&base.join("resourcepacks")) {
        let (mut out, mut dirs, mut bytes) = (Vec::new(), Vec::new(), 0);
        walk(
            base,
            Path::new("resourcepacks"),
            MAX_FILE,
            MAX_PACK_FILES,
            u64::MAX,
            &mut bytes,
            &mut out,
            &mut dirs,
            &mut files,
        );
        files.packs = out;
    }
    if choice.worlds && is_real_dir(&base.join("saves")) {
        let saves = base.join("saves");
        let mut worlds: Vec<(u64, String)> = fs::read_dir(&saves)
            .map(|d| {
                d.flatten()
                    .filter_map(|e| {
                        let name = e.file_name().to_str()?.to_string();
                        let t = e.file_type().ok()?;
                        if t.is_symlink() {
                            return Some((u64::MAX, name));
                        }
                        if !t.is_dir() || name.starts_with('.') {
                            return None;
                        }
                        let played = fs::metadata(e.path().join("level.dat"))
                            .or_else(|_| e.metadata())
                            .and_then(|m| m.modified())
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map_or(0, |d| d.as_secs());
                        Some((played, name))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // The most recently played first, so the limits leave out the oldest.
        worlds.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let (mut count, mut bytes) = (0usize, 0u64);
        for (played, name) in worlds {
            let rel = Path::new("saves").join(&name);
            if played == u64::MAX {
                files.skip(rel, SkipWhy::Link);
                continue;
            }
            let (mut out, mut dirs) = (Vec::new(), Vec::new());
            let room = MAX_WORLD_FILES.saturating_sub(count);
            if !walk(
                base,
                &rel,
                MAX_FILE,
                room,
                MAX_WORLD_BYTES,
                &mut bytes,
                &mut out,
                &mut dirs,
                &mut files,
            ) {
                // A world that doesn't fit whole isn't copied at all, nor any older one.
                break;
            }
            count += out.len();
            files.worlds.push((name, out, dirs));
        }
    }
    files
}

fn is_real_dir(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok_and(|m| m.is_dir())
}

fn is_link(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok_and(|m| m.is_symlink())
}

fn canonical(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Imports `choice` from `source` into every target. `stamp` names the backup folders (the Unix
/// time); `progress(done, total)` counts files. Never fails as a whole: a target that can't be
/// written says why in its report, and the others go on.
pub fn import(
    source: &Source,
    choice: Choice,
    targets: &[Target],
    stamp: u64,
    progress: &mut dyn FnMut(u64, u64),
) -> Report {
    let files = gather(source, choice);
    let options_text = if choice.options || choice.resource_packs {
        read_text(&source.game_dir.join("options.txt"))
    } else {
        None
    };
    let source_options = options_text.as_deref().map(Options::parse);
    let src_dir = canonical(&source.game_dir);
    let full_files = (files.config.len()
        + files.packs.len()
        + files.worlds.iter().map(|w| w.1.len()).sum::<usize>()) as u64;
    let total: u64 = targets
        .iter()
        .map(|t| {
            1 + if source.runs(&t.version) {
                full_files
            } else {
                0
            }
        })
        .sum();
    let mut done = 0u64;
    let mut report = Report {
        stamp,
        targets: Vec::new(),
        skipped: files.skipped.clone(),
        skipped_more: files.skipped_more,
    };
    for target in targets {
        let full = source.runs(&target.version);
        let mut tr = TargetReport {
            id: target.id.clone(),
            full,
            ..TargetReport::default()
        };
        if !full {
            let ticked = [
                (choice.options, Part::Settings),
                (choice.mod_settings, Part::ModSettings),
                (choice.resource_packs, Part::ResourcePacks),
                (choice.worlds, Part::Worlds),
            ];
            tr.left_out = ticked
                .into_iter()
                .filter(|(on, part)| *on && *part != Part::Settings)
                .map(|(_, p)| p)
                .collect();
        }
        let dst_dir = canonical(&target.dir);
        if dst_dir.starts_with(&src_dir) || src_dir.starts_with(&dst_dir) {
            tr.error = Some(format!(
                "{} is the folder being imported from",
                target.dir.display()
            ));
            done += 1 + if full { full_files } else { 0 };
            progress(done, total);
            report.targets.push(tr);
            continue;
        }
        let mut writer = Writer {
            dir: &target.dir,
            backup_name: format!("import-backup-{stamp}"),
            backup: None,
        };
        let result = (|| -> io::Result<()> {
            if let Some(src) = &source_options {
                let mut picks = Vec::new();
                if choice.options {
                    picks.push(if full {
                        Pick::AllSettings
                    } else {
                        Pick::Shared
                    });
                }
                if choice.resource_packs && full {
                    picks.push(Pick::ResourcePacks);
                }
                if !picks.is_empty() {
                    let path = target.dir.join("options.txt");
                    let old_text = read_text(&path);
                    let old = old_text.as_deref().map(Options::parse);
                    let seed: Vec<(&str, &str)> = target
                        .seed
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();
                    let (merged, taken) = options::merge(old.as_ref(), src, &picks, &seed);
                    tr.settings = taken;
                    let text = merged.render();
                    if old_text.as_deref() != Some(text.as_str()) {
                        writer.put_bytes(Path::new("options.txt"), text.as_bytes())?;
                        tr.files += 1;
                    }
                }
            }
            done += 1;
            progress(done, total);
            if !full {
                return Ok(());
            }
            for e in files.config.iter().chain(&files.packs) {
                if !target.keeps(&e.rel) && writer.put_file(&source.game_dir, &e.rel)? {
                    tr.files += 1;
                }
                done += 1;
                progress(done, total);
            }
            for (name, entries, dirs) in &files.worlds {
                let dest = target.dir.join("saves").join(name);
                if fs::symlink_metadata(&dest).is_ok() {
                    tr.worlds_kept += 1;
                } else {
                    tr.files += writer.put_world(&source.game_dir, name, entries, dirs)?;
                }
                done += entries.len() as u64;
                progress(done, total);
            }
            Ok(())
        })();
        if let Err(e) = result {
            tr.error = Some(e.to_string());
        }
        tr.backup = writer.backup;
        report.targets.push(tr);
    }
    report
}

/// Writes into one instance, moving what it replaces into the backup folder.
struct Writer<'a> {
    dir: &'a Path,
    backup_name: String,
    backup: Option<PathBuf>,
}

impl Writer<'_> {
    fn backup_dir(&mut self) -> io::Result<PathBuf> {
        if let Some(b) = &self.backup {
            return Ok(b.clone());
        }
        // Two imports in the same second get their own folders.
        let mut dir = self.dir.join(&self.backup_name);
        let mut n = 2;
        while fs::symlink_metadata(&dir).is_ok() {
            dir = self.dir.join(format!("{}-{n}", self.backup_name));
            n += 1;
        }
        fs::create_dir_all(&dir)?;
        self.backup = Some(dir.clone());
        Ok(dir)
    }

    /// Moves the instance's `rel` into the backup folder, if it exists.
    fn save_old(&mut self, rel: &Path) -> io::Result<()> {
        let path = self.dir.join(rel);
        if fs::symlink_metadata(&path).is_err() {
            return Ok(());
        }
        let to = self.backup_dir()?.join(rel);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        if fs::rename(&path, &to).is_err() {
            fs::copy(&path, &to)?;
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn put_bytes(&mut self, rel: &Path, bytes: &[u8]) -> io::Result<()> {
        let path = self.dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = tmp_name(&path);
        fs::write(&tmp, bytes)?;
        self.save_old(rel)?;
        fs::rename(&tmp, &path)
    }

    /// Copies the source's `rel` over the instance's. False when they are already the same.
    fn put_file(&mut self, src_base: &Path, rel: &Path) -> io::Result<bool> {
        let src = src_base.join(rel);
        // Checked again right before reading: never a link.
        let m = fs::symlink_metadata(&src)?;
        if !m.is_file() {
            return Ok(false);
        }
        let dest = self.dir.join(rel);
        if let Ok(old) = fs::symlink_metadata(&dest) {
            if old.is_dir() {
                return Err(io::Error::other(format!(
                    "{} is a folder in the instance",
                    rel.display()
                )));
            }
            if old.is_file() && old.len() == m.len() && same_bytes(&src, &dest)? {
                return Ok(false);
            }
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = tmp_name(&dest);
        fs::copy(&src, &tmp)?;
        self.save_old(rel)?;
        fs::rename(&tmp, &dest)?;
        Ok(true)
    }

    /// Copies a whole world into a staging folder, then renames it into `saves/`.
    fn put_world(
        &mut self,
        src_base: &Path,
        name: &str,
        entries: &[Entry],
        dirs: &[PathBuf],
    ) -> io::Result<u32> {
        let saves = self.dir.join("saves");
        fs::create_dir_all(&saves)?;
        let staging = saves.join(format!(".{name}.srl-import"));
        let _ = fs::remove_dir_all(&staging);
        let world = Path::new("saves").join(name);
        let inside = |rel: &Path| rel.strip_prefix(&world).map(|r| staging.join(r));
        let copy = || -> io::Result<u32> {
            for d in dirs {
                if let Ok(d) = inside(d) {
                    fs::create_dir_all(d)?;
                }
            }
            let mut n = 0;
            for e in entries {
                let src = src_base.join(&e.rel);
                if !fs::symlink_metadata(&src)?.is_file() {
                    continue;
                }
                if let Ok(dest) = inside(&e.rel) {
                    fs::copy(&src, dest)?;
                    n += 1;
                }
            }
            fs::rename(&staging, saves.join(name))?;
            Ok(n)
        };
        let result = copy();
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }
}

fn tmp_name(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".srl-import-part");
    path.with_file_name(name)
}

/// Whether two files of the same length hold the same bytes (big ones are taken as different).
fn same_bytes(a: &Path, b: &Path) -> io::Result<bool> {
    if fs::metadata(a)?.len() > 64 * 1024 * 1024 {
        return Ok(false);
    }
    let (mut fa, mut fb) = (fs::File::open(a)?, fs::File::open(b)?);
    let (mut ba, mut bb) = (vec![0u8; 64 * 1024], vec![0u8; 64 * 1024]);
    loop {
        let na = read_full(&mut fa, &mut ba)?;
        let nb = read_full(&mut fb, &mut bb)?;
        if na != nb || ba[..na] != bb[..nb] {
            return Ok(false);
        }
        if na == 0 {
            return Ok(true);
        }
    }
}

fn read_full(f: &mut fs::File, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}
