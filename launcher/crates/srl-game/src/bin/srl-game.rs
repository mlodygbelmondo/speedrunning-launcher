//! `srl-game`: installs and starts a mode from the command line, for tests and CI. Uses
//! `$SRL_HOME` (else the OS data directory), like the app.
//!
//! ```text
//! srl-game install <mode> [--refresh]
//! srl-game command <mode> [launch options]      print the launch command
//! srl-game launch  <mode> [launch options] [--until-ready SECS [--settle SECS] [--ready-cmd CMD]]
//!
//! launch options: --name NAME  --memory MB  --width W --height H  --jvm-arg ARG (repeatable)
//!                 --java PATH  --refresh
//! ```
//!
//! `launch` installs first (fast when everything is verified), starts the game and waits for it
//! to exit. With `--until-ready`, it instead waits until the log shows the title screen is
//! loading (`Sound engine started` or the block atlas), waits `--settle` seconds (default 10),
//! runs `--ready-cmd` through the shell (e.g. a screenshot), then stops the game politely and
//! reports the exit. It fails if the game isn't ready within SECS.
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use srl_core::{Downloader, Mode, OfflineAccount, Paths, Progress, Stage};
use srl_game::{GameInstaller, LaunchOptions, READY_MARKERS};

struct Args {
    command: String,
    mode: Mode,
    refresh: bool,
    name: String,
    memory: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
    jvm_args: Vec<String>,
    java: Option<String>,
    until_ready: Option<u64>,
    settle: u64,
    ready_cmd: Option<String>,
}

const USAGE: &str =
    "usage: srl-game <install|command|launch> <ranked|rsg|fsg|ssg|draftout> [options]
  --refresh           refetch metadata lists now
  --name NAME         offline player name (default Player)
  --memory MB         -Xmx (default: the mode's)
  --width W --height H  initial window size
  --jvm-arg ARG       extra JVM argument (repeatable)
  --java PATH         use this java instead of the installed runtime
  --until-ready SECS  launch: stop once the title screen loads, fail after SECS
  --settle SECS       launch --until-ready: wait this long after the marker (default 10)
  --ready-cmd CMD     launch --until-ready: run CMD (via the shell) before stopping";

fn parse() -> Result<Args> {
    let mut it = std::env::args().skip(1);
    let command = it.next().unwrap_or_default();
    if command.is_empty() || command == "help" || command == "--help" {
        println!("{USAGE}");
        std::process::exit(0);
    }
    let mode_id = it.next().context(USAGE)?;
    let mode = Mode::from_id(&mode_id).with_context(|| format!("unknown mode {mode_id}"))?;
    let mut a = Args {
        command,
        mode,
        refresh: false,
        name: "Player".into(),
        memory: None,
        width: None,
        height: None,
        jvm_args: Vec::new(),
        java: None,
        until_ready: None,
        settle: 10,
        ready_cmd: None,
    };
    while let Some(flag) = it.next() {
        let mut value = || it.next().with_context(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--refresh" => a.refresh = true,
            "--name" => a.name = value()?,
            "--memory" => a.memory = Some(value()?.parse()?),
            "--width" => a.width = Some(value()?.parse()?),
            "--height" => a.height = Some(value()?.parse()?),
            "--jvm-arg" => a.jvm_args.push(value()?),
            "--java" => a.java = Some(value()?),
            "--until-ready" => a.until_ready = Some(value()?.parse()?),
            "--settle" => a.settle = value()?.parse()?,
            "--ready-cmd" => a.ready_cmd = Some(value()?),
            _ => bail!("unknown option {flag}\n{USAGE}"),
        }
    }
    if !OfflineAccount::is_valid_name(&a.name) {
        bail!("{:?} is not a valid Minecraft name", a.name);
    }
    Ok(a)
}

/// Prints a line per stage every 25 %, so CI logs stay short. Stages run in parallel, so each
/// keeps its own last quarter.
fn progress_printer() -> srl_core::ProgressFn {
    let last: Arc<Mutex<Vec<(Stage, u32)>>> = Arc::default();
    Arc::new(move |p: Progress| {
        let quarter = (p.fraction() * 4.0) as u32;
        let mut last = last.lock().unwrap();
        let entry = match last.iter().position(|(s, _)| *s == p.stage) {
            Some(i) if last[i].1 == quarter => return,
            Some(i) => &mut last[i],
            None => {
                last.push((p.stage, quarter));
                last.last_mut().unwrap()
            }
        };
        entry.1 = quarter;
        println!("  {:?}: {}/{}", p.stage, p.done, p.total);
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse()?;
    let paths = Paths::from_env()?;
    println!("data: {}", paths.root().display());
    let mut installer = GameInstaller::new(paths.clone(), Downloader::new()?);
    if args.refresh {
        installer = installer.with_meta_max_age(Duration::ZERO);
    }

    let started = Instant::now();
    let installed = installer
        .install(args.mode, Some(progress_printer()))
        .await
        .with_context(|| format!("installing {}", args.mode.id()))?;
    println!(
        "installed {} ({}, Java {} at {}) in {:.2}s",
        args.mode.id(),
        installed.version_id,
        installed.java.version,
        installed.java.executable.display(),
        started.elapsed().as_secs_f64()
    );
    for w in &installed.warnings {
        println!("warning: {w}");
    }
    if args.command == "install" {
        return Ok(());
    }

    let mut opts = LaunchOptions::new(args.mode, OfflineAccount::new(&args.name));
    if let Some(m) = args.memory {
        opts.max_memory_mb = m;
    }
    if let (Some(w), Some(h)) = (args.width, args.height) {
        opts.window_size = Some((w, h));
    }
    opts.extra_jvm_args = args.jvm_args.clone();
    opts.java_override = args.java.as_ref().map(Into::into);

    match args.command.as_str() {
        "command" => {
            let cmd = installer.command(&installed, &opts)?;
            println!("cwd: {}", cmd.cwd.display());
            println!("{}", cmd.program.display());
            for a in &cmd.args {
                println!("  {a}");
            }
            Ok(())
        }
        "launch" => launch(&installer, &installed, &opts, &args).await,
        other => bail!("unknown command {other}\n{USAGE}"),
    }
}

async fn launch(
    installer: &GameInstaller,
    installed: &srl_game::Installed,
    opts: &LaunchOptions,
    args: &Args,
) -> Result<()> {
    let started = Instant::now();
    let game = installer.launch(installed, opts).await?;
    println!(
        "started pid {}, log {}",
        game.pid(),
        game.log_path().display()
    );
    let Some(timeout) = args.until_ready else {
        let exit = game.wait().await;
        println!("exited: {exit:?}");
        return Ok(());
    };

    let deadline = started + Duration::from_secs(timeout);
    let marker = loop {
        if let Ok(log) = tokio::fs::read_to_string(game.log_path()).await
            && let Some(m) = READY_MARKERS.iter().find(|m| log.contains(**m))
        {
            break *m;
        }
        if let Some(exit) = game.exit() {
            bail!("the game exited before the title screen: {exit:?}");
        }
        if Instant::now() > deadline {
            let exit = game.kill().await;
            bail!("not ready after {timeout}s; killed: {exit:?}");
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    println!(
        "ready after {:.1}s ({marker:?})",
        started.elapsed().as_secs_f64()
    );
    tokio::time::sleep(Duration::from_secs(args.settle)).await;
    if let Some(cmd) = &args.ready_cmd {
        let status = shell(cmd).status().await?;
        println!("ready-cmd: {status}");
    }
    let stopping = Instant::now();
    let report = game.stop_detailed(Duration::from_secs(30)).await;
    println!(
        "stopped in {:.1}s: {:?} (asked by {:?}, escalated: {:?})",
        stopping.elapsed().as_secs_f64(),
        report.exit,
        report.request,
        report.escalated
    );
    Ok(())
}

fn shell(cmd: &str) -> tokio::process::Command {
    if cfg!(windows) {
        let mut c = tokio::process::Command::new("cmd");
        c.args(["/C", cmd]);
        c
    } else {
        let mut c = tokio::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    }
}
