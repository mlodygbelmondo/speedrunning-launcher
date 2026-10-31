//! Prints what the Stats page would get, for checking against real files and the live APIs.
//!
//! ```text
//! SRL_HOME=... cargo run -p srl-stats --example show_stats -- [--igt-dir DIR]... [--ranked NAME] [--draftout NAME]
//! ```
//!
//! `--igt-dir` replaces the default SpeedRunIGT folders (`~/speedrunigt`, the OS data folder);
//! give it the `speedrunigt` folder of the `user.home` the game ran with.
use std::path::PathBuf;

use anyhow::Result;
use srl_core::{Downloader, Mode, Paths};
use srl_stats::{IgtSources, Refresh, Run, Stats};

#[tokio::main]
async fn main() -> Result<()> {
    let mut igt_dirs = Vec::new();
    let (mut ranked, mut draftout) = (None, None);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--igt-dir" => igt_dirs.push(PathBuf::from(args.next().unwrap_or_default())),
            "--ranked" => ranked = args.next(),
            "--draftout" => draftout = args.next(),
            other => anyhow::bail!("unknown argument {other}"),
        }
    }
    let paths = Paths::from_env()?;
    let sources = if igt_dirs.is_empty() {
        IgtSources::for_user()
    } else {
        IgtSources::with_global_dirs(igt_dirs)
    };
    let stats = Stats::with_sources(paths, &Downloader::new()?, sources);

    for pass in ["first", "second"] {
        let local = stats.local().await;
        println!(
            "scan ({pass}): {} files, {} parsed, {} skipped, {} unreadable folders, {} ms",
            local.files,
            local.parsed,
            local.skipped.count,
            local.unreadable.len(),
            local.scan_ms
        );
        if pass == "second" {
            break;
        }
        for (mode, h) in &local.modes {
            print_history(mode.id(), h);
        }
        if local.unattributed.run_count > 0 {
            print_history("unattributed", &local.unattributed);
        }
        if let Some(l) = &local.latest_world {
            println!(
                "latest world: {} ({:?})",
                l.world_path.display(),
                l.mode.map(Mode::id)
            );
        }
        for (path, why) in &local.skipped.examples {
            println!("  skipped {}: {why}", path.display());
        }
    }

    if let Some(name) = ranked {
        match stats.ranked(&name, Refresh::IfStale).await {
            Ok(o) => {
                let p = &o.profile.value;
                println!(
                    "ranked {}: elo {:?} rank {:?} tier {:?}, season {}W {}L of {}, best {:?} ms, age {:?}",
                    p.nickname,
                    p.elo,
                    p.rank,
                    p.tier,
                    p.season.wins,
                    p.season.losses,
                    p.season.played,
                    p.season.best_time_ms,
                    o.profile.age()
                );
                for m in o.matches.iter().flat_map(|m| m.value.iter()).take(5) {
                    println!(
                        "  match {} {:?} vs {:?}: {:?} in {} ms, elo {:?}",
                        m.id,
                        m.kind,
                        m.opponent.as_ref().map(|o| &o.nickname),
                        m.outcome,
                        m.time_ms,
                        m.elo_change
                    );
                }
                if let Some(r) = &o.weekly_race {
                    println!(
                        "  weekly race #{} ends {} ({} on the board), mine: {:?}, last finished: {:?}",
                        r.value.id,
                        r.value.ends_at_ms,
                        r.value.leaderboard.len(),
                        o.my_weekly_race,
                        o.my_last_weekly_race
                    );
                }
                println!("  players online: {:?}", o.players_online.map(|p| p.value));
            }
            Err(e) => println!("ranked {name}: {e}"),
        }
    }
    if let Some(name) = draftout {
        match stats.draftout(&name, Refresh::IfStale).await {
            Ok(d) => println!(
                "draftout {}: elo {:?} ({}), {}W {}L {}D, {} recent matches",
                d.value.profile.username,
                d.value.profile.elo,
                d.value.profile.rank_name,
                d.value.profile.wins,
                d.value.profile.losses,
                d.value.profile.draws,
                d.value.matches.len()
            ),
            Err(e) => println!("draftout {name}: {e}"),
        }
    }
    Ok(())
}

fn print_history(label: &str, h: &srl_stats::RunHistory) {
    println!(
        "{label}: {} runs, {} completed, PB {:?}",
        h.run_count,
        h.completion_count,
        h.headline_pb().map(|r| r.record.igt_ms)
    );
    for c in &h.categories {
        println!(
            "  {} {:?}: {} runs, {} completed ({} runs / {} completed excluded: cheats, LAN or \
             co-op), PB {:?}, sum of best {:?}, best segments {}",
            c.key.category,
            c.key.run_type,
            c.runs,
            c.completions,
            c.excluded,
            c.excluded_completions,
            c.pb.as_ref().map(|r| r.record.igt_ms),
            c.sum_of_best_ms,
            c.best_segments.len()
        );
    }
    for r in h.recent(10) {
        println!("  {}", history_line(r));
    }
}

fn history_line(r: &Run) -> String {
    let furthest = r.record.splits.iter().rev().find_map(|s| s.kind);
    format!(
        "{} [{}] {} IGT {} ms RTA {} ms, {}, furthest {:?}, date {}",
        r.id,
        r.mode.map(|m| m.id()).unwrap_or("-"),
        r.record.world_name,
        r.record.igt_ms,
        r.record.rta_ms,
        if r.record.completed {
            "completed"
        } else {
            "reset"
        },
        furthest,
        r.record.date_ms
    )
}
