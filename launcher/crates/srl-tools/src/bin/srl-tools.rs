//! `srl-tools`: the tools API from the command line, for tests. Uses `$SRL_HOME` like the app.
//!
//! ```text
//! srl-tools status                          each tool's state on this system
//! srl-tools install <tool>                  install or update ninjabrain-bot | toolscreen
//! srl-tools fetch-toolscreen [x64|arm64]    download Toolscreen and check its DLLs (any OS)
//! srl-tools run-ninjabrain <java> [--for SECS] [--ready-cmd CMD]
//!                                           start Ninjabrain Bot like a launch does, wait SECS
//!                                           (default: until it exits), run CMD, stop it
//! srl-tools wrap <mode> <program> [args...] print the command waywall would run
//! srl-tools launch <mode> [--tools a,b] [--glfw PATH] [--until-ready SECS] [--settle SECS]
//!                         [--ready-cmd CMD]
//!                                           the Play path: install the game (srl-game) and the
//!                                           tools, prepare, spawn, start the tools; then wait
//!                                           for the title screen, run CMD, stop the game and
//!                                           wait for the session to stop its tools
//! ```
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use srl_core::OfflineAccount;
use srl_core::{Downloader, Mode, Paths, Progress};
use srl_game::{GameInstaller, LaunchCommand, LaunchOptions, READY_MARKERS, rules::Arch};
use srl_tools::{
    LegalDlls, SideProcess, TOOLS, ToolId, ToolSelection, Tools, legal_dlls, ninjabrain,
    toolscreen, waywall,
};

const USAGE: &str = "usage: srl-tools <status | install <tool> | fetch-toolscreen [x64|arm64] | run-ninjabrain <java> [--for SECS] [--ready-cmd CMD] | wrap <mode> <program> [args...] | launch <mode> [--tools a,b] [--glfw PATH] [--until-ready SECS] [--settle SECS] [--ready-cmd CMD]>";

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn progress() -> srl_core::ProgressFn {
    std::sync::Arc::new(|p: Progress| {
        if p.done == p.total {
            eprintln!("  {:?}: {}/{} {:?}", p.stage, p.done, p.total, p.unit);
        }
    })
}

async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths = Paths::from_env()?;
    let downloader = Downloader::new()?;
    let tools = Tools::new(paths.clone(), downloader.clone());
    match args.first().map(String::as_str) {
        Some("status") => {
            println!("data: {}", paths.root().display());
            for info in TOOLS.iter() {
                println!(
                    "{:<15} {:?}\n                pinned {:?} ({}), {} by {}, {}",
                    info.name,
                    tools.status(info.id),
                    info.pinned,
                    info.pin_reason,
                    info.licence,
                    info.credit,
                    info.homepage
                );
            }
            Ok(())
        }
        Some("install") => {
            let id = args.get(1).context(USAGE)?;
            let tool = ToolId::from_id(id).with_context(|| format!("unknown tool {id}"))?;
            let started = Instant::now();
            tools.install(tool, Some(progress())).await?;
            println!(
                "installed in {:.1}s: {:?}",
                started.elapsed().as_secs_f64(),
                tools.status(tool)
            );
            if tool == ToolId::NinjabrainBot {
                let jar = ninjabrain::jar(&paths);
                let sha = srl_core::download::file_sha256(&jar)?;
                println!(
                    "{} sha256 {sha} (pinned {})",
                    jar.display(),
                    ninjabrain::SHA256
                );
            }
            Ok(())
        }
        Some("fetch-toolscreen") => {
            let arch = match args.get(1).map(String::as_str) {
                None | Some("x64") => Arch::X86_64,
                Some("arm64") => Arch::Aarch64,
                Some(other) => bail!("unknown arch {other}"),
            };
            let legal = LegalDlls::load(&paths, &downloader, legal_dlls::MAX_AGE).await?;
            println!("legal-dlls.csv: {} rows", legal.len());
            let inj = toolscreen::install_for(&paths, &downloader, arch, &legal, Some(progress()))
                .await?;
            println!("Toolscreen {} for {arch:?}, injection order:", inj.version);
            for dll in &inj.dlls {
                let sha = srl_core::download::file_sha512(dll)?;
                let row = legal.get(&sha).context("not in legal-dlls.csv")?;
                println!(
                    "  {}\n    sha512 {sha}\n    legal-dlls.csv: {} {}",
                    dll.display(),
                    row.file,
                    row.version
                );
            }
            Ok(())
        }
        Some("run-ninjabrain") => {
            let java = PathBuf::from(args.get(1).context(USAGE)?);
            let mut secs = None;
            let mut ready_cmd = None;
            let mut it = args.iter().skip(2);
            while let Some(flag) = it.next() {
                match flag.as_str() {
                    "--for" => secs = Some(it.next().context("--for SECS")?.parse::<u64>()?),
                    "--ready-cmd" => {
                        ready_cmd = Some(it.next().context("--ready-cmd CMD")?.clone())
                    }
                    other => bail!("unknown option {other}\n{USAGE}"),
                }
            }
            let others = ninjabrain::running(&[]);
            if !others.is_empty() {
                println!("already running: pids {others:?}; not starting another");
                return Ok(());
            }
            if !ninjabrain::verify(&paths).await? {
                bail!("Ninjabrain Bot is not installed or its jar doesn't match the pinned sha256");
            }
            let log = paths.logs().join("ninjabrain-bot.log");
            let cmd = ninjabrain::command(&java, &ninjabrain::jar(&paths));
            println!("starting: {} {}", cmd.program.display(), cmd.args.join(" "));
            let p = SideProcess::spawn(ToolId::NinjabrainBot, &cmd, &log).await?;
            println!("pid {}, log {}", p.pid(), log.display());
            match secs {
                None => p.wait().await,
                Some(s) => {
                    tokio::time::sleep(Duration::from_secs(s)).await;
                    if !p.is_running() {
                        bail!("Ninjabrain Bot exited early; see {}", log.display());
                    }
                    println!("running after {s}s: {:?}", ninjabrain::running(&[]));
                    if let Some(c) = ready_cmd {
                        let status = tokio::process::Command::new("sh")
                            .args(["-c", &c])
                            .status()
                            .await?;
                        println!("ready-cmd: {status}");
                    }
                    let t = Instant::now();
                    let how = p.stop(srl_tools::TOOL_STOP_GRACE).await;
                    println!(
                        "stopped ({how:?}) in {:.2}s; still running: {:?}",
                        t.elapsed().as_secs_f64(),
                        ninjabrain::running(&[])
                    );
                }
            }
            Ok(())
        }
        Some("launch") => {
            let mode_id = args.get(1).context(USAGE)?;
            let mode = Mode::from_id(mode_id).with_context(|| format!("unknown mode {mode_id}"))?;
            let mut selection = ToolSelection::defaults(mode, srl_tools::current_os());
            let mut glfw = None;
            let mut until_ready = 300u64;
            let mut settle = 5u64;
            let mut ready_cmd = None;
            let mut it = args.iter().skip(2);
            while let Some(flag) = it.next() {
                let mut value = || it.next().with_context(|| format!("{flag} needs a value"));
                match flag.as_str() {
                    "--tools" => {
                        selection = ToolSelection::default();
                        for id in value()?.split(',').filter(|s| !s.is_empty()) {
                            let t = ToolId::from_id(id)
                                .with_context(|| format!("unknown tool {id}"))?;
                            selection.set(t, true);
                        }
                    }
                    "--glfw" => glfw = Some(PathBuf::from(value()?)),
                    "--until-ready" => until_ready = value()?.parse()?,
                    "--settle" => settle = value()?.parse()?,
                    "--ready-cmd" => ready_cmd = Some(value()?.clone()),
                    other => bail!("unknown option {other}\n{USAGE}"),
                }
            }
            let tools = tools.with_glfw(glfw);
            let installer = GameInstaller::new(paths.clone(), downloader.clone());
            let installed = installer.install(mode, None).await?;
            let notes = tools
                .install_selected(mode, &selection, Some(progress()))
                .await;
            for n in &notes {
                println!("install note: {n}");
            }
            let opts = LaunchOptions::new(mode, OfflineAccount::new("Player"));
            let cmd = installer.command(&installed, &opts)?;
            let prepared = tools.prepare(mode, cmd, &selection).await;
            println!(
                "selection: {:?}; will run: {:?}",
                selection.0, prepared.tools
            );
            println!(
                "command: {} {} ...",
                prepared.command.program.display(),
                prepared
                    .command
                    .args
                    .iter()
                    .take(4)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            println!("env: {:?}", prepared.command.env);
            let started = Instant::now();
            let game = installer.spawn(mode, &prepared.command).await?;
            let session = tools
                .start(prepared, &game, &installed.java.executable)
                .await;
            println!(
                "game pid {}; tools started {:?}, injected {:?}, running {:?}",
                game.pid(),
                session.started,
                session.injected,
                tools.running()
            );
            for n in &session.notes {
                println!("note: {n}");
            }
            let deadline = started + Duration::from_secs(until_ready);
            loop {
                if let Ok(log) = tokio::fs::read_to_string(game.log_path()).await
                    && READY_MARKERS.iter().any(|m| log.contains(*m))
                {
                    break;
                }
                if let Some(exit) = game.exit() {
                    bail!("the game exited before the title screen: {exit:?}");
                }
                if Instant::now() > deadline {
                    game.kill().await;
                    bail!("not ready after {until_ready}s");
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            println!("ready after {:.1}s", started.elapsed().as_secs_f64());
            tokio::time::sleep(Duration::from_secs(settle)).await;
            if let Some(c) = ready_cmd {
                let status = tokio::process::Command::new("sh")
                    .args(["-c", &c])
                    .status()
                    .await?;
                println!("ready-cmd: {status}");
            }
            let report = game.stop_detailed(Duration::from_secs(30)).await;
            println!("game stopped: {report:?}");
            tokio::time::timeout(Duration::from_secs(20), session.finished())
                .await
                .context("the session didn't stop its tools within 20 s")?;
            println!(
                "session finished; launcher's tools still running: {:?}; ninjabrain processes: {:?}",
                tools.running(),
                ninjabrain::running(&[])
            );
            Ok(())
        }
        Some("wrap") => {
            let mode_id = args.get(1).context(USAGE)?;
            let mode = Mode::from_id(mode_id).with_context(|| format!("unknown mode {mode_id}"))?;
            let program = args.get(2).context(USAGE)?;
            let ww = waywall::find().context("waywall is not on PATH")?;
            let home = std::env::var_os("HOME").map(PathBuf::from);
            let glfw =
                waywall::find_glfw(None, home.as_deref(), LegalDlls::cached(&paths).as_ref());
            let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
            println!(
                "check: {:?}",
                waywall::check(Some(&ww), wayland, mode, glfw.as_ref())
            );
            let cmd = LaunchCommand {
                program: program.into(),
                args: args[3..].to_vec(),
                cwd: paths.instance(mode),
                env: vec![],
            };
            let w = waywall::wrap(&cmd, &ww, mode, glfw.as_ref());
            println!(
                "env: {:?}\n{} {}",
                w.env,
                w.program.display(),
                w.args.join(" ")
            );
            Ok(())
        }
        _ => {
            println!("{USAGE}");
            Ok(())
        }
    }
}
