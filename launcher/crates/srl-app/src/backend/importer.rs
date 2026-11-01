//! The import extension of the facade: "Import my current settings" over srl-import. The
//! Import page asks for the player's instances in other launchers (`scan`), reads a folder the
//! player picked (`inspect`) and copies the ticked parts into every mode and the practice
//! instance (`import`). The answers come as `Event::Import`.
//!
//! - `RealImport` runs srl-import on the backend's blocking pool. The search roots are this
//!   computer's (`SearchRoots::from_env`, or the fake home in `SRL_IMPORT_HOME` for tests). An
//!   instance whose game runs or installs is left out of an import (the game would write its
//!   options.txt over the imported one when it quits) and the result says so.
//! - `FakeImport` answers with sample instances and a sample result, and writes nothing.
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use srl_core::{Instance, Mode, Paths};
use srl_import::{Choice, Contents, Launcher, Report, SearchRoots, Source, Target, TargetReport};

use super::{Event, Events, pipeline::DRAFTOUT_SEED};

pub trait Importer {
    /// What was found so far. No I/O.
    fn snapshot(&self) -> ImportSnapshot;
    /// Looks for the player's instances again; the answer is `ImportEvent::Sources`.
    fn scan(&self);
    /// A folder the player picked; the answer is `ImportEvent::Chosen`.
    fn inspect(&self, dir: PathBuf);
    /// Copies `choice` from `source` into every mode and the practice instance: `Progress`,
    /// then `Done`. Does nothing while an import runs.
    fn import(&self, source: Source, choice: Choice);
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportSnapshot {
    /// A scan has answered.
    pub scanned: bool,
    /// The player's instances, best match first.
    pub sources: Vec<Source>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImportEvent {
    Sources(Vec<Source>),
    /// What a picked folder holds (empty: nothing to import), or that it is this launcher's own
    /// data (`own`), which it can't import from.
    Chosen {
        dir: PathBuf,
        found: Vec<Source>,
        own: bool,
    },
    Progress {
        done: u64,
        total: u64,
    },
    Done(Box<ImportResult>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportResult {
    pub report: Report,
    /// Instances left alone because their game ran or installed.
    pub busy: Vec<Instance>,
    /// The folder with every instance, where each backup is (`<id>/import-backup-<stamp>`).
    pub instances: PathBuf,
}

/// The instance a report's target id names.
pub fn instance_of(id: &str) -> Option<Instance> {
    if id == Instance::Practice.id() {
        return Some(Instance::Practice);
    }
    // The shared 1.16.1 instance stands for its four modes; RSG speaks for it.
    if id == srl_core::shared::DIR {
        return Some(Instance::Mode(Mode::Rsg));
    }
    Mode::from_id(id).map(Instance::Mode)
}

/// Every instance the import goes into, in the order the result lists them: the shared 1.16.1
/// instance (id `mcsr`, its modes' `Instance` is RSG's), Draftout, then practice. Draftout's
/// options.txt keeps the line `seed_options` gives a new one. The shared instance keeps its
/// per-mode files (Atum's settings): the source's may hold an SSG seed, and in place they would
/// become the settings of whichever mode plays next.
pub fn targets(paths: &Paths) -> Vec<(Instance, Target)> {
    let mut out: Vec<(Instance, Target)> = [Mode::Rsg, Mode::Draftout]
        .into_iter()
        .map(|m| {
            let id = Instance::Mode(m).dir_id();
            let t = Target::new(&id, paths.instance(m), m.minecraft_version());
            let t = if m == Mode::Draftout {
                t.with_seed(DRAFTOUT_SEED.0, DRAFTOUT_SEED.1)
            } else {
                srl_core::shared::PER_MODE_FILES
                    .into_iter()
                    .fold(t, Target::keeping)
            };
            (Instance::Mode(m), t)
        })
        .collect();
    out.push((
        Instance::Practice,
        Target::new(
            &Instance::Practice.id(),
            paths.practice_instance(),
            srl_practice::PRACTICE.minecraft_version,
        ),
    ));
    out
}

/// Which of the ticked parts a source can bring at all: one it doesn't have, or one no instance
/// can take (another Minecraft version than every instance's, so only keys come over), is off.
pub fn effective(choice: Choice, source: &Source, versions: &[&str]) -> Choice {
    let c = &source.contents;
    let same = versions.iter().any(|v| source.runs(v));
    Choice {
        options: choice.options && c.options,
        mod_settings: choice.mod_settings && same && c.mod_files > 0,
        resource_packs: choice.resource_packs && same && c.resource_packs > 0,
        worlds: choice.worlds && same && c.worlds > 0,
    }
}

/// The Minecraft versions of the launcher's instances (`effective`).
pub fn versions() -> Vec<&'static str> {
    let mut v: Vec<&str> = Mode::ALL.iter().map(|m| m.minecraft_version()).collect();
    v.push(srl_practice::PRACTICE.minecraft_version);
    v.sort();
    v.dedup();
    v
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Whether `dir` is inside the launcher's data folder (or is it).
pub fn inside(dir: &Path, root: &Path) -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    canon(dir).starts_with(canon(root))
}

/// `RealBackend`'s import.
pub struct RealImport {
    paths: Paths,
    roots: SearchRoots,
    /// Whether an instance's game runs or installs right now.
    busy: Arc<dyn Fn(Instance) -> bool + Send + Sync>,
    shared: Arc<Mutex<ImportSnapshot>>,
    running: Arc<AtomicBool>,
    events: Events,
    rt: tokio::runtime::Handle,
}

impl RealImport {
    pub fn new(
        paths: Paths,
        busy: Arc<dyn Fn(Instance) -> bool + Send + Sync>,
        events: Events,
        rt: tokio::runtime::Handle,
    ) -> Self {
        Self {
            paths,
            roots: SearchRoots::from_env(),
            busy,
            shared: Arc::default(),
            running: Arc::default(),
            events,
            rt,
        }
    }
}

impl Importer for RealImport {
    fn snapshot(&self) -> ImportSnapshot {
        self.shared.lock().unwrap().clone()
    }

    fn scan(&self) {
        let (roots, shared, events) =
            (self.roots.clone(), self.shared.clone(), self.events.clone());
        self.rt.spawn_blocking(move || {
            let started = Instant::now();
            let found = srl_import::detect(&roots);
            super::real::log(format_args!(
                "import: {} instances found in {} ms{}",
                found.len(),
                started.elapsed().as_millis(),
                found
                    .iter()
                    .map(|s| format!(
                        "\n  {} {:?} {} {}",
                        s.launcher.name(),
                        s.name,
                        s.version.as_deref().unwrap_or("?"),
                        s.game_dir.display()
                    ))
                    .collect::<String>()
            ));
            *shared.lock().unwrap() = ImportSnapshot {
                scanned: true,
                sources: found.clone(),
            };
            let _ = events.unbounded_send(Event::Import(ImportEvent::Sources(found)));
        });
    }

    fn inspect(&self, dir: PathBuf) {
        let (root, events) = (self.paths.root().to_path_buf(), self.events.clone());
        self.rt.spawn_blocking(move || {
            let own = inside(&dir, &root);
            let found = if own {
                Vec::new()
            } else {
                srl_import::inspect(&dir)
            };
            super::real::log(format_args!(
                "import: {} picked: {} instances{}",
                dir.display(),
                found.len(),
                if own {
                    " (the launcher's own data)"
                } else {
                    ""
                }
            ));
            let _ = events.unbounded_send(Event::Import(ImportEvent::Chosen { dir, found, own }));
        });
    }

    fn import(&self, source: Source, choice: Choice) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let (paths, busy, running, events) = (
            self.paths.clone(),
            self.busy.clone(),
            self.running.clone(),
            self.events.clone(),
        );
        self.rt.spawn_blocking(move || {
            let (mut into, mut skipped) = (Vec::new(), Vec::new());
            for (instance, target) in targets(&paths) {
                if busy(instance) {
                    skipped.push(instance);
                } else {
                    into.push(target);
                }
            }
            let choice = effective(choice, &source, &versions());
            super::real::log(format_args!(
                "import: from {:?} {:?} ({}) {:?}, into {}; left alone while busy: {:?}",
                source.launcher,
                source.name,
                source.game_dir.display(),
                choice,
                into.iter()
                    .map(|t| t.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                skipped
            ));
            let mut last = Instant::now() - Duration::from_secs(1);
            let report = srl_import::import(&source, choice, &into, now(), &mut |done, total| {
                if done == total || last.elapsed() >= Duration::from_millis(50) {
                    last = Instant::now();
                    let _ =
                        events.unbounded_send(Event::Import(ImportEvent::Progress { done, total }));
                }
            });
            for t in &report.targets {
                super::real::log(format_args!(
                    "import: {}: {} files, {} settings{}{}{}",
                    t.id,
                    t.files,
                    t.settings,
                    if t.full { "" } else { ", keys only" },
                    t.backup
                        .as_ref()
                        .map(|b| format!(", backup in {}", b.display()))
                        .unwrap_or_default(),
                    t.error
                        .as_ref()
                        .map(|e| format!(", stopped: {e}"))
                        .unwrap_or_default(),
                ));
            }
            if !report.skipped.is_empty() {
                super::real::log(format_args!(
                    "import: left out {} files: {:?}",
                    report.skipped.len() as u32 + report.skipped_more,
                    report.skipped
                ));
            }
            running.store(false, Ordering::SeqCst);
            let _ =
                events.unbounded_send(Event::Import(ImportEvent::Done(Box::new(ImportResult {
                    report,
                    busy: skipped,
                    instances: paths.root().join("instances"),
                }))));
        });
    }
}

/// The fake's starting state (`SRL_SCREEN=import*`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImportPreset {
    /// Four instances found (three on 1.16.1).
    #[default]
    Found,
    /// The scan hasn't answered.
    Scanning,
    /// Nothing found.
    Nothing,
    /// Only instances on other versions.
    Other,
    /// An import 40 % through.
    Importing,
    /// An import done: everything into the 1.16.1 instances, keys into Draftout, RSG's backup.
    Done,
}

/// `FakeBackend`'s import: sample instances, a 1.5 s import, a sample result. Writes nothing.
pub struct FakeImport {
    preset: ImportPreset,
    data_dir: PathBuf,
    shared: Arc<Mutex<ImportSnapshot>>,
    running: Arc<AtomicBool>,
    events: Events,
}

const DAY: u64 = 86_400;

fn sample(
    launcher: Launcher,
    name: &str,
    dir: &str,
    version: &str,
    days_ago: u64,
    contents: Contents,
) -> Source {
    Source {
        launcher,
        name: name.into(),
        game_dir: PathBuf::from(dir),
        version: Some(version.into()),
        last_played: Some(now().saturating_sub(days_ago * DAY + 3600)),
        contents,
    }
}

fn speedrun_contents(worlds: u32) -> Contents {
    Contents {
        options: true,
        mod_files: 23,
        mods: vec![
            "StandardSettings",
            "SpeedRunIGT",
            "Atum",
            "SeedQueue",
            "Sodium",
        ],
        resource_packs: 2,
        worlds,
    }
}

/// The fake's instances.
pub fn sample_sources(preset: ImportPreset) -> Vec<Source> {
    let modern = sample(
        Launcher::Official,
        ".minecraft",
        "C:\\Users\\Violet\\AppData\\Roaming\\.minecraft",
        "1.21.4",
        94,
        Contents {
            options: true,
            resource_packs: 4,
            worlds: 6,
            ..Contents::default()
        },
    );
    match preset {
        ImportPreset::Nothing | ImportPreset::Scanning => Vec::new(),
        ImportPreset::Other => vec![
            modern,
            sample(
                Launcher::Prism,
                "Survival 1.20.1",
                "C:\\Users\\Violet\\AppData\\Roaming\\PrismLauncher\\instances\\Survival\\minecraft",
                "1.20.1",
                130,
                Contents {
                    options: true,
                    mod_files: 8,
                    ..Contents::default()
                },
            ),
        ],
        _ => vec![
            sample(
                Launcher::MultiMc,
                "RSG",
                "C:\\Users\\Violet\\Desktop\\MultiMC\\instances\\RSG\\.minecraft",
                "1.16.1",
                2,
                speedrun_contents(1284),
            ),
            sample(
                Launcher::Prism,
                "Ranked",
                "C:\\Users\\Violet\\AppData\\Roaming\\PrismLauncher\\instances\\Ranked\\minecraft",
                "1.16.1",
                9,
                speedrun_contents(41),
            ),
            sample(
                Launcher::McsrLauncher,
                "MCSR Ranked",
                "C:\\Users\\Violet\\AppData\\Local\\MCSRLauncher\\launcher\\instances\\mcsr-ranked\\.minecraft",
                "1.16.1",
                31,
                Contents {
                    worlds: 0,
                    resource_packs: 0,
                    ..speedrun_contents(0)
                },
            ),
            modern,
        ],
    }
}

/// The fake's result for an import of `source` into every instance (`targets`).
fn sample_result(data_dir: &Path, source: &Source, choice: Choice) -> ImportResult {
    let paths = Paths::new(data_dir);
    let stamp = now();
    let targets = targets(&paths)
        .into_iter()
        .map(|(instance, t)| {
            let full = source.runs(&t.version);
            let rsg = instance == Instance::Mode(Mode::Rsg);
            TargetReport {
                id: t.id.clone(),
                full,
                settings: if full { 117 } else { 35 },
                files: if full { 31 } else { 1 },
                // RSG was played before: its old files went into the backup.
                backup: rsg.then(|| t.dir.join(format!("import-backup-{stamp}"))),
                left_out: if full {
                    Vec::new()
                } else {
                    [
                        (choice.mod_settings, srl_import::Part::ModSettings),
                        (choice.resource_packs, srl_import::Part::ResourcePacks),
                        (choice.worlds, srl_import::Part::Worlds),
                    ]
                    .into_iter()
                    .filter(|(on, _)| *on)
                    .map(|(_, p)| p)
                    .collect()
                },
                ..TargetReport::default()
            }
        })
        .collect();
    ImportResult {
        report: Report {
            stamp,
            targets,
            skipped: Vec::new(),
            skipped_more: 0,
        },
        busy: Vec::new(),
        instances: paths.root().join("instances"),
    }
}

impl FakeImport {
    pub fn new(events: Events, preset: ImportPreset, data_dir: PathBuf) -> Self {
        let this = Self {
            preset,
            data_dir,
            shared: Arc::default(),
            running: Arc::default(),
            events,
        };
        // The states in the middle of, or after, an import show at once.
        match preset {
            ImportPreset::Importing => {
                this.answer();
                this.running.store(true, Ordering::SeqCst);
                let _ = this
                    .events
                    .unbounded_send(Event::Import(ImportEvent::Progress {
                        done: 412,
                        total: 1038,
                    }));
            }
            ImportPreset::Done => {
                this.answer();
                let sources = sample_sources(preset);
                let result = sample_result(&this.data_dir, &sources[0], Choice::default());
                let _ = this
                    .events
                    .unbounded_send(Event::Import(ImportEvent::Done(Box::new(result))));
            }
            _ => {}
        }
        this
    }

    fn answer(&self) {
        if self.preset == ImportPreset::Scanning {
            return;
        }
        let sources = sample_sources(self.preset);
        *self.shared.lock().unwrap() = ImportSnapshot {
            scanned: true,
            sources: sources.clone(),
        };
        let _ = self
            .events
            .unbounded_send(Event::Import(ImportEvent::Sources(sources)));
    }
}

impl Importer for FakeImport {
    fn snapshot(&self) -> ImportSnapshot {
        self.shared.lock().unwrap().clone()
    }

    fn scan(&self) {
        if !self.running.load(Ordering::SeqCst) {
            self.answer();
        }
    }

    fn inspect(&self, dir: PathBuf) {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Minecraft".into());
        let found = vec![Source {
            launcher: Launcher::Folder,
            name,
            game_dir: dir.clone(),
            version: Some(srl_import::VERSION_1_16_1.into()),
            last_played: Some(now() - 5 * DAY),
            contents: speedrun_contents(12),
        }];
        let _ = self
            .events
            .unbounded_send(Event::Import(ImportEvent::Chosen {
                dir,
                found,
                own: false,
            }));
    }

    fn import(&self, source: Source, choice: Choice) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let (events, running, data_dir) = (
            self.events.clone(),
            self.running.clone(),
            self.data_dir.clone(),
        );
        std::thread::spawn(move || {
            let total = 240;
            for done in (0..=total).step_by(24) {
                let _ = events.unbounded_send(Event::Import(ImportEvent::Progress { done, total }));
                std::thread::sleep(Duration::from_millis(150));
            }
            let choice = effective(choice, &source, &versions());
            let result = sample_result(&data_dir, &source, choice);
            running.store(false, Ordering::SeqCst);
            let _ = events.unbounded_send(Event::Import(ImportEvent::Done(Box::new(result))));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_instance_is_a_target_and_draftout_keeps_its_seed() {
        let paths = Paths::new("/srl");
        let t = targets(&paths);
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].1.id, "mcsr");
        for (instance, target) in &t {
            assert_eq!(instance_of(&target.id), Some(*instance));
            assert_eq!(target.dir, paths.instance_dir(*instance));
        }
        let draftout = &t[1].1;
        assert_eq!(draftout.version, "26.3");
        assert_eq!(
            draftout.seed,
            [("onboardAccessibility".to_string(), "false".to_string())]
        );
        assert_eq!(t[2].1.version, "1.16.1");
        assert!(
            t.iter()
                .filter(|(i, _)| *i != Instance::Mode(Mode::Draftout))
                .all(|(_, t)| t.seed.is_empty())
        );
        assert_eq!(versions(), ["1.16.1", "26.3"]);
    }

    #[test]
    fn parts_a_source_lacks_or_no_instance_takes_are_off() {
        let all = Choice {
            worlds: true,
            ..Choice::default()
        };
        let s = &sample_sources(ImportPreset::Found)[2];
        // No worlds and no packs in MCSR Ranked's sample.
        let c = effective(all, s, &versions());
        assert!(c.options && c.mod_settings && !c.resource_packs && !c.worlds);
        // 1.21.4: only the settings (as keys) can come over.
        let modern = &sample_sources(ImportPreset::Found)[3];
        assert_eq!(
            effective(all, modern, &versions()),
            Choice {
                options: true,
                mod_settings: false,
                resource_packs: false,
                worlds: false
            }
        );
        // A Draftout instance (26.3) gives Draftout everything.
        let mut draftout = modern.clone();
        draftout.version = Some("26.3".into());
        assert!(effective(all, &draftout, &versions()).worlds);
    }

    /// An SSG instance's Atum settings hold its seed. In the shared instance they belong to one
    /// mode (`srl_core::shared`), and the import can't tell which: imported, they would become
    /// the settings of whichever mode plays next, and RSG would reset into the SSG seed.
    #[test]
    fn the_shared_instance_keeps_its_own_atum_settings() {
        let root = std::env::temp_dir().join(format!("srl-import-atum-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let write = |p: &Path, text: &str| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        let game = root.join("multimc/instances/SSG/.minecraft");
        write(&game.join("options.txt"), "version:2567\nfov:0.25\n");
        write(
            &game.join("config/mcsr/atum.json"),
            r#"{"seed":"-3294725893620991126"}"#,
        );
        write(
            &game.join("config/atum/atum.properties"),
            "seed=-3294725893620991126\n",
        );
        write(&game.join("config/mcsr/seedqueue.json"), "{}");
        let paths = Paths::new(root.join("data"));
        let shared = paths.instance(Mode::Rsg);
        // RSG played here before: its Atum settings, without a seed.
        write(&shared.join("config/mcsr/atum.json"), r#"{"seed":""}"#);
        let source = Source {
            launcher: Launcher::MultiMc,
            name: "SSG".into(),
            game_dir: game.clone(),
            version: Some(srl_import::VERSION_1_16_1.into()),
            last_played: None,
            contents: srl_import::contents(&game),
        };
        let into: Vec<Target> = targets(&paths).into_iter().map(|(_, t)| t).collect();
        let report = srl_import::import(&source, Choice::default(), &into, 7, &mut |_, _| {});
        assert!(report.targets.iter().all(|t| t.error.is_none()));
        // Everything else came over.
        assert!(shared.join("config/mcsr/seedqueue.json").is_file());
        assert_eq!(
            std::fs::read_to_string(shared.join("config/mcsr/atum.json")).unwrap(),
            r#"{"seed":""}"#
        );
        assert!(!shared.join("config/atum/atum.properties").exists());
        assert!(
            !shared
                .join("import-backup-7/config/mcsr/atum.json")
                .exists()
        );
        // RSG plays: still no seed.
        srl_core::shared::switch_to(&paths, Mode::Rsg).unwrap();
        assert_eq!(
            std::fs::read_to_string(shared.join("config/mcsr/atum.json")).unwrap(),
            r#"{"seed":""}"#
        );
        // The practice instance has no modes: it takes them.
        assert!(
            paths
                .practice_instance()
                .join("config/mcsr/atum.json")
                .is_file()
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_folder_in_the_data_folder_is_the_launchers_own() {
        let root = std::env::temp_dir().join(format!("srl-import-own-{}", std::process::id()));
        std::fs::create_dir_all(root.join("instances/rsg")).unwrap();
        assert!(inside(&root.join("instances/rsg"), &root));
        assert!(inside(&root, &root));
        assert!(!inside(&std::env::temp_dir(), &root));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
