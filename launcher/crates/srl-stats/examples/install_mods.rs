//! Installs a mode's legal mod set into `$SRL_HOME/instances/<mode>/mods`, the same way Play does.
//! Together with `srl-game install` (the game itself) it gives a playable instance without the
//! GPUI app, so the real-file check in NOTES.md can produce SpeedRunIGT records.
//!
//! ```text
//! SRL_HOME=... cargo run -p srl-stats --example install_mods -- rsg
//! ```
use anyhow::{Context, Result};
use srl_core::{Downloader, Mode, Paths};
use srl_mods::{InstanceGuard, ModMeta, Os, ResolveOptions};

#[tokio::main]
async fn main() -> Result<()> {
    let id = std::env::args().nth(1).unwrap_or_else(|| "rsg".into());
    let mode = Mode::from_id(&id).with_context(|| format!("unknown mode {id}"))?;
    let paths = Paths::from_env()?;
    let downloader = Downloader::new()?;
    let meta = ModMeta::load(&paths, &downloader, true).await?;
    let set = srl_mods::resolve(&meta, mode, Os::current(), &ResolveOptions::default())?;
    let guard = InstanceGuard::new();
    let report = srl_mods::install(&paths, &downloader, &set, &guard, None).await?;
    println!(
        "{}: {} mods, changed: {}",
        mode.id(),
        set.modids().len(),
        report.changed()
    );
    Ok(())
}
