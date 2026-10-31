//! JSON Lines report (SPEC.md "Report protocol"), appended to `$BAKEOFF_OUT` and flushed per line.
use std::{
    fs::{File, OpenOptions},
    io::Write,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};

static OUT: OnceLock<Mutex<File>> = OnceLock::new();

/// Unix epoch in milliseconds with sub-ms precision.
pub fn epoch_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// Opens the report file and emits `start`. Returns the phase duration when scenario mode is on.
pub fn init() -> Option<u64> {
    let t = epoch_ms();
    let path = std::env::var_os("BAKEOFF_OUT").filter(|p| !p.is_empty())?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap_or_else(|e| panic!("cannot open BAKEOFF_OUT {path:?}: {e}"));
    let _ = OUT.set(Mutex::new(file));
    report(json!({
        "ev": "start",
        "t": t,
        "stack": "gpui",
        "pid": std::process::id(),
        "version": concat!(
            "gpui zed@fc9258b4 (",
            env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"), ")"
        ),
    }));
    let ms = std::env::var("BAKEOFF_PHASE_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(20_000);
    Some(ms)
}

pub fn report(v: Value) {
    let Some(out) = OUT.get() else { return };
    let mut line = v.to_string();
    line.push('\n');
    if let Ok(mut f) = out.lock() {
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
    }
}
