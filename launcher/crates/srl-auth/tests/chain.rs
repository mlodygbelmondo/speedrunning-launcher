//! The whole login and refresh chain against the local mock (`mock/mod.rs`). Every test uses the
//! file keystore in its own folder: no test touches the keyring of the machine it runs on.
mod mock;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use mock::{BrowserMode, Mock, Poll};
use srl_auth::{
    Auth, AuthConfig, AuthError, KeystoreChoice, KeystoreKind, LoginEvent, LoginEventFn, LoginStep,
    Method, Ownership, XboxProblem,
};

struct Env {
    mock: Mock,
    root: PathBuf,
    auth: Auth,
    events: Arc<Mutex<Vec<LoginEvent>>>,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn root(name: &str) -> PathBuf {
    // Tests write a few small files only; `SRL_HOME` style folder under the target dir's tmp.
    let d = std::env::temp_dir().join(format!("srl-auth-chain-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

async fn env(name: &str) -> Env {
    env_with(name, |_| {}).await
}

async fn env_with(name: &str, tweak: impl FnOnce(&mut AuthConfig)) -> Env {
    let mock = Mock::start().await;
    let root = root(name);
    let mut cfg = mock.config();
    tweak(&mut cfg);
    let auth = Auth::with_keystore(
        &srl_core::Paths::new(&root),
        Some(cfg),
        KeystoreChoice::FileOnly,
    );
    Env {
        mock,
        root,
        auth,
        events: Arc::new(Mutex::new(Vec::new())),
    }
}

impl Env {
    fn sink(&self) -> LoginEventFn {
        let events = self.events.clone();
        Arc::new(move |e| events.lock().unwrap().push(e))
    }

    async fn login(&self) -> Result<srl_auth::AccountInfo, AuthError> {
        self.auth.login(self.sink()).await
    }

    fn events(&self) -> Vec<LoginEvent> {
        self.events.lock().unwrap().clone()
    }

    fn calls(&self, what: &'static str) -> u32 {
        self.mock.with(|s| s.calls(what))
    }
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd {
            let p = e.unwrap().path();
            if p.is_dir() {
                out.extend(files(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}

/// No file in the data folder holds any token the mock issued, in the clear.
fn assert_no_token_on_disk(env: &Env) {
    let tokens = env.mock.with(|s| s.all_tokens.clone());
    assert!(!tokens.is_empty());
    for f in files(&env.root) {
        let text = String::from_utf8_lossy(&std::fs::read(&f).unwrap()).into_owned();
        for t in &tokens {
            assert!(!text.contains(t.as_str()), "{} holds {t}", f.display());
        }
    }
}

#[tokio::test]
async fn browser_login_runs_the_whole_chain() {
    let env = env("browser").await;
    // The player never enters the device code: the browser wins.
    let account = env.login().await.unwrap();
    assert_eq!(account.name, "Violet_Runner");
    assert_eq!(account.uuid.simple().to_string(), mock::PROFILE_ID);
    let skin = account.skin.clone().unwrap();
    assert_eq!(skin.url, "https://textures.minecraft.net/texture/abc");
    assert_eq!(skin.variant, "SLIM");
    assert_eq!(account.ownership, Ownership::Purchased);

    let events = env.events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, LoginEvent::Browser { opened: true, url }
        if url.contains("code_challenge=")))
    );
    assert!(events.iter().any(
        |e| matches!(e, LoginEvent::DeviceCode { user_code, qr_url, .. }
        if user_code == "ABCD1234" && qr_url == "https://www.microsoft.com/link?otc=ABCD1234")
    ));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, LoginEvent::Step(LoginStep::SignedIn(Method::Browser))))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, LoginEvent::Step(LoginStep::Profile)))
    );
    for what in [
        "authorize",
        "token:code",
        "xbl",
        "xsts",
        "mc_login",
        "entitlements",
        "profile",
    ] {
        assert_eq!(env.calls(what), 1, "{what}");
    }

    let list = env.auth.accounts().await.unwrap();
    assert_eq!(list.active, Some(account.uuid));
    assert_eq!(list.keystore, Some(KeystoreKind::File));
    assert_eq!(list.accounts, vec![account.clone()]);
    assert_no_token_on_disk(&env);

    // A fresh 24 h token: the launch uses it without any network call.
    let ready = env.auth.prepare_launch(account.uuid).await.unwrap();
    assert!(ready.warning.is_none());
    let s = &ready.session;
    assert_eq!(
        s.access_token.expose(),
        env.mock.with(|m| m.mc.clone()).unwrap()
    );
    assert_eq!(s.name, "Violet_Runner");
    assert_eq!(s.uuid, account.uuid);
    assert_eq!(s.user_type, "msa");
    assert_eq!(s.xuid, "2535400000000001");
    assert!(!s.client_id.is_empty());
    assert_eq!(env.calls("mc_login"), 1);
    // Nothing printable leaks a token.
    let printed = format!("{ready:?} {:?} {:?}", env.events(), env.auth);
    for t in env.mock.with(|m| m.all_tokens.clone()) {
        assert!(!printed.contains(&t), "Debug shows {t}");
    }
}

#[tokio::test]
async fn device_code_wins_after_pending_and_slow_down() {
    let env = env("device").await;
    env.mock.with(|s| {
        s.browser = BrowserMode::Ignore;
        s.device_interval = 2;
        s.polls = [Poll::Pending, Poll::SlowDown, Poll::Pending, Poll::Approve].into();
    });
    let account = env.login().await.unwrap();
    assert_eq!(account.name, "Violet_Runner");
    assert!(
        env.events()
            .iter()
            .any(|e| matches!(e, LoginEvent::Step(LoginStep::SignedIn(Method::DeviceCode))))
    );
    assert_eq!(env.calls("token:code"), 0);
    assert_eq!(env.calls("token:device"), 4);
    // Interval 2 units (20 ms each) until slow_down, then 2 + 5 = 7 units.
    let t = env.mock.with(|s| s.poll_times.clone());
    let gap = |i: usize| t[i + 1] - t[i];
    let unit = Duration::from_millis(20);
    assert!(gap(0) >= unit * 2 && gap(0) < unit * 7, "{:?}", gap(0));
    assert!(gap(1) >= unit * 7, "after slow_down: {:?}", gap(1));
    assert!(gap(2) >= unit * 7, "slow_down sticks: {:?}", gap(2));
}

#[tokio::test]
async fn device_code_expired_token_ends_that_path() {
    let env = env("expired").await;
    env.mock.with(|s| {
        s.browser = BrowserMode::Ignore;
        s.polls = [Poll::Pending, Poll::Expired].into();
    });
    // The browser path waits out its (test) timeout of 3 s; the device code is already gone.
    let err = env.login().await.unwrap_err();
    assert!(
        matches!(err, AuthError::TimedOut | AuthError::DeviceCodeExpired),
        "{err:?}"
    );
    assert!(env.events().iter().any(|e| matches!(
        e,
        LoginEvent::DeviceCodeFailed(AuthError::DeviceCodeExpired)
    )));
    assert_eq!(env.calls("token:device"), 2);
    assert!(env.auth.accounts().await.unwrap().accounts.is_empty());
}

#[tokio::test]
async fn device_code_expires_locally_too() {
    let env = env("expired-local").await;
    env.mock.with(|s| {
        s.browser = BrowserMode::Ignore;
        s.device_expires_in = 3; // 3 units of 20 ms, interval 1: a couple of polls at most
    });
    let err = env.login().await.unwrap_err();
    assert!(matches!(
        err,
        AuthError::TimedOut | AuthError::DeviceCodeExpired
    ));
    assert!(env.events().iter().any(|e| matches!(
        e,
        LoginEvent::DeviceCodeFailed(AuthError::DeviceCodeExpired)
    )));
    assert!(env.calls("token:device") <= 3);
}

#[tokio::test]
async fn a_declined_device_code_ends_the_login() {
    let env = env("declined").await;
    env.mock.with(|s| {
        s.browser = BrowserMode::Ignore;
        s.polls = [Poll::Declined].into();
    });
    assert_eq!(env.login().await.unwrap_err(), AuthError::Declined);
}

#[tokio::test]
async fn a_state_mismatch_is_rejected_and_the_code_never_used() {
    let env = env("state").await;
    env.mock.with(|s| {
        s.browser = BrowserMode::WrongState;
        // Public client flows off in the app registration: the device code path fails too.
        s.device_code_error = Some(serde_json::json!({
            "error": "invalid_client", "error_codes": [7000218],
            "error_description": "AADSTS7000218: The request body must contain client_assertion"}));
    });
    let err = env.login().await.unwrap_err();
    assert_eq!(err, AuthError::StateMismatch);
    assert_eq!(
        env.calls("token:code"),
        0,
        "a code with the wrong state must not be exchanged"
    );
    assert!(env.events().iter().any(|e| matches!(
        e,
        LoginEvent::DeviceCodeFailed(AuthError::AppMisconfigured(_))
    )));
    assert!(env.auth.accounts().await.unwrap().accounts.is_empty());
}

#[tokio::test]
async fn xsts_errors_become_reasons() {
    let env = env("xsts").await;
    for (code, want) in [
        (2148916233, XboxProblem::NoXboxAccount),
        (2148916235, XboxProblem::CountryUnavailable),
        (2148916236, XboxProblem::AgeVerification),
        (2148916237, XboxProblem::AgeVerification),
        (2148916238, XboxProblem::ChildAccount),
        (2148916227, XboxProblem::Banned),
        (2148916229, XboxProblem::GuardianBlocked),
        (2148916234, XboxProblem::TermsNotAccepted),
        (2148916262, XboxProblem::Other(2148916262)),
    ] {
        env.mock.with(|s| s.xsts_xerr = Some(code));
        assert_eq!(
            env.login().await.unwrap_err(),
            AuthError::Xbox(want),
            "{code}"
        );
    }
    assert!(env.auth.accounts().await.unwrap().accounts.is_empty());
}

#[tokio::test]
async fn ownership_and_approval_errors() {
    let env = env("owner").await;
    env.mock.with(|s| {
        s.profile_found = false;
        s.entitlements = vec![];
    });
    assert_eq!(env.login().await.unwrap_err(), AuthError::NotOwned);
    env.mock
        .with(|s| s.entitlements = vec!["product_minecraft", "game_minecraft"]);
    assert_eq!(env.login().await.unwrap_err(), AuthError::NoProfile);
    env.mock
        .with(|s| s.entitlements = vec!["product_game_pass_pc"]);
    assert_eq!(env.login().await.unwrap_err(), AuthError::GamePassNoProfile);
    // A Game Pass account with a profile plays (the profile decides).
    env.mock.with(|s| s.profile_found = true);
    assert_eq!(env.login().await.unwrap().ownership, Ownership::GamePass);

    env.mock.with(|s| {
        s.mc_login_status = Some((
            403,
            r#"{"path":"/authentication/login_with_xbox","errorMessage":"Invalid app registration, see https://aka.ms/AppRegInfo for more information"}"#,
        ))
    });
    assert_eq!(env.login().await.unwrap_err(), AuthError::ClientNotApproved);
    env.mock.with(|s| {
        s.mc_login_status = Some((429, ""));
        s.mc_login_retry_after = Some("30");
    });
    assert_eq!(
        env.login().await.unwrap_err(),
        AuthError::RateLimited {
            retry_after: Some(Duration::from_secs(30))
        }
    );
}

#[tokio::test]
async fn launch_renews_from_the_latest_good_step() {
    let env = env("margins").await;
    // A Minecraft token with 7 h left is under the 8 h launch margin.
    env.mock.with(|s| s.mc_expires_in = 7 * 3600);
    let account = env.login().await.unwrap();
    let old = env.mock.with(|s| s.mc.clone().unwrap());
    let ready = env.auth.prepare_launch(account.uuid).await.unwrap();
    assert_ne!(ready.session.access_token.expose(), old);
    // XSTS (16 h) was still good: only login_with_xbox again.
    assert_eq!(
        (
            env.calls("mc_login"),
            env.calls("xsts"),
            env.calls("token:refresh")
        ),
        (2, 1, 0)
    );

    // 9 h: fine for a launch, but the background refresh (12 h) renews it.
    env.mock.with(|s| s.mc_expires_in = 9 * 3600);
    env.auth.prepare_launch(account.uuid).await.unwrap(); // the 7 h token: renewed to 9 h
    assert_eq!(env.calls("mc_login"), 3);
    env.auth.prepare_launch(account.uuid).await.unwrap(); // 9 h left: cached
    assert_eq!(env.calls("mc_login"), 3);
    let r = env.auth.refresh_in_background().await;
    assert_eq!(r.len(), 1);
    assert!(r[0].1.is_ok());
    assert_eq!(env.calls("mc_login"), 4);

    // Short-lived XSTS and Xbox tokens (under the 5 min step margin) and MSA access token: the
    // next renewal starts from the refresh token, which rotates.
    env.mock.with(|s| {
        s.mc_expires_in = 60;
        s.xsts_lifetime_s = 60;
        s.xbl_lifetime_s = 60;
        s.msa_expires_in = 60;
    });
    env.login().await.unwrap(); // the same account again, now with 60 s tokens everywhere
    let before = env.calls("token:refresh");
    env.auth.prepare_launch(account.uuid).await.unwrap(); // all under margin: full refresh
    assert_eq!(env.calls("token:refresh"), before + 1);
    env.auth.prepare_launch(account.uuid).await.unwrap(); // again, with the rotated token
    assert_eq!(env.calls("token:refresh"), before + 2);
    assert_no_token_on_disk(&env);
}

#[tokio::test]
async fn a_revoked_refresh_token_asks_for_a_new_login() {
    let env = env("revoked").await;
    env.mock.with(|s| {
        s.mc_expires_in = 60;
        s.xsts_lifetime_s = 60;
        s.xbl_lifetime_s = 60;
        s.msa_expires_in = 60;
    });
    let account = env.login().await.unwrap();
    env.mock.with(|s| s.refresh_invalid = true);
    let err = env.auth.prepare_launch(account.uuid).await.unwrap_err();
    assert!(err.needs_relogin(), "{err:?}");
    let list = env.auth.accounts().await.unwrap();
    assert!(list.accounts[0].needs_login.is_some());
    assert!(files(&env.root.join("auth/tokens")).is_empty());
    // The background refresh leaves it alone now; a new login clears the badge.
    assert!(env.auth.refresh_in_background().await.is_empty());
    env.mock.with(|s| s.refresh_invalid = false);
    env.login().await.unwrap();
    assert!(
        env.auth.accounts().await.unwrap().accounts[0]
            .needs_login
            .is_none()
    );
}

#[tokio::test]
async fn offline_launch_uses_the_saved_token_with_a_warning() {
    let env = env("offline").await;
    env.mock.with(|s| s.mc_expires_in = 2 * 3600);
    let account = env.login().await.unwrap();
    let token = env.mock.with(|s| s.mc.clone().unwrap());
    env.mock.with(|s| s.offline = true);
    let ready = env.auth.prepare_launch(account.uuid).await.unwrap();
    assert_eq!(ready.session.access_token.expose(), token);
    assert!(
        matches!(ready.warning, Some(AuthError::Network(_))),
        "{:?}",
        ready.warning
    );
}

#[tokio::test]
async fn an_account_in_use_is_never_refreshed_in_the_background() {
    let env = env("inuse").await;
    env.mock.with(|s| s.mc_expires_in = 10 * 3600);
    let account = env.login().await.unwrap();
    let token = env.mock.with(|s| s.mc.clone().unwrap());
    let guard = env.auth.mark_in_use(account.uuid);
    // 10 h left is under the background margin, but a game runs with the account.
    assert!(env.auth.refresh_in_background().await.is_empty());
    // A second instance launched while the first runs shares the token (it has the 8 h).
    let ready = env.auth.prepare_launch(account.uuid).await.unwrap();
    assert_eq!(ready.session.access_token.expose(), token);
    assert!(ready.warning.is_none());
    assert_eq!(env.calls("mc_login"), 1);
    drop(guard);
    assert_eq!(env.auth.refresh_in_background().await.len(), 1);
    assert_eq!(env.calls("mc_login"), 2);
}

/// A crashed wall instance relaunched near the end of the token's day must not start with
/// minutes left: the launch renews even while other instances run (they keep their old token).
#[tokio::test]
async fn a_launch_while_in_use_still_needs_the_launch_margin() {
    let env = env("inuse-margin").await;
    env.mock.with(|s| s.mc_expires_in = 600);
    let account = env.login().await.unwrap();
    let old = env.mock.with(|s| s.mc.clone().unwrap());
    let _guard = env.auth.mark_in_use(account.uuid);
    env.mock.with(|s| s.mc_expires_in = 86400);
    let ready = env.auth.prepare_launch(account.uuid).await.unwrap();
    assert_ne!(ready.session.access_token.expose(), old);
    assert!(ready.session.expires_at - chrono::Utc::now() > chrono::Duration::hours(23));
    assert!(ready.warning.is_none());
    assert_eq!(env.calls("mc_login"), 2);
    // The next instance of the wall finds the fresh token: no second renewal.
    let next = env.auth.prepare_launch(account.uuid).await.unwrap();
    assert_eq!(
        next.session.access_token.expose(),
        ready.session.access_token.expose()
    );
    assert_eq!(env.calls("mc_login"), 2);
}

/// Offline, a wall of instances asks the network once, not once per instance.
#[tokio::test]
async fn a_failed_renewal_is_not_retried_for_every_instance() {
    let env = env("offline-wall").await;
    env.mock.with(|s| s.mc_expires_in = 2 * 3600);
    let account = env.login().await.unwrap();
    env.mock.with(|s| s.offline = true);
    let first = env.auth.prepare_launch(account.uuid).await.unwrap();
    assert!(matches!(first.warning, Some(AuthError::Network(_))));
    let tries = env.calls("dropped");
    assert!(tries > 0);
    for _ in 0..5 {
        let next = env.auth.prepare_launch(account.uuid).await.unwrap();
        assert!(matches!(next.warning, Some(AuthError::Network(_))));
    }
    assert_eq!(env.calls("dropped"), tries);
}

#[tokio::test]
async fn a_launch_with_a_cached_token_changes_nothing_on_disk() {
    let env = env("cached").await;
    let account = env.login().await.unwrap();
    let before = env.auth.accounts().await.unwrap().accounts[0].refreshed_at;
    let accounts_json = std::fs::read(env.root.join("accounts.json")).unwrap();
    let tokens_file = env
        .root
        .join("auth/tokens")
        .join(format!("{}.bin", account.uuid.as_hyphenated()));
    let sealed = std::fs::read(&tokens_file).unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    env.auth.prepare_launch(account.uuid).await.unwrap();
    assert_eq!(
        env.auth.accounts().await.unwrap().accounts[0].refreshed_at,
        before
    );
    assert_eq!(
        std::fs::read(env.root.join("accounts.json")).unwrap(),
        accounts_json
    );
    assert_eq!(std::fs::read(&tokens_file).unwrap(), sealed);
}

/// Microsoft's "the user must be present" errors on the refresh token ask for a new login (the
/// "Log in again" badge); `temporarily_unavailable` is a passing error (launch with a warning).
#[tokio::test]
async fn refresh_errors_that_need_the_player_or_just_time() {
    for (error, relogin) in [
        ("interaction_required", true),
        ("consent_required", true),
        ("login_required", true),
        ("temporarily_unavailable", false),
    ] {
        let env = env(error).await;
        env.mock.with(|s| {
            s.mc_expires_in = 3600;
            s.xsts_lifetime_s = 60;
            s.xbl_lifetime_s = 60;
            s.msa_expires_in = 60;
        });
        let account = env.login().await.unwrap();
        env.mock.with(|s| {
            s.refresh_error = Some(serde_json::json!({
                "error": error, "error_codes": [50076],
                "error_description": format!("AADSTS50076: {error}")}))
        });
        let r = env.auth.prepare_launch(account.uuid).await;
        assert_eq!(env.calls("token:refresh"), 1, "{error}");
        let badge = env.auth.accounts().await.unwrap().accounts[0]
            .needs_login
            .clone();
        if relogin {
            let err = r.unwrap_err();
            assert!(err.needs_relogin(), "{error}: {err:?}");
            assert!(badge.is_some(), "{error}");
        } else {
            let ready = r.unwrap();
            assert!(
                matches!(ready.warning, Some(AuthError::Service { status: 503, .. })),
                "{error}: {:?}",
                ready.warning
            );
            assert!(badge.is_none(), "{error}");
        }
    }
}

#[tokio::test]
async fn another_client_id_means_a_new_login() {
    let env = env("clientid").await;
    let account = env.login().await.unwrap();
    let mut other = env.mock.config();
    other.client_id = "99999999-2222-3333-4444-555555555555".into();
    let auth = Auth::with_keystore(
        &srl_core::Paths::new(&env.root),
        Some(other),
        KeystoreChoice::FileOnly,
    );
    // A refresh token works only with the app that got it, so another build asks for a new login
    // (even while the Minecraft token is fresh, like Prism).
    let err = auth.prepare_launch(account.uuid).await.unwrap_err();
    assert!(err.needs_relogin(), "{err:?}");
}

#[tokio::test]
async fn logging_out_removes_everything() {
    let env = env("logout").await;
    assert!(!env.auth.accounts().await.unwrap().signed_in_once);
    let account = env.login().await.unwrap();
    assert!(env.auth.accounts().await.unwrap().signed_in_once);
    env.auth.remove(account.uuid).await.unwrap();
    let list = env.auth.accounts().await.unwrap();
    assert!(list.accounts.is_empty() && list.active.is_none() && list.keystore.is_none());
    // The offline policy remembers that someone signed in here once.
    assert!(list.signed_in_once);
    let left: Vec<_> = files(&env.root)
        .into_iter()
        .filter(|p| !p.ends_with("accounts.json"))
        .collect();
    assert!(left.is_empty(), "{left:?}");
    assert_eq!(
        env.auth.prepare_launch(account.uuid).await.unwrap_err(),
        AuthError::UnknownAccount
    );
}

#[tokio::test]
async fn without_a_client_id_nothing_reaches_the_network() {
    let root = root("noclient");
    let auth = Auth::with_keystore(&srl_core::Paths::new(&root), None, KeystoreChoice::FileOnly);
    assert!(!auth.is_available());
    assert_eq!(
        auth.login(Arc::new(|_| {})).await.unwrap_err(),
        AuthError::NotAvailable
    );
    assert!(auth.accounts().await.unwrap().accounts.is_empty());
    assert!(auth.refresh_in_background().await.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_cached_step_the_server_revoked_falls_back_to_the_refresh_token() {
    let env = env("revoked-step").await;
    env.mock.with(|s| s.mc_expires_in = 7 * 3600);
    let account = env.login().await.unwrap();
    // The server forgets our XSTS token early: login_with_xbox answers 401 to it.
    env.mock.with(|s| s.xsts = Some("forgotten".into()));
    let ready = env.auth.prepare_launch(account.uuid).await.unwrap();
    assert!(ready.warning.is_none());
    assert_eq!(env.calls("token:refresh"), 1);
    assert_eq!(env.calls("mc_login"), 3); // 1 at login, 1 refused, 1 after the full chain
}
