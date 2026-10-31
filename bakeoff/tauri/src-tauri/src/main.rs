// Tauri 2 shell for the Speedrunluncher bake-off. See bakeoff/SPEC.md.
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Manager, RunEvent, State, WebviewWindow};

const STACK: &str = "tauri";

fn epoch_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// JSONL sink for $BAKEOFF_OUT. Every line is written and flushed immediately.
struct Out {
    file: Option<Mutex<File>>,
    phase_ms: u64,
    /// Deep-link screen id from $BAKEOFF_SCREEN (SCREENS.md). Ignored in scenario mode.
    screen: Option<String>,
    /// Start language from $BAKEOFF_LANG ("en" | "pl"). Ignored in scenario mode.
    lang: Option<String>,
}

/// Non-empty env var, only outside scenario mode.
fn ui_env(name: &str, scenario: bool) -> Option<String> {
    std::env::var(name).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty() && !scenario)
}

impl Out {
    fn from_env() -> Self {
        let file = std::env::var_os("BAKEOFF_OUT").filter(|p| !p.is_empty()).map(|p| {
            let f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&p)
                .unwrap_or_else(|e| panic!("cannot open BAKEOFF_OUT {p:?}: {e}"));
            Mutex::new(f)
        });
        let phase_ms = std::env::var("BAKEOFF_PHASE_MS")
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(20_000);
        let screen = ui_env("BAKEOFF_SCREEN", file.is_some());
        let lang = ui_env("BAKEOFF_LANG", file.is_some());
        Out { file, phase_ms, screen, lang }
    }

    fn line(&self, s: &str) {
        if let Some(f) = &self.file {
            let mut f = f.lock().unwrap();
            let _ = f.write_all(s.trim_end().as_bytes());
            let _ = f.write_all(b"\n");
            let _ = f.flush();
        }
    }

    fn json(&self, v: serde_json::Value) {
        self.line(&v.to_string());
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    scenario: bool,
    phase_ms: u64,
    stack: &'static str,
    screen: Option<String>,
    lang: Option<String>,
}

#[tauri::command]
fn report(line: String, out: State<'_, Out>) {
    out.line(&line);
}

#[tauri::command]
fn config(out: State<'_, Out>) -> Config {
    Config { scenario: out.file.is_some(), phase_ms: out.phase_ms, stack: STACK, screen: out.screen.clone(), lang: out.lang.clone() }
}

#[tauri::command]
async fn minimize(window: WebviewWindow) {
    let _ = window.minimize();
}

/// Destroy the webview window but keep the process alive (RunEvent::ExitRequested is
/// prevented). The host owns the rest of the `destroyed` phase, then emits `done` and exits 0.
#[tauri::command]
async fn destroy(window: WebviewWindow, app: AppHandle) {
    let phase_ms = app.state::<Out>().phase_ms;
    std::thread::spawn(move || {
        let _ = window.destroy();
        std::thread::sleep(Duration::from_millis(phase_ms));
        app.state::<Out>().json(serde_json::json!({ "ev": "done", "t": epoch_ms() }));
        app.exit(0);
    });
}

fn main() {
    let t_start = epoch_ms();
    let out = Out::from_env();
    let version = format!(
        "tauri {} / webview {}",
        tauri::VERSION,
        tauri::webview_version().unwrap_or_else(|_| "unknown".into())
    );
    out.json(serde_json::json!({
        "ev": "start",
        "t": t_start,
        "stack": STACK,
        "pid": std::process::id(),
        "version": version,
    }));

    let app = tauri::Builder::default()
        .manage(out)
        .invoke_handler(tauri::generate_handler![report, config, minimize, destroy])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| {
        if let RunEvent::ExitRequested { code, api, .. } = event {
            // code == None: the last window went away (our destroy). Keep running.
            // code == Some(_): explicit app.exit(), let it through.
            if code.is_none() {
                api.prevent_exit();
            }
        }
    });
}
