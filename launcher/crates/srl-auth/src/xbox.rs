//! From an MSA access token to a Minecraft profile: Xbox Live user token, XSTS token for
//! `rp://api.minecraftservices.com/`, Minecraft `login_with_xbox`, entitlements and the profile.
//!
//! Request and response shapes adapted from Pandora Launcher's `crates/auth/src/authenticator.rs`
//! and `models.rs` (MIT), including its clock-skew handling (expiry = now + NotAfter - IssueInstant).
//! The error mapping (XErr, 403 "Invalid app registration", 404 profile) follows research 1.3.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use reqwest::{RequestBuilder, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    config::AuthConfig,
    error::{AuthError, XboxProblem, clip, network, retry_after},
    secret::Secret,
};

const XBOX: &str = "Xbox Live";
const MINECRAFT: &str = "Minecraft services";

/// An Xbox Live user token (about 14 days) or an XSTS token (about 16 hours), with the user hash.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct XboxToken {
    pub token: Secret,
    pub uhs: String,
    /// The Xbox user ID, when the service includes it in the display claims.
    #[serde(default)]
    pub xid: Option<String>,
    pub expires_at: DateTime<Utc>,
}

/// A Minecraft access token (24 hours): what the game gets as `--accessToken`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MinecraftToken {
    pub token: Secret,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skin {
    /// Always https (the API still hands out some http URLs).
    pub url: String,
    /// `CLASSIC` or `SLIM`.
    pub variant: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub uuid: Uuid,
    pub name: String,
    /// The active skin, if the profile has one (a default skin has none).
    pub skin: Option<Skin>,
}

/// What `entitlements/license` lists. Shown for information only: neither Prism nor anyone else
/// verifies its signature, and the profile is the authority on "can play" (research 1.5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entitlements {
    pub items: Vec<String>,
}

impl Entitlements {
    /// Bought the game (`product_minecraft` or `game_minecraft`).
    pub fn owns_game(&self) -> bool {
        self.items
            .iter()
            .any(|i| i == "product_minecraft" || i == "game_minecraft")
    }

    /// A PC Game Pass entitlement. The exact names are from community docs, not Mojang, so any
    /// item that mentions Game Pass counts.
    pub fn game_pass(&self) -> bool {
        self.items.iter().any(|i| i.contains("game_pass"))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxResponse {
    issue_instant: Option<DateTime<Utc>>,
    not_after: Option<DateTime<Utc>>,
    token: String,
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XstsError {
    #[serde(rename = "XErr")]
    xerr: Option<u64>,
}

async fn send(service: &'static str, req: RequestBuilder) -> Result<Response, AuthError> {
    let resp = req
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| network(service, &e))?;
    if resp.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err(AuthError::RateLimited {
            retry_after: retry_after(resp.headers()),
        });
    }
    Ok(resp)
}

async fn body(service: &'static str, resp: Response) -> Result<(StatusCode, Vec<u8>), AuthError> {
    let status = resp.status();
    let bytes = resp.bytes().await.map_err(|e| network(service, &e))?;
    Ok((status, bytes.to_vec()))
}

fn service_error(service: &'static str, status: StatusCode, body: &[u8]) -> AuthError {
    AuthError::Service {
        service,
        status: status.as_u16(),
        detail: clip(&String::from_utf8_lossy(body)),
    }
}

fn unreadable(service: &'static str, status: StatusCode) -> AuthError {
    AuthError::Service {
        service,
        status: status.as_u16(),
        detail: "unreadable response".into(),
    }
}

/// Expiry on our clock: the token's lifetime as the server states it, counted from now, so a wrong
/// system clock doesn't make a fresh token look expired.
fn expiry(
    issued: Option<DateTime<Utc>>,
    not_after: Option<DateTime<Utc>>,
    default: Duration,
) -> DateTime<Utc> {
    let lifetime = match (issued, not_after) {
        (Some(i), Some(n)) if n > i => n - i,
        _ => default,
    };
    Utc::now() + lifetime
}

fn parse_xbox(status: StatusCode, bytes: &[u8], default: Duration) -> Result<XboxToken, AuthError> {
    let r: XboxResponse = serde_json::from_slice(bytes).map_err(|_| unreadable(XBOX, status))?;
    let claims = r
        .display_claims
        .xui
        .first()
        .ok_or_else(|| unreadable(XBOX, status))?;
    let field = |k: &str| claims.get(k).and_then(|v| v.as_str()).map(str::to_string);
    Ok(XboxToken {
        token: Secret::new(r.token),
        uhs: field("uhs").ok_or_else(|| unreadable(XBOX, status))?,
        xid: field("xid"),
        expires_at: expiry(r.issue_instant, r.not_after, default),
    })
}

/// MSA access token -> Xbox Live user token.
pub async fn xbox_user(
    http: &reqwest::Client,
    cfg: &AuthConfig,
    msa_access: &Secret,
) -> Result<XboxToken, AuthError> {
    let req = http
        .post(cfg.endpoints.xbox_user.clone())
        .header("x-xbl-contract-version", "1")
        .json(&json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", msa_access.expose()),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        }));
    let (status, bytes) = body(XBOX, send(XBOX, req).await?).await?;
    if !status.is_success() {
        return Err(service_error(XBOX, status, &bytes));
    }
    parse_xbox(status, &bytes, Duration::hours(12))
}

/// Xbox Live user token -> XSTS token for Minecraft services. A 401 carries `XErr`: the reason
/// this account can't use Xbox Live.
pub async fn xsts(
    http: &reqwest::Client,
    cfg: &AuthConfig,
    xbl: &XboxToken,
) -> Result<XboxToken, AuthError> {
    let req = http
        .post(cfg.endpoints.xsts.clone())
        .header("x-xbl-contract-version", "1")
        .json(&json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl.token.expose()] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }));
    let (status, bytes) = body(XBOX, send(XBOX, req).await?).await?;
    if status == StatusCode::UNAUTHORIZED
        && let Ok(XstsError { xerr: Some(code) }) = serde_json::from_slice(&bytes)
    {
        return Err(AuthError::Xbox(XboxProblem::from_xerr(code)));
    }
    if !status.is_success() {
        return Err(service_error(XBOX, status, &bytes));
    }
    let token = parse_xbox(status, &bytes, Duration::hours(8))?;
    // Prism checks this too: two different hashes mean the tokens belong to different users.
    if token.uhs != xbl.uhs {
        return Err(AuthError::Service {
            service: XBOX,
            status: status.as_u16(),
            detail: "the user hash changed between Xbox Live and XSTS".into(),
        });
    }
    Ok(token)
}

#[derive(Deserialize)]
struct LoginResponse {
    access_token: String,
    expires_in: Option<i64>,
}

/// XSTS token -> Minecraft access token. A 403 "Invalid app registration" means Mojang hasn't
/// allow-listed our client ID.
pub async fn minecraft_login(
    http: &reqwest::Client,
    cfg: &AuthConfig,
    xsts: &XboxToken,
) -> Result<MinecraftToken, AuthError> {
    let identity = format!("XBL3.0 x={};{}", xsts.uhs, xsts.token.expose());
    let req = http
        .post(cfg.endpoints.minecraft_login.clone())
        .json(&json!({ "identityToken": identity }));
    let (status, bytes) = body(MINECRAFT, send(MINECRAFT, req).await?).await?;
    if status == StatusCode::FORBIDDEN {
        let text = String::from_utf8_lossy(&bytes);
        if text.contains("Invalid app registration") || text.contains("AppRegInfo") {
            return Err(AuthError::ClientNotApproved);
        }
    }
    if !status.is_success() {
        return Err(service_error(MINECRAFT, status, &bytes));
    }
    let r: LoginResponse =
        serde_json::from_slice(&bytes).map_err(|_| unreadable(MINECRAFT, status))?;
    Ok(MinecraftToken {
        token: Secret::new(r.access_token),
        expires_at: Utc::now() + Duration::seconds(r.expires_in.unwrap_or(86400)),
    })
}

#[derive(Deserialize)]
struct EntitlementsResponse {
    #[serde(default)]
    items: Vec<EntitlementItem>,
}

#[derive(Deserialize)]
struct EntitlementItem {
    name: String,
}

pub async fn entitlements(
    http: &reqwest::Client,
    cfg: &AuthConfig,
    mc: &MinecraftToken,
) -> Result<Entitlements, AuthError> {
    let mut url = cfg.endpoints.entitlements.clone();
    url.query_pairs_mut()
        .append_pair("requestId", &Uuid::new_v4().to_string());
    let req = http.get(url).bearer_auth(mc.token.expose());
    let (status, bytes) = body(MINECRAFT, send(MINECRAFT, req).await?).await?;
    if !status.is_success() {
        return Err(service_error(MINECRAFT, status, &bytes));
    }
    let r: EntitlementsResponse =
        serde_json::from_slice(&bytes).map_err(|_| unreadable(MINECRAFT, status))?;
    Ok(Entitlements {
        items: r.items.into_iter().map(|i| i.name).collect(),
    })
}

#[derive(Deserialize)]
struct ProfileResponse {
    id: String,
    name: String,
    #[serde(default)]
    skins: Vec<SkinResponse>,
}

#[derive(Deserialize)]
struct SkinResponse {
    state: String,
    url: String,
    #[serde(default)]
    variant: Option<String>,
}

/// The profile, or `None` on 404 `NOT_FOUND` (the account has no Minecraft profile).
pub async fn profile(
    http: &reqwest::Client,
    cfg: &AuthConfig,
    mc: &MinecraftToken,
) -> Result<Option<Profile>, AuthError> {
    let req = http
        .get(cfg.endpoints.profile.clone())
        .bearer_auth(mc.token.expose());
    let (status, bytes) = body(MINECRAFT, send(MINECRAFT, req).await?).await?;
    if status == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(service_error(MINECRAFT, status, &bytes));
    }
    let r: ProfileResponse =
        serde_json::from_slice(&bytes).map_err(|_| unreadable(MINECRAFT, status))?;
    let uuid = Uuid::parse_str(&r.id).map_err(|_| unreadable(MINECRAFT, status))?;
    let skin = r
        .skins
        .into_iter()
        .find(|s| s.state.eq_ignore_ascii_case("ACTIVE"))
        .map(|s| Skin {
            url: match s.url.strip_prefix("http://") {
                Some(rest) => format!("https://{rest}"),
                None => s.url,
            },
            variant: s.variant.unwrap_or_else(|| "CLASSIC".into()),
        });
    Ok(Some(Profile {
        uuid,
        name: r.name,
        skin,
    }))
}

/// The `xuid` claim of a Minecraft access token (a JWT), for the game's `--xuid` (telemetry only).
/// The signature isn't checked: the value is informational and came straight from Mojang over TLS.
pub fn xuid_from_access_token(token: &Secret) -> Option<String> {
    let payload = token.expose().split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    match claims.get("xuid")? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xuid_comes_from_the_jwt_payload() {
        let payload = URL_SAFE_NO_PAD.encode(br#"{"xuid":"2535400000000001","sub":"x"}"#);
        let t = Secret::new(format!("eyJhbGciOiJIUzI1NiJ9.{payload}.sig"));
        assert_eq!(
            xuid_from_access_token(&t).as_deref(),
            Some("2535400000000001")
        );
        assert_eq!(xuid_from_access_token(&Secret::new("opaque")), None);
    }

    #[test]
    fn xbox_expiry_uses_the_stated_lifetime() {
        let body = br#"{"IssueInstant":"2020-01-01T00:00:00.0000000Z","NotAfter":"2020-01-15T00:00:00.0000000Z",
            "Token":"t","DisplayClaims":{"xui":[{"uhs":"123"}]}}"#;
        let t = parse_xbox(StatusCode::OK, body, Duration::hours(1)).unwrap();
        let left = t.expires_at - Utc::now();
        assert!(left > Duration::days(13) && left <= Duration::days(14));
        assert_eq!(t.uhs, "123");
        assert_eq!(t.xid, None);
    }

    #[test]
    fn entitlement_kinds() {
        let e = |items: &[&str]| Entitlements {
            items: items.iter().map(|s| s.to_string()).collect(),
        };
        assert!(e(&["product_minecraft", "game_minecraft"]).owns_game());
        assert!(e(&["product_game_pass_pc"]).game_pass());
        assert!(!e(&[]).owns_game() && !e(&[]).game_pass());
    }
}
