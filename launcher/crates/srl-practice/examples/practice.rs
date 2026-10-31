//! `practice`: drives the srl-practice API from the command line, for live checks. Uses
//! `$SRL_HOME` like the app.
//!
//! ```text
//! cargo run -p srl-practice --example practice -- list
//! cargo run -p srl-practice --example practice -- install <id>... | all
//! cargo run -p srl-practice --example practice -- status
//! cargo run -p srl-practice --example practice -- check            hashes and level.dat of installed maps
//! cargo run -p srl-practice --example practice -- plan-update <id>
//! cargo run -p srl-practice --example practice -- reset <id> [--keep]
//! cargo run -p srl-practice --example practice -- uninstall <id>
//! ```
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::{Context, Result, bail};
use srl_core::{Downloader, Paths};
use srl_practice::{
    MapStatus, Practice, Progress, ProgressFn, Step, catalogue::DATA_VERSION_1_16_1, level,
};

/// Prints a line per step every 25 %.
fn printer(id: &str) -> ProgressFn {
    let id = id.to_string();
    let last: Arc<Mutex<(Option<Step>, u64)>> = Arc::default();
    Arc::new(move |p: Progress| {
        let quarter = (p.done * 4).checked_div(p.total).unwrap_or(0);
        let mut l = last.lock().unwrap();
        if l.0 == Some(p.step) && l.1 == quarter {
            return;
        }
        *l = (Some(p.step), quarter);
        println!("  {id}: {:?} {}/{}", p.step, p.done, p.total);
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths = Paths::from_env()?;
    println!("data: {}", paths.root().display());
    let practice = Practice::new(paths.clone(), Downloader::new()?)?;
    let cmd = args.first().map(String::as_str).unwrap_or("list");
    let ids: Vec<String> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .cloned()
        .collect();
    match cmd {
        "list" => {
            for m in &practice.catalogue().maps {
                match &m.download {
                    Some(d) => {
                        println!("{:40} {:8} {:>10} B  {}", m.id, d.version, d.size, m.credit)
                    }
                    None => println!(
                        "{:40} link only ({:?}): {}",
                        m.id,
                        m.link_only.as_ref().map(|l| l.reason),
                        m.source_page
                    ),
                }
            }
        }
        "install" => {
            let ids: Vec<String> = if ids == ["all"] {
                practice
                    .catalogue()
                    .maps
                    .iter()
                    .filter(|m| m.is_installable())
                    .map(|m| m.id.clone())
                    .collect()
            } else {
                ids
            };
            for id in ids {
                let started = Instant::now();
                let out = practice
                    .install(&id, Some(printer(&id)))
                    .await
                    .with_context(|| format!("installing {id}"))?;
                for w in &out.worlds {
                    println!(
                        "installed {id} {} in {:.1}s: saves/{} shows as {:?}, DataVersion {:?}",
                        out.version,
                        started.elapsed().as_secs_f64(),
                        w.folder,
                        w.name,
                        w.data_version
                    );
                }
                for w in &out.warnings {
                    println!("  warning: {w:?}");
                }
            }
        }
        "status" => {
            for (id, s) in practice.statuses().await? {
                match s {
                    MapStatus::Installed(i) => println!(
                        "{id}: {} (stored {}, in store: {}, update: {:?}) {:?}",
                        i.version,
                        i.stored_version,
                        i.stored,
                        i.update,
                        i.worlds
                            .iter()
                            .map(|w| (&w.folder, w.state))
                            .collect::<Vec<_>>()
                    ),
                    other => println!("{id}: {other:?}"),
                }
            }
        }
        "check" => check(&practice, &paths).await?,
        "plan-update" => {
            for id in ids {
                println!("{id}: {:?}", practice.plan_update(&id).await?);
            }
        }
        "reset" => {
            let keep = args.iter().any(|a| a == "--keep");
            for id in ids {
                let out = practice.reset(&id, keep, Some(printer(&id))).await?;
                println!("reset {id}: {:?} carried {:?}", out.worlds, out.carried);
            }
        }
        "uninstall" => {
            for id in ids {
                println!("{id}: {:?}", practice.uninstall(&id).await?);
            }
        }
        other => bail!("unknown command {other}"),
    }
    Ok(())
}

/// For every installed map: the stored zip's sha512 and size against the catalogue, and each
/// world's level.dat read from saves/.
async fn check(practice: &Practice, paths: &Paths) -> Result<()> {
    let mut bad = 0;
    for (id, s) in practice.statuses().await? {
        let MapStatus::Installed(i) = s else { continue };
        let m = practice.catalogue().get(&id).unwrap();
        let d = m.download.as_ref().unwrap();
        let store = paths.store_sha512(&d.sha512);
        let size = std::fs::metadata(&store)?.len();
        let sha = srl_core::download::file_sha512(&store)?;
        let hash_ok = sha == d.sha512 && size == d.size;
        bad += usize::from(!hash_ok);
        println!(
            "{id} {}: store sha512 {} size {size} {}",
            i.version,
            &sha[..16],
            if hash_ok { "= catalogue" } else { "MISMATCH" }
        );
        for w in &i.worlds {
            let info = level::read_level_dat_file(
                &paths
                    .practice_instance()
                    .join("saves")
                    .join(&w.folder)
                    .join("level.dat"),
            )?;
            let fits = info.data_version == Some(DATA_VERSION_1_16_1);
            bad += usize::from(!fits);
            println!(
                "    saves/{}: DataVersion {:?} ({}), saved by {:?}, LevelName {:?}",
                w.folder,
                info.data_version,
                if fits { "fits 1.16.1" } else { "MISMATCH" },
                info.version_name,
                info.display_name()
            );
        }
    }
    if bad > 0 {
        bail!("{bad} problems");
    }
    Ok(())
}
