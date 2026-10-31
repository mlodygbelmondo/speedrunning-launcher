//! The interactive Microsoft sign-in: the system browser (authorization code + PKCE, loopback
//! redirect) and the device code flow run at the same time; the first to finish wins and the
//! other is dropped, which closes its listener or stops its polling (research 1.5 and 1.9).
use std::{pin::pin, sync::Arc};

use chrono::{DateTime, Utc};
use futures::FutureExt;
use reqwest::Url;

use crate::{
    config::AuthConfig,
    error::AuthError,
    loopback::Loopback,
    msa::{self, MsaTokens},
    pkce::{Pkce, new_state},
};

/// Which path the player used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Browser,
    DeviceCode,
}

/// The chain after the Microsoft sign-in, for a progress line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginStep {
    /// Microsoft accepted the sign-in.
    SignedIn(Method),
    /// Xbox Live user token and XSTS.
    Xbox,
    /// Minecraft services login.
    Minecraft,
    /// Entitlements and profile.
    Profile,
}

/// What the login page shows while it waits.
#[derive(Clone, Debug)]
pub enum LoginEvent {
    /// The Microsoft page for the browser path. `opened` is false when no browser could be
    /// started: show the link to copy instead. The URL holds no secret (the PKCE challenge is
    /// public by design; the state is single-use).
    Browser {
        url: String,
        opened: bool,
    },
    /// The device code path: show the code, the address and a QR code of `qr_url`
    /// (`qr_matrix`). Valid until `expires_at`.
    DeviceCode {
        user_code: String,
        verification_uri: String,
        qr_url: String,
        expires_at: DateTime<Utc>,
    },
    /// The device code path can't be used (Microsoft refused it, no network); the browser path
    /// still runs. Hide the code panel.
    DeviceCodeFailed(AuthError),
    /// The browser path failed (e.g. a state mismatch); the device code path still runs.
    BrowserFailed(AuthError),
    Step(LoginStep),
}

/// Called from the tokio runtime, possibly from several tasks: keep it cheap (store the event,
/// notify the UI), like `srl_core::ProgressFn`.
pub type LoginEventFn = Arc<dyn Fn(LoginEvent) + Send + Sync>;

pub(crate) fn authorize_url(
    cfg: &AuthConfig,
    redirect_uri: &str,
    challenge: &str,
    state: &str,
) -> Url {
    let mut url = cfg.endpoints.authorize.clone();
    url.query_pairs_mut()
        .append_pair("client_id", &cfg.client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_mode", "query")
        .append_pair("scope", msa::SCOPE)
        .append_pair("state", state)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "select_account");
    url
}

async fn browser_flow(
    http: &reqwest::Client,
    cfg: &AuthConfig,
    events: &LoginEventFn,
) -> Result<MsaTokens, AuthError> {
    let lb = Loopback::bind()
        .await
        .map_err(|e| AuthError::Network(format!("a local port for the browser ({e})")))?;
    let redirect_uri = lb.redirect_uri(&cfg.redirect_host);
    let pkce = Pkce::new();
    let state = new_state();
    let url = authorize_url(cfg, &redirect_uri, &pkce.challenge, &state);
    // A browser that fails to open doesn't end this path: the player can copy the link.
    let opened = (cfg.open_browser)(url.as_str()).is_ok();
    events(LoginEvent::Browser {
        url: url.into(),
        opened,
    });
    let code = lb.wait_for_code(&state, cfg.browser_timeout).await?;
    msa::exchange_code(http, cfg, &code, &pkce.verifier, &redirect_uri).await
}

async fn device_flow(
    http: &reqwest::Client,
    cfg: &AuthConfig,
    events: &LoginEventFn,
) -> Result<MsaTokens, AuthError> {
    let dc = msa::start_device_code(http, cfg).await?;
    events(LoginEvent::DeviceCode {
        user_code: dc.user_code.clone(),
        verification_uri: dc.verification_uri.clone(),
        qr_url: dc.qr_url.clone(),
        expires_at: dc.expires_at,
    });
    msa::poll_device_code(http, cfg, &dc).await
}

/// Runs both paths until one signs in. One failing leaves the other running; a "no" from the
/// player on either ends both.
pub(crate) async fn sign_in(
    http: &reqwest::Client,
    cfg: &AuthConfig,
    events: &LoginEventFn,
) -> Result<(MsaTokens, Method), AuthError> {
    let mut browser = pin!(browser_flow(http, cfg, events).fuse());
    let mut device = pin!(device_flow(http, cfg, events).fuse());
    let mut browser_err: Option<AuthError> = None;
    let mut device_err: Option<AuthError> = None;
    loop {
        futures::select! {
            r = browser => match r {
                Ok(t) => return Ok((t, Method::Browser)),
                Err(AuthError::Declined) => return Err(AuthError::Declined),
                Err(e) => {
                    events(LoginEvent::BrowserFailed(e.clone()));
                    browser_err = Some(e);
                }
            },
            r = device => match r {
                Ok(t) => return Ok((t, Method::DeviceCode)),
                Err(AuthError::Declined) => return Err(AuthError::Declined),
                Err(e) => {
                    events(LoginEvent::DeviceCodeFailed(e.clone()));
                    device_err = Some(e);
                }
            },
            complete => break,
        }
    }
    Err(pick_error(
        browser_err.unwrap_or(AuthError::TimedOut),
        device_err.unwrap_or(AuthError::TimedOut),
    ))
}

/// Both paths failed: report the one that explains more. Running out of time says little; a
/// misconfigured app or a state mismatch says a lot.
fn pick_error(browser: AuthError, device: AuthError) -> AuthError {
    let vague = |e: &AuthError| {
        matches!(
            e,
            AuthError::TimedOut | AuthError::DeviceCodeExpired | AuthError::Network(_)
        )
    };
    if vague(&browser) && !vague(&device) {
        device
    } else {
        browser
    }
}

/// A QR code as a square of modules, row by row, `true` = dark. The UI draws it (no image crate
/// needed); add a quiet zone of 4 modules around it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrMatrix {
    pub width: usize,
    pub dark: Vec<bool>,
}

pub fn qr_matrix(data: &str) -> Option<QrMatrix> {
    let code = qrcode::QrCode::new(data.as_bytes()).ok()?;
    Some(QrMatrix {
        width: code.width(),
        dark: code
            .to_colors()
            .into_iter()
            .map(|c| c == qrcode::Color::Dark)
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_url_carries_pkce_and_state() {
        let cfg = AuthConfig::new("11111111-2222-3333-4444-555555555555").unwrap();
        let url = authorize_url(&cfg, "http://localhost:4321", "CHALLENGE", "STATE");
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(url.host_str(), Some("login.microsoftonline.com"));
        assert!(url.path().starts_with("/consumers/"));
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["code_challenge"], "CHALLENGE");
        assert_eq!(q["state"], "STATE");
        assert_eq!(q["redirect_uri"], "http://localhost:4321");
        assert_eq!(q["scope"], "XboxLive.signin offline_access");
        assert_eq!(q["response_type"], "code");
    }

    #[test]
    fn the_more_telling_error_wins() {
        assert_eq!(
            pick_error(AuthError::TimedOut, AuthError::AppMisconfigured("x".into())),
            AuthError::AppMisconfigured("x".into())
        );
        assert_eq!(
            pick_error(AuthError::StateMismatch, AuthError::DeviceCodeExpired),
            AuthError::StateMismatch
        );
    }

    #[test]
    fn qr_for_the_link() {
        let m = qr_matrix("https://www.microsoft.com/link?otc=ABCD1234").unwrap();
        assert_eq!(m.dark.len(), m.width * m.width);
        assert!(m.width >= 21);
        // The top-left finder pattern starts with a dark module.
        assert!(m.dark[0]);
    }
}
