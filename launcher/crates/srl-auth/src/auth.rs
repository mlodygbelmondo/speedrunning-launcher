//! `Auth`: the one handle the app uses. Cheap to clone; every method is async and meant for the
//! app's background tokio runtime.
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use chrono::Utc;
use uuid::Uuid;

use crate::{
    config::AuthConfig,
    error::AuthError,
    keystore::{KeystoreChoice, KeystoreKind},
    login::{self, LoginEvent, LoginEventFn, LoginStep},
    msa,
    secret::Secret,
    session::MinecraftSession,
    store::{AccountInfo, AccountsFile, Ownership, Store},
    tokens::{BACKGROUND_MARGIN, LAUNCH_MARGIN, MsaAccess, Plan, Tokens},
    xbox::{self, Profile},
};

/// The accounts as the UI lists them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountList {
    pub accounts: Vec<AccountInfo>,
    pub active: Option<Uuid>,
    /// Where the token key is: `Some(File)` deserves a quiet note in the UI ("saved sign-ins are
    /// protected by file permissions only"). `None` before the first login.
    pub keystore: Option<KeystoreKind>,
    /// Someone has signed in here once with an account that owns the game, even if that account
    /// is gone since. Files from before this field count an account in the list as that.
    pub signed_in_once: bool,
}

/// A session ready for the game, from `Auth::prepare_launch`.
#[derive(Clone, Debug)]
pub struct LaunchSession {
    pub session: MinecraftSession,
    /// Set when the token could not be renewed for a passing reason (no network, rate limit) and
    /// the launch goes ahead with the saved token, which is still valid but has less than the
    /// launch margin left. The UI may show "signed in until HH:MM".
    pub warning: Option<AuthError>,
}

struct Inner {
    root: PathBuf,
    config: Option<AuthConfig>,
    http: reqwest::Client,
    store: tokio::sync::Mutex<Store>,
    in_use: Mutex<HashMap<Uuid, usize>>,
    /// The last passing failure to renew each account at launch, and when (see
    /// `prepare_launch`). In memory only: a restart tries again at once.
    failures: Mutex<HashMap<Uuid, (Instant, AuthError)>>,
}

/// After a passing failure to renew (no network, rate limit), launches within this long reuse the
/// saved token with the same warning instead of trying again. A wall starts its instances within
/// seconds; a player who clicks Play a minute later gets a fresh try.
pub const RETRY_AFTER_FAILURE: Duration = Duration::from_secs(60);

/// What `renew` did.
enum Renewal {
    /// The Minecraft token already had the margin left: nothing changed, nothing to save.
    Cached,
    /// New tokens; the fresh profile when it could be read (name or skin may have changed).
    Renewed(Option<Profile>),
}

#[derive(Clone)]
pub struct Auth {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Auth")
            .field("root", &self.inner.root)
            .field("config", &self.inner.config)
            .finish_non_exhaustive()
    }
}

/// Held while a game runs with an account (`Auth::mark_in_use`). The background refresh leaves the
/// account alone until every guard is dropped: a new token wouldn't reach the running game.
pub struct InUse {
    inner: Arc<Inner>,
    uuid: Uuid,
}

impl Drop for InUse {
    fn drop(&mut self) {
        let mut map = self.inner.in_use.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(n) = map.get_mut(&self.uuid) {
            *n -= 1;
            if *n == 0 {
                map.remove(&self.uuid);
            }
        }
    }
}

impl Auth {
    /// `config` is `AuthConfig::detect()`. The keystore choice comes from `SRL_KEYSTORE`.
    pub fn new(paths: &srl_core::Paths, config: Option<AuthConfig>) -> Self {
        Self::with_keystore(paths, config, KeystoreChoice::from_env())
    }

    pub fn with_keystore(
        paths: &srl_core::Paths,
        config: Option<AuthConfig>,
        keystore: KeystoreChoice,
    ) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(srl_core::USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client with static settings");
        let root = paths.root().to_path_buf();
        Self {
            inner: Arc::new(Inner {
                store: tokio::sync::Mutex::new(Store::new(&root, keystore)),
                root,
                config,
                http,
                in_use: Mutex::new(HashMap::new()),
                failures: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Whether this build can log in at all (it has a client ID).
    pub fn is_available(&self) -> bool {
        self.inner.config.is_some()
    }

    fn config(&self) -> Result<&AuthConfig, AuthError> {
        self.inner.config.as_ref().ok_or(AuthError::NotAvailable)
    }

    /// The saved accounts. Works in a build without a client ID too (it just can't renew them).
    /// Doesn't wait for a refresh in progress: `accounts.json` is only ever replaced whole (temp
    /// file + rename), so reading it without the lock sees the old or the new list, never half.
    pub async fn accounts(&self) -> Result<AccountList, AuthError> {
        let file = Store::load(&self.inner.root)?;
        Ok(AccountList {
            signed_in_once: file.signed_in_once || !file.accounts.is_empty(),
            accounts: file.accounts,
            active: file.active,
            keystore: file.key_store,
        })
    }

    pub async fn set_active(&self, uuid: Uuid) -> Result<(), AuthError> {
        let store = self.inner.store.lock().await;
        let mut file = Store::load(&self.inner.root)?;
        if !file.accounts.iter().any(|a| a.uuid == uuid) {
            return Err(AuthError::UnknownAccount);
        }
        file.active = Some(uuid);
        store.save(&file)
    }

    /// Logs an account out: its tokens are deleted, then the account. The last one out also
    /// removes the key from the keystore.
    pub async fn remove(&self, uuid: Uuid) -> Result<(), AuthError> {
        let mut store = self.inner.store.lock().await;
        let mut file = Store::load(&self.inner.root)?;
        store.delete_tokens(uuid);
        file.accounts.retain(|a| a.uuid != uuid);
        if file.active == Some(uuid) {
            file.active = file.accounts.first().map(|a| a.uuid);
        }
        if file.accounts.is_empty() {
            store.forget_key(&mut file).await;
        }
        store.save(&file)
    }

    /// While the returned guard lives, the background refresh skips the account.
    pub fn mark_in_use(&self, uuid: Uuid) -> InUse {
        *self
            .inner
            .in_use
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(uuid)
            .or_default() += 1;
        InUse {
            inner: self.inner.clone(),
            uuid,
        }
    }

    fn recent_failure(&self, uuid: Uuid) -> Option<AuthError> {
        let map = self
            .inner
            .failures
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.get(&uuid)
            .filter(|(at, _)| at.elapsed() < RETRY_AFTER_FAILURE)
            .map(|(_, e)| e.clone())
    }

    fn note_failure(&self, uuid: Uuid, e: &AuthError) {
        self.inner
            .failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(uuid, (Instant::now(), e.clone()));
    }

    fn forget_failure(&self, uuid: Uuid) {
        self.inner
            .failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&uuid);
    }

    fn is_in_use(&self, uuid: Uuid) -> bool {
        self.inner
            .in_use
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(&uuid)
    }

    /// The interactive login: browser and device code at once (`events` tells the page what to
    /// show), then Xbox, XSTS, Minecraft, entitlements and profile. On success the account is
    /// saved and becomes the active one. Cancel by dropping the future (abort the task): the
    /// loopback listener closes and the polling stops.
    pub async fn login(&self, events: LoginEventFn) -> Result<AccountInfo, AuthError> {
        let cfg = self.config()?;
        let http = &self.inner.http;
        let (msa_tokens, method) = login::sign_in(http, cfg, &events).await?;
        events(LoginEvent::Step(LoginStep::SignedIn(method)));

        events(LoginEvent::Step(LoginStep::Xbox));
        let xbl = xbox::xbox_user(http, cfg, &msa_tokens.access).await?;
        let xsts = xbox::xsts(http, cfg, &xbl).await?;
        events(LoginEvent::Step(LoginStep::Minecraft));
        let mc = xbox::minecraft_login(http, cfg, &xsts).await?;
        events(LoginEvent::Step(LoginStep::Profile));
        // Entitlements only explain a missing profile; the profile decides.
        let entitlements = xbox::entitlements(http, cfg, &mc).await;
        let Some(profile) = xbox::profile(http, cfg, &mc).await? else {
            return Err(match &entitlements {
                Ok(e) if e.owns_game() => AuthError::NoProfile,
                Ok(e) if e.game_pass() => AuthError::GamePassNoProfile,
                _ => AuthError::NotOwned,
            });
        };
        let ownership = match &entitlements {
            Ok(e) if e.owns_game() => Ownership::Purchased,
            Ok(e) if e.game_pass() => Ownership::GamePass,
            _ => Ownership::Unknown,
        };
        let tokens = Tokens {
            client_id: cfg.client_id.clone(),
            // Without `offline_access` there is no refresh token: the session works for a day,
            // then the account asks for a new login.
            msa_refresh: msa_tokens.refresh.unwrap_or_else(|| Secret::new("")),
            msa_access: Some(MsaAccess {
                token: msa_tokens.access,
                expires_at: msa_tokens.expires_at,
            }),
            xbl: Some(xbl),
            xsts: Some(xsts),
            minecraft: Some(mc),
        };

        let mut store = self.inner.store.lock().await;
        let mut file = Store::load(&self.inner.root)?;
        store.write_tokens(&mut file, profile.uuid, &tokens).await?;
        let now = Utc::now();
        let info = match file.accounts.iter_mut().find(|a| a.uuid == profile.uuid) {
            Some(a) => {
                apply_profile(a, &profile);
                a.ownership = ownership;
                a.refreshed_at = Some(now);
                a.needs_login = None;
                a.clone()
            }
            None => {
                let a = AccountInfo {
                    uuid: profile.uuid,
                    name: profile.name.clone(),
                    skin: profile.skin.clone(),
                    ownership,
                    added_at: now,
                    refreshed_at: Some(now),
                    needs_login: None,
                };
                file.accounts.push(a.clone());
                a
            }
        };
        file.active = Some(info.uuid);
        file.signed_in_once = true;
        store.save(&file)?;
        Ok(info)
    }

    /// The session to start the game with, renewed first if the Minecraft token has less than
    /// `LAUNCH_MARGIN` (8 h) left. Call it right before every launch. All instances started
    /// together share the one session: the first launch renews, the rest find a fresh token.
    ///
    /// An account in use (`mark_in_use`) is renewed here too when its token is inside the margin:
    /// the running games keep their old token, which stays valid until it expires (research 1.5),
    /// while the new instance must not start with minutes left. After a passing failure to renew,
    /// launches in the next `RETRY_AFTER_FAILURE` reuse the saved token with the same warning
    /// instead of waiting for the network again, once per instance of a wall.
    pub async fn prepare_launch(&self, uuid: Uuid) -> Result<LaunchSession, AuthError> {
        let cfg = self.config()?;
        let mut store = self.inner.store.lock().await;
        let mut file = Store::load(&self.inner.root)?;
        if !file.accounts.iter().any(|a| a.uuid == uuid) {
            return Err(AuthError::UnknownAccount);
        }
        let mut tokens = match store.read_tokens(&mut file, uuid).await {
            Ok(t) => t,
            Err(e) => return Err(self.fail(&mut store, &mut file, uuid, e)),
        };
        if tokens.plan(Utc::now(), LAUNCH_MARGIN) != Plan::UseCached
            && let Some(e) = self.recent_failure(uuid)
            && let Some(mc) = tokens.usable_minecraft(Utc::now())
        {
            return Ok(LaunchSession {
                session: session(&file, uuid, mc),
                warning: Some(e),
            });
        }
        let result = self.renew(cfg, &mut tokens, LAUNCH_MARGIN).await;
        match result {
            Ok(renewal) => {
                self.forget_failure(uuid);
                if let Renewal::Renewed(profile) = renewal {
                    store.write_tokens(&mut file, uuid, &tokens).await?;
                    self.note_refresh(&mut file, uuid, profile.as_ref());
                    store.save(&file)?;
                }
                let mc = tokens
                    .minecraft
                    .clone()
                    .expect("renew leaves a Minecraft token");
                Ok(LaunchSession {
                    session: session(&file, uuid, &mc),
                    warning: None,
                })
            }
            Err(e) if e.is_transient() && tokens.usable_minecraft(Utc::now()).is_some() => {
                let _ = store.write_tokens(&mut file, uuid, &tokens).await;
                self.note_failure(uuid, &e);
                let mc = tokens.minecraft.clone().expect("checked above");
                Ok(LaunchSession {
                    session: session(&file, uuid, &mc),
                    warning: Some(e),
                })
            }
            Err(e) => {
                if !e.needs_relogin() {
                    // Keep what did work (a rotated refresh token above all).
                    let _ = store.write_tokens(&mut file, uuid, &tokens).await;
                }
                Err(self.fail(&mut store, &mut file, uuid, e))
            }
        }
    }

    /// Renews, one account at a time, every account whose Minecraft token has less than
    /// `BACKGROUND_MARGIN` (12 h) left, skipping accounts in use or waiting for a login. Call it
    /// once at launcher start (and, say, hourly). Returns what happened per account that needed it.
    pub async fn refresh_in_background(&self) -> Vec<(Uuid, Result<(), AuthError>)> {
        let Ok(cfg) = self.config() else {
            return Vec::new();
        };
        let Ok(list) = self.accounts().await else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for account in list.accounts {
            if account.needs_login.is_some() || self.is_in_use(account.uuid) {
                continue;
            }
            if !out.is_empty() {
                // Spread the calls out, like Prism; rate limits are per IP.
                let jitter = 1000 + u64::from(crate::pkce::random_bytes::<1>()[0]) * 12;
                tokio::time::sleep(Duration::from_millis(jitter)).await;
            }
            if let Some(r) = self.refresh_one(cfg, account.uuid).await {
                out.push((account.uuid, r));
            }
        }
        out
    }

    /// `None`: nothing to do.
    async fn refresh_one(&self, cfg: &AuthConfig, uuid: Uuid) -> Option<Result<(), AuthError>> {
        let mut store = self.inner.store.lock().await;
        let mut file = match Store::load(&self.inner.root) {
            Ok(f) => f,
            Err(e) => return Some(Err(e)),
        };
        let mut tokens = match store.read_tokens(&mut file, uuid).await {
            Ok(t) => t,
            Err(e) => return Some(Err(self.fail(&mut store, &mut file, uuid, e))),
        };
        if tokens.plan(Utc::now(), BACKGROUND_MARGIN) == Plan::UseCached {
            return None;
        }
        Some(
            match self.renew(cfg, &mut tokens, BACKGROUND_MARGIN).await {
                Ok(Renewal::Cached) => Ok(()),
                Ok(Renewal::Renewed(profile)) => {
                    self.forget_failure(uuid);
                    store
                        .write_tokens(&mut file, uuid, &tokens)
                        .await
                        .and_then(|_| {
                            self.note_refresh(&mut file, uuid, profile.as_ref());
                            store.save(&file)
                        })
                }
                Err(e) => {
                    if !e.needs_relogin() {
                        let _ = store.write_tokens(&mut file, uuid, &tokens).await;
                    }
                    Err(self.fail(&mut store, &mut file, uuid, e))
                }
            },
        )
    }

    /// A relogin-type error drops the tokens and flags the account; others pass through.
    fn fail(
        &self,
        store: &mut Store,
        file: &mut AccountsFile,
        uuid: Uuid,
        e: AuthError,
    ) -> AuthError {
        if let AuthError::ReloginRequired(reason) = &e {
            store.delete_tokens(uuid);
            if let Some(a) = file.accounts.iter_mut().find(|a| a.uuid == uuid) {
                a.needs_login = Some(reason.clone());
            }
            let _ = store.save(file);
        }
        e
    }

    fn note_refresh(&self, file: &mut AccountsFile, uuid: Uuid, profile: Option<&Profile>) {
        if let Some(a) = file.accounts.iter_mut().find(|a| a.uuid == uuid) {
            a.refreshed_at = Some(Utc::now());
            a.needs_login = None;
            if let Some(p) = profile.filter(|p| p.uuid == uuid) {
                apply_profile(a, p);
            }
        }
    }

    /// Brings the chain up to `margin`, from the latest good step. If a cached intermediate token
    /// turns out to be rejected (revoked early), starts once more from the refresh token.
    async fn renew(
        &self,
        cfg: &AuthConfig,
        tokens: &mut Tokens,
        margin: chrono::Duration,
    ) -> Result<Renewal, AuthError> {
        if tokens.client_id != cfg.client_id {
            return Err(AuthError::ReloginRequired(
                "this build uses a different Microsoft app".into(),
            ));
        }
        let plan = tokens.plan(Utc::now(), margin);
        if plan == Plan::UseCached {
            return Ok(Renewal::Cached);
        }
        match self.run_chain(cfg, tokens, plan).await {
            Err(AuthError::Service {
                status: 400 | 401, ..
            }) if plan != Plan::MsaRefresh => {
                tokens.msa_access = None;
                tokens.xbl = None;
                tokens.xsts = None;
                self.run_chain(cfg, tokens, Plan::MsaRefresh).await?;
            }
            other => other?,
        }
        let mc = tokens
            .minecraft
            .as_ref()
            .expect("the chain ends with a Minecraft token");
        // The name or skin may have changed; a failure here doesn't spoil the fresh token.
        Ok(Renewal::Renewed(
            xbox::profile(&self.inner.http, cfg, mc)
                .await
                .ok()
                .flatten(),
        ))
    }

    async fn run_chain(
        &self,
        cfg: &AuthConfig,
        tokens: &mut Tokens,
        plan: Plan,
    ) -> Result<(), AuthError> {
        let http = &self.inner.http;
        if plan == Plan::MsaRefresh {
            if tokens.msa_refresh.is_empty() {
                return Err(AuthError::ReloginRequired("no saved sign-in".into()));
            }
            let t = msa::refresh(http, cfg, &tokens.msa_refresh).await?;
            if let Some(r) = t.refresh {
                tokens.msa_refresh = r;
            }
            tokens.msa_access = Some(MsaAccess {
                token: t.access,
                expires_at: t.expires_at,
            });
        }
        if matches!(plan, Plan::MsaRefresh | Plan::XboxUser) {
            let access = tokens.msa_access.as_ref().expect("set above or planned");
            tokens.xbl = Some(xbox::xbox_user(http, cfg, &access.token).await?);
        }
        if matches!(plan, Plan::MsaRefresh | Plan::XboxUser | Plan::Xsts) {
            let xbl = tokens.xbl.as_ref().expect("set above or planned");
            tokens.xsts = Some(xbox::xsts(http, cfg, xbl).await?);
        }
        let xsts = tokens.xsts.as_ref().expect("set above or planned");
        tokens.minecraft = Some(xbox::minecraft_login(http, cfg, xsts).await?);
        Ok(())
    }
}

fn apply_profile(a: &mut AccountInfo, p: &Profile) {
    a.name = p.name.clone();
    a.skin = p.skin.clone();
}

fn session(file: &AccountsFile, uuid: Uuid, mc: &xbox::MinecraftToken) -> MinecraftSession {
    let name = file
        .accounts
        .iter()
        .find(|a| a.uuid == uuid)
        .map(|a| a.name.clone())
        .unwrap_or_default();
    MinecraftSession {
        name,
        uuid,
        xuid: xbox::xuid_from_access_token(&mc.token).unwrap_or_default(),
        access_token: mc.token.clone(),
        client_id: file.install_id.clone(),
        user_type: "msa".to_string(),
        expires_at: mc.expires_at,
    }
}
