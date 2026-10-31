//! A local stand-in for login.microsoftonline.com, Xbox Live and Minecraft services, plus a fake
//! browser. It checks what the real services check (PKCE verifier against the challenge, redirect
//! URI, rotated refresh tokens, each token handed to the next step) and can be told to fail in
//! every way the research lists.
#![allow(dead_code)]

use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Instant,
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, SecondsFormat, Utc};
use reqwest::Url;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

pub const CLIENT_ID: &str = "11111111-2222-3333-4444-555555555555";
pub const UHS: &str = "1234567890123456";
pub const PROFILE_ID: &str = "069a79f444e94726a5befca90e38aaf5";

/// What the fake browser does with the authorize URL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserMode {
    /// Follows it: the mock redirects to the loopback with a code and the right state.
    Approve,
    /// The mock redirects with someone else's state.
    WrongState,
    /// Nothing happens (no browser, or the player uses the phone).
    Ignore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Poll {
    Pending,
    SlowDown,
    Approve,
    Expired,
    Declined,
}

pub struct State {
    pub browser: BrowserMode,
    pub device_code_error: Option<Value>,
    pub device_expires_in: u64,
    pub device_interval: u64,
    /// Answers to device code polls, in order; when empty, `Pending`.
    pub polls: VecDeque<Poll>,
    pub poll_times: Vec<Instant>,
    pub msa_expires_in: i64,
    pub xbl_lifetime_s: i64,
    pub xsts_lifetime_s: i64,
    pub mc_expires_in: i64,
    pub xsts_xerr: Option<u64>,
    pub mc_login_status: Option<(u16, &'static str)>,
    pub mc_login_retry_after: Option<&'static str>,
    pub profile_found: bool,
    /// The profile the mock answers with (another one makes a second account).
    pub profile_name: String,
    pub profile_id: String,
    pub entitlements: Vec<&'static str>,
    pub refresh_invalid: bool,
    /// An OAuth error body (HTTP 400) for every refresh-token request.
    pub refresh_error: Option<Value>,
    /// Drop every connection without an answer (the network is down).
    pub offline: bool,

    // What the mock handed out last, and what it saw.
    pub challenge: Option<String>,
    pub redirect_uri: Option<String>,
    pub issued_code: Option<String>,
    pub refresh_token: Option<String>,
    pub msa_access: Option<String>,
    pub xbl: Option<String>,
    pub xsts: Option<String>,
    pub mc: Option<String>,
    pub counter: u64,
    pub calls: HashMap<&'static str, u32>,
    /// Every token the mock issued, to scan files for leaks.
    pub all_tokens: Vec<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            browser: BrowserMode::Approve,
            device_code_error: None,
            device_expires_in: 900,
            device_interval: 1,
            polls: VecDeque::new(),
            poll_times: Vec::new(),
            msa_expires_in: 3600,
            xbl_lifetime_s: 14 * 86400,
            xsts_lifetime_s: 16 * 3600,
            mc_expires_in: 86400,
            xsts_xerr: None,
            mc_login_status: None,
            mc_login_retry_after: None,
            profile_found: true,
            profile_name: "Violet_Runner".into(),
            profile_id: PROFILE_ID.into(),
            entitlements: vec!["product_minecraft", "game_minecraft"],
            refresh_invalid: false,
            refresh_error: None,
            offline: false,
            challenge: None,
            redirect_uri: None,
            issued_code: None,
            refresh_token: None,
            msa_access: None,
            xbl: None,
            xsts: None,
            mc: None,
            counter: 0,
            calls: HashMap::new(),
            all_tokens: Vec::new(),
        }
    }
}

impl State {
    fn token(&mut self, kind: &str) -> String {
        self.counter += 1;
        let t = format!("{kind}-SECRET-{}", self.counter);
        self.all_tokens.push(t.clone());
        t
    }

    fn count(&mut self, what: &'static str) {
        *self.calls.entry(what).or_default() += 1;
    }

    pub fn calls(&self, what: &'static str) -> u32 {
        self.calls.get(what).copied().unwrap_or(0)
    }
}

pub struct Mock {
    pub addr: SocketAddr,
    pub state: Arc<Mutex<State>>,
}

struct Request {
    method: String,
    url: Url,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Request {
    fn form(&self) -> HashMap<String, String> {
        url::form_urlencoded_parse(&self.body)
    }
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
    fn bearer(&self) -> Option<&str> {
        self.headers
            .get("authorization")
            .and_then(|v| v.strip_prefix("Bearer "))
    }
}

mod url {
    use std::collections::HashMap;
    pub fn form_urlencoded_parse(body: &[u8]) -> HashMap<String, String> {
        reqwest::Url::parse(&format!("http://x/?{}", String::from_utf8_lossy(body)))
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect()
    }
}

struct Response {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: String,
}

fn json_resp(status: u16, v: Value) -> Response {
    Response {
        status,
        headers: vec![("Content-Type", "application/json".into())],
        body: v.to_string(),
    }
}

fn oauth_error(error: &str, codes: &[u64]) -> Response {
    json_resp(
        400,
        json!({"error": error, "error_description": format!("{error} (mock)"), "error_codes": codes}),
    )
}

impl Mock {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let st = state.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    continue;
                };
                let st = st.clone();
                tokio::spawn(async move {
                    let Some(req) = read_request(&mut stream, addr).await else {
                        return;
                    };
                    {
                        let mut s = st.lock().unwrap();
                        if s.offline {
                            s.count("dropped");
                            return; // drop the connection: no answer
                        }
                    }
                    let resp = handle(&st, &req);
                    let mut head = format!(
                        "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
                        resp.status,
                        resp.body.len()
                    );
                    for (k, v) in &resp.headers {
                        head.push_str(&format!("{k}: {v}\r\n"));
                    }
                    head.push_str("\r\n");
                    let _ = stream.write_all(head.as_bytes()).await;
                    let _ = stream.write_all(resp.body.as_bytes()).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        Self { addr, state }
    }

    pub fn base(&self) -> Url {
        Url::parse(&format!("http://{}/", self.addr)).unwrap()
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        f(&mut self.state.lock().unwrap())
    }

    /// An `AuthConfig` against this mock, with a fake browser and fast device polling.
    pub fn config(&self) -> srl_auth::AuthConfig {
        let mut cfg = srl_auth::AuthConfig::new(CLIENT_ID).unwrap();
        cfg.endpoints = srl_auth::Endpoints::under(&self.base());
        cfg.poll_unit = std::time::Duration::from_millis(20);
        cfg.browser_timeout = std::time::Duration::from_secs(3);
        let st = self.state.clone();
        cfg.open_browser = Arc::new(move |url: &str| {
            let mode = st.lock().unwrap().browser;
            if mode == BrowserMode::Ignore {
                return Ok(());
            }
            // The browser: GET the authorize page, follow the redirect to the loopback.
            let url = url.to_string();
            tokio::spawn(async move {
                let _ = reqwest::get(url).await;
            });
            Ok(())
        });
        cfg
    }
}

async fn read_request(stream: &mut tokio::net::TcpStream, addr: SocketAddr) -> Option<Request> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut req = httparse::Request::new(&mut headers);
        if let Ok(httparse::Status::Complete(head_len)) = req.parse(&buf) {
            let hs: HashMap<String, String> = req
                .headers
                .iter()
                .map(|h| {
                    (
                        h.name.to_ascii_lowercase(),
                        String::from_utf8_lossy(h.value).into_owned(),
                    )
                })
                .collect();
            let len: usize = hs
                .get("content-length")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let method = req.method?.to_string();
            let path = req.path?.to_string();
            while buf.len() < head_len + len {
                let n = stream.read(&mut chunk).await.ok()?;
                if n == 0 {
                    return None;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            return Some(Request {
                method,
                url: Url::parse(&format!("http://{addr}{path}")).unwrap(),
                headers: hs,
                body: buf[head_len..head_len + len].to_vec(),
            });
        }
    }
}

fn xbox_body(token: &str, lifetime_s: i64) -> Value {
    let now = Utc::now();
    json!({
        "IssueInstant": now.to_rfc3339_opts(SecondsFormat::Micros, true),
        "NotAfter": (now + Duration::seconds(lifetime_s)).to_rfc3339_opts(SecondsFormat::Micros, true),
        "Token": token,
        "DisplayClaims": {"xui": [{"uhs": UHS}]},
    })
}

fn msa_tokens(s: &mut State) -> Response {
    let access = s.token("MSA");
    let refresh = s.token("REFRESH");
    s.msa_access = Some(access.clone());
    s.refresh_token = Some(refresh.clone());
    json_resp(
        200,
        json!({"token_type": "Bearer", "scope": "XboxLive.signin offline_access",
               "expires_in": s.msa_expires_in, "access_token": access, "refresh_token": refresh}),
    )
}

fn handle(st: &Arc<Mutex<State>>, req: &Request) -> Response {
    let mut s = st.lock().unwrap();
    let q: HashMap<String, String> = req.url.query_pairs().into_owned().collect();
    match (req.method.as_str(), req.url.path()) {
        ("GET", "/consumers/oauth2/v2.0/authorize") => {
            s.count("authorize");
            assert_eq!(q["client_id"], CLIENT_ID);
            assert_eq!(q["code_challenge_method"], "S256");
            s.challenge = Some(q["code_challenge"].clone());
            s.redirect_uri = Some(q["redirect_uri"].clone());
            if s.browser == BrowserMode::Ignore {
                // A sign-in page nobody finishes (an opener that does fetch the link, like the
                // app's against this mock, gets here).
                return Response {
                    status: 200,
                    headers: vec![("Content-Type", "text/html".into())],
                    body: "<p>mock sign-in page</p>".into(),
                };
            }
            let code = format!("CODE-{}", s.counter);
            s.issued_code = Some(code.clone());
            let state = if s.browser == BrowserMode::WrongState {
                "someone-elses-state".to_string()
            } else {
                q["state"].clone()
            };
            let mut loc = Url::parse(&q["redirect_uri"]).unwrap();
            // The listener binds 127.0.0.1; the test machine resolves localhost either way, so
            // use the literal address to keep the test independent of /etc/hosts.
            loc.set_host(Some("127.0.0.1")).unwrap();
            loc.query_pairs_mut()
                .append_pair("code", &code)
                .append_pair("state", &state);
            Response {
                status: 302,
                headers: vec![("Location", loc.to_string())],
                body: String::new(),
            }
        }
        ("POST", "/consumers/oauth2/v2.0/devicecode") => {
            s.count("devicecode");
            if let Some(err) = s.device_code_error.clone() {
                return json_resp(400, err);
            }
            let f = req.form();
            assert_eq!(f["client_id"], CLIENT_ID);
            assert_eq!(f["scope"], "XboxLive.signin offline_access");
            json_resp(
                200,
                json!({"user_code": "ABCD1234", "device_code": "DEVICE-SECRET",
                       "verification_uri": "https://www.microsoft.com/link",
                       "expires_in": s.device_expires_in, "interval": s.device_interval,
                       "message": "mock"}),
            )
        }
        ("POST", "/consumers/oauth2/v2.0/token") => {
            let f = req.form();
            assert_eq!(f["client_id"], CLIENT_ID);
            match f["grant_type"].as_str() {
                "authorization_code" => {
                    s.count("token:code");
                    let verifier = &f["code_verifier"];
                    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
                    if Some(&challenge) != s.challenge.as_ref()
                        || Some(&f["code"]) != s.issued_code.as_ref()
                        || Some(&f["redirect_uri"]) != s.redirect_uri.as_ref()
                    {
                        return oauth_error("invalid_grant", &[70000]);
                    }
                    s.issued_code = None; // single use
                    msa_tokens(&mut s)
                }
                "urn:ietf:params:oauth:grant-type:device_code" => {
                    s.count("token:device");
                    s.poll_times.push(Instant::now());
                    assert_eq!(f["device_code"], "DEVICE-SECRET");
                    match s.polls.pop_front().unwrap_or(Poll::Pending) {
                        Poll::Pending => oauth_error("authorization_pending", &[70016]),
                        Poll::SlowDown => oauth_error("slow_down", &[]),
                        Poll::Expired => oauth_error("expired_token", &[70020]),
                        Poll::Declined => oauth_error("authorization_declined", &[70019]),
                        Poll::Approve => msa_tokens(&mut s),
                    }
                }
                "refresh_token" => {
                    s.count("token:refresh");
                    if let Some(err) = s.refresh_error.clone() {
                        return json_resp(400, err);
                    }
                    if s.refresh_invalid || Some(&f["refresh_token"]) != s.refresh_token.as_ref() {
                        return oauth_error("invalid_grant", &[70008]);
                    }
                    msa_tokens(&mut s)
                }
                other => panic!("unexpected grant {other}"),
            }
        }
        ("POST", "/user/authenticate") => {
            s.count("xbl");
            assert_eq!(
                req.headers
                    .get("x-xbl-contract-version")
                    .map(String::as_str),
                Some("1")
            );
            let b = req.json();
            let want = format!("d={}", s.msa_access.clone().unwrap_or_default());
            if b["Properties"]["RpsTicket"] != want.as_str()
                || b["RelyingParty"] != "http://auth.xboxlive.com"
            {
                return Response {
                    status: 400,
                    headers: vec![],
                    body: String::new(),
                };
            }
            let t = s.token("XBL");
            s.xbl = Some(t.clone());
            json_resp(200, xbox_body(&t, s.xbl_lifetime_s))
        }
        ("POST", "/xsts/authorize") => {
            s.count("xsts");
            let b = req.json();
            if b["Properties"]["UserTokens"][0] != s.xbl.clone().unwrap_or_default().as_str()
                || b["RelyingParty"] != "rp://api.minecraftservices.com/"
            {
                return Response {
                    status: 400,
                    headers: vec![],
                    body: String::new(),
                };
            }
            if let Some(code) = s.xsts_xerr {
                return json_resp(
                    401,
                    json!({"Identity": "0", "XErr": code, "Message": "",
                           "Redirect": "https://start.ui.xboxlive.com/CreateAccount"}),
                );
            }
            let t = s.token("XSTS");
            s.xsts = Some(t.clone());
            json_resp(200, xbox_body(&t, s.xsts_lifetime_s))
        }
        ("POST", "/authentication/login_with_xbox") => {
            s.count("mc_login");
            if let Some((status, body)) = s.mc_login_status {
                let mut r = Response {
                    status,
                    headers: vec![],
                    body: body.to_string(),
                };
                if let Some(ra) = s.mc_login_retry_after {
                    r.headers.push(("Retry-After", ra.to_string()));
                }
                return r;
            }
            let want = format!("XBL3.0 x={UHS};{}", s.xsts.clone().unwrap_or_default());
            if req.json()["identityToken"] != want.as_str() {
                return Response {
                    status: 401,
                    headers: vec![],
                    body: String::new(),
                };
            }
            s.counter += 1;
            let claims = URL_SAFE_NO_PAD
                .encode(json!({"xuid": "2535400000000001", "n": s.counter}).to_string());
            let t = format!("eyJhbGciOiJIUzI1NiJ9.{claims}.MC-SECRET-{}", s.counter);
            s.all_tokens.push(t.clone());
            s.mc = Some(t.clone());
            json_resp(
                200,
                json!({"username": "some-uuid", "roles": [], "access_token": t,
                       "token_type": "Bearer", "expires_in": s.mc_expires_in}),
            )
        }
        ("GET", "/entitlements/license") => {
            s.count("entitlements");
            assert!(q.contains_key("requestId"));
            if req.bearer() != s.mc.as_deref() {
                return Response {
                    status: 401,
                    headers: vec![],
                    body: String::new(),
                };
            }
            let items: Vec<Value> = s
                .entitlements
                .iter()
                .map(|n| json!({"name": n, "source": "PURCHASE"}))
                .collect();
            json_resp(200, json!({"items": items, "signature": "x", "keyId": "1"}))
        }
        ("GET", "/minecraft/profile") => {
            s.count("profile");
            if req.bearer() != s.mc.as_deref() {
                return Response {
                    status: 401,
                    headers: vec![],
                    body: String::new(),
                };
            }
            if !s.profile_found {
                return json_resp(
                    404,
                    json!({"path": "/minecraft/profile", "errorType": "NOT_FOUND", "error": "NOT_FOUND"}),
                );
            }
            json_resp(
                200,
                json!({"id": s.profile_id, "name": s.profile_name,
                       "skins": [{"id": "s", "state": "ACTIVE",
                                  "url": "http://textures.minecraft.net/texture/abc", "variant": "SLIM"}],
                       "capes": []}),
            )
        }
        (m, p) => panic!("mock: unexpected {m} {p}"),
    }
}
