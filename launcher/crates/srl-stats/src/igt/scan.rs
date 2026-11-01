//! Scanning the record files, incrementally.
//!
//! A player who resets a lot has thousands of records (every reset is one), and the Stats page
//! and Home ask for them often. So each scan:
//!
//! 1. lists the global `records/` folders and every `instances/<mode>/saves/*/speedrunigt/`;
//! 2. stats each `*.json`/`record.json` and reuses the parsed result from the index when size and
//!    mtime are unchanged (one `stat` per file, no read);
//! 3. parses only new or changed files;
//! 4. writes the index back (only if something changed), keeping only files that still exist.
//!
//! The index is `Paths::meta()/stats/igt-index.json`. A missing, unreadable or older-format index
//! just means a full parse. Malformed files are counted in `Skipped` and remembered too, so a
//! broken file costs one parse, not one per scan.
//!
//! The modes remembered for attribution rule 4 are not in the index: losing them can't be undone
//! (the worlds that proved them may be gone), while the index is only a speed-up that a format
//! change throws away. They live in their own small file, `Paths::meta()/stats/attribution.json`
//! (record id to mode), with its own version. An entry is dropped only when its record file is
//! gone from a records folder that was listed without error; a folder that can't be listed is
//! reported in `LocalStats::unreadable` and prunes nothing.
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{Instant, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use srl_core::{Mode, Paths};

use super::{
    IgtSources, LatestWorld,
    history::{Run, RunHistory, WorldRef},
    record::{self, Record},
};
use crate::cache::write_atomic;

/// Bump when `Entry` or `Record` change shape: an index of another version is ignored (a full
/// parse). 2: `Record::open_lan_ms`.
const INDEX_VERSION: u32 = 2;
/// The attribution file's format. A file of another version is left untouched (a newer launcher
/// wrote it; a downgrade must not erase it) and nothing is remembered until it is readable.
const ATTRIBUTION_VERSION: u32 = 1;
/// How many skipped files `Skipped::examples` lists.
const SKIPPED_EXAMPLES: usize = 5;
/// Rule 3 with no shared timeline accepts a global record only when it was written after the
/// world folder was created; this allows for coarse file-time granularity.
const CREATED_SLACK_MS: i64 = 2000;

/// What the Stats page and Home read: runs per mode, plus the runs from elsewhere.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalStats {
    /// Every mode with at least one run. Modes without runs are absent (Draftout never has any:
    /// its 26.3 set has no SpeedRunIGT).
    pub modes: BTreeMap<Mode, RunHistory>,
    /// Global records we cannot tie to one of our instances (see the rule in `igt/mod.rs`).
    pub unattributed: RunHistory,
    /// `latest_world.json`, when it parsed.
    pub latest_world: Option<LatestWorld>,
    pub skipped: Skipped,
    /// Folders that exist but could not be listed (a records folder or an instance's `saves`),
    /// with the reason. Their runs are missing from this result; show a small note, like
    /// `skipped`, never an error state.
    pub unreadable: Vec<(PathBuf, String)>,
    /// Record files looked at in this scan.
    pub files: u32,
    /// Of those, how many had to be read and parsed (the rest came from the index).
    pub parsed: u32,
    pub scan_ms: u64,
}

impl LocalStats {
    /// A mode's history; an empty one when the mode has no runs. A reference, not a copy: the
    /// page and the Home tiles call this while rendering, and a history can hold tens of
    /// thousands of runs.
    pub fn mode(&self, mode: Mode) -> &RunHistory {
        static EMPTY: RunHistory = RunHistory {
            runs: Vec::new(),
            categories: Vec::new(),
            run_count: 0,
            completion_count: 0,
        };
        self.modes.get(&mode).unwrap_or(&EMPTY)
    }
}

/// Files that were not records. Never fatal; shown as a small note ("3 files could not be read").
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skipped {
    pub count: u32,
    /// The first few, with the reason.
    pub examples: Vec<(PathBuf, String)>,
}

impl Skipped {
    fn add(&mut self, path: &Path, why: &str) {
        self.count += 1;
        if self.examples.len() < SKIPPED_EXAMPLES {
            self.examples.push((path.to_path_buf(), why.to_string()));
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Index {
    version: u32,
    /// Keyed by the file's path (lossy UTF-8).
    files: HashMap<String, Entry>,
}

/// `attribution.json`: global record id (its file stem) -> the mode it was attributed to in an
/// earlier scan (attribution rule 4).
#[derive(Serialize, Deserialize)]
struct Attribution {
    version: u32,
    modes: HashMap<String, Mode>,
}

/// The remembered modes, and whether the file may be written (false when it holds another
/// version).
fn load_attribution(path: &Path) -> (HashMap<String, Mode>, bool) {
    let Ok(bytes) = fs::read(path) else {
        return (HashMap::new(), true);
    };
    #[derive(Deserialize)]
    struct Version {
        version: u32,
    }
    match serde_json::from_slice::<Version>(&bytes) {
        Ok(v) if v.version != ATTRIBUTION_VERSION => (HashMap::new(), false),
        // A broken file (a crash mid-write can't produce one, a disk error can) is replaced.
        _ => (
            serde_json::from_slice::<Attribution>(&bytes)
                .map(|a| a.modes)
                .unwrap_or_default(),
            true,
        ),
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    size: u64,
    mtime_ns: u128,
    parsed: Result<Record, String>,
}

struct Scanner {
    old: Index,
    new: Index,
    skipped: Skipped,
    files: u32,
    parsed: u32,
}

impl Scanner {
    /// The parsed file, from the index when unchanged. `None` when the file vanished or is not a
    /// record (counted as skipped).
    fn load(&mut self, path: &Path) -> Option<Entry> {
        let meta = fs::metadata(path).ok()?;
        if !meta.is_file() {
            return None;
        }
        self.files += 1;
        let mtime_ns = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let key = path.to_string_lossy().into_owned();
        let entry = match self.old.files.remove(&key) {
            Some(e) if e.size == meta.len() && e.mtime_ns == mtime_ns => e,
            _ => {
                self.parsed += 1;
                let parsed = fs::read(path)
                    .map_err(|e| format!("unreadable: {e}"))
                    .and_then(|b| record::parse(&b));
                Entry {
                    size: meta.len(),
                    mtime_ns,
                    parsed,
                }
            }
        };
        self.new.files.insert(key, entry.clone());
        if let Err(why) = &entry.parsed {
            self.skipped.add(path, why);
            return None;
        }
        Some(entry)
    }
}

struct WorldRecord {
    world: WorldRef,
    /// `<mode>/<folder>` in the launcher's own directories, `<instance dir>/<folder>` in an extra
    /// one: the run's id when no global record matches it.
    key: String,
    record: Record,
    /// When the world folder was created (epoch ms), where the file system knows.
    created_ms: Option<i64>,
}

struct GlobalRecord {
    id: String,
    record: Record,
}

/// Scans everything and builds the histories. Blocking file I/O: call it from
/// `tokio::task::spawn_blocking` (the `Stats` facade does). Never fails; a broken index or an
/// unwritable meta folder only cost speed.
pub fn scan(paths: &Paths, sources: &IgtSources) -> LocalStats {
    scan_with(paths, sources, &[])
}

/// `scan`, also reading the worlds of `extra` game directories (the player's own instances),
/// whose runs count for the mode given with each.
pub fn scan_with(paths: &Paths, sources: &IgtSources, extra: &[(PathBuf, Mode)]) -> LocalStats {
    let started = Instant::now();
    let index_path = index_path(paths);
    let old = fs::read(&index_path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Index>(&b).ok())
        .filter(|i| i.version == INDEX_VERSION)
        .unwrap_or_default();
    let attribution_path = attribution_path(paths);
    let (old_sticky, attribution_writable) = load_attribution(&attribution_path);
    let mut unreadable = Vec::new();
    let mut sc = Scanner {
        old,
        new: Index {
            version: INDEX_VERSION,
            ..Default::default()
        },
        skipped: Skipped::default(),
        files: 0,
        parsed: 0,
    };

    // Global records. The same id in two global folders (an update moving the folder) is one run.
    let mut globals: Vec<GlobalRecord> = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut latest_world = None;
    // False when a records folder exists but could not be listed: then no remembered mode is
    // dropped, because its record may still be there.
    let mut globals_listed = true;
    for dir in &sources.global_dirs {
        if latest_world.is_none() {
            latest_world = read_latest_world(paths, extra, &dir.join("latest_world.json"));
        }
        let records = dir.join("records");
        let entries = match list(&records) {
            Ok(entries) => entries,
            Err(e) => {
                globals_listed = false;
                unreadable.push((records, e.to_string()));
                continue;
            }
        };
        for path in entries {
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let Some(id) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
                continue;
            };
            if !seen_ids.insert(id.clone()) {
                continue;
            }
            if let Some(e) = sc.load(&path)
                && let Ok(record) = e.parsed
            {
                globals.push(GlobalRecord { id, record });
            }
        }
    }

    // World records in our instances.
    let mut worlds: Vec<WorldRecord> = Vec::new();
    let world_modes = srl_core::shared::WorldModes::load(paths);
    let mut game_dirs: Vec<(PathBuf, Mode, Option<String>)> = super::game_dirs()
        .into_iter()
        .map(|m| (paths.instance(m), m, None))
        .collect();
    for (dir, mode) in extra {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        game_dirs.push((dir.clone(), *mode, Some(name)));
    }
    for (game_dir, dir_mode, extra_name) in game_dirs {
        let saves = game_dir.join("saves");
        let entries = match list(&saves) {
            Ok(entries) => entries,
            Err(e) => {
                unreadable.push((saves, e.to_string()));
                continue;
            }
        };
        for world_dir in entries {
            let file = world_dir.join("speedrunigt").join("record.json");
            if let Some(e) = sc.load(&file)
                && let Ok(record) = e.parsed
            {
                let dir_name = world_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let created_ms = fs::metadata(&world_dir)
                    .and_then(|m| m.created())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64);
                let mode = if extra_name.is_none() && dir_mode.is_shared() {
                    world_modes.mode_of(&world_dir, created_ms)
                } else {
                    dir_mode
                };
                let key = match &extra_name {
                    Some(d) => format!("{d}/{dir_name}"),
                    None => format!("{}/{dir_name}", mode.id()),
                };
                worlds.push(WorldRecord {
                    key,
                    world: WorldRef {
                        mode,
                        dir_name,
                        path: world_dir.clone(),
                    },
                    record,
                    created_ms,
                });
            }
        }
    }

    let (runs, mut sticky) = attribute(globals, worlds, &old_sticky);
    // Keep the older memory of records that still exist (unparsable this time, say) or could not
    // be checked; drop only records that are gone from a folder that was listed.
    for (id, mode) in &old_sticky {
        if !globals_listed || seen_ids.contains(id) {
            sticky.entry(id.clone()).or_insert(*mode);
        }
    }
    if attribution_writable
        && sticky != old_sticky
        && let Ok(bytes) = serde_json::to_vec(&Attribution {
            version: ATTRIBUTION_VERSION,
            modes: sticky,
        })
    {
        let _ = write_atomic(&attribution_path, &bytes);
    }

    let mut by_mode: BTreeMap<Mode, Vec<Run>> = BTreeMap::new();
    let mut unattributed = Vec::new();
    for run in runs {
        match run.mode {
            Some(m) => by_mode.entry(m).or_default().push(run),
            None => unattributed.push(run),
        }
    }

    // Save the index only when it changed: a scan that parsed nothing and saw no file vanish
    // writes nothing.
    let changed = sc.parsed > 0 || !sc.old.files.is_empty();
    if changed && let Ok(bytes) = serde_json::to_vec(&sc.new) {
        let _ = write_atomic(&index_path, &bytes);
    }

    LocalStats {
        modes: by_mode
            .into_iter()
            .map(|(m, runs)| (m, RunHistory::new(runs)))
            .collect(),
        unattributed: RunHistory::new(unattributed),
        latest_world,
        skipped: sc.skipped,
        unreadable,
        files: sc.files,
        parsed: sc.parsed,
        scan_ms: started.elapsed().as_millis() as u64,
    }
}

pub(crate) fn index_path(paths: &Paths) -> PathBuf {
    paths.meta().join("stats").join("igt-index.json")
}

pub(crate) fn attribution_path(paths: &Paths) -> PathBuf {
    paths.meta().join("stats").join("attribution.json")
}

/// The folder's entries, sorted. A missing folder is empty (no SpeedRunIGT yet, no worlds yet);
/// any other error is returned, because "empty" would wrongly prune remembered modes.
fn list(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    out.sort();
    Ok(out)
}

fn read_latest_world(paths: &Paths, extra: &[(PathBuf, Mode)], file: &Path) -> Option<LatestWorld> {
    #[derive(Deserialize)]
    struct Raw {
        world_path: String,
        #[serde(default)]
        mods: Vec<String>,
        version: Option<String>,
        category: Option<String>,
    }
    let raw: Raw = serde_json::from_slice(&fs::read(file).ok()?).ok()?;
    let world_path = PathBuf::from(&raw.world_path);
    Some(LatestWorld {
        mode: super::mode_of_world_in(paths, extra, &world_path).map(|(m, _)| m),
        world_path,
        category: raw.category,
        version: raw.version,
        mods: raw.mods,
    })
}

/// True when `newer` can be a later write of the same run as `older`: SpeedRunIGT only ever adds
/// timeline entries and time, so `older`'s timeline is a prefix of `newer`'s and no time went down.
fn continues(older: &Record, newer: &Record) -> bool {
    older.date_ms <= newer.date_ms
        && older.igt_raw_ms <= newer.igt_raw_ms
        && older.rta_ms <= newer.rta_ms
        && older.category == newer.category
        && older.run_type == newer.run_type
        && older.splits.len() <= newer.splits.len()
        && older
            .splits
            .iter()
            .zip(&newer.splits)
            .all(|(a, b)| a.name == b.name && a.igt_ms == b.igt_ms)
}

/// Rule 3's evidence that an older global write comes from this world's run and not from another
/// launcher's world with the same Atum name ("Random Speedrun #8" exists in every instance that
/// ever reset 8 times): a non-empty timeline that the world record continues (split names and
/// times equal to the tick do not repeat by chance), or a global written after the world folder
/// was created. An empty-timeline reset from before the folder existed is someone else's; a
/// copied or restored world gets a new creation time, so it is only matched by its timeline.
fn same_run_evidence(global: &Record, world: &WorldRecord) -> bool {
    !global.splits.is_empty()
        || world
            .created_ms
            .is_some_and(|created| global.date_ms >= created - CREATED_SLACK_MS)
}

/// Applies the attribution rule (`igt/mod.rs`) and merges each global record with its world
/// record. Returns the runs and the modes of the global records attributed in this scan.
///
/// Each world absorbs at most one global record: a run is one run, so a second global that looks
/// like the same write or an older one is not merged into it (rule 2: the first in sorted order
/// wins; rule 3: when several qualify, none does).
fn attribute(
    globals: Vec<GlobalRecord>,
    worlds: Vec<WorldRecord>,
    old_sticky: &HashMap<String, Mode>,
) -> (Vec<Run>, HashMap<String, Mode>) {
    // A copied world (Make a copy or an import with worlds) has the very same record as the
    // world it was copied from: one run, counted once, in the first folder scanned (the
    // launcher's own come before the custom instances).
    // Keyed like `Record::same_write`.
    let mut worlds = worlds;
    let mut kept: HashSet<(String, i64, u64, u64)> = HashSet::new();
    worlds.retain(|w| {
        let r = &w.record;
        kept.insert((r.world_name.clone(), r.date_ms, r.rta_ms, r.igt_raw_ms))
    });
    let mut by_name: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, w) in worlds.iter().enumerate() {
        by_name.entry(&w.record.world_name).or_default().push(i);
    }
    // Rule 2 first, for every global, so that rule 3 never takes a world another global matched
    // exactly.
    let mut taken: HashSet<usize> = HashSet::new();
    let exact: Vec<Option<usize>> = globals
        .iter()
        .map(|g| {
            let i = by_name.get(g.record.world_name.as_str()).and_then(|c| {
                c.iter()
                    .copied()
                    .find(|&i| worlds[i].record.same_write(&g.record))
            })?;
            taken.insert(i).then_some(i)
        })
        .collect();
    // Rule 3 candidates: one world of that name across all our instances, not taken, whose record
    // continues this one, with evidence that it is the same run.
    let older: Vec<Option<usize>> = globals
        .iter()
        .zip(&exact)
        .map(|(g, exact)| {
            if exact.is_some() {
                return None;
            }
            // Several instances can have a world of that name (each Atum counts from #1): the
            // one whose record qualifies, if only one does.
            let c = by_name.get(g.record.world_name.as_str())?;
            let mut fits = c.iter().copied().filter(|&i| {
                let w = &worlds[i];
                !taken.contains(&i)
                    && continues(&g.record, &w.record)
                    && same_run_evidence(&g.record, w)
            });
            let i = fits.next()?;
            fits.next().is_none().then_some(i)
        })
        .collect();
    // world -> the globals that qualify for it. One: it is that run. Several: the one an earlier
    // scan already gave this world's mode, if exactly one was (it was unique then); else none.
    let mut claims: HashMap<usize, Vec<usize>> = HashMap::new();
    for (gi, w) in older.iter().enumerate() {
        if let Some(w) = w {
            claims.entry(*w).or_default().push(gi);
        }
    }
    let mut older_match: HashMap<usize, usize> = HashMap::new();
    for (w, gs) in claims {
        let pick = match gs.as_slice() {
            &[g] => Some(g),
            _ => {
                let mode = worlds[w].world.mode;
                let known: Vec<usize> = gs
                    .iter()
                    .copied()
                    .filter(|&g| old_sticky.get(&globals[g].id) == Some(&mode))
                    .collect();
                match known.as_slice() {
                    &[g] => Some(g),
                    _ => None,
                }
            }
        };
        if let Some(g) = pick {
            older_match.insert(g, w);
        }
    }

    let mut used_worlds = HashSet::new();
    let mut runs = Vec::new();
    let mut sticky = HashMap::new();
    for (gi, (g, exact)) in globals.into_iter().zip(exact).enumerate() {
        let world = exact.or(older_match.get(&gi).copied());
        let (mode, world_ref, record) = match world {
            Some(i) => {
                used_worlds.insert(i);
                let w = &worlds[i];
                // The world copy may be the newer write (the game stopped before the run's leave).
                let record = if w.record.date_ms > g.record.date_ms {
                    w.record.clone()
                } else {
                    g.record
                };
                (Some(w.world.mode), Some(w.world.clone()), record)
            }
            // Rule 4.
            None => (old_sticky.get(&g.id).copied(), None, g.record),
        };
        if let Some(m) = mode {
            sticky.insert(g.id.clone(), m);
        }
        runs.push(Run {
            id: format!("g:{}", g.id),
            mode,
            record,
            world: world_ref,
        });
    }
    // Rule 1: world records no global matched (records turned off, or the global deleted).
    for (i, w) in worlds.into_iter().enumerate() {
        if used_worlds.contains(&i) {
            continue;
        }
        runs.push(Run {
            id: format!("w:{}", w.key),
            mode: Some(w.world.mode),
            record: w.record,
            world: Some(w.world),
        });
    }
    (runs, sticky)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::igt::{history::Segment, record::RunType};

    /// A scratch folder under the target directory's temp space, removed on drop.
    pub(crate) struct Scratch(pub PathBuf);
    impl Scratch {
        pub fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("srl-stats-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Scratch(p)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn copy_tree(from: &Path, to: &Path) {
        for e in fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            let dest = to.join(e.file_name());
            if e.file_type().unwrap().is_dir() {
                fs::create_dir_all(&dest).unwrap();
                copy_tree(&e.path(), &dest);
            } else {
                fs::copy(e.path(), &dest).unwrap();
            }
        }
    }

    /// The files SpeedRunIGT 16.0 wrote in a real RSG session (see NOTES.md): 7 global records,
    /// 8 worlds (the 8th's game was killed before it left the world, so it has no global record),
    /// one completed run (with cheats, to get through the splits quickly).
    fn real_fixture(name: &str) -> (Scratch, Paths, IgtSources) {
        let s = Scratch::new(name);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/real");
        copy_tree(&root, &s.0);
        let paths = Paths::new(s.0.join("srl"));
        let latest = s.0.join("home/speedrunigt/latest_world.json");
        let text = fs::read_to_string(&latest).unwrap();
        let home = paths.root().to_string_lossy().replace('\\', "/");
        fs::write(&latest, text.replace("SRL_HOME", &home)).unwrap();
        let sources = IgtSources::with_global_dirs([s.0.join("home/speedrunigt")]);
        (s, paths, sources)
    }

    #[test]
    fn real_session_attributes_everything_to_rsg() {
        let (_s, paths, sources) = real_fixture("real");
        let stats = scan(&paths, &sources);
        assert_eq!(stats.skipped.count, 0);
        assert_eq!(stats.files, 15);
        assert_eq!(stats.parsed, 15);
        assert_eq!(stats.unattributed.run_count, 0);
        let rsg = stats.mode(Mode::Rsg);
        assert_eq!(rsg.run_count, 8, "7 global + 1 world-only");
        assert_eq!(rsg.completion_count, 1);
        assert_eq!(rsg.runs[0].id, "w:rsg/Random Speedrun #8");
        assert!(rsg.runs[1..].iter().all(|r| r.id.starts_with("g:")));
        assert!(rsg.runs.iter().all(|r| r.world.is_some()));
        let any = rsg.category("ANY", &RunType::RandomSeed).unwrap();
        // The completed run had cheats on (LAN), so it is counted but is not a PB, and it is not
        // in `reached` either.
        assert_eq!(any.completions, 1);
        assert_eq!((any.excluded, any.excluded_completions), (1, 1));
        assert!(any.pb.is_none());
        // Only that run got past the overworld, so no eligible run reached any route point.
        assert!(any.reached.is_empty(), "{:?}", any.reached);
        assert!(!any.reached.contains_key(&Segment::Finish));
        let done = rsg.runs.iter().find(|r| r.record.completed).unwrap();
        assert_eq!(done.record.world_name, "Random Speedrun #4");
        assert_eq!(done.record.igt_raw_ms, 248_040);
        assert_eq!(
            done.record.igt_ms, 261_116,
            "retimed: random seed Any% under 13 min"
        );
        assert_eq!(
            stats.latest_world.as_ref().and_then(|l| l.mode),
            Some(Mode::Rsg)
        );
        assert!(!stats.modes.contains_key(&Mode::Draftout));

        // Second scan: everything from the index.
        let again = scan(&paths, &sources);
        assert_eq!((again.files, again.parsed), (15, 0));
        assert_eq!(again.modes, stats.modes);
    }

    /// A custom instance's worlds count for the mode it plays like, with ids of their own.
    #[test]
    fn extra_game_dirs_count_for_their_mode() {
        let (s, paths, sources) = real_fixture("extra");
        let custom = s.0.join("srl/instances/custom-1");
        fs::create_dir_all(&custom).unwrap();
        fs::rename(
            paths.instance(Mode::Rsg).join("saves"),
            custom.join("saves"),
        )
        .unwrap();
        let stats = scan_with(&paths, &sources, &[(custom.clone(), Mode::Fsg)]);
        assert_eq!(stats.mode(Mode::Rsg).run_count, 0);
        let fsg = stats.mode(Mode::Fsg);
        assert_eq!(fsg.run_count, 8, "7 global + 1 world-only");
        assert!(
            fsg.runs
                .iter()
                .any(|r| r.id == "w:custom-1/Random Speedrun #8")
        );
        assert_eq!(stats.unattributed.run_count, 0);

        // A copy of those worlds in a second instance: the same runs, counted once.
        let copy = s.0.join("srl/instances/custom-2");
        fs::create_dir_all(copy.join("saves")).unwrap();
        copy_tree(&custom.join("saves"), &copy.join("saves"));
        let both = scan_with(
            &paths,
            &sources,
            &[(custom.clone(), Mode::Fsg), (copy, Mode::Ssg)],
        );
        assert_eq!(both.mode(Mode::Fsg).run_count, 8);
        assert_eq!(both.mode(Mode::Ssg).run_count, 0);
        assert_eq!(both.unattributed.run_count, 0);
        assert_eq!(
            super::super::mode_of_world_in(
                &paths,
                &[(custom.clone(), Mode::Fsg)],
                &custom.join("saves/W")
            ),
            Some((Mode::Fsg, "W".into()))
        );
    }

    #[test]
    fn deleted_worlds_keep_their_mode_and_unknown_ones_stay_unattributed() {
        let (s, paths, sources) = real_fixture("sticky");
        scan(&paths, &sources);
        // The player deletes every world; a record from another launcher appears.
        fs::remove_dir_all(paths.instance(Mode::Rsg).join("saves")).unwrap();
        let other = s.0.join("home/speedrunigt/records/00000000-other.json");
        fs::write(
            &other,
            br#"{"is_completed":true,"date":1,"final_igt":500000,"final_rta":510000,
                "category":"ANY","run_type":"random_seed","world_name":"Random Speedrun #1"}"#,
        )
        .unwrap();
        let stats = scan(&paths, &sources);
        assert_eq!(stats.parsed, 1);
        let rsg = stats.mode(Mode::Rsg);
        assert_eq!(rsg.run_count, 7, "the world-only run went with its world");
        assert!(rsg.runs.iter().all(|r| r.world.is_none()));
        assert_eq!(stats.unattributed.run_count, 1);
        assert_eq!(stats.unattributed.runs[0].id, "g:00000000-other");
    }

    #[test]
    fn malformed_files_are_skipped_and_counted_once() {
        let (s, paths, sources) = real_fixture("bad");
        let recs = s.0.join("home/speedrunigt/records");
        fs::write(recs.join("broken.json"), b"{\"is_completed\": tru").unwrap();
        fs::write(recs.join("empty.json"), b"").unwrap();
        fs::write(recs.join("notes.txt"), b"ignored: not .json").unwrap();
        let w = paths.instance(Mode::Fsg).join("saves/W/speedrunigt");
        fs::create_dir_all(&w).unwrap();
        fs::write(w.join("record.json"), b"[1,2]").unwrap();
        let stats = scan(&paths, &sources);
        assert_eq!(stats.skipped.count, 3);
        assert_eq!(stats.mode(Mode::Rsg).run_count, 8);
        let again = scan(&paths, &sources);
        assert_eq!((again.skipped.count, again.parsed), (3, 0));
    }

    #[test]
    fn a_name_clash_with_another_launcher_is_not_attributed() {
        // Our RSG world "Random Speedrun #1" (a reset at 14 s) and a global record of a different
        // run with the same world name: its timeline does not continue ours, so it is not ours.
        let (s, paths, sources) = real_fixture("clash");
        let recs = s.0.join("home/speedrunigt/records");
        fs::write(
            recs.join("ffffffff-clash.json"),
            br#"{"is_completed":false,"date":1793015400000,"final_igt":99000,"final_rta":99999,
                "category":"ANY","run_type":"random_seed","world_name":"Random Speedrun #1",
                "timelines":[{"name":"enter_nether","igt":90000,"rta":91000}]}"#,
        )
        .unwrap();
        let stats = scan(&paths, &sources);
        assert_eq!(stats.unattributed.run_count, 1);
        assert_eq!(stats.mode(Mode::Rsg).run_count, 8);
    }

    const EIGHT: &str = "saves/Random Speedrun #8/speedrunigt/record.json";

    fn read_json(path: &Path) -> serde_json::Value {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    fn write_json(path: &Path, v: &serde_json::Value) {
        fs::write(path, serde_json::to_vec(v).unwrap()).unwrap();
    }

    /// World #8's record (its game was killed, so it has no global record), with `edit` applied
    /// to it and written back.
    fn edit_eight(paths: &Paths, edit: impl FnOnce(&mut serde_json::Value)) -> serde_json::Value {
        let file = paths.instance(Mode::Rsg).join(EIGHT);
        let mut rec = read_json(&file);
        edit(&mut rec);
        write_json(&file, &rec);
        rec
    }

    /// A global record `id` that is an older write of `world`'s run, or looks like one: `ago`
    /// ms earlier, with 4 s less IGT and RTA and the same timeline.
    fn older_global(s: &Scratch, id: &str, world: &serde_json::Value, ago: i64) {
        let mut rec = world.clone();
        rec["date"] = serde_json::json!(world["date"].as_i64().unwrap() - ago);
        rec["final_igt"] = serde_json::json!(world["final_igt"].as_i64().unwrap() - 4000);
        rec["final_rta"] = serde_json::json!(world["final_rta"].as_i64().unwrap() - 4000);
        rec["retimed_igt"] = rec["final_igt"].clone();
        write_json(
            &s.0.join(format!("home/speedrunigt/records/{id}.json")),
            &rec,
        );
    }

    #[test]
    fn a_newer_world_copy_is_the_same_run() {
        // The game was stopped mid-run: the global copy is from the last leave, the world copy
        // from a later pause. One run, with the newer times. The shared timeline is the evidence
        // (the scratch copy of the world folder is younger than the record).
        let (s, paths, sources) = real_fixture("newer");
        let world = edit_eight(&paths, |r| {
            r["timelines"] =
                serde_json::json!([{"name": "enter_nether", "igt": 9000, "rta": 9100}]);
        });
        older_global(&s, "88888888-eight", &world, 5000);
        let stats = scan(&paths, &sources);
        let rsg = stats.mode(Mode::Rsg);
        assert_eq!(rsg.run_count, 8);
        let eight = rsg
            .runs
            .iter()
            .find(|r| r.id == "g:88888888-eight")
            .unwrap();
        assert_eq!(eight.record.igt_raw_ms, 16_580, "the newer world copy wins");
        assert!(eight.world.is_some());
    }

    #[test]
    fn an_older_reset_without_splits_needs_a_date_after_the_world_was_made() {
        let (s, paths, sources) = real_fixture("created");
        let created = fs::metadata(paths.instance(Mode::Rsg).join("saves/Random Speedrun #8"))
            .and_then(|m| m.created())
            .ok();
        if created.is_none() {
            eprintln!("this file system has no creation times; only the timeline rule applies");
            return;
        }
        // Dates after the scratch folder was made: written during this world's life.
        let now = crate::cache::now_ms();
        let world = edit_eight(&paths, |r| r["date"] = serde_json::json!(now + 60_000));
        older_global(&s, "88888888-eight", &world, 30_000);
        let stats = scan(&paths, &sources);
        assert_eq!(stats.mode(Mode::Rsg).run_count, 8);
        assert_eq!(stats.unattributed.run_count, 0);
        assert!(
            stats
                .mode(Mode::Rsg)
                .runs
                .iter()
                .any(|r| r.id == "g:88888888-eight" && r.world.is_some())
        );
        // A second candidate appears later: the first one keeps the world (it was the only one
        // when it was matched), the newcomer is not merged.
        older_global(&s, "88888888-other", &world, 20_000);
        let stats = scan(&paths, &sources);
        let rsg = stats.mode(Mode::Rsg);
        assert_eq!(rsg.run_count, 8);
        assert!(
            rsg.runs
                .iter()
                .any(|r| r.id == "g:88888888-eight" && r.world.is_some())
        );
        assert_eq!(stats.unattributed.run_count, 1);
        assert_eq!(stats.unattributed.runs[0].id, "g:88888888-other");

        // Two candidates from the start: ambiguous, so neither is merged into the world.
        let (s, paths, sources) = real_fixture("created-two");
        let world = edit_eight(&paths, |r| r["date"] = serde_json::json!(now + 60_000));
        older_global(&s, "88888888-eight", &world, 30_000);
        older_global(&s, "88888888-other", &world, 20_000);
        let stats = scan(&paths, &sources);
        let rsg = stats.mode(Mode::Rsg);
        assert_eq!(rsg.run_count, 8, "7 global + world #8 on its own");
        assert!(rsg.runs.iter().any(|r| r.id == "w:rsg/Random Speedrun #8"));
        assert_eq!(stats.unattributed.run_count, 2);
    }

    /// The reviewer's probe: resets from another launcher named like our world #8 (Atum counts
    /// per instance), a week older and without splits, are not merged into it, are counted once,
    /// and do not become RSG runs after the world is deleted.
    #[test]
    fn foreign_resets_with_our_world_name_are_not_merged() {
        let (s, paths, sources) = real_fixture("foreign");
        let world = read_json(&paths.instance(Mode::Rsg).join(EIGHT));
        let week = 7 * 86_400_000;
        let mut foreign = world.clone();
        foreign["final_igt"] = serde_json::json!(5000);
        foreign["retimed_igt"] = serde_json::json!(5000);
        foreign["final_rta"] = serde_json::json!(5100);
        foreign["date"] = serde_json::json!(world["date"].as_i64().unwrap() - week);
        let recs = s.0.join("home/speedrunigt/records");
        write_json(&recs.join("aaaaaaaa-prism-5s.json"), &foreign);
        let stats = scan(&paths, &sources);
        assert_eq!(stats.mode(Mode::Rsg).run_count, 8);
        assert_eq!(stats.unattributed.run_count, 1);

        foreign["final_igt"] = serde_json::json!(9000);
        foreign["retimed_igt"] = serde_json::json!(9000);
        foreign["final_rta"] = serde_json::json!(9100);
        write_json(&recs.join("bbbbbbbb-prism-9s.json"), &foreign);
        let stats = scan(&paths, &sources);
        let rsg = stats.mode(Mode::Rsg);
        assert_eq!(rsg.run_count, 8);
        let eight = rsg
            .runs
            .iter()
            .find(|r| r.record.world_name == "Random Speedrun #8")
            .unwrap();
        assert_eq!(
            (eight.id.as_str(), eight.record.igt_raw_ms),
            ("w:rsg/Random Speedrun #8", 16_580)
        );
        assert_eq!(stats.unattributed.run_count, 2);

        fs::remove_dir_all(paths.instance(Mode::Rsg).join("saves")).unwrap();
        let stats = scan(&paths, &sources);
        assert_eq!(
            stats.mode(Mode::Rsg).run_count,
            7,
            "the 7 real global records"
        );
        assert_eq!(stats.unattributed.run_count, 2);
    }

    /// Remembered modes (rule 4) survive what the index does not: a format change, a broken
    /// index, and a scan that could not list the records folder.
    #[test]
    fn remembered_modes_outlive_the_index() {
        let (_s, paths, sources) = real_fixture("memory");
        scan(&paths, &sources);
        fs::remove_dir_all(paths.instance(Mode::Rsg).join("saves")).unwrap();
        let rsg_runs = |stats: &LocalStats| stats.mode(Mode::Rsg).run_count;
        assert_eq!(rsg_runs(&scan(&paths, &sources)), 7);

        // An index of another version, then a truncated one.
        let index = index_path(&paths);
        fs::write(&index, br#"{"version":999,"files":{}}"#).unwrap();
        let stats = scan(&paths, &sources);
        assert_eq!((rsg_runs(&stats), stats.parsed), (7, 7));
        fs::write(&index, b"{\"version\":2,\"fi").unwrap();
        assert_eq!(rsg_runs(&scan(&paths, &sources)), 7);
        fs::remove_file(&index).unwrap();
        assert_eq!(rsg_runs(&scan(&paths, &sources)), 7);
        assert_eq!(scan(&paths, &sources).unattributed.run_count, 0);

        // An attribution file of another version is left alone (a newer launcher wrote it).
        let memory = attribution_path(&paths);
        let saved = fs::read(&memory).unwrap();
        fs::write(&memory, br#"{"version":99,"modes":{},"more":1}"#).unwrap();
        let stats = scan(&paths, &sources);
        assert_eq!((rsg_runs(&stats), stats.unattributed.run_count), (0, 7));
        assert_eq!(
            fs::read(&memory).unwrap(),
            br#"{"version":99,"modes":{},"more":1}"#
        );
        fs::write(&memory, &saved).unwrap();
        assert_eq!(rsg_runs(&scan(&paths, &sources)), 7);
    }

    /// A records folder that can't be listed is reported and prunes nothing.
    #[cfg(unix)]
    #[test]
    fn an_unreadable_records_folder_prunes_nothing() {
        use std::os::unix::fs::PermissionsExt;
        let (s, paths, sources) = real_fixture("unreadable");
        scan(&paths, &sources);
        fs::remove_dir_all(paths.instance(Mode::Rsg).join("saves")).unwrap();
        let recs = s.0.join("home/speedrunigt/records");
        fs::set_permissions(&recs, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read_dir(&recs).is_ok() {
            // Root reads it anyway; nothing to test.
            fs::set_permissions(&recs, fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }
        let stats = scan(&paths, &sources);
        fs::set_permissions(&recs, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(stats.unreadable.len(), 1);
        assert_eq!(stats.unreadable[0].0, recs);
        assert_eq!(stats.mode(Mode::Rsg).run_count, 0);
        let stats = scan(&paths, &sources);
        assert!(stats.unreadable.is_empty());
        assert_eq!(stats.mode(Mode::Rsg).run_count, 7);
        assert_eq!(stats.unattributed.run_count, 0);
    }
}
