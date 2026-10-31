//! The headless command line, for CI and support. It opens no window and runs the same pipeline
//! as the Play button (`backend/pipeline.rs`), with the same data directory and settings:
//!
//! ```text
//! speedrunning-launcher --install <mode>    install or repair: Java, Minecraft, Fabric, the mods
//!                                           and the tools the mode starts with
//! speedrunning-launcher --launch <mode>     install, start the game with its tools, wait for it
//! speedrunning-launcher --status            what is installed (no network)
//! speedrunning-launcher --tools             each tool's state (no network)
//! speedrunning-launcher --install-tool <t>  install or update a tool, with its hashes checked
//! ```
//!
//! It exits 0 on success, 1 on failure and 2 on a usage error. Without arguments the binary opens
//! the window instead.
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use srl_core::{Mode, OfflineAccount, Paths, Progress, ProgressFn, Stage};
use srl_tools::{ToolId, ToolSelection};

use crate::backend::{
    FailDetail, Settings, account,
    pipeline::{self, Core, Local, Prepared, Startup},
};

const USAGE: &str = "\
usage: speedrunning-launcher [command]

Without a command it opens the launcher. Commands (no window):
  --install <mode>     install or repair a mode: Java, Minecraft, Fabric Loader, its mods and the
                       tools it starts with
  --launch <mode>      install, start the game with its tools and wait until it exits
  --status             what is installed, from local files only
  --tools              each tool's state, from local files only
  --install-tool <t>   install or update a tool (ninjabrain-bot, toolscreen), hashes checked
  --help               this text

<mode> is ranked, rsg, fsg, ssg or draftout. Options for --launch:
  --name NAME          offline player name (default: the saved one, else Player)
  --memory MB          -Xmx for this run (default: the saved setting)
  --tools LIST         the tools to start, comma-separated, or none (default: the saved choice)
  --until-ready SECS   stop the game once its title screen loads; fail if that takes over SECS
  --settle SECS        with --until-ready: wait this long after it loads first (default 10)
  --ready-cmd CMD      with --until-ready: run CMD through the shell before stopping

Data lives in $SRL_HOME if set, else the OS data directory.";

enum Command {
    Help,
    Status,
    Tools,
    Install(Mode),
    InstallTool(ToolId),
    Launch(Mode, LaunchArgs),
}

#[derive(Default)]
struct LaunchArgs {
    name: Option<String>,
    memory: Option<u32>,
    tools: Option<ToolSelection>,
    until_ready: Option<u64>,
    settle: Option<u64>,
    ready_cmd: Option<String>,
}

/// `--tools ninjabrain-bot,waywall` or `--tools none`.
fn parse_tools(list: &str) -> Result<ToolSelection> {
    let mut sel = ToolSelection::default();
    if list == "none" {
        return Ok(sel);
    }
    for id in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        sel.set(
            ToolId::from_id(id).with_context(|| format!("unknown tool {id:?}"))?,
            true,
        );
    }
    Ok(sel)
}

/// Runs the command line if there is one. `None`: no arguments, open the window.
pub fn run(args: Vec<String>) -> Option<i32> {
    if args.is_empty() {
        return None;
    }
    attach_console();
    let code = match parse(args) {
        Ok(Command::Help) => {
            println!("{USAGE}");
            0
        }
        Ok(cmd) => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .expect("a tokio runtime");
            match rt.block_on(execute(cmd)) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("error: {e:#}");
                    1
                }
            }
        }
        Err(e) => {
            eprintln!("{e:#}\n\n{USAGE}");
            2
        }
    };
    Some(code)
}

fn parse(args: Vec<String>) -> Result<Command> {
    let mut it = args.into_iter();
    let command = it.next().unwrap_or_default();
    let mut mode = || -> Result<Mode> {
        let id = it
            .next()
            .with_context(|| format!("{command} needs a mode"))?;
        Mode::from_id(&id).with_context(|| format!("unknown mode {id:?}"))
    };
    let cmd = match command.as_str() {
        "--help" | "-h" | "help" => return Ok(Command::Help),
        "--status" => Command::Status,
        "--tools" => Command::Tools,
        "--install" => Command::Install(mode()?),
        "--install-tool" => {
            let id = it.next().context("--install-tool needs a tool")?;
            Command::InstallTool(
                ToolId::from_id(&id).with_context(|| format!("unknown tool {id:?}"))?,
            )
        }
        "--launch" => Command::Launch(mode()?, LaunchArgs::default()),
        other => bail!("unknown command {other:?}"),
    };
    let Command::Launch(m, mut la) = cmd else {
        if let Some(extra) = it.next() {
            bail!("unexpected argument {extra:?}");
        }
        return Ok(cmd);
    };
    while let Some(flag) = it.next() {
        let mut value = || it.next().with_context(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--name" => la.name = Some(value()?),
            "--memory" => la.memory = Some(value()?.parse().context("--memory")?),
            "--tools" => la.tools = Some(parse_tools(&value()?)?),
            "--until-ready" => la.until_ready = Some(value()?.parse().context("--until-ready")?),
            "--settle" => la.settle = Some(value()?.parse().context("--settle")?),
            "--ready-cmd" => la.ready_cmd = Some(value()?),
            _ => bail!("unknown option {flag:?}"),
        }
    }
    if let Some(name) = &la.name
        && !OfflineAccount::is_valid_name(name)
    {
        bail!("{name:?} is not a valid Minecraft name (3 to 16 of A-Z, a-z, 0-9, _)");
    }
    Ok(Command::Launch(m, la))
}

async fn execute(cmd: Command) -> Result<()> {
    let paths = Paths::from_env()?;
    println!("data: {}", paths.root().display());
    // As the window does: earlier versions had a directory per 1.16.1 mode.
    match srl_core::shared::migrate(&paths) {
        Ok(m) if m.is_empty() => {}
        Ok(m) => println!("one directory for the 1.16.1 modes: {m:?}"),
        Err(e) => eprintln!("moving the 1.16.1 modes into one directory: {e:#}"),
    }
    let core = Core::new(paths)?;
    match cmd {
        Command::Help => Ok(()),
        Command::Status => {
            status(&core);
            Ok(())
        }
        Command::Tools => {
            tools_status(&core);
            Ok(())
        }
        Command::Install(mode) => {
            let settings = saved_settings(&core);
            prepare(&core, mode, &settings, &settings.tool_selection(mode)).await?;
            Ok(())
        }
        Command::InstallTool(tool) => install_tool(&core, tool).await,
        Command::Launch(mode, args) => launch(&core, mode, args).await,
    }
}

fn tools_status(core: &Core) {
    for id in ToolId::ALL {
        let info = id.info();
        println!(
            "{:15} {:?} (installs {:?}; {}, {})",
            info.name,
            core.tools().status(id),
            core.tools().offer(id).map(|o| o.version),
            info.licence,
            info.homepage
        );
    }
}

/// Installs one tool and prints each file it put on disk with its hash, so a check outside the
/// launcher (CI) can compare them with the published ones.
async fn install_tool(core: &Core, tool: ToolId) -> Result<()> {
    let started = Instant::now();
    core.install_tool(tool, Some(progress_printer()))
        .await
        .with_context(|| format!("installing {}", tool.info().name))?;
    let status = core.tools().status(tool);
    println!(
        "installed {} in {:.2}s: {status:?}",
        tool.id(),
        started.elapsed().as_secs_f64()
    );
    let srl_tools::ToolStatus::Installed(t) = status else {
        bail!("{} is not installed after its install", tool.info().name);
    };
    let files: Vec<std::path::PathBuf> = if t.path.is_dir() {
        let mut v: Vec<_> = std::fs::read_dir(&t.path)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file())
            .collect();
        v.sort();
        v
    } else {
        vec![t.path.clone()]
    };
    for f in files {
        let (kind, hash) = if f.extension().is_some_and(|e| e == "jar") {
            ("sha256", srl_core::download::file_sha256(&f)?)
        } else {
            ("sha512", srl_core::download::file_sha512(&f)?)
        };
        println!("file {} {kind} {hash}", f.display());
    }
    Ok(())
}

fn status(core: &Core) {
    for mode in Mode::ALL {
        let line = match core.local_state(mode) {
            Local::NotInstalled => "not installed".to_string(),
            Local::Ready(r) => format!(
                "ready: {}, Java {}, {} mods, {} MB on disk",
                r.version_id,
                r.java_version,
                r.mods.len(),
                r.disk / 1_000_000
            ),
            Local::Damaged(r) => format!(
                "needs repair (a mod jar is missing or changed): {}",
                r.version_id
            ),
        };
        println!("{:9} {line}", mode.id());
    }
}

/// The window's saved settings: the CLI installs the same optional mods and starts the game with
/// the same name and memory.
fn saved_settings(core: &Core) -> Settings {
    Settings::load(&Settings::file(core.paths())).unwrap_or_default()
}

async fn prepare(
    core: &Core,
    mode: Mode,
    settings: &Settings,
    tools: &ToolSelection,
) -> Result<Prepared> {
    let started = Instant::now();
    let prepared = core
        .prepare(mode, &settings.mod_rules(), tools, Some(progress_printer()))
        .await
        .with_context(|| format!("installing {}", mode.id()))?;
    let i = &prepared.installed;
    let r = &prepared.report;
    println!(
        "installed {} in {:.2}s: Minecraft {}, Fabric Loader {}, Java {} ({})",
        mode.id(),
        started.elapsed().as_secs_f64(),
        i.minecraft_version,
        i.fabric_loader,
        i.java.version,
        i.java.executable.display()
    );
    println!(
        "mods: {} in {} ({} added, {} updated, {} removed, {} unchanged)",
        prepared.mods.mods.len(),
        srl_mods::mods_dir(core.paths(), mode).display(),
        r.added.len(),
        r.updated.len(),
        r.removed.len(),
        r.unchanged
    );
    for p in &r.set_aside {
        println!("moved out of mods/ (not part of the set): {}", p.display());
    }
    for m in &prepared.mods.mods {
        println!("  {} {} ({})", m.name, m.version, m.file_name);
    }
    for note in &prepared.notes {
        println!("note: {note}");
    }
    println!(
        "tools for {}: {}",
        mode.id(),
        tool_names(prepared.tools.0.iter().copied())
    );
    for note in &prepared.tool_notes {
        println!("tools: {note}");
    }
    Ok(prepared)
}

fn tool_names(tools: impl Iterator<Item = ToolId>) -> String {
    let names: Vec<&str> = tools.map(|t| t.info().name).collect();
    if names.is_empty() {
        "none".into()
    } else {
        names.join(", ")
    }
}

async fn launch(core: &Core, mode: Mode, args: LaunchArgs) -> Result<()> {
    let started = Instant::now();
    let mut settings = saved_settings(core);
    let tools = args
        .tools
        .clone()
        .unwrap_or_else(|| settings.tool_selection(mode));
    let prepared = prepare(core, mode, &settings, &tools).await?;
    if args.name.is_some() {
        settings.player_name = args.name.clone();
    }
    if let Some(mb) = args.memory {
        settings.set_memory_mb(mode, mb);
    }
    // The same account as the Play button: the active Microsoft account (a build with Microsoft
    // login), else the offline name.
    let auth = srl_auth::Auth::new(core.paths(), account::auth_config());
    let signed_in = account::launch_account(&auth, pipeline::offline_account(&settings), || {
        println!("signing in to Minecraft")
    })
    .await
    .map_err(|p| anyhow::anyhow!("the Microsoft sign-in failed: {p}"))?;
    if let (Some(w), Some(until)) = (&signed_in.warning, signed_in.valid_until) {
        println!("couldn't renew the sign-in ({w}); it is valid until {until}");
    }
    let mut game = core.launch(&prepared, &settings, signed_in.account).await?;
    game.account_in_use = signed_in.in_use;
    println!("game arguments: {}", game.game_args.join(" "));
    println!(
        "started pid {} {:.2}s after the command, as {} ({}) with {} MB; log {}",
        game.process.pid(),
        started.elapsed().as_secs_f64(),
        game.launched.player,
        if game.launched.microsoft {
            "Microsoft"
        } else {
            "offline"
        },
        game.launched.memory_mb,
        game.process.log_path().display()
    );
    println!(
        "running with: {}",
        tool_names(game.launched.tools.iter().copied())
    );
    for note in &game.launched.tool_notes {
        println!("tools: {note}");
    }
    let limit = args
        .until_ready
        .map(Duration::from_secs)
        .unwrap_or(pipeline::STARTUP_LIMIT);
    let startup = game.startup(limit).await;
    let log = game.process.log_path().to_path_buf();
    match startup {
        Startup::Ready(after) => println!(
            "title screen loading {:.1}s after the process started",
            after.as_secs_f64()
        ),
        Startup::Crashed(reason) => {
            game.process.kill().await;
            game.wait().await;
            bail!(pipeline::describe(&FailDetail::Stopped(reason), &log));
        }
        Startup::Exited(exit) if !exit.success => {
            game.wait().await;
            bail!(pipeline::describe(
                &pipeline::startup_failure(exit, &log),
                &log
            ));
        }
        Startup::Exited(exit) => {
            game.wait().await;
            println!("the game exited while starting: {exit:?}");
            return Ok(());
        }
        Startup::TimedOut if args.until_ready.is_some() => {
            let exit = game.process.kill().await;
            game.wait().await;
            bail!(
                "no title screen within {}s; killed the game ({exit:?}). Log: {}",
                limit.as_secs(),
                log.display()
            );
        }
        Startup::TimedOut => println!("no title screen marker in the log yet; waiting"),
    }
    if args.until_ready.is_none() {
        let exit = game.wait().await;
        println!("exited: {exit:?}");
        return Ok(());
    }
    tokio::time::sleep(Duration::from_secs(args.settle.unwrap_or(10))).await;
    if let Some(cmd) = &args.ready_cmd {
        let status = shell(cmd).status().await?;
        println!("ready-cmd: {status}");
    }
    if let Some(exit) = game.process.exit() {
        game.wait().await;
        bail!(
            "the game exited after its title screen loaded: {exit:?}. Log: {}",
            log.display()
        );
    }
    let stopping = Instant::now();
    let report = game.process.stop_detailed(Duration::from_secs(30)).await;
    println!(
        "stopped in {:.1}s: exit {:?} (asked by {:?}, escalated: {:?})",
        stopping.elapsed().as_secs_f64(),
        report.exit.code,
        report.request,
        report.escalated
    );
    game.wait().await;
    println!(
        "its tools closed {:.1}s after the stop",
        stopping.elapsed().as_secs_f64()
    );
    Ok(())
}

/// A line per stage every 25 %, so CI logs stay short. Stages run in parallel, so each keeps
/// its own last quarter.
fn progress_printer() -> ProgressFn {
    let last: Arc<Mutex<Vec<(Stage, u32)>>> = Arc::default();
    Arc::new(move |p: Progress| {
        let quarter = (p.fraction() * 4.0) as u32;
        let mut last = last.lock().unwrap();
        match last.iter_mut().find(|(s, _)| *s == p.stage) {
            Some((_, q)) if *q == quarter => return,
            Some((_, q)) => *q = quarter,
            None => last.push((p.stage, quarter)),
        }
        println!("  {:?}: {}/{} {:?}", p.stage, p.done, p.total, p.unit);
    })
}

fn shell(cmd: &str) -> tokio::process::Command {
    let (program, flag) = if cfg!(windows) {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };
    let mut c = tokio::process::Command::new(program);
    c.args([flag, cmd]);
    c
}

/// The release build on Windows is a GUI program (no console window when double-clicked), so
/// its output would go nowhere when run from a terminal. Attach to the terminal's console unless
/// the output is already redirected (CI pipes it).
#[cfg(windows)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE,
    };
    // SAFETY: plain Win32 calls without pointers.
    unsafe {
        if GetStdHandle(STD_OUTPUT_HANDLE).is_null() {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

#[cfg(not(windows))]
fn attach_console() {}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn parses_the_commands() {
        assert!(matches!(
            parse(args("--install rsg")),
            Ok(Command::Install(Mode::Rsg))
        ));
        let Ok(Command::Launch(Mode::Draftout, la)) = parse(args(
            "--launch draftout --name Violet_Runner --until-ready 120",
        )) else {
            panic!("launch");
        };
        assert_eq!(la.name.as_deref(), Some("Violet_Runner"));
        assert_eq!(la.until_ready, Some(120));
        assert!(parse(args("--install")).is_err());
        assert!(parse(args("--install aa")).is_err());
        assert!(parse(args("--install rsg --name x")).is_err());
        assert!(parse(args("--launch rsg --name no spaces")).is_err());
        assert!(parse(args("--frobnicate")).is_err());
        let Ok(Command::Launch(_, la)) = parse(args("--launch rsg --tools none")) else {
            panic!("tools none");
        };
        assert!(la.tools.unwrap().0.is_empty());
        let Ok(Command::Launch(_, la)) = parse(args("--launch rsg --tools ninjabrain-bot,waywall"))
        else {
            panic!("tools list");
        };
        assert!(la.tools.unwrap().has(ToolId::Waywall));
        assert!(parse(args("--launch rsg --tools nope")).is_err());
        assert!(matches!(
            parse(args("--install-tool toolscreen")),
            Ok(Command::InstallTool(ToolId::Toolscreen))
        ));
        assert!(parse(args("--install-tool")).is_err());
    }
}
