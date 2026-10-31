//! `practice_game`: a stand-in for the integration after M2. Installs 1.16.1 with Fabric through
//! srl-game (the RSG mode's game files, which the practice instance shares), puts the
//! [`srl_practice::PRACTICE`] mods from mcsr-meta into `instances/practice/mods`, and starts the
//! game with `instances/practice` as its game directory. Then it waits until the title screen
//! loads, prints `READY`, and keeps the game running until `--stop-file` appears (or
//! `--timeout` passes), so a script can drive the game meanwhile (xdotool in Xvfb).
//!
//! ```text
//! SRL_HOME=... cargo run -p srl-practice --example practice_game -- \
//!     [--stop-file PATH] [--timeout SECS] [--width W --height H]
//! ```
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use srl_core::{Download, Downloader, Hash, Mode, OfflineAccount, Paths};
use srl_game::{GameInstaller, LaunchOptions, READY_MARKERS};
use srl_mods::{ModMeta, Os, ResolveOptions};
use srl_practice::PRACTICE;

#[tokio::main]
async fn main() -> Result<()> {
    let mut stop_file: Option<PathBuf> = None;
    let mut timeout = 600u64;
    let (mut width, mut height) = (1280u32, 720u32);
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut v = || it.next().context("missing value");
        match a.as_str() {
            "--stop-file" => stop_file = Some(v()?.into()),
            "--timeout" => timeout = v()?.parse()?,
            "--width" => width = v()?.parse()?,
            "--height" => height = v()?.parse()?,
            _ => bail!("unknown option {a}"),
        }
    }

    let paths = Paths::from_env()?;
    let downloader = Downloader::new()?;
    println!("data: {}", paths.root().display());

    // The game: 1.16.1 + Fabric + Java 21, the same files as the 1.16.1 modes.
    let installer = GameInstaller::new(paths.clone(), downloader.clone());
    let mut installed = installer.install(Mode::Rsg, None).await?;
    let instance = paths.practice_instance();
    installed.game_dir = instance.clone();
    println!(
        "game {} (Fabric {}), Java {}",
        installed.version_id, installed.fabric_loader, installed.java.version
    );

    // The mods: PRACTICE's ids, as resolved from mcsr-meta for 1.16.1 (the RSG resolution has
    // every one of them; the integration will resolve the profile directly).
    let meta = ModMeta::load(&paths, &downloader, true).await?;
    let set = srl_mods::resolve(&meta, Mode::Rsg, Os::current(), &ResolveOptions::default())?;
    let mods_dir = instance.join("mods");
    std::fs::create_dir_all(&mods_dir)?;
    let mut wanted = Vec::new();
    for id in PRACTICE.mod_ids(false, Os::current() == Os::MacOs) {
        let Some(m) = set.get(id) else {
            bail!("{id} is not in the resolved 1.16.1 set");
        };
        let store = paths.store_sha512(&m.sha512);
        downloader
            .download(&Download {
                url: m.url.clone(),
                dest: store.clone(),
                hash: Hash::Sha512(m.sha512.clone()),
                size: Some(m.size),
            })
            .await?;
        std::fs::copy(&store, mods_dir.join(&m.file_name))?;
        wanted.push(m.file_name.clone());
        println!("mod {id} {}", m.version);
    }
    for e in std::fs::read_dir(&mods_dir)?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !wanted.contains(&name) {
            std::fs::remove_file(e.path())?;
        }
    }

    let mut opts = LaunchOptions::new(Mode::Rsg, OfflineAccount::new("Practice_Bot"));
    opts.window_size = Some((width, height));
    let started = Instant::now();
    let game = installer.launch(&installed, &opts).await?;
    println!(
        "started pid {}, log {}",
        game.pid(),
        game.log_path().display()
    );
    let deadline = started + Duration::from_secs(timeout);
    loop {
        if let Ok(log) = tokio::fs::read_to_string(game.log_path()).await
            && READY_MARKERS.iter().any(|m| log.contains(m))
        {
            break;
        }
        if let Some(exit) = game.exit() {
            bail!("the game exited before the title screen: {exit:?}");
        }
        if Instant::now() > deadline {
            game.kill().await;
            bail!("not ready in {timeout}s");
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    println!("READY after {:.1}s", started.elapsed().as_secs_f64());
    loop {
        if stop_file.as_ref().is_some_and(|f| f.exists()) || Instant::now() > deadline {
            break;
        }
        if let Some(exit) = game.exit() {
            println!("the game exited: {exit:?}");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let exit = game.stop(Duration::from_secs(30)).await;
    println!("stopped: {exit:?}");
    Ok(())
}
