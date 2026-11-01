//! Minecraft itself: version metadata, libraries, natives, assets, Java runtimes, Fabric Loader
//! and starting the game. See launcher/README.md, stream "game".
//!
//! # API
//!
//! ```no_run
//! # async fn demo() -> anyhow::Result<()> {
//! use srl_core::{Downloader, Mode, OfflineAccount, Paths};
//! use srl_game::{GameInstaller, LaunchOptions};
//!
//! let installer = GameInstaller::new(Paths::from_env()?, Downloader::new()?);
//! // Idempotent. The second call takes a stat per file and no network (see `verify`).
//! let installed = installer.install(Mode::Rsg, None).await?;
//! let opts = LaunchOptions::new(Mode::Rsg, OfflineAccount::new("Violet_Runner"));
//! let game = installer.launch(&installed, &opts).await?;
//! println!("pid {} logging to {}", game.pid(), game.log_path().display());
//! let exit = game.wait().await; // or game.stop(grace) / game.kill()
//! # Ok(()) }
//! ```
//!
//! - [`GameInstaller::install`] makes everything a mode needs present and verified: the Mojang
//!   version JSON (sha1 from the version manifest), the Fabric Loader profile merged into it,
//!   the client jar, libraries (Maven layout under `libraries/`), natives (extracted for
//!   1.16.1, plain classpath jars for 26.x), the asset index and objects, the log4j config and
//!   the mode's Java runtime. It reports [`srl_core::Progress`] for `Metadata`, `Java`, `Game`
//!   and `Assets`, and returns an [`Installed`]. It never touches `instances/<mode>/mods`
//!   (srl-mods owns that); it only creates the instance directory.
//! - [`GameInstaller::launch`] starts the game with the account in [`LaunchOptions`] (offline, or
//!   a Microsoft session as [`GameAccount::Microsoft`]) and returns a
//!   [`GameProcess`]: `pid`, `is_running`, `wait`, `stop` (closes the window so the game saves,
//!   then SIGTERM, then kill; see [`stop`]), `kill`. stdout
//!   and stderr go to `logs/<mode>-latest.log`. No console window on Windows (`javaw.exe`,
//!   `CREATE_NO_WINDOW`). The game is not tied to the handle and survives a launcher exit.
//! - [`GameInstaller::command`] builds the same command line without starting it, and
//!   [`GameInstaller::spawn`] starts such a command (after srl-tools wrapped it, for example).
//! - Per-mode choices (Java version, minimum Fabric Loader, JVM flags, default memory) are in
//!   [`profile`], with the reasons.
//! - **The practice instance** is not a mode. [`GameInstaller::install_target`] with
//!   [`Target::practice`] installs its game (the Minecraft version, Java and Fabric Loader floor
//!   the caller passes from srl-practice's `PRACTICE`) into `instances/practice`; its game logs to
//!   `logs/practice-latest.log`. `install(mode)` is `install_target(Target::mode(mode))`, so the
//!   modes go through exactly the same code as before.
//!
//! Metadata lists (Mojang's manifest and Java runtime list, Fabric's loader list) are cached in
//! `meta/` and refreshed when older than [`GameInstaller::with_meta_max_age`] (one hour by
//! default). If the network is down the cached copies are used and a note lands in
//! [`Installed::warnings`]; an install with everything cached needs no network at all.
//!
//! # CLI
//!
//! `src/bin/srl-game.rs` drives the same API for tests and CI; run it with
//! `cargo run -p srl-game -- help`. It uses `$SRL_HOME` like everything else.
pub mod fabric;
pub mod java;
pub mod launch;
pub mod libraries;
pub mod meta;
pub mod natives;
pub mod profile;
pub mod rules;
pub mod schema;
pub mod stop;
pub mod util;
pub mod verify;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use srl_core::{
    Download, Downloader, Hash, Instance, Mode, Paths, Progress, ProgressFn, Stage, Unit,
};

pub use java::JavaRuntime;
pub use launch::{
    Escalation, GameAccount, GameExit, GameProcess, HIDDEN_TOKEN, LaunchCommand, LaunchOptions,
    OnlineSession, READY_MARKERS, StopReport,
};
pub use profile::{ModeProfile, Target, profile};
pub use rules::Platform;
pub use stop::QuitRequest;

use crate::{
    meta::MetaCache,
    rules::Features,
    schema::{
        AssetIndex, JAVA_RUNTIMES_URL, JavaRuntimeManifest, JavaRuntimes, VERSION_MANIFEST_URL,
        VersionJson, VersionManifest,
    },
    util::join_safe,
    verify::Stamp,
};

/// Everything needed to start a mode (or the practice instance), as left on disk by
/// [`GameInstaller::install`] or [`GameInstaller::install_target`].
#[derive(Clone, Debug)]
pub struct Installed {
    /// The mode, or the practice instance.
    pub instance: Instance,
    /// What it was installed for: its Java, Fabric floor, JVM flags and memory.
    pub profile: ModeProfile,
    /// E.g. `1.16.1`.
    pub minecraft_version: String,
    /// The merged profile id, e.g. `fabric-loader-0.20.1-1.16.1`.
    pub version_id: String,
    pub fabric_loader: String,
    pub java: JavaRuntime,
    /// Vanilla merged with the Fabric profile.
    pub version: VersionJson,
    /// Absolute jar paths in launch order: Fabric, vanilla libraries, the client jar last.
    pub classpath: Vec<PathBuf>,
    pub natives_dir: PathBuf,
    pub libraries_dir: PathBuf,
    pub assets_dir: PathBuf,
    /// Asset index id, e.g. `1.16` or `34`.
    pub asset_index: String,
    /// The log4j argument template (`-Dlog4j.configurationFile=${path}`) and the config file.
    pub log_config: Option<(String, PathBuf)>,
    /// `instances/<mode>` (or `instances/practice`), passed as `--gameDir` and used as the
    /// working directory.
    pub game_dir: PathBuf,
    /// Offline fallbacks and similar notes worth showing the user.
    pub warnings: Vec<String>,
}

impl Installed {
    /// The mode, `None` for the practice instance.
    pub fn mode(&self) -> Option<Mode> {
        self.instance.mode()
    }
}

pub struct GameInstaller {
    paths: Paths,
    downloader: Downloader,
    meta_max_age: Duration,
    fabric_loader: Option<String>,
    platform: Platform,
}

impl GameInstaller {
    pub fn new(paths: Paths, downloader: Downloader) -> Self {
        Self {
            paths,
            downloader,
            meta_max_age: Duration::from_secs(3600),
            fabric_loader: None,
            platform: Platform::current(),
        }
    }

    /// How old cached metadata lists may be before they are refreshed. `Duration::ZERO` always
    /// asks the network first (still falling back to the cache when it is down).
    pub fn with_meta_max_age(mut self, max_age: Duration) -> Self {
        self.meta_max_age = max_age;
        self
    }

    /// Pins the Fabric Loader version for every mode instead of the newest stable one.
    pub fn with_fabric_loader(mut self, version: impl Into<String>) -> Self {
        self.fabric_loader = Some(version.into());
        self
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn platform(&self) -> &Platform {
        &self.platform
    }

    /// Installs or repairs everything `mode` needs except its mods. Safe to call before every
    /// launch: files that are already verified are not hashed or downloaded again.
    pub async fn install(&self, mode: Mode, progress: Option<ProgressFn>) -> Result<Installed> {
        self.install_target(&Target::mode(mode), progress).await
    }

    /// [`GameInstaller::install`] for any target: a mode, or the practice instance
    /// ([`Target::practice`]). Its verification record is `meta/verified/<instance id>.json` and
    /// its game directory `Paths::instance_dir`; everything else (libraries, assets, Java) is
    /// shared by path with the modes on the same Minecraft version.
    pub async fn install_target(
        &self,
        target: &Target,
        progress: Option<ProgressFn>,
    ) -> Result<Installed> {
        let prof = target.profile;
        let game = prof.minecraft_version;
        let meta = MetaCache::new(&self.downloader, self.meta_max_age);

        // 1. Metadata: vanilla (then its asset index), Fabric and Java in parallel.
        const META_STEPS: u64 = 5;
        let meta_done = AtomicU64::new(0);
        let step = || {
            let done = meta_done.fetch_add(1, Ordering::Relaxed) + 1;
            if let Some(p) = &progress {
                p(Progress {
                    stage: Stage::Metadata,
                    done,
                    total: META_STEPS,
                    unit: Unit::Files,
                });
            }
        };
        let vanilla = async {
            let v = self.vanilla_version(&meta, game).await?;
            step();
            let index = self.asset_index(&meta, &v).await?;
            step();
            anyhow::Ok((v, index))
        };
        let fabric = async {
            let loader = match &self.fabric_loader {
                Some(v) => v.clone(),
                None => {
                    fabric::resolve_loader(&meta, &self.paths, game, prof.min_fabric_loader).await?
                }
            };
            step();
            let (loader, profile) = match fabric::load_profile(&meta, &self.paths, game, &loader)
                .await
            {
                Ok(profile) => (loader, profile),
                // The cached list can name a loader whose profile never arrived (it came out,
                // and the network failed before its profile did). An installed one still
                // starts the game; a pinned version is the caller's choice and stays an error.
                Err(err) if self.fabric_loader.is_none() => {
                    let Some(v) =
                        fabric::newest_installed(&self.paths, game, prof.min_fabric_loader)
                            .filter(|v| *v != loader)
                    else {
                        return Err(err);
                    };
                    meta.warn(format!(
                            "Fabric Loader {loader} is unavailable ({err:#}); using the installed loader {v}"
                        ));
                    let profile = fabric::load_profile(&meta, &self.paths, game, &v).await?;
                    (v, profile)
                }
                Err(err) => return Err(err),
            };
            step();
            anyhow::Ok((loader, profile))
        };
        let java = async {
            java::check_component(prof.java_component)?;
            let r = match self.java_manifest(&meta, prof.java_component).await {
                Ok((version, manifest)) => (version, Some(manifest)),
                // The runtime list names a build whose file list can't be fetched now: a runtime
                // an earlier Play finished still starts the game.
                Err(err) => {
                    let Some((version, _)) = java::newest_complete(
                        &self.paths.java(),
                        prof.java_component,
                        &self.platform,
                        None,
                    ) else {
                        return Err(err);
                    };
                    meta.warn(format!(
                        "the {} runtime is unavailable ({err:#}); using the installed Java {version}",
                        prof.java_component
                    ));
                    (version, None)
                }
            };
            step();
            anyhow::Ok(r)
        };
        let ((vanilla, index), (loader, fabric_profile), (java_version, java_manifest)) =
            tokio::try_join!(vanilla, fabric, java)?;

        // 2. What has to be on disk.
        let version = libraries::merge(vanilla, fabric_profile);
        let resolved =
            libraries::resolve(&version.libraries, &self.platform, &Features::default())?;
        let libs_dir = self.paths.libraries();
        let lib_download = |f: &libraries::LibraryFile| -> Result<Download> {
            let sha1 = f
                .sha1
                .clone()
                .with_context(|| format!("library {} has no sha1", f.path))?;
            Ok(Download {
                url: f.url.clone(),
                dest: join_safe(&libs_dir, &f.path)?,
                hash: Hash::Sha1(sha1),
                size: f.size,
            })
        };
        let mut game_files = Vec::new();
        let mut classpath = Vec::new();
        for f in &resolved.classpath {
            let d = lib_download(f)?;
            classpath.push(d.dest.clone());
            game_files.push(d);
        }
        let mut native_sources = Vec::new();
        for n in &resolved.natives {
            let d = lib_download(&n.file)?;
            native_sources.push(natives::NativeSource {
                jar: d.dest.clone(),
                sha1: n.file.sha1.clone().unwrap_or_default(),
                exclude: n.exclude.clone(),
            });
            game_files.push(d);
        }
        let client = version
            .downloads
            .as_ref()
            .with_context(|| format!("{game} has no client download"))?
            .client
            .clone();
        let client_jar = self.paths.version_dir(game).join(format!("{game}.jar"));
        classpath.push(client_jar.clone());
        game_files.push(Download {
            url: client.url,
            dest: client_jar,
            hash: Hash::Sha1(client.sha1),
            size: Some(client.size),
        });
        let mut log_config = None;
        if let Some(target) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
            let dest = join_safe(&self.paths.assets().join("log_configs"), &target.file.id)?;
            log_config = Some((target.argument.clone(), dest.clone()));
            game_files.push(Download {
                url: target.file.url.clone(),
                dest,
                hash: Hash::Sha1(target.file.sha1.clone()),
                size: Some(target.file.size),
            });
        }

        let java_home = java::home(&self.paths.java(), prof.java_component, &java_version)?;
        let java_plan = match &java_manifest {
            Some(m) => Some(java::plan(m, &java_home)?),
            None => None,
        };

        if index.is_virtual || index.map_to_resources {
            bail!("asset indexes for Minecraft 1.7 and older are not supported");
        }
        let objects = self.paths.assets().join("objects");
        let mut by_hash: HashMap<&str, u64> = HashMap::new();
        for o in index.objects.values() {
            by_hash.insert(o.hash.as_str(), o.size);
        }
        let mut asset_files = Vec::with_capacity(by_hash.len());
        for (hash, size) in by_hash {
            let prefix = hash.get(..2).context("bad asset hash")?;
            if !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                bail!("bad asset hash {hash:?}");
            }
            asset_files.push(Download {
                url: format!("{}/{prefix}/{hash}", schema::ASSETS_URL),
                dest: objects.join(prefix).join(hash),
                hash: Hash::Sha1(hash.to_string()),
                size: Some(size),
            });
        }

        // 3. Download what isn't verified yet, the three stages in parallel.
        let stamp_path = self
            .paths
            .meta()
            .join("verified")
            .join(format!("{}.json", target.instance.id()));
        let mut stamp = Stamp::load(&stamp_path);
        let stale = |items: &[Download]| -> Vec<Download> {
            items
                .iter()
                .filter(|d| !stamp.is_fresh(d))
                .cloned()
                .collect()
        };
        let (java_todo, game_todo, assets_todo) = (
            java_plan
                .as_ref()
                .map(|p| stale(&p.downloads))
                .unwrap_or_default(),
            stale(&game_files),
            stale(&asset_files),
        );
        let (java_got, game_got, assets_got) = tokio::join!(
            self.downloader
                .download_all(java_todo, Stage::Java, progress.clone()),
            self.downloader
                .download_all(game_todo, Stage::Game, progress.clone()),
            self.downloader
                .download_all(assets_todo, Stage::Assets, progress.clone()),
        );
        game_got?;
        assets_got?;
        // A Java update cut off halfway (no network) leaves the previous runtime whole in its own
        // directory (`java::home`): the game starts with that one, and the next Play retries.
        let (java_version, java_home, java_plan) = match java_got {
            Ok(()) => (java_version, java_home, java_plan),
            Err(err) => {
                let Some((version, home)) = java::newest_complete(
                    &self.paths.java(),
                    prof.java_component,
                    &self.platform,
                    Some(&java_version),
                ) else {
                    return Err(err);
                };
                meta.warn(format!(
                    "Java {java_version} couldn't be downloaded ({err:#}); using the installed Java {version}"
                ));
                (version, home, None)
            }
        };

        // 4. Finish: Java permissions and links, natives, the instance directory.
        let natives_dir = self.paths.natives(game);
        let game_dir = self.paths.instance_dir(target.instance);
        {
            let java_plan = java_plan.clone();
            let natives_dir = natives_dir.clone();
            let game_dir = game_dir.clone();
            tokio::task::spawn_blocking(move || -> Result<()> {
                if let Some(plan) = &java_plan {
                    java::finish(plan)?;
                }
                std::fs::create_dir_all(&natives_dir)?;
                if !native_sources.is_empty() {
                    natives::extract_all(&native_sources, &natives_dir)?;
                }
                std::fs::create_dir_all(game_dir.join("mods"))?;
                Ok(())
            })
            .await??;
        }
        let executable = java::executable(&java_home, &self.platform);
        if !executable.is_file() {
            bail!("the Java runtime has no {}", executable.display());
        }
        if java_plan.is_some()
            && let Err(e) = java::mark_complete(&java_home)
        {
            meta.warn(format!(
                "couldn't note {} as complete: {e}",
                java_home.display()
            ));
        }

        for d in java_plan
            .iter()
            .flat_map(|p| &p.downloads)
            .chain(&game_files)
            .chain(&asset_files)
        {
            stamp.record(d);
        }
        stamp
            .save(&stamp_path)
            .with_context(|| format!("saving {}", stamp_path.display()))?;

        Ok(Installed {
            instance: target.instance,
            profile: prof,
            minecraft_version: game.to_string(),
            version_id: version.id.clone(),
            fabric_loader: loader,
            java: JavaRuntime {
                component: prof.java_component.to_string(),
                version: java_version,
                home: java_home,
                executable,
            },
            asset_index: index_id(&version)?,
            version,
            classpath,
            natives_dir,
            libraries_dir: libs_dir,
            assets_dir: self.paths.assets(),
            log_config,
            game_dir,
            warnings: meta.warnings.into_inner().unwrap(),
        })
    }

    /// The command [`GameInstaller::launch`] would run.
    pub fn command(&self, installed: &Installed, opts: &LaunchOptions) -> Result<LaunchCommand> {
        launch::build_command(installed, opts, &self.platform)
    }

    /// Starts the game. Logs go to `logs/<mode>-latest.log` (`logs/practice-latest.log`).
    pub async fn launch(&self, installed: &Installed, opts: &LaunchOptions) -> Result<GameProcess> {
        let cmd = self.command(installed, opts)?;
        self.spawn(installed.instance, &cmd).await
    }

    /// Starts a command from [`GameInstaller::command`], possibly changed on the way (srl-tools
    /// wraps it for waywall). Logs go to `logs/<id>-latest.log`, as with `launch`.
    pub async fn spawn(
        &self,
        instance: impl Into<Instance>,
        cmd: &LaunchCommand,
    ) -> Result<GameProcess> {
        let instance = instance.into();
        let log = self
            .paths
            .logs()
            .join(format!("{}-latest.log", instance.id()));
        launch::spawn(cmd, instance, log).await
    }

    /// The vanilla version JSON, verified against the manifest's sha1.
    async fn vanilla_version(&self, meta: &MetaCache<'_>, game: &str) -> Result<VersionJson> {
        let manifest_path = self
            .paths
            .meta()
            .join("mojang")
            .join("version_manifest_v2.json");
        let json_path = self.paths.version_dir(game).join(format!("{game}.json"));
        match meta
            .list::<VersionManifest>(VERSION_MANIFEST_URL, &manifest_path)
            .await
        {
            Ok(manifest) => {
                let Some(link) = manifest.versions.iter().find(|v| v.id == game) else {
                    bail!(
                        "Minecraft {game} is not in Mojang's version manifest (newest release: {})",
                        manifest.latest.release
                    );
                };
                meta.hashed(&link.url, &json_path, &link.sha1).await
            }
            Err(err) => match meta.cached_only(&json_path).await {
                Ok(v) => {
                    meta.warn(format!(
                        "Mojang's version manifest is unavailable ({err:#}); using the cached {game}.json unverified"
                    ));
                    Ok(v)
                }
                Err(_) => Err(err),
            },
        }
    }

    async fn asset_index(&self, meta: &MetaCache<'_>, v: &VersionJson) -> Result<AssetIndex> {
        let link = v
            .asset_index
            .as_ref()
            .with_context(|| format!("{} has no asset index", v.id))?;
        let path = join_safe(
            &self.paths.assets().join("indexes"),
            &format!("{}.json", link.id),
        )?;
        meta.hashed(&link.url, &path, &link.sha1).await
    }

    /// The runtime's version name and file manifest for this platform.
    async fn java_manifest(
        &self,
        meta: &MetaCache<'_>,
        component: &str,
    ) -> Result<(String, JavaRuntimeManifest)> {
        let all: JavaRuntimes = meta
            .list(
                JAVA_RUNTIMES_URL,
                &self
                    .paths
                    .meta()
                    .join("mojang")
                    .join("java-runtime-all.json"),
            )
            .await?;
        let mut key = java::platform_key(&self.platform)?;
        let mut build = all
            .get(key)
            .and_then(|c| c.get(component))
            .and_then(|b| b.first());
        // Apple Silicon without a native build: the Intel one runs under Rosetta.
        if build.is_none() && key == "mac-os-arm64" {
            key = "mac-os";
            build = all
                .get(key)
                .and_then(|c| c.get(component))
                .and_then(|b| b.first());
        }
        let build = build
            .with_context(|| format!("Mojang has no {component} runtime for {key}"))?
            .clone();
        let path = self
            .paths
            .meta()
            .join("java")
            .join(format!("{component}-{key}.json"));
        let manifest = meta
            .hashed(&build.manifest.url, &path, &build.manifest.sha1)
            .await?;
        Ok((build.version.name, manifest))
    }
}

fn index_id(v: &VersionJson) -> Result<String> {
    v.asset_index
        .as_ref()
        .map(|a| a.id.clone())
        .or_else(|| v.assets.clone())
        .with_context(|| format!("{} has no asset index", v.id))
}
