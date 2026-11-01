//! The Microsoft account extension of the facade: srl-auth for Settings (the accounts, the login
//! dialog) and for Play (who the game plays as). Like `tools.rs`: the UI reads
//! `Accounts::snapshot` once, then applies `Event::Account` (the whole new snapshot each time),
//! and asks for work through the trait. Every call returns at once; srl-auth runs on the backend's
//! tokio runtime.
//!
//! - **Off without a client ID** (`AuthConfig::detect()` is `None`, the shipped build today):
//!   `AccountSnapshot::available` is false, Settings says so in the player name's hint, Play is
//!   offline.
//! - **Who plays** is decided in one place, `play_as` (Piotr, 2026-10-26): the active Microsoft
//!   account when there is one, else the offline name, for every mode. A build that can log in
//!   opens offline play only after someone has signed in here once.
//! - **Play** calls `launch_account` before the game starts: `prepare_launch` (the "Signing in"
//!   stage), then `mark_in_use` for as long as the game runs.
//! - **Faces** come from the skin (`skin.url`, textures.minecraft.net only): the 8x8 face at (8,8)
//!   with the hat layer at (40,8) over it, cached in `meta/skins/`.
//!
//! `RealAccounts` is what `RealBackend` serves; `FakeAccounts` has fixed data and a simulated login
//! for screenshots (`SRL_FAKE`, the `account*`, `login*` and `msa-*` screen ids).
use std::{
    collections::HashMap,
    fmt,
    io::Cursor,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use chrono::{DateTime, Utc};
use srl_auth::{
    AccountInfo, Auth, AuthConfig, AuthError, Endpoints, InUse, KeystoreKind, LoginEvent,
    LoginStep, MinecraftSession, QrMatrix, XboxProblem,
};
use srl_core::{OfflineAccount, Paths};
use srl_game::{GameAccount, OnlineSession};
use uuid::Uuid;

use super::{Event, Events, real::log};

/// An `AuthError` as the UI says it: the copy key (`AuthError::copy_key`, texts merged from
/// srl-auth's `copy/*.json`), the XErr number for `{code}`, a page that helps, and what kind of
/// trouble it is. `english` is the `Display` text, for the launcher log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountProblem {
    pub key: &'static str,
    pub code: Option<u64>,
    pub help_url: Option<&'static str>,
    /// The saved sign-in is gone for good: the player must log in again.
    pub relogin: bool,
    /// The keyring is locked or doesn't answer: unlock it and try again (not a lost sign-in).
    pub keystore: bool,
    pub english: String,
}

impl AccountProblem {
    pub fn of(e: &AuthError) -> Self {
        Self {
            key: e.copy_key(),
            code: match e {
                AuthError::Xbox(XboxProblem::Other(c)) => Some(*c),
                _ => None,
            },
            help_url: e.help_url(),
            relogin: e.needs_relogin(),
            keystore: matches!(e, AuthError::Keystore(_)),
            english: e.to_string(),
        }
    }
}

impl AccountProblem {
    /// Play in a build that can log in, before anyone has signed in here (`PlayAs::SignInFirst`).
    pub fn sign_in_first() -> Self {
        Self {
            key: SIGN_IN_FIRST,
            code: None,
            help_url: None,
            relogin: true,
            keystore: false,
            english: "nobody has signed in with Microsoft here yet, so offline play is closed"
                .into(),
        }
    }
}

impl fmt::Display for AccountProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.english)
    }
}

/// An account's face: 8x8 pixels, row by row, `0xRRGGBBAA`.
#[derive(Clone, PartialEq, Eq)]
pub struct Face(pub [u32; 64]);

impl fmt::Debug for Face {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Face")
    }
}

/// One account as the Settings list shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct AccountView {
    pub uuid: Uuid,
    pub name: String,
    /// `None` until the skin is fetched, or when the account has none (the default skin): the UI
    /// draws the name's initial instead.
    pub face: Option<Arc<Face>>,
    /// The saved sign-in can't be renewed: a "Log in again" badge.
    pub needs_login: bool,
}

/// The browser path of a login in progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserPath {
    pub url: String,
    /// `false`: no browser could be started; the dialog shows the link to copy.
    pub opened: bool,
}

/// The device code path of a login in progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevicePath {
    pub user_code: String,
    pub verification_uri: String,
    /// The QR code of the link that fills in the code (`qr_matrix`), `None` if it can't be made.
    pub qr: Option<Arc<QrMatrix>>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LoginOutcome {
    Waiting,
    /// Logged in; the account is saved and active.
    Done(AccountView),
    Failed(AccountProblem),
}

/// The login dialog, from `LoginEvent`s.
#[derive(Clone, Debug, PartialEq)]
pub struct LoginView {
    pub browser: Option<BrowserPath>,
    pub browser_failed: Option<AccountProblem>,
    pub device: Option<DevicePath>,
    /// The phone path can't be used: its panel hides.
    pub device_failed: Option<AccountProblem>,
    /// The last step of the chain after the Microsoft sign-in.
    pub step: Option<LoginStep>,
    pub outcome: LoginOutcome,
}

impl Default for LoginView {
    fn default() -> Self {
        Self {
            browser: None,
            browser_failed: None,
            device: None,
            device_failed: None,
            step: None,
            outcome: LoginOutcome::Waiting,
        }
    }
}

impl LoginView {
    fn apply(&mut self, e: LoginEvent) {
        match e {
            LoginEvent::Browser { url, opened } => self.browser = Some(BrowserPath { url, opened }),
            LoginEvent::DeviceCode {
                user_code,
                verification_uri,
                qr_url,
                expires_at,
            } => {
                self.device = Some(DevicePath {
                    user_code,
                    verification_uri,
                    qr: srl_auth::qr_matrix(&qr_url).map(Arc::new),
                    expires_at,
                })
            }
            LoginEvent::DeviceCodeFailed(e) => self.device_failed = Some(AccountProblem::of(&e)),
            LoginEvent::BrowserFailed(e) => self.browser_failed = Some(AccountProblem::of(&e)),
            LoginEvent::Step(s) => self.step = Some(s),
        }
    }
}

/// Everything the UI shows about Microsoft accounts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccountSnapshot {
    /// This build can log in (it has a client ID). Without it Settings shows one line and Play is
    /// offline, whatever `accounts.json` holds.
    pub available: bool,
    pub accounts: Vec<AccountView>,
    pub active: Option<Uuid>,
    /// Someone has signed in here once (`AccountList::signed_in_once`): in a build that can log
    /// in, offline play is open only after that (`play_as`).
    pub signed_in_once: bool,
    /// The token key is in a file, not the system keyring: a quiet note.
    pub keystore_file: bool,
    /// The last account action that failed (reading the list, log out, switching), shown under
    /// the list until the next action.
    pub problem: Option<AccountProblem>,
    /// The login dialog, while it is open.
    pub login: Option<LoginView>,
}

impl AccountSnapshot {
    /// The account Play uses, if any (`play_as`): the active one, in a build that can log in.
    pub fn playing(&self) -> Option<&AccountView> {
        let active = self.accounts.iter().find(|a| Some(a.uuid) == self.active);
        match play_as(
            self.available,
            self.signed_in_once,
            active.map(|a| (a.uuid, a.needs_login)),
        ) {
            PlayAs::Offline | PlayAs::SignInFirst => None,
            PlayAs::Microsoft(_) | PlayAs::Relogin(_) => active,
        }
    }

    /// The name of the Microsoft account Play uses, if any. The stats stream defaults the MCSR
    /// Ranked name to it.
    pub fn active_name(&self) -> Option<&str> {
        self.playing().map(|a| a.name.as_str())
    }
}

/// Who a Play starts the game as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayAs {
    /// The offline name from Settings.
    Offline,
    /// The active Microsoft account.
    Microsoft(Uuid),
    /// The active Microsoft account, whose sign-in has to be done again first: Play stops and
    /// the login opens.
    Relogin(Uuid),
    /// Offline, but nobody has signed in here yet in a build that can log in: Play stops and the
    /// login opens (`SIGN_IN_FIRST`).
    SignInFirst,
}

/// The copy key of the `AccountProblem` Play stops with on `PlayAs::SignInFirst`.
pub const SIGN_IN_FIRST: &str = "account.sign_in_first";

/// **The offline policy, in one place** (Piotr, 2026-10-26, Prism's rule):
/// - A build without Microsoft login (no client ID, every build until Mojang approves ours)
///   plays offline with the name from Settings, whatever `accounts.json` holds.
/// - A build with it plays offline only after someone has signed in here once with an account
///   that owns the game (`signed_in_once`; logging out later doesn't undo it). Before that, Play
///   asks for a sign-in: Mojang doesn't approve launchers that let anyone play offline.
/// - With a Microsoft account active, every mode plays as it.
///
/// With no account active, Ranked and Draftout start offline too (their online parts then need a
/// Microsoft account, which Home says).
pub fn play_as(available: bool, signed_in_once: bool, active: Option<(Uuid, bool)>) -> PlayAs {
    match active {
        _ if !available => PlayAs::Offline,
        Some((uuid, true)) => PlayAs::Relogin(uuid),
        Some((uuid, false)) => PlayAs::Microsoft(uuid),
        None if signed_in_once => PlayAs::Offline,
        None => PlayAs::SignInFirst,
    }
}

/// The account a game starts with, from `launch_account`.
pub struct LaunchAccount {
    pub account: GameAccount,
    /// Held for as long as the game runs: srl-auth's background refresh leaves the account alone
    /// (a new token wouldn't reach the running game).
    pub in_use: Option<InUse>,
    /// The sign-in couldn't be renewed for a passing reason; the saved token works until then.
    pub valid_until: Option<DateTime<Utc>>,
    /// Why it couldn't be renewed, for the log.
    pub warning: Option<AccountProblem>,
}

impl LaunchAccount {
    fn offline(account: OfflineAccount) -> Self {
        Self {
            account: GameAccount::Offline(account),
            in_use: None,
            valid_until: None,
            warning: None,
        }
    }
}

/// The game's account for a Play, shared by the window and the command line: `offline` unless
/// `play_as` says Microsoft, then `prepare_launch` (after `signing_in`, which shows the stage).
/// Errors are the ones that stop the start: no sign-in here yet (`SIGN_IN_FIRST`), a sign-in to do
/// again, a locked keyring, or a sign-in that can't be renewed and has expired.
pub async fn launch_account(
    auth: &Auth,
    offline: OfflineAccount,
    signing_in: impl FnOnce(),
) -> Result<LaunchAccount, AccountProblem> {
    if !auth.is_available() {
        return Ok(LaunchAccount::offline(offline));
    }
    let list = match auth.accounts().await {
        Ok(l) => l,
        Err(e) => {
            // A damaged list doesn't stop Play: nobody is active, and whoever broke it had one.
            log(format_args!(
                "accounts: can't read the list ({e}); playing offline"
            ));
            return Ok(LaunchAccount::offline(offline));
        }
    };
    let active = list
        .accounts
        .iter()
        .find(|a| Some(a.uuid) == list.active)
        .map(|a| (a.uuid, a.needs_login.is_some()));
    let uuid = match play_as(true, list.signed_in_once, active) {
        PlayAs::Offline => return Ok(LaunchAccount::offline(offline)),
        PlayAs::SignInFirst => return Err(AccountProblem::sign_in_first()),
        PlayAs::Relogin(_) => {
            return Err(AccountProblem::of(&AuthError::ReloginRequired(
                "the account is marked for a new login".into(),
            )));
        }
        PlayAs::Microsoft(uuid) => uuid,
    };
    signing_in();
    let ready = auth
        .prepare_launch(uuid)
        .await
        .map_err(|e| AccountProblem::of(&e))?;
    let warning = ready.warning.as_ref().map(AccountProblem::of);
    Ok(LaunchAccount {
        account: GameAccount::Microsoft(online_session(&ready.session)),
        in_use: Some(auth.mark_in_use(uuid)),
        valid_until: warning.is_some().then_some(ready.session.expires_at),
        warning,
    })
}

/// srl-auth's session as srl-game takes it: the placeholders exactly as `placeholders()` gives
/// them (srl-game does not depend on srl-auth).
pub fn online_session(s: &MinecraftSession) -> OnlineSession {
    OnlineSession {
        name: s.name.clone(),
        uuid: s.uuid,
        placeholders: s
            .placeholders()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    }
}

/// `SRL_MSA_BASE_URL`: every Microsoft, Xbox and Minecraft URL under this base, for srl-auth's
/// mock (`examples/mock-services.rs`). Loopback only, so a sign-in can never be sent to another
/// machine. The "browser" is then a plain HTTP GET of the sign-in link (which the mock answers),
/// never the system browser: the tests run in Xvfb and a real browser would open on the desktop.
/// `SRL_MSA_NO_BROWSER=1` makes it fail, to show the "copy this link" state.
pub const BASE_URL_ENV: &str = "SRL_MSA_BASE_URL";

/// `AuthConfig::detect()` with the dev switch above applied.
pub fn auth_config() -> Option<AuthConfig> {
    let mut cfg = AuthConfig::detect()?;
    let Ok(base) = std::env::var(BASE_URL_ENV) else {
        return Some(cfg);
    };
    let base = match reqwest::Url::parse(base.trim()) {
        Ok(u) if is_loopback(&u) => u,
        _ => {
            log(format_args!(
                "{BASE_URL_ENV}: not a loopback http URL, ignored"
            ));
            return Some(cfg);
        }
    };
    log(format_args!(
        "accounts: Microsoft login against {base} (test services)"
    ));
    cfg.endpoints = Endpoints::under(&base);
    let no_browser = std::env::var("SRL_MSA_NO_BROWSER").is_ok_and(|v| v.trim() == "1");
    cfg.open_browser = Arc::new(move |url: &str| {
        if no_browser {
            return Err(std::io::Error::other("SRL_MSA_NO_BROWSER"));
        }
        let url = url.to_string();
        tokio::spawn(async move {
            let _ = reqwest::get(url).await;
        });
        Ok(())
    });
    Some(cfg)
}

fn is_loopback(u: &reqwest::Url) -> bool {
    u.scheme() == "http"
        && match u.host_str() {
            Some("localhost") => true,
            Some(h) => h
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback()),
            None => false,
        }
}

/// The Microsoft account side of the facade. Like `Backend`: methods return at once, changes
/// arrive as `Event::Account`, and idle sends nothing.
pub trait Accounts {
    /// Cheap: no I/O.
    fn snapshot(&self) -> AccountSnapshot;
    /// Opens the login dialog and starts the login (browser and phone at once). Does nothing
    /// while one runs or without login in this build.
    fn login(&self);
    /// Cancel: ends the login where it is (the browser listener closes, the polling stops) and
    /// closes the dialog.
    fn cancel_login(&self);
    /// Closes the dialog after the login ended.
    fn close_login(&self);
    /// Logs the account out: its tokens are deleted, then the account.
    fn logout(&self, uuid: Uuid);
    /// Play uses this account from now on.
    fn set_active(&self, uuid: Uuid);
    /// Renews the accounts' sign-ins in the background, once, after the first paint.
    fn refresh_in_background(&self);
}

/// Sends the snapshot when `f` changed it.
fn update(state: &Mutex<AccountSnapshot>, events: &Events, f: impl FnOnce(&mut AccountSnapshot)) {
    let mut s = state.lock().unwrap();
    let before = s.clone();
    f(&mut s);
    if *s != before {
        let _ = events.unbounded_send(Event::Account(Box::new(s.clone())));
    }
}

/// Cheap to clone (one `Arc`): the practice game signs in through the same accounts.
#[derive(Clone)]
pub struct RealAccounts {
    inner: Arc<Inner>,
}

struct Inner {
    auth: Auth,
    rt: tokio::runtime::Handle,
    events: Events,
    state: Mutex<AccountSnapshot>,
    /// The login running now: its number and its task.
    login: Mutex<Option<(u64, tokio::task::JoinHandle<()>)>>,
    /// Numbers the logins, so the events of a cancelled one are dropped.
    login_gen: AtomicU64,
    /// Faces by skin URL. `None`: fetched, but there is none to show.
    faces: Mutex<HashMap<String, Option<Arc<Face>>>>,
    /// The skin URL each saved account wears, from the last list.
    skin_urls: Mutex<HashMap<Uuid, String>>,
    skins_dir: PathBuf,
    http: reqwest::Client,
}

impl RealAccounts {
    /// Reads `accounts.json` (a small file, no network) and the cached faces, so the first frame
    /// already names the right account; missing faces are fetched in the background.
    pub fn new(auth: Auth, paths: &Paths, events: Events, rt: tokio::runtime::Handle) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(srl_core::USER_AGENT)
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();
        let inner = Arc::new(Inner {
            state: Mutex::new(AccountSnapshot {
                available: auth.is_available(),
                ..AccountSnapshot::default()
            }),
            auth,
            rt,
            events,
            login: Mutex::new(None),
            login_gen: AtomicU64::new(0),
            faces: Mutex::new(HashMap::new()),
            skin_urls: Mutex::new(HashMap::new()),
            skins_dir: paths.meta().join("skins"),
            http,
        });
        let list = inner.rt.block_on(inner.auth.accounts());
        inner.apply_list(list);
        let s = inner.state.lock().unwrap().clone();
        log(format_args!(
            "accounts: Microsoft login {}; {} saved, active {}",
            if s.available {
                "available"
            } else {
                "off (no client ID in this build)"
            },
            s.accounts.len(),
            s.playing()
                .map_or("none (offline)".to_string(), |a| a.name.clone()),
        ));
        Self { inner }
    }

    /// The `Auth` handle, for Play.
    pub fn auth(&self) -> &Auth {
        &self.inner.auth
    }
}

impl Inner {
    fn update(&self, f: impl FnOnce(&mut AccountSnapshot)) {
        update(&self.state, &self.events, f);
    }

    /// Puts a fresh list into the snapshot, with the faces known so far; fetches the others.
    fn apply_list(self: &Arc<Self>, list: Result<srl_auth::AccountList, AuthError>) {
        let list = match list {
            Ok(l) => l,
            Err(e) => {
                log(format_args!("accounts: {e}"));
                self.update(|s| s.problem = Some(AccountProblem::of(&e)));
                return;
            }
        };
        *self.skin_urls.lock().unwrap() = list
            .accounts
            .iter()
            .filter_map(|a| Some((a.uuid, a.skin.as_ref()?.url.clone())))
            .collect();
        let views: Vec<AccountView> = list.accounts.iter().map(|a| self.view(a)).collect();
        self.update(|s| {
            s.accounts = views;
            s.active = list.active;
            s.signed_in_once = list.signed_in_once;
            s.keystore_file = list.keystore == Some(KeystoreKind::File);
        });
        // A build without login shows no accounts, so it has no faces to fetch either.
        if !self.auth.is_available() {
            return;
        }
        for a in &list.accounts {
            if let Some(url) = a.skin.as_ref().map(|s| s.url.clone())
                && !self.faces.lock().unwrap().contains_key(&url)
            {
                let me = self.clone();
                self.rt.spawn(async move { me.fetch_face(url).await });
            }
        }
    }

    fn view(&self, a: &AccountInfo) -> AccountView {
        let face = a.skin.as_ref().and_then(|s| {
            let mut faces = self.faces.lock().unwrap();
            match faces.get(&s.url) {
                Some(f) => f.clone(),
                None => {
                    // The cached skin, if an earlier start fetched it (a few KB read).
                    let f = skin_file(&self.skins_dir, &s.url)
                        .and_then(|p| std::fs::read(p).ok())
                        .and_then(|b| face_from_skin(&b))
                        .map(Arc::new);
                    if f.is_some() {
                        faces.insert(s.url.clone(), f.clone());
                    }
                    f
                }
            }
        });
        AccountView {
            uuid: a.uuid,
            name: a.name.clone(),
            face,
            needs_login: a.needs_login.is_some(),
        }
    }

    async fn fetch_face(self: Arc<Self>, url: String) {
        let face = match self.download_skin(&url).await {
            Ok(Some(bytes)) => face_from_skin(&bytes).map(Arc::new),
            Ok(None) => None,
            Err(e) => {
                log(format_args!("accounts: no skin from {url}: {e:#}"));
                None
            }
        };
        self.faces.lock().unwrap().insert(url.clone(), face.clone());
        let Some(face) = face else { return };
        let wearing: Vec<Uuid> = self
            .skin_urls
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, u)| **u == url)
            .map(|(id, _)| *id)
            .collect();
        self.update(|s| {
            for v in s.accounts.iter_mut().filter(|v| wearing.contains(&v.uuid)) {
                v.face = Some(face.clone());
            }
            if let Some(LoginView {
                outcome: LoginOutcome::Done(v),
                ..
            }) = s.login.as_mut()
                && wearing.contains(&v.uuid)
            {
                v.face = Some(face.clone());
            }
        });
    }

    /// The skin PNG, from the cache or Mojang's texture server (then cached). `None` for a URL
    /// on any other host.
    async fn download_skin(&self, url: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let (Some(id), Some(file)) = (skin_id(url), skin_file(&self.skins_dir, url)) else {
            return Ok(None);
        };
        if let Ok(b) = tokio::fs::read(&file).await {
            return Ok(Some(b));
        }
        // Against the test services the skins are made up: asking Mojang for them only fails.
        if std::env::var_os(BASE_URL_ENV).is_some() {
            return Ok(None);
        }
        let https = format!("https://textures.minecraft.net/texture/{id}");
        let resp = self.http.get(&https).send().await?.error_for_status()?;
        let bytes = resp.bytes().await?;
        if bytes.len() > 256 * 1024 {
            anyhow::bail!("{} bytes is too big for a skin", bytes.len());
        }
        tokio::fs::create_dir_all(&self.skins_dir).await?;
        let part = file.with_extension("png.part");
        tokio::fs::write(&part, &bytes).await?;
        tokio::fs::rename(&part, &file).await?;
        Ok(Some(bytes.to_vec()))
    }

    async fn reload(self: &Arc<Self>) {
        let list = self.auth.accounts().await;
        self.apply_list(list);
    }

    /// Runs an account action on the runtime; a failure shows under the list.
    fn act<F>(self: &Arc<Self>, what: &'static str, f: impl FnOnce(Auth) -> F + Send + 'static)
    where
        F: std::future::Future<Output = Result<(), AuthError>> + Send + 'static,
    {
        let me = self.clone();
        self.update(|s| s.problem = None);
        self.rt.spawn(async move {
            let r = f(me.auth.clone()).await;
            if let Err(e) = &r {
                log(format_args!("accounts: {what} failed: {e}"));
                me.update(|s| s.problem = Some(AccountProblem::of(e)));
            }
            me.reload().await;
        });
    }
}

impl Accounts for RealAccounts {
    fn snapshot(&self) -> AccountSnapshot {
        self.inner.state.lock().unwrap().clone()
    }

    fn login(&self) {
        let inner = &self.inner;
        if !inner.auth.is_available() {
            return;
        }
        let mut slot = inner.login.lock().unwrap();
        if slot.as_ref().is_some_and(|(_, task)| !task.is_finished()) {
            return;
        }
        let id = inner.login_gen.fetch_add(1, Ordering::SeqCst) + 1;
        log(format_args!("accounts: login started"));
        inner.update(|s| {
            s.login = Some(LoginView::default());
            s.problem = None;
        });
        let me = inner.clone();
        let events: srl_auth::LoginEventFn = {
            let me = me.clone();
            Arc::new(move |e: LoginEvent| {
                if me.login_gen.load(Ordering::SeqCst) != id {
                    return;
                }
                if let LoginEvent::BrowserFailed(e) | LoginEvent::DeviceCodeFailed(e) = &e {
                    log(format_args!("accounts: one login path failed: {e}"));
                }
                me.update(|s| {
                    if let Some(l) = s.login.as_mut() {
                        l.apply(e);
                    }
                });
            })
        };
        let task = inner.rt.spawn(async move {
            let result = me.auth.login(events).await;
            if me.login_gen.load(Ordering::SeqCst) != id {
                return;
            }
            let outcome = match result {
                Ok(info) => {
                    log(format_args!(
                        "accounts: logged in as {} ({})",
                        info.name, info.uuid
                    ));
                    me.reload().await;
                    LoginOutcome::Done(me.view(&info))
                }
                Err(e) => {
                    log(format_args!("accounts: login failed: {e}"));
                    LoginOutcome::Failed(AccountProblem::of(&e))
                }
            };
            me.update(|s| {
                if let Some(l) = s.login.as_mut() {
                    l.outcome = outcome;
                }
            });
        });
        *slot = Some((id, task));
    }

    fn cancel_login(&self) {
        let inner = &self.inner;
        inner.login_gen.fetch_add(1, Ordering::SeqCst);
        if let Some((_, task)) = inner.login.lock().unwrap().take()
            && !task.is_finished()
        {
            // Dropping the login future closes the loopback listener and stops the polling.
            task.abort();
            log(format_args!("accounts: login cancelled"));
        }
        inner.update(|s| s.login = None);
    }

    fn close_login(&self) {
        self.cancel_login();
    }

    fn logout(&self, uuid: Uuid) {
        log(format_args!("accounts: log out {uuid}"));
        self.inner.act(
            "log out",
            move |auth| async move { auth.remove(uuid).await },
        );
    }

    fn set_active(&self, uuid: Uuid) {
        log(format_args!("accounts: switch to {uuid}"));
        self.inner
            .act("switching accounts", move |auth| async move {
                auth.set_active(uuid).await
            });
    }

    fn refresh_in_background(&self) {
        let me = self.inner.clone();
        if !me.auth.is_available() {
            return;
        }
        self.inner.rt.spawn(async move {
            let results = me.auth.refresh_in_background().await;
            for (uuid, r) in &results {
                match r {
                    Ok(()) => log(format_args!("accounts: renewed {uuid} in the background")),
                    Err(e) => log(format_args!(
                        "accounts: couldn't renew {uuid} in the background: {e}"
                    )),
                }
            }
            if !results.is_empty() {
                me.reload().await;
            }
        });
    }
}

/// The fake's starting states (`SRL_SCREEN`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountsPreset {
    /// The shipped build: no client ID, Settings shows one line, Play is offline.
    Unavailable,
    /// Login available, no account yet.
    Empty,
    /// Two accounts: Violet_Runner active, the other one waiting for a new login; the key in a
    /// file (the quiet note).
    List,
    /// The login dialog, waiting: the browser opened, the phone code and its QR.
    Login,
    /// The same, but no browser could be opened: the link to copy.
    LoginNoBrowser,
    /// Signed in to Microsoft, the chain at the Minecraft step.
    LoginSteps,
    /// Logged in.
    LoginDone,
    /// Xbox Live refused: a child account (with the help link).
    LoginError,
    /// Violet_Runner logged in and active, no dialog (the `msa-*` screen ids).
    SignedIn,
}

/// `FakeAccounts`' fixed data and simulated login.
pub struct FakeAccounts {
    state: Arc<Mutex<AccountSnapshot>>,
    events: Events,
    login_gen: Arc<AtomicU64>,
}

const FAKE_ACTIVE: &str = "069a79f4-44e9-4726-a5be-fca90e38aaf5";
const FAKE_OTHER: &str = "61699b2e-d327-4a01-9f1e-0ea8c3f06bc6";

fn fake_face(avatar: &str) -> Option<Arc<Face>> {
    crate::assets::bytes(&format!("demo/avatars/{avatar}.png"))
        .and_then(face_from_picture)
        .map(Arc::new)
}

fn fake_account(main: bool) -> AccountView {
    if main {
        AccountView {
            uuid: Uuid::parse_str(FAKE_ACTIVE).unwrap(),
            name: "Violet_Runner".into(),
            face: fake_face("042"),
            needs_login: false,
        }
    } else {
        AccountView {
            uuid: Uuid::parse_str(FAKE_OTHER).unwrap(),
            name: "LateNightRuns".into(),
            face: fake_face("146"),
            needs_login: true,
        }
    }
}

/// A login view part way, as the fake shows it.
fn fake_login(browser_opened: bool) -> LoginView {
    let user_code = "RW7K-Q2PX".replace('-', "");
    LoginView {
        browser: Some(BrowserPath {
            url: "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize?client_id=\
                  11111111-2222-3333-4444-555555555555&response_type=code&scope=XboxLive.signin\
                  %20offline_access"
                .into(),
            opened: browser_opened,
        }),
        browser_failed: None,
        device: Some(DevicePath {
            qr: srl_auth::qr_matrix(&format!("https://www.microsoft.com/link?otc={user_code}"))
                .map(Arc::new),
            user_code,
            verification_uri: "https://www.microsoft.com/link".into(),
            expires_at: Utc::now() + chrono::Duration::minutes(15),
        }),
        device_failed: None,
        step: None,
        outcome: LoginOutcome::Waiting,
    }
}

impl FakeAccounts {
    pub fn new(events: Events, preset: AccountsPreset) -> Self {
        use AccountsPreset::*;
        let mut s = AccountSnapshot {
            available: preset != Unavailable,
            ..AccountSnapshot::default()
        };
        match preset {
            Unavailable | Empty | Login | LoginNoBrowser | LoginSteps | LoginError => {}
            List | LoginDone | SignedIn => {
                s.accounts = vec![fake_account(true)];
                s.active = Some(s.accounts[0].uuid);
                s.signed_in_once = true;
            }
        }
        if preset == List {
            s.accounts.push(fake_account(false));
            s.keystore_file = true;
        }
        s.login = match preset {
            Login => Some(fake_login(true)),
            LoginNoBrowser => Some(fake_login(false)),
            LoginSteps => Some(LoginView {
                step: Some(LoginStep::Minecraft),
                ..fake_login(true)
            }),
            LoginDone => Some(LoginView {
                step: Some(LoginStep::Profile),
                outcome: LoginOutcome::Done(fake_account(true)),
                ..fake_login(true)
            }),
            LoginError => Some(LoginView {
                step: Some(LoginStep::Xbox),
                outcome: LoginOutcome::Failed(AccountProblem::of(&AuthError::Xbox(
                    XboxProblem::ChildAccount,
                ))),
                ..fake_login(true)
            }),
            _ => None,
        };
        Self {
            state: Arc::new(Mutex::new(s)),
            events,
            login_gen: Arc::new(AtomicU64::new(0)),
        }
    }

    fn update(&self, f: impl FnOnce(&mut AccountSnapshot)) {
        update(&self.state, &self.events, f);
    }

    /// The state, for the fake backend's games (who they play as).
    pub fn shared(&self) -> Arc<Mutex<AccountSnapshot>> {
        self.state.clone()
    }
}

impl Accounts for FakeAccounts {
    fn snapshot(&self) -> AccountSnapshot {
        self.state.lock().unwrap().clone()
    }

    /// A simulated login: the dialog at once, the phone "approves" after 4 s, then the chain one
    /// step a second, and Violet_Runner is logged in.
    fn login(&self) {
        if !self.state.lock().unwrap().available {
            return;
        }
        let id = self.login_gen.fetch_add(1, Ordering::SeqCst) + 1;
        self.update(|s| s.login = Some(fake_login(true)));
        let (state, events, gen_) = (
            self.state.clone(),
            self.events.clone(),
            self.login_gen.clone(),
        );
        std::thread::spawn(move || {
            let steps = [
                LoginStep::SignedIn(srl_auth::Method::DeviceCode),
                LoginStep::Xbox,
                LoginStep::Minecraft,
                LoginStep::Profile,
            ];
            std::thread::sleep(Duration::from_secs(4));
            for step in steps {
                if gen_.load(Ordering::SeqCst) != id {
                    return;
                }
                update(&state, &events, |s| {
                    if let Some(l) = s.login.as_mut() {
                        l.step = Some(step);
                    }
                });
                std::thread::sleep(Duration::from_secs(1));
            }
            if gen_.load(Ordering::SeqCst) != id {
                return;
            }
            update(&state, &events, |s| {
                let a = fake_account(true);
                s.signed_in_once = true;
                if !s.accounts.iter().any(|x| x.uuid == a.uuid) {
                    s.accounts.insert(0, a.clone());
                }
                s.active = Some(a.uuid);
                if let Some(l) = s.login.as_mut() {
                    l.outcome = LoginOutcome::Done(a);
                }
            });
        });
    }

    fn cancel_login(&self) {
        self.login_gen.fetch_add(1, Ordering::SeqCst);
        self.update(|s| s.login = None);
    }

    fn close_login(&self) {
        self.cancel_login();
    }

    fn logout(&self, uuid: Uuid) {
        self.update(|s| {
            s.accounts.retain(|a| a.uuid != uuid);
            if s.active == Some(uuid) {
                s.active = s.accounts.first().map(|a| a.uuid);
            }
        });
    }

    fn set_active(&self, uuid: Uuid) {
        self.update(|s| {
            if s.accounts.iter().any(|a| a.uuid == uuid) {
                s.active = Some(uuid);
            }
        });
    }

    fn refresh_in_background(&self) {}
}

/// Where a skin is cached: `meta/skins/<texture id>.png`, only for Mojang's texture server.
fn skin_file(dir: &std::path::Path, url: &str) -> Option<PathBuf> {
    let id = skin_id(url)?;
    Some(dir.join(format!("{id}.png")))
}

/// The texture id of a skin on textures.minecraft.net (the hex hash at the end of the URL).
/// Other hosts get no face: the profile API only ever names this one.
fn skin_id(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://textures.minecraft.net/texture/")
        .or_else(|| url.strip_prefix("http://textures.minecraft.net/texture/"))?;
    (!rest.is_empty() && rest.len() <= 128 && rest.bytes().all(|b| b.is_ascii_hexdigit()))
        .then_some(rest)
}

/// The RGBA pixels of a PNG: width, height and 4 bytes per pixel.
fn decode_rgba(bytes: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let (w, h) = (reader.info().width as usize, reader.info().height as usize);
    if w > 1024 || h > 1024 {
        return None;
    }
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => return None,
    };
    let mut out = Vec::with_capacity(w * h * 4);
    for px in buf[..w * h * channels].chunks_exact(channels) {
        out.extend_from_slice(&match channels {
            4 => [px[0], px[1], px[2], px[3]],
            3 => [px[0], px[1], px[2], 255],
            2 => [px[0], px[0], px[0], px[1]],
            _ => [px[0], px[0], px[0], 255],
        });
    }
    Some((w, h, out))
}

/// The face of a Minecraft skin (64x64, or the old 64x32): the 8x8 face at (8,8), with the hat
/// layer at (40,8) drawn over it where it isn't transparent.
pub fn face_from_skin(bytes: &[u8]) -> Option<Face> {
    let (w, h, px) = decode_rgba(bytes)?;
    if w != 64 || !(h == 64 || h == 32) {
        return None;
    }
    let at = |x: usize, y: usize| -> [f32; 4] {
        let i = (y * w + x) * 4;
        [px[i], px[i + 1], px[i + 2], px[i + 3]].map(|c| c as f32 / 255.)
    };
    let mut face = [0u32; 64];
    for y in 0..8 {
        for x in 0..8 {
            let base = at(8 + x, 8 + y);
            let hat = at(40 + x, 8 + y);
            let a = hat[3];
            let mix = |i: usize| (hat[i] * a + base[i] * (1. - a)) * 255.;
            let [r, g, b] = [mix(0), mix(1), mix(2)].map(|c| c.round() as u32);
            face[y * 8 + x] = (r << 24) | (g << 16) | (b << 8) | 0xFF;
        }
    }
    Some(Face(face))
}

/// A face from a square picture of one (the demo avatars, 8 px per cell): the centre of each
/// cell. For the fake backend's accounts.
pub fn face_from_picture(bytes: &[u8]) -> Option<Face> {
    let (w, h, px) = decode_rgba(bytes)?;
    let mut face = [0u32; 64];
    for r in 0..8 {
        for c in 0..8 {
            let (x, y) = ((c * w + w / 2) / 8, (r * h + h / 2) / 8);
            let i = (y * w + x) * 4;
            face[r * 8 + c] = u32::from_be_bytes([px[i], px[i + 1], px[i + 2], 0xFF]);
        }
    }
    Some(Face(face))
}

// srl-auth's own mock of every service, shared with its tests (a path relative to this file's
// folder, so it lives outside the inline test module).
#[cfg(test)]
#[path = "../../../srl-auth/tests/mock/mod.rs"]
mod mock_services;

#[cfg(test)]
mod tests {
    use std::{path::Path, time::Instant};

    use srl_auth::KeystoreChoice;

    use super::*;

    use super::mock_services as mock;

    #[test]
    fn play_as_is_the_one_offline_policy() {
        let id = Uuid::from_u128(7);
        assert_eq!(play_as(true, true, None), PlayAs::Offline);
        assert_eq!(
            play_as(true, true, Some((id, false))),
            PlayAs::Microsoft(id)
        );
        assert_eq!(play_as(true, true, Some((id, true))), PlayAs::Relogin(id));
        // Offline only after a sign-in here, in a build that can sign in.
        assert_eq!(play_as(true, false, None), PlayAs::SignInFirst);
        // A build without Microsoft login plays offline, whatever accounts.json says.
        assert_eq!(play_as(false, false, None), PlayAs::Offline);
        assert_eq!(play_as(false, true, Some((id, false))), PlayAs::Offline);

        let a = |needs_login| AccountView {
            uuid: id,
            name: "Violet_Runner".into(),
            face: None,
            needs_login,
        };
        let mut s = AccountSnapshot {
            available: true,
            accounts: vec![a(false)],
            active: Some(id),
            signed_in_once: true,
            ..AccountSnapshot::default()
        };
        assert_eq!(s.active_name(), Some("Violet_Runner"));
        s.available = false;
        assert_eq!(s.active_name(), None);
        s.available = true;
        s.active = None;
        assert_eq!(s.active_name(), None);
    }

    #[test]
    fn problems_carry_what_the_ui_needs() {
        let p = AccountProblem::of(&AuthError::Xbox(XboxProblem::Other(2148916262)));
        assert_eq!((p.key, p.code), ("auth.error.xbox_other", Some(2148916262)));
        let p = AccountProblem::of(&AuthError::Xbox(XboxProblem::ChildAccount));
        assert!(p.help_url.is_some() && !p.relogin);
        let p = AccountProblem::of(&AuthError::Keystore("locked".into()));
        assert!(p.keystore && p.key == "auth.error.keystore");
        assert!(AccountProblem::of(&AuthError::ReloginRequired("x".into())).relogin);
    }

    fn png(w: u32, h: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().unwrap();
            let data: Vec<u8> = (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .flat_map(|(x, y)| pixel(x, y))
                .collect();
            writer.write_image_data(&data).unwrap();
        }
        out
    }

    #[test]
    fn the_face_is_the_head_with_its_hat() {
        // Face pixels red; the hat covers the top row with opaque blue, the rest transparent.
        let skin = png(64, 64, |x, y| match (x, y) {
            (8..16, 8..16) => [255, 0, 0, 255],
            (40..48, 8) => [0, 0, 255, 255],
            _ => [0, 0, 0, 0],
        });
        let face = face_from_skin(&skin).unwrap();
        assert_eq!(face.0[0], 0x0000FFFF);
        assert_eq!(face.0[7], 0x0000FFFF);
        assert_eq!(face.0[8], 0xFF0000FF);
        assert_eq!(face.0[63], 0xFF0000FF);
        // A 64x32 skin from before 1.8 works too; anything else is not a skin.
        assert!(face_from_skin(&png(64, 32, |_, _| [9, 9, 9, 255])).is_some());
        assert!(face_from_skin(&png(32, 32, |_, _| [9, 9, 9, 255])).is_none());
        assert!(face_from_skin(b"not a png").is_none());
    }

    #[test]
    fn skins_only_from_mojangs_texture_server() {
        let id = "3b60a1f6d562f52aaebbf1434f1de147933a3affe0e764fa49ea057536623cd3";
        assert_eq!(
            skin_id(&format!("http://textures.minecraft.net/texture/{id}")),
            Some(id)
        );
        assert_eq!(
            skin_id("https://textures.minecraft.net/texture/abc"),
            Some("abc")
        );
        assert_eq!(skin_id("https://example.org/texture/abc"), None);
        assert_eq!(skin_id("https://textures.minecraft.net/texture/../x"), None);
        assert!(is_loopback(&"http://127.0.0.1:9/".parse().unwrap()));
        assert!(is_loopback(&"http://[::1]:9/".parse().unwrap()));
        assert!(!is_loopback(&"http://10.0.0.1/".parse().unwrap()));
        assert!(!is_loopback(&"https://127.0.0.1/".parse().unwrap()));
    }

    /// A folder of its own for each test (a few small files).
    fn root(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("srl-app-account-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn auth(root: &Path, cfg: Option<AuthConfig>) -> Auth {
        Auth::with_keystore(&Paths::new(root), cfg, KeystoreChoice::FileOnly)
    }

    #[tokio::test]
    async fn without_login_every_play_is_offline() {
        let dir = root("offline");
        let a = launch_account(
            &auth(&dir, None),
            OfflineAccount::new("Violet_Runner"),
            || panic!("no sign-in stage offline"),
        )
        .await
        .unwrap();
        assert!(matches!(&a.account, GameAccount::Offline(o) if o.name == "Violet_Runner"));
        assert!(a.in_use.is_none() && a.valid_until.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Against srl-auth's mock: before any login Play asks for one (offline is closed); after a
    /// login every Play signs in, gets the session's placeholders (`--userType msa`) and holds the
    /// account in use; a sign-in that can't be renewed starts anyway with "valid until"; after
    /// logging out, Play is offline.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn play_signs_in_with_the_active_account() {
        let mock = mock::Mock::start().await;
        let dir = root("play");
        let auth = auth(&dir, Some(mock.config()));
        let offline = || OfflineAccount::new("Violet_Runner");
        let p = launch_account(&auth, offline(), || panic!("nobody is active"))
            .await
            .err()
            .expect("offline is closed before the first sign-in");
        assert_eq!(p.key, SIGN_IN_FIRST);
        assert!(p.relogin, "Play offers the sign-in");

        // A token with 2 h left: under the launch margin, so Play renews it.
        mock.with(|s| s.mc_expires_in = 2 * 3600);
        auth.login(Arc::new(|_| {})).await.unwrap();
        let mut stage = false;
        let a = launch_account(&auth, offline(), || stage = true)
            .await
            .unwrap();
        assert!(stage, "the Signing in stage shows");
        let GameAccount::Microsoft(session) = &a.account else {
            panic!("offline with an active account")
        };
        let p: HashMap<_, _> = session.placeholders.iter().cloned().collect();
        assert_eq!(p["user_type"], "msa");
        assert_eq!(p["auth_uuid"], mock::PROFILE_ID);
        assert!(p["auth_access_token"].contains("MC-SECRET"));
        assert!(a.in_use.is_some() && a.warning.is_none());
        assert!(!format!("{:?}", a.account).contains("MC-SECRET"));
        drop(a);

        // The network goes down with a token that still works: start anyway, with a warning.
        mock.with(|s| s.offline = true);
        let a = launch_account(&auth, offline(), || {}).await.unwrap();
        assert!(a.account.is_microsoft());
        assert!(a.valid_until.is_some());
        assert_eq!(
            a.warning.as_ref().map(|w| w.key),
            Some("auth.error.network")
        );
        drop(a);

        // Logged out: offline stays open, the sign-in happened.
        let uuid = auth.accounts().await.unwrap().active.unwrap();
        auth.remove(uuid).await.unwrap();
        let a = launch_account(&auth, offline(), || panic!("nobody is active"))
            .await
            .unwrap();
        assert!(matches!(&a.account, GameAccount::Offline(o) if o.name == "Violet_Runner"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A revoked sign-in (the refresh token refused, no usable token left) stops Play and asks
    /// for a new login, every time after that too.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_revoked_sign_in_asks_for_a_new_login() {
        let mock = mock::Mock::start().await;
        let dir = root("relogin");
        let auth = auth(&dir, Some(mock.config()));
        // Every cached step is already past its margin, so Play goes back to the refresh token.
        mock.with(|s| {
            s.mc_expires_in = 60;
            s.xsts_lifetime_s = 60;
            s.xbl_lifetime_s = 60;
            s.msa_expires_in = 60;
        });
        auth.login(Arc::new(|_| {})).await.unwrap();
        mock.with(|s| s.refresh_invalid = true);
        let p = launch_account(&auth, OfflineAccount::new("x"), || {})
            .await
            .err()
            .expect("a revoked sign-in can't start the game");
        assert!(p.relogin, "{p:?}");
        let p = launch_account(&auth, OfflineAccount::new("x"), || {})
            .await
            .err()
            .unwrap();
        assert!(p.relogin, "{p:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Waits for an account snapshot `done` accepts.
    fn wait_for(
        rx: &mut futures::channel::mpsc::UnboundedReceiver<Event>,
        secs: u64,
        mut done: impl FnMut(&AccountSnapshot) -> bool,
    ) -> AccountSnapshot {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            assert!(Instant::now() < deadline, "timed out");
            match rx.try_recv() {
                Ok(Event::Account(s)) if done(&s) => return *s,
                Ok(_) => {}
                Err(futures::channel::mpsc::TryRecvError::Closed) => panic!("channel closed"),
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    }

    /// The Settings side end to end against the mock: the dialog's states from the login
    /// events, the saved account, a second one, switching and logging out, and Cancel ending a
    /// login where it is (no more polling).
    #[test]
    fn settings_login_switch_logout_and_cancel() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let mock = rt.block_on(mock::Mock::start());
        let dir = root("settings");
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let accounts = RealAccounts::new(
            auth(&dir, Some(mock.config())),
            &Paths::new(&dir),
            tx,
            rt.handle().clone(),
        );
        assert!(accounts.snapshot().available && accounts.snapshot().accounts.is_empty());

        // The phone path: the browser is never used, the phone approves on its second poll.
        mock.with(|s| {
            s.browser = mock::BrowserMode::Ignore;
            s.polls = [mock::Poll::Pending, mock::Poll::Approve].into();
        });
        accounts.login();
        let s = wait_for(&mut rx, 10, |s| {
            s.login.as_ref().is_some_and(|l| l.device.is_some())
        });
        let device = s.login.unwrap().device.unwrap();
        assert_eq!(device.user_code, "ABCD1234");
        assert!(device.qr.is_some());
        let s = wait_for(&mut rx, 10, |s| {
            matches!(
                s.login.as_ref().map(|l| &l.outcome),
                Some(LoginOutcome::Done(_))
            )
        });
        let first = s.accounts[0].uuid;
        assert_eq!(s.active, Some(first));
        assert_eq!(s.active_name(), Some("Violet_Runner"));
        accounts.close_login();
        assert!(accounts.snapshot().login.is_none());

        // A second account through the browser, which becomes the active one.
        mock.with(|s| {
            s.browser = mock::BrowserMode::Approve;
            s.profile_name = "LateNightRuns".into();
            s.profile_id = "61699b2ed3274a019f1e0ea8c3f06bc6".into();
        });
        accounts.login();
        let s = wait_for(&mut rx, 10, |s| s.accounts.len() == 2);
        assert_eq!(s.active_name(), Some("LateNightRuns"));
        accounts.close_login();
        accounts.set_active(first);
        wait_for(&mut rx, 5, |s| s.active == Some(first));
        accounts.logout(first);
        let s = wait_for(&mut rx, 5, |s| s.accounts.len() == 1);
        assert_eq!(s.active_name(), Some("LateNightRuns"));

        // Cancel: the dialog closes and the device code polling stops.
        mock.with(|s| s.browser = mock::BrowserMode::Ignore);
        accounts.login();
        wait_for(&mut rx, 10, |s| {
            s.login.as_ref().is_some_and(|l| l.device.is_some())
        });
        std::thread::sleep(Duration::from_millis(300));
        accounts.cancel_login();
        assert!(accounts.snapshot().login.is_none());
        std::thread::sleep(Duration::from_millis(200));
        let polls = mock.with(|s| s.calls("token:device"));
        std::thread::sleep(Duration::from_millis(1500));
        assert_eq!(
            mock.with(|s| s.calls("token:device")),
            polls,
            "still polling"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
