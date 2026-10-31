//! srl-auth's test mock of Microsoft, Xbox Live and Minecraft services (`tests/mock`) as a
//! program, so the app's login dialog and a Microsoft launch can be tried end to end without a
//! real account or client ID. Only for development: it hands out fake tokens ("…-SECRET-n", easy
//! to grep for in logs) and listens on 127.0.0.1 only.
//!
//! ```text
//! cargo run -p srl-auth --example mock-services
//! SRL_MSA_CLIENT_ID=11111111-2222-3333-4444-555555555555 SRL_MSA_BASE_URL=<base> \
//!   SRL_KEYSTORE=file speedrunning-launcher
//! ```
//!
//! It prints `base <url>` and `control <addr>`. The control port takes one command per line and
//! answers `ok` (or `error ...`):
//!
//! - `browser approve|ignore|wrong-state`: what the browser path does (the app's dev browser
//!   only fetches the page; `ignore` leaves it waiting like a player who uses the phone)
//! - `poll pending|slow-down|approve|expired|declined`: queue the next phone-code poll answer
//! - `xerr <code>|none`: the XSTS error (2148916238 is the child account)
//! - `profile <name> <id>`: the Minecraft profile the next login gets
//! - `offline on|off`: drop every connection (the network is down)
//! - `refresh-invalid on|off`: the saved sign-in was revoked
//! - `mc-expires <seconds>`: the Minecraft token's lifetime
//! - `msa-expires <seconds>`: the Microsoft access token's lifetime (0: every renewal needs the
//!   refresh token)
//! - `xbox-expires <seconds>`: the Xbox Live and XSTS tokens' lifetime (with `msa-expires 0` and
//!   a short `mc-expires`, the next launch has to use the refresh token)
//! - `calls <what>`: how often an endpoint was called (e.g. `token:device`)

#[path = "../tests/mock/mod.rs"]
mod mock;

use std::sync::Arc;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};

#[tokio::main]
async fn main() {
    let mock = Arc::new(mock::Mock::start().await);
    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("base {}", mock.base());
    println!("control {}", control.local_addr().unwrap());
    println!("client id {}", mock::CLIENT_ID);
    loop {
        let Ok((stream, _)) = control.accept().await else {
            continue;
        };
        let mock = mock.clone();
        tokio::spawn(async move {
            let (read, mut write) = stream.into_split();
            let mut lines = BufReader::new(read).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let answer = match command(&mock, &line) {
                    Ok(s) => s,
                    Err(e) => format!("error {e}"),
                };
                println!("> {line}: {answer}");
                if write
                    .write_all(format!("{answer}\n").as_bytes())
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
    }
}

fn command(mock: &mock::Mock, line: &str) -> Result<String, String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let on = |w: Option<&&str>| match w.copied() {
        Some("on") => Ok(true),
        Some("off") => Ok(false),
        _ => Err("expected on or off".to_string()),
    };
    match words.as_slice() {
        ["browser", mode] => {
            let mode = match *mode {
                "approve" => mock::BrowserMode::Approve,
                "ignore" => mock::BrowserMode::Ignore,
                "wrong-state" => mock::BrowserMode::WrongState,
                _ => return Err("expected approve, ignore or wrong-state".into()),
            };
            mock.with(|s| s.browser = mode);
        }
        ["poll", answer] => {
            let poll = match *answer {
                "pending" => mock::Poll::Pending,
                "slow-down" => mock::Poll::SlowDown,
                "approve" => mock::Poll::Approve,
                "expired" => mock::Poll::Expired,
                "declined" => mock::Poll::Declined,
                _ => return Err("expected pending, slow-down, approve, expired or declined".into()),
            };
            mock.with(|s| s.polls.push_back(poll));
        }
        ["xerr", "none"] => mock.with(|s| s.xsts_xerr = None),
        ["xerr", code] => {
            let code = code.parse().map_err(|_| "expected a number".to_string())?;
            mock.with(|s| s.xsts_xerr = Some(code));
        }
        ["profile", name, id] => mock.with(|s| {
            s.profile_name = (*name).to_string();
            s.profile_id = (*id).to_string();
        }),
        ["offline", _] => {
            let v = on(words.get(1))?;
            mock.with(|s| s.offline = v);
        }
        ["refresh-invalid", _] => {
            let v = on(words.get(1))?;
            mock.with(|s| s.refresh_invalid = v);
        }
        ["mc-expires", secs] => {
            let secs = secs.parse().map_err(|_| "expected seconds".to_string())?;
            mock.with(|s| s.mc_expires_in = secs);
        }
        ["msa-expires", secs] => {
            let secs = secs.parse().map_err(|_| "expected seconds".to_string())?;
            mock.with(|s| s.msa_expires_in = secs);
        }
        ["xbox-expires", secs] => {
            let secs = secs.parse().map_err(|_| "expected seconds".to_string())?;
            mock.with(|s| {
                s.xbl_lifetime_s = secs;
                s.xsts_lifetime_s = secs;
            });
        }
        ["calls", what] => {
            let what: &'static str = Box::leak(what.to_string().into_boxed_str());
            return Ok(mock.with(|s| s.calls(what)).to_string());
        }
        _ => return Err(format!("unknown command: {line}")),
    }
    Ok("ok".into())
}
