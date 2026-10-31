//! The Microsoft account (MSA) step: the OAuth 2.0 token endpoint for the authorization code
//! (with PKCE), the device code flow (RFC 8628) and refresh-token renewal.
//!
//! Written against Microsoft's identity platform docs; the shape of the calls follows Pandora
//! Launcher's `crates/auth/src/authenticator.rs` (MIT), which uses the `oauth2` crate. We talk to
//! the endpoint directly instead, to map every error code ourselves and to control the polling.
use chrono::{DateTime, Utc};
use reqwest::{StatusCode, Url};
use serde::Deserialize;
use tokio::time::Instant;

use crate::{
    config::AuthConfig,
    error::{AuthError, clip, network, retry_after},
    secret::Secret,
};

/// Xbox sign-in plus a refresh token. `XboxLive.signin` only works on the `consumers` authority.
pub const SCOPE: &str = "XboxLive.signin offline_access";

const SERVICE: &str = "Microsoft login";

/// What the token endpoint gives back.
#[derive(Debug)]
pub struct MsaTokens {
    pub access: Secret,
    pub expires_at: DateTime<Utc>,
    /// Microsoft rotates it on every use: always store the new one.
    pub refresh: Option<Secret>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

#[derive(Deserialize, Default)]
struct OAuthError {
    #[serde(default)]
    error: String,
    #[serde(default)]
    error_description: String,
    #[serde(default)]
    error_codes: Vec<u64>,
}

enum TokenFailure {
    OAuth(StatusCode, OAuthError),
    Other(AuthError),
}

impl From<AuthError> for TokenFailure {
    fn from(e: AuthError) -> Self {
        Self::Other(e)
    }
}

async fn token_request(
    http: &reqwest::Client,
    cfg: &AuthConfig,
    form: &[(&str, &str)],
) -> Result<MsaTokens, TokenFailure> {
    let resp = http
        .post(cfg.endpoints.token.clone())
        .header("Accept", "application/json")
        .form(form)
        .send()
        .await
        .map_err(|e| network(SERVICE, &e))?;
    let status = resp.status();
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(AuthError::RateLimited {
            retry_after: retry_after(resp.headers()),
        }
        .into());
    }
    let body = resp.bytes().await.map_err(|e| network(SERVICE, &e))?;
    if status.is_success() {
        let t: TokenResponse = serde_json::from_slice(&body).map_err(|_| AuthError::Service {
            service: SERVICE,
            status: status.as_u16(),
            detail: "unreadable token response".into(),
        })?;
        return Ok(MsaTokens {
            access: Secret::new(t.access_token),
            expires_at: Utc::now() + chrono::Duration::seconds(t.expires_in.unwrap_or(3600)),
            refresh: t.refresh_token.map(Secret::new),
        });
    }
    match serde_json::from_slice::<OAuthError>(&body) {
        Ok(e) if !e.error.is_empty() => Err(TokenFailure::OAuth(status, e)),
        _ => Err(AuthError::Service {
            service: SERVICE,
            status: status.as_u16(),
            detail: clip(&String::from_utf8_lossy(&body)),
        }
        .into()),
    }
}

/// The errors every grant shares. `invalid_grant` is left to the caller: it means different
/// things for a code and for a refresh token.
fn map_oauth(status: StatusCode, e: &OAuthError) -> AuthError {
    let has = |code: u64| {
        e.error_codes.contains(&code) || e.error_description.contains(&format!("AADSTS{code}:"))
    };
    if has(70002) || has(7000218) {
        return AuthError::AppMisconfigured("public client flows are not allowed".into());
    }
    if has(700016) {
        return AuthError::AppMisconfigured("the client ID is unknown to Microsoft".into());
    }
    if has(50011) {
        return AuthError::AppMisconfigured("the redirect URI is not registered".into());
    }
    match e.error.as_str() {
        "invalid_client" | "unauthorized_client" => AuthError::AppMisconfigured(e.error.clone()),
        "access_denied" | "authorization_declined" => AuthError::Declined,
        "expired_token" | "code_expired" => AuthError::DeviceCodeExpired,
        // Microsoft's documented "try again later": reported like a 503 so `is_transient()`
        // holds (a launch goes ahead with the saved token, the background refresh retries).
        "temporarily_unavailable" => AuthError::Service {
            service: SERVICE,
            status: status.as_u16().max(503),
            detail: clip(&format!("{}: {}", e.error, e.error_description)),
        },
        _ => AuthError::Service {
            service: SERVICE,
            status: status.as_u16(),
            detail: clip(&format!("{}: {}", e.error, e.error_description)),
        },
    }
}

/// Trades the browser's authorization code (and our PKCE verifier) for tokens.
pub async fn exchange_code(
    http: &reqwest::Client,
    cfg: &AuthConfig,
    code: &Secret,
    verifier: &Secret,
    redirect_uri: &str,
) -> Result<MsaTokens, AuthError> {
    let form = [
        ("client_id", cfg.client_id.as_str()),
        ("grant_type", "authorization_code"),
        ("code", code.expose()),
        ("redirect_uri", redirect_uri),
        ("code_verifier", verifier.expose()),
        ("scope", SCOPE),
    ];
    token_request(http, cfg, &form).await.map_err(|f| match f {
        TokenFailure::Other(e) => e,
        TokenFailure::OAuth(_, e) if e.error == "invalid_grant" => AuthError::Service {
            service: SERVICE,
            status: 400,
            detail: "the sign-in code was rejected (expired or already used)".into(),
        },
        TokenFailure::OAuth(s, e) => map_oauth(s, &e),
    })
}

/// A fresh access token from the refresh token. `invalid_grant` (revoked, expired after 90 days
/// without use, password changed) means only an interactive login helps, and so do the errors
/// Microsoft documents for "the user must be present": `interaction_required` (MFA, a new
/// sign-in), `consent_required` and `login_required`. Without that they would be a generic
/// service error on every Play and the account would never get its "Log in again" badge.
pub async fn refresh(
    http: &reqwest::Client,
    cfg: &AuthConfig,
    refresh_token: &Secret,
) -> Result<MsaTokens, AuthError> {
    let form = [
        ("client_id", cfg.client_id.as_str()),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token.expose()),
        ("scope", SCOPE),
    ];
    token_request(http, cfg, &form).await.map_err(|f| match f {
        TokenFailure::Other(e) => e,
        TokenFailure::OAuth(s, e) => refresh_error(s, &e),
    })
}

fn refresh_error(status: StatusCode, e: &OAuthError) -> AuthError {
    match e.error.as_str() {
        "invalid_grant" => {
            AuthError::ReloginRequired("Microsoft no longer accepts the saved sign-in".into())
        }
        "interaction_required" | "login_required" => {
            AuthError::ReloginRequired("Microsoft wants you to sign in again".into())
        }
        "consent_required" => {
            AuthError::ReloginRequired("Microsoft needs your consent again".into())
        }
        _ => map_oauth(status, e),
    }
}

/// A started device code flow: what the player types (or scans) on another device.
#[derive(Clone, Debug)]
pub struct DeviceCode {
    pub user_code: String,
    /// Where to type it, e.g. `https://www.microsoft.com/link`.
    pub verification_uri: String,
    /// The same page with the code filled in, for the QR code.
    pub qr_url: String,
    pub expires_at: DateTime<Utc>,
    pub(crate) device_code: Secret,
    pub(crate) interval_s: u64,
    pub(crate) deadline: Instant,
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    user_code: String,
    device_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    expires_in: u64,
    #[serde(default)]
    interval: Option<u64>,
}

pub async fn start_device_code(
    http: &reqwest::Client,
    cfg: &AuthConfig,
) -> Result<DeviceCode, AuthError> {
    let resp = http
        .post(cfg.endpoints.device_code.clone())
        .header("Accept", "application/json")
        .form(&[("client_id", cfg.client_id.as_str()), ("scope", SCOPE)])
        .send()
        .await
        .map_err(|e| network(SERVICE, &e))?;
    let status = resp.status();
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(AuthError::RateLimited {
            retry_after: retry_after(resp.headers()),
        });
    }
    let body = resp.bytes().await.map_err(|e| network(SERVICE, &e))?;
    if !status.is_success() {
        return Err(match serde_json::from_slice::<OAuthError>(&body) {
            Ok(e) if !e.error.is_empty() => map_oauth(status, &e),
            _ => AuthError::Service {
                service: SERVICE,
                status: status.as_u16(),
                detail: clip(&String::from_utf8_lossy(&body)),
            },
        });
    }
    let r: DeviceCodeResponse = serde_json::from_slice(&body).map_err(|_| AuthError::Service {
        service: SERVICE,
        status: status.as_u16(),
        detail: "unreadable device code response".into(),
    })?;
    let qr_url = r
        .verification_uri_complete
        .clone()
        .unwrap_or_else(|| qr_url(&r.verification_uri, &r.user_code));
    Ok(DeviceCode {
        qr_url,
        expires_at: Utc::now() + chrono::Duration::seconds(r.expires_in as i64),
        deadline: Instant::now() + cfg.poll_unit * r.expires_in as u32,
        interval_s: r.interval.unwrap_or(5).max(1),
        user_code: r.user_code,
        device_code: Secret::new(r.device_code),
        verification_uri: r.verification_uri,
    })
}

/// Microsoft's link page takes the code as `otc`, which is what Prism and the official apps put in
/// their QR codes; anything else gets the plain page (the player types the code).
fn qr_url(verification_uri: &str, user_code: &str) -> String {
    match Url::parse(verification_uri) {
        Ok(mut u) if u.path().trim_end_matches('/').ends_with("/link") => {
            u.query_pairs_mut().append_pair("otc", user_code);
            u.into()
        }
        _ => verification_uri.to_string(),
    }
}

/// Polls until the player has entered the code. RFC 8628: `authorization_pending` keeps the
/// interval, `slow_down` adds five seconds to it for the rest of the flow, `expired_token` ends it.
/// A network blip doesn't end it either: the code is valid for 15 minutes.
pub async fn poll_device_code(
    http: &reqwest::Client,
    cfg: &AuthConfig,
    dc: &DeviceCode,
) -> Result<MsaTokens, AuthError> {
    let mut interval = dc.interval_s;
    loop {
        tokio::time::sleep(cfg.poll_unit * interval as u32).await;
        if Instant::now() >= dc.deadline {
            return Err(AuthError::DeviceCodeExpired);
        }
        let form = [
            ("client_id", cfg.client_id.as_str()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", dc.device_code.expose()),
        ];
        match token_request(http, cfg, &form).await {
            Ok(tokens) => return Ok(tokens),
            Err(TokenFailure::OAuth(s, e)) => match e.error.as_str() {
                "authorization_pending" => {}
                "slow_down" => interval += 5,
                // A passing outage: the code stays valid, keep polling.
                "temporarily_unavailable" => {}
                _ => return Err(map_oauth(s, &e)),
            },
            Err(TokenFailure::Other(AuthError::Network(_))) => {}
            Err(TokenFailure::Other(AuthError::RateLimited { .. })) => interval += 5,
            Err(TokenFailure::Other(e)) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_url_fills_in_the_code_on_microsofts_page() {
        assert_eq!(
            qr_url("https://www.microsoft.com/link", "ABCD1234"),
            "https://www.microsoft.com/link?otc=ABCD1234"
        );
        assert_eq!(
            qr_url("https://example.org/device", "X"),
            "https://example.org/device"
        );
    }

    #[test]
    fn oauth_errors_map_to_reasons() {
        let e = |error: &str, codes: Vec<u64>, desc: &str| OAuthError {
            error: error.into(),
            error_description: desc.into(),
            error_codes: codes,
        };
        let bad = StatusCode::BAD_REQUEST;
        assert!(matches!(
            map_oauth(bad, &e("invalid_request", vec![7000218], "")),
            AuthError::AppMisconfigured(_)
        ));
        assert!(matches!(
            map_oauth(
                bad,
                &e("invalid_request", vec![], "AADSTS70002: public client")
            ),
            AuthError::AppMisconfigured(_)
        ));
        assert!(matches!(
            map_oauth(bad, &e("unauthorized_client", vec![700016], "")),
            AuthError::AppMisconfigured(_)
        ));
        assert_eq!(
            map_oauth(bad, &e("authorization_declined", vec![], "")),
            AuthError::Declined
        );
        assert_eq!(
            map_oauth(bad, &e("expired_token", vec![], "")),
            AuthError::DeviceCodeExpired
        );
        assert!(matches!(
            map_oauth(bad, &e("server_error", vec![], "boom")),
            AuthError::Service { status: 400, .. }
        ));
        assert!(map_oauth(bad, &e("temporarily_unavailable", vec![], "")).is_transient());
    }

    #[test]
    fn refresh_errors_that_need_the_player_ask_for_a_new_login() {
        let e = |error: &str| OAuthError {
            error: error.into(),
            error_description: format!("AADSTS50076: {error}"),
            error_codes: vec![50076],
        };
        let bad = StatusCode::BAD_REQUEST;
        for error in [
            "invalid_grant",
            "interaction_required",
            "consent_required",
            "login_required",
        ] {
            assert!(refresh_error(bad, &e(error)).needs_relogin(), "{error}");
        }
        let busy = refresh_error(bad, &e("temporarily_unavailable"));
        assert!(busy.is_transient() && !busy.needs_relogin(), "{busy:?}");
        let other = refresh_error(bad, &e("invalid_request"));
        assert!(!other.is_transient() && !other.needs_relogin(), "{other:?}");
    }
}
