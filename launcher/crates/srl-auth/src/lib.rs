//! Microsoft login for Speedrunning Launcher: everything between "Log in with Microsoft" and the
//! `--accessToken` the game starts with. MCSR Ranked and Draftout need it: the Ranked mod asks
//! `api.minecraftservices.com/player/certificates` with the game's token, so an offline account
//! can't play them.
//!
//! Status: complete and tested against a local mock of every service, **switched off** until
//! Mojang approves our own client ID (`docs/microsoft-login.md`, in Polish, walks through the Azure
//! registration and Mojang's form). No live login has been done: there is no client ID yet.
//!
//! # How it works (research 03, section 1)
//!
//! 1. **Sign-in** (`Auth::login`): the system browser (authorization code + PKCE S256, redirect to
//!    a loopback listener on `127.0.0.1` and `[::1]`, same ephemeral port, `state` checked) and
//!    the device code
//!    flow (code + QR for a phone) run at the same time. The first to finish wins; the other is
//!    dropped. A "no" on either ends both.
//! 2. **Chain:** MSA (`consumers`, `XboxLive.signin offline_access`) -> Xbox Live user token ->
//!    XSTS for `rp://api.minecraftservices.com/` -> Minecraft `login_with_xbox` -> entitlements ->
//!    profile (name, UUID, skin). Every failure the research lists has its own `AuthError` with a
//!    reason the player can act on (XSTS `XErr` codes, no ownership, Game Pass without a profile,
//!    owns the game without a profile, our client ID not approved, rate limits).
//! 3. **Tokens:** each step is cached with its expiry; a renewal restarts from the latest step
//!    still good. `prepare_launch` renews when the Minecraft token has under 8 h left
//!    (`LAUNCH_MARGIN`); `refresh_in_background` when under 12 h (`BACKGROUND_MARGIN`). Tokens are
//!    encrypted on disk with a key kept in the OS keystore (Credential Manager, Keychain, Secret
//!    Service via oo7), or in a 0600 / DPAPI key file when there is none. `accounts.json` holds
//!    only names, UUIDs, skins and the active account. `Debug` never prints a token (`Secret`).
//! 4. **Client ID:** never in the source. `AuthConfig::detect()` reads `SRL_MSA_CLIENT_ID` at run
//!    time, else at build time (`option_env!`); without it Microsoft login is off.
//! 5. **Launch:** `MinecraftSession` (name, uuid, access_token, xuid, client_id, user_type "msa");
//!    `session.rs` documents exactly which game arguments srl-game fills from it.
//!
//! # API
//!
//! ```ignore
//! let auth = srl_auth::Auth::new(&paths, srl_auth::AuthConfig::detect());
//! auth.is_available();                       // false: "not available in this build yet"
//! auth.accounts().await?;                    // AccountList { accounts, active, keystore, signed_in_once }
//! auth.login(events).await?;                 // AccountInfo; events: Arc<dyn Fn(LoginEvent)>
//! auth.set_active(uuid).await?; auth.remove(uuid).await?;
//! let ready = auth.prepare_launch(uuid).await?;   // LaunchSession { session, warning }
//! let _guard = auth.mark_in_use(uuid);       // while the game runs: no background refresh
//! auth.refresh_in_background().await;        // at start, one account at a time
//! ```
//!
//! Everything async runs on the app's tokio runtime; nothing blocks a thread except the macOS
//! Keychain and Windows Credential Manager calls, which are short (macOS ones go through
//! `spawn_blocking`). To cancel a login, drop its future (abort the task). `SRL_KEYSTORE=file`
//! keeps the key in the file even when a keyring exists (agents and tests must set it, so they
//! never touch the keyring of the desktop they run on).
//!
//! # For the UI
//!
//! **Where:** an "Account" section (Settings, or the name field on Home). Until the build has a
//! client ID it shows one line: "Microsoft login isn't available in this build yet. Offline
//! accounts play RSG, FSG and SSG; Ranked and Draftout need it." (`auth.error.not_available`).
//! Never show a login button that can't work.
//!
//! **Empty state** (available, no accounts): "Log in with Microsoft" and one sentence on why:
//! Ranked and Draftout need a Microsoft account that owns Minecraft: Java Edition.
//!
//! **Loading (login):** one dialog, both paths at once, no "which method?" question:
//! - left: "Continue in your browser" with the state of `LoginEvent::Browser` (`opened == false`:
//!   "No browser opened. Copy this link:" and the URL);
//! - right: "Or on your phone": the QR (`qr_matrix(qr_url)`, quiet zone 4 modules), the address
//!   (`verification_uri`) and the `user_code` in large monospace, with a countdown to `expires_at`.
//!   Hide it on `DeviceCodeFailed`;
//! - after the Microsoft step: a progress line from `LoginEvent::Step` (Xbox, Minecraft, Profile);
//! - a Cancel button that aborts the task.
//!
//! **Accounts list:** face from `skin.url` (8x8 at (8,8) plus the hat layer at (40,8)), name, a
//! check on the active one, "Log out" (`remove`). `needs_login: Some(_)` shows a "Log in again"
//! badge. `keystore == Some(File)` adds a quiet note: "Saved sign-ins are protected by file
//! permissions only (the system keyring couldn't be used when they were first saved)." The key
//! never moves between the keyring and the file on its own: a keyring that is locked or slow later
//! gives `AuthError::Keystore` ("unlock it and try again"), also for a new login.
//!
//! **Errors:** show `AuthError` through `copy_key()` (texts in `copy/en.json` and `copy/pl.json`
//! next to this crate, to merge into the app's copy files), with a button to `help_url()` when it
//! has one. `ClientNotApproved` is not the player's fault: say "Mojang hasn't approved this
//! launcher yet", never fall back to offline silently.
//!
//! **Play:** before starting a game with a Microsoft account, `prepare_launch` (usually instant;
//! a few seconds when it renews; up to the 30 s HTTP timeout when the network hangs). Call it for
//! every instance: the first renews if needed, the rest find the fresh token, and after a passing
//! failure the next minute's calls answer at once with the same warning. On `warning: Some(_)`
//! start anyway and show "Couldn't renew the sign-in; it is valid until HH:MM"
//! (`session.expires_at`). On `needs_relogin()` errors, send the player to the login. On
//! `AuthError::Keystore`, say "unlock your keyring and try again" (it is not a lost sign-in).
//!
//! **What the copy may honestly claim:**
//! - "Your password never reaches the launcher": true, the player types it on Microsoft's page.
//! - "Tokens are stored encrypted, with the key in your system keyring": true when
//!   `keystore == Some(Os)`; with `File` say "protected by file permissions" instead.
//! - "The launcher sends your sign-in only to Microsoft, Xbox Live and Mojang": true for this
//!   crate; the game itself then uses the token (Ranked calls Mojang and the Ranked API).
//! - Do NOT claim the token is hidden from other programs: it is on the game's command line, as
//!   Minecraft requires, readable by any process of the same user for its 24 hours.
//! - Do NOT claim it works until Mojang has approved the client ID and a live login was tried.
pub mod auth;
pub mod config;
pub mod error;
pub mod keystore;
pub mod login;
pub mod loopback;
pub mod msa;
pub mod pkce;
pub mod secret;
pub mod session;
pub mod store;
pub mod tokens;
pub mod xbox;

pub use auth::{AccountList, Auth, InUse, LaunchSession};
pub use config::{AuthConfig, BrowserOpener, CLIENT_ID_ENV, Endpoints};
pub use error::{AuthError, XboxProblem};
pub use keystore::{KEYSTORE_ENV, KeystoreChoice, KeystoreKind};
pub use login::{LoginEvent, LoginEventFn, LoginStep, Method, QrMatrix, qr_matrix};
pub use secret::Secret;
pub use session::MinecraftSession;
pub use store::{AccountInfo, Ownership};
pub use tokens::{BACKGROUND_MARGIN, LAUNCH_MARGIN, STEP_MARGIN};
pub use xbox::Skin;
