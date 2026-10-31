//! Times the record scan on a synthetic history: N global records and N worlds with their
//! `record.json` (the real-session record format, with varied dates, times and splits), under
//! `DIR`. Prints a cold scan (empty index), a warm scan (nothing changed) and a scan after 10 new
//! runs.
//!
//! ```text
//! cargo run --release -p srl-stats --example bench_scan -- DIR [N]
//! ```
use std::{fs, path::PathBuf, time::Instant};

use anyhow::Result;
use srl_core::{Mode, Paths};
use srl_stats::IgtSources;

fn record(i: usize) -> String {
    let date = 1_793_000_000_000i64 + i as i64 * 60_000;
    let completed = i.is_multiple_of(50);
    let names = [
        "enter_nether",
        "enter_bastion",
        "enter_fortress",
        "nether_travel",
        "enter_stronghold",
        "enter_end",
    ];
    // Most resets die early; a few go far.
    let reached = if completed { 6 } else { (i * 7) % 4 };
    let timelines: Vec<String> = names[..reached]
        .iter()
        .enumerate()
        .map(|(k, n)| {
            let t = 90_000 + k as u64 * 60_000 + (i as u64 * 37) % 20_000;
            format!(r#"{{"name":"{n}","igt":{t},"rta":{}}}"#, t + 5_000)
        })
        .collect();
    let igt = if completed {
        540_000 + (i as u64 * 131) % 120_000
    } else {
        5_000 + (i as u64 * 977) % 200_000
    };
    format!(
        r#"{{"mc_version":"1.16.1","speedrunigt_version":"16.0+1.16.1","category":"ANY","run_type":"random_seed","is_completed":{completed},"is_coop":false,"is_hardcore":false,"world_name":"Random Speedrun #{i}","is_cheat_allowed":false,"default_gamemode":0,"date":{date},"retimed_igt":{igt},"final_igt":{igt},"stats_igt":0,"final_rta":{},"open_lan":null,"timelines":[{}],"advancements":{{}},"stats":{{}}}}"#,
        igt + 9_000,
        timelines.join(",")
    )
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().expect("usage: bench_scan DIR [N]"));
    let n: usize = args.next().map(|s| s.parse()).transpose()?.unwrap_or(5000);
    let _ = fs::remove_dir_all(&dir);
    let paths = Paths::new(dir.join("srl"));
    let global = dir.join("home/speedrunigt");
    fs::create_dir_all(global.join("records"))?;
    let write_run = |i: usize| -> Result<()> {
        let rec = record(i);
        fs::write(global.join(format!("records/{i:08x}-0000.json")), &rec)?;
        let w = paths
            .instance(Mode::Rsg)
            .join(format!("saves/Random Speedrun #{i}/speedrunigt"));
        fs::create_dir_all(&w)?;
        fs::write(w.join("record.json"), &rec)?;
        Ok(())
    };
    for i in 0..n {
        write_run(i)?;
    }
    let sources = IgtSources::with_global_dirs([global.clone()]);
    for label in ["cold", "warm"] {
        let t = Instant::now();
        let s = srl_stats::igt::scan(&paths, &sources);
        let rsg = s.mode(Mode::Rsg);
        println!(
            "{label}: {:.0} ms ({} files, {} parsed), {} runs, {} completed, PB {:?}, sum of best {:?}",
            t.elapsed().as_secs_f64() * 1000.0,
            s.files,
            s.parsed,
            rsg.run_count,
            rsg.completion_count,
            rsg.headline_pb().map(|r| r.record.igt_ms),
            rsg.categories.first().and_then(|c| c.sum_of_best_ms)
        );
    }
    for i in n..n + 10 {
        write_run(i)?;
    }
    let t = Instant::now();
    let s = srl_stats::igt::scan(&paths, &sources);
    println!(
        "10 new runs: {:.0} ms ({} files, {} parsed), {} runs",
        t.elapsed().as_secs_f64() * 1000.0,
        s.files,
        s.parsed,
        s.mode(Mode::Rsg).run_count
    );
    let index = paths.meta().join("stats/igt-index.json");
    println!("index: {} KB", fs::metadata(index)?.len() / 1024);
    fs::remove_dir_all(&dir)?;
    Ok(())
}
