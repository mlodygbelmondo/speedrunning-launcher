//! What a build needs to log in: our own Entra application (client) ID and the service URLs.
//!
//! The client ID is never written in the source. Mojang allow-lists each launcher's own ID, and
//! Microsoft's identity platform terms forbid using another application's credentials, so a fork or
//! a dev build without its own ID simply has no Microsoft login (`AuthConfig::detect()` is `None`).
use std::{sync::Arc, time::Duration};

use reqwest::Url;

/// The variable that carries the client ID: read at build time (`option_env!`), and at run time,
/// where it wins, so a new ID can be tried without a rebuild.
pub const CLIENT_ID_ENV: &str = "SRL_MSA_CLIENT_ID";

/// The ID baked in at build time, if the build set `SRL_MSA_CLIENT_ID`.
const BUILD_CLIENT_ID: Option<&str> = option_env!("SRL_MSA_CLIENT_ID");

/// Client IDs of other launchers. Using one would break Microsoft's terms and get both launchers in
/// trouble with Mojang, so `detect` refuses them even when someone sets one on purpose.
const OTHER_LAUNCHERS: &[&str] = &[
    // Pandora Launcher, `crates/auth/src/constants.rs` (read in ~/.cache/srl-ref).
    "e5226706-5096-431d-9516-ae48fe263401",
    // Prism Launcher and MultiMC, from memory of their CMake files: not checked against source.
    "c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb",
    "499546d9-bbfe-4b9b-a086-eb3d75afb78f",
];

/// Opens a URL in the user's default browser. The default calls the OS (`open` crate: `xdg-open`,
/// `ShellExecute`, `open`); tests replace it with a fake browser.
pub type BrowserOpener = Arc<dyn Fn(&str) -> std::io::Result<()> + Send + Sync>;

/// Every URL the login talks to. All of them are configurable so tests run the whole chain against
/// a local mock server.
#[derive(Clone, Debug)]
pub struct Endpoints {
    pub authorize: Url,
    pub token: Url,
    pub device_code: Url,
    pub xbox_user: Url,
    pub xsts: Url,
    pub minecraft_login: Url,
    pub entitlements: Url,
    pub profile: Url,
}

impl Endpoints {
    /// The real services. `consumers`: `XboxLive.signin` only works for personal Microsoft accounts.
    pub fn microsoft() -> Self {
        let u = |s: &str| Url::parse(s).expect("static URL");
        Self {
            authorize: u("https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize"),
            token: u("https://login.microsoftonline.com/consumers/oauth2/v2.0/token"),
            device_code: u("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode"),
            xbox_user: u("https://user.auth.xboxlive.com/user/authenticate"),
            xsts: u("https://xsts.auth.xboxlive.com/xsts/authorize"),
            minecraft_login: u("https://api.minecraftservices.com/authentication/login_with_xbox"),
            entitlements: u("https://api.minecraftservices.com/entitlements/license"),
            profile: u("https://api.minecraftservices.com/minecraft/profile"),
        }
    }

    /// Every endpoint under one base URL, with the real paths: what the mock server serves.
    pub fn under(base: &Url) -> Self {
        let real = Self::microsoft();
        let at = |u: &Url| base.join(&u.path()[1..]).expect("path joins a base URL");
        Self {
            authorize: at(&real.authorize),
            token: at(&real.token),
            device_code: at(&real.device_code),
            xbox_user: at(&real.xbox_user),
            xsts: at(&real.xsts),
            minecraft_login: at(&real.minecraft_login),
            entitlements: at(&real.entitlements),
            profile: at(&real.profile),
        }
    }
}

/// Everything a login needs. `None` from `detect()` means this build has no Microsoft login.
#[derive(Clone)]
pub struct AuthConfig {
    /// Our Entra application (client) ID, a GUID.
    pub client_id: String,
    pub endpoints: Endpoints,
    /// The host in the loopback redirect URI, which must match the app registration exactly
    /// (only the port may differ). The Azure portal accepts `http://localhost` but rejects
    /// `http://127.0.0.1` outside the manifest editor, so the default is `localhost`. The listener
    /// itself binds 127.0.0.1 (and [::1] on the same port when it can, as browsers may resolve
    /// `localhost` to either).
    pub redirect_host: String,
    /// How long the browser flow waits for the redirect before it gives up (the device code flow
    /// ends when its code expires, 15 minutes at Microsoft).
    pub browser_timeout: Duration,
    /// One "second" of the device code polling interval. One second in production; tests shrink it
    /// so `slow_down` (+5 s) does not make them slow.
    pub poll_unit: Duration,
    pub open_browser: BrowserOpener,
}

impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthConfig")
            .field("client_id", &self.client_id)
            .field("endpoints", &self.endpoints)
            .field("redirect_host", &self.redirect_host)
            .field("browser_timeout", &self.browser_timeout)
            .field("poll_unit", &self.poll_unit)
            .finish_non_exhaustive()
    }
}

impl AuthConfig {
    /// The production config for `client_id`, or `None` if it isn't a usable ID of our own.
    pub fn new(client_id: &str) -> Option<Self> {
        let client_id = client_id.trim().to_ascii_lowercase();
        if !is_guid(&client_id) || OTHER_LAUNCHERS.contains(&client_id.as_str()) {
            return None;
        }
        Some(Self {
            client_id,
            endpoints: Endpoints::microsoft(),
            redirect_host: "localhost".to_string(),
            browser_timeout: Duration::from_secs(10 * 60),
            poll_unit: Duration::from_secs(1),
            open_browser: Arc::new(|url| open::that_detached(url)),
        })
    }

    /// The client ID from `SRL_MSA_CLIENT_ID` at run time, else at build time. `None` (no Microsoft
    /// login in this build) when neither is set, or the value is not a GUID, or it belongs to
    /// another launcher.
    pub fn detect() -> Option<Self> {
        let runtime = std::env::var(CLIENT_ID_ENV).ok();
        Self::from_sources(runtime.as_deref(), BUILD_CLIENT_ID)
    }

    fn from_sources(runtime: Option<&str>, build: Option<&str>) -> Option<Self> {
        fn pick(v: Option<&str>) -> Option<&str> {
            v.map(str::trim).filter(|v| !v.is_empty())
        }
        Self::new(pick(runtime).or(pick(build))?)
    }
}

/// `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`, hex digits.
fn is_guid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, n)| g.len() == n && g.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OURS: &str = "11111111-2222-3333-4444-555555555555";

    #[test]
    fn client_id_sources() {
        assert!(AuthConfig::from_sources(None, None).is_none());
        assert!(AuthConfig::from_sources(Some("  "), None).is_none());
        assert_eq!(
            AuthConfig::from_sources(None, Some(OURS))
                .unwrap()
                .client_id,
            OURS
        );
        // Run time wins over build time.
        let other = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        assert_eq!(
            AuthConfig::from_sources(Some(other), Some(OURS))
                .unwrap()
                .client_id,
            other
        );
        // Not a GUID, or another launcher's ID: no login.
        assert!(AuthConfig::from_sources(Some("00000000402b5328"), None).is_none());
        assert!(
            AuthConfig::from_sources(Some("E5226706-5096-431D-9516-AE48FE263401"), None).is_none()
        );
    }

    #[test]
    fn endpoints_under_a_base_keep_the_paths() {
        let base = Url::parse("http://127.0.0.1:9/").unwrap();
        let e = Endpoints::under(&base);
        assert_eq!(
            e.token.as_str(),
            "http://127.0.0.1:9/consumers/oauth2/v2.0/token"
        );
        assert_eq!(e.xsts.as_str(), "http://127.0.0.1:9/xsts/authorize");
        assert_eq!(e.profile.as_str(), "http://127.0.0.1:9/minecraft/profile");
    }
}
