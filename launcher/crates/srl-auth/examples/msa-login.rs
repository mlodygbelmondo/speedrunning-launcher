//! A terminal login, for checking a new client ID before the UI exists (docs/microsoft-login.md,
//! part C). It opens the system browser and prints the device code; until Mojang approves the ID
//! the chain stops at Minecraft with `ClientNotApproved`, which is the expected result then.
//!
//! ```sh
//! SRL_MSA_CLIENT_ID=<guid> SRL_HOME=<data folder> cargo run -p srl-auth --example msa-login
//! # options: --no-browser (only print the link), --timeout <seconds> (browser path, default 600)
//! ```
use std::{sync::Arc, time::Duration};

use srl_auth::{Auth, AuthConfig, LoginEvent};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(mut cfg) = AuthConfig::detect() else {
        eprintln!("Microsoft login isn't available: set SRL_MSA_CLIENT_ID to our own client ID.");
        std::process::exit(2);
    };
    if args.iter().any(|a| a == "--no-browser") {
        cfg.open_browser = Arc::new(|_| Err(std::io::Error::other("--no-browser")));
    }
    if let Some(secs) = args
        .iter()
        .position(|a| a == "--timeout")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
    {
        cfg.browser_timeout = Duration::from_secs(secs);
    }
    let paths = srl_core::Paths::from_env().expect("a data folder");
    let auth = Auth::new(&paths, Some(cfg));
    let events = Arc::new(|e: LoginEvent| match e {
        LoginEvent::Browser { url, opened } => {
            let how = if opened {
                "opened in the browser"
            } else {
                "open this in a browser"
            };
            println!("Browser ({how}):\n  {url}");
        }
        LoginEvent::DeviceCode {
            user_code,
            verification_uri,
            expires_at,
            ..
        } => {
            println!("Or go to {verification_uri} and enter {user_code} (valid until {expires_at})")
        }
        LoginEvent::DeviceCodeFailed(e) => println!("Device code unavailable: {e}"),
        LoginEvent::BrowserFailed(e) => println!("Browser path failed: {e}"),
        LoginEvent::Step(s) => println!("... {s:?}"),
    });
    match auth.login(events).await {
        Ok(account) => {
            println!("Logged in: {} ({})", account.name, account.uuid);
            match auth.prepare_launch(account.uuid).await {
                Ok(ready) => println!(
                    "Session ready for the game: {:?} (warning: {:?})",
                    ready.session, ready.warning
                ),
                Err(e) => println!("prepare_launch failed: {e} [{}]", e.copy_key()),
            }
        }
        Err(e) => {
            println!("Login failed: {e} [{}]", e.copy_key());
            std::process::exit(1);
        }
    }
}
