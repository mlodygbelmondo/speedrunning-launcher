//! Everything that can go wrong in a login or a refresh, each with a reason a player can act on.
//!
//! `Display` is the English message. The app's copy files translate by `copy_key()`
//! (`copy/en.json` and `copy/pl.json` next to this crate hold the texts to merge into the app's).
//! No variant ever carries a token: server messages are kept only after `clip()`, and the services
//! never echo tokens in their error bodies.
use std::time::Duration;

/// Why Xbox Live refused the account (`XErr` on HTTP 401 from `xsts/authorize`). The codes are
/// from minecraft.wiki and Prism's `XboxAuthorizationStep::processSTSError` (research 1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XboxProblem {
    /// 2148916233: the account never used Xbox services, so it has no Xbox profile.
    NoXboxAccount,
    /// 2148916235: Xbox Live is not available in the account's country or region.
    CountryUnavailable,
    /// 2148916236 and 2148916237: South Korean age verification. Prism reads 237 as "playtime
    /// limit reached"; the sources disagree, so the message names both.
    AgeVerification,
    /// 2148916238: an account under 18 that no adult has added to a Microsoft family.
    ChildAccount,
    /// 2148916227: banned from Xbox Live.
    Banned,
    /// 2148916229: a parent or guardian has not allowed online play.
    GuardianBlocked,
    /// 2148916234: the Xbox terms of use have not been accepted yet.
    TermsNotAccepted,
    /// Any other code (2148916262 is listed on minecraft.wiki without a meaning).
    Other(u64),
}

impl XboxProblem {
    pub fn from_xerr(code: u64) -> Self {
        match code {
            2148916233 => Self::NoXboxAccount,
            2148916235 => Self::CountryUnavailable,
            2148916236 | 2148916237 => Self::AgeVerification,
            2148916238 => Self::ChildAccount,
            2148916227 => Self::Banned,
            2148916229 => Self::GuardianBlocked,
            2148916234 => Self::TermsNotAccepted,
            other => Self::Other(other),
        }
    }

    pub fn copy_key(self) -> &'static str {
        match self {
            Self::NoXboxAccount => "auth.error.xbox_no_account",
            Self::CountryUnavailable => "auth.error.xbox_country",
            Self::AgeVerification => "auth.error.xbox_age_verification",
            Self::ChildAccount => "auth.error.xbox_child_account",
            Self::Banned => "auth.error.xbox_banned",
            Self::GuardianBlocked => "auth.error.xbox_guardian",
            Self::TermsNotAccepted => "auth.error.xbox_terms",
            Self::Other(_) => "auth.error.xbox_other",
        }
    }

    /// Where the player fixes it, for a button next to the message.
    pub fn help_url(self) -> Option<&'static str> {
        match self {
            Self::NoXboxAccount | Self::TermsNotAccepted => Some("https://www.xbox.com/live"),
            Self::AgeVerification => Some("https://login.live.com"),
            Self::ChildAccount => {
                Some("https://help.minecraft.net/hc/en-us/articles/4408968616077")
            }
            Self::GuardianBlocked => Some("https://account.microsoft.com/family"),
            Self::CountryUnavailable | Self::Banned | Self::Other(_) => None,
        }
    }

    fn message(self) -> String {
        match self {
            Self::NoXboxAccount => "This Microsoft account has no Xbox profile yet. Sign in once at xbox.com (or minecraft.net) to create one, then try again.".into(),
            Self::CountryUnavailable => "Xbox Live is not available in this account's country or region, so it cannot sign in to Minecraft. A VPN does not help: the block follows the account's region.".into(),
            Self::AgeVerification => "Microsoft needs age verification for this account (South Korea), or its playtime limit is reached. Sign in at login.live.com to verify, then try again.".into(),
            Self::ChildAccount => "This is a child account (under 18). An adult must add it to a Microsoft family before it can play online.".into(),
            Self::Banned => "This account is banned from Xbox Live, so it cannot sign in to Minecraft.".into(),
            Self::GuardianBlocked => "A parent or guardian has not allowed online play for this account. They can change it at account.microsoft.com/family.".into(),
            Self::TermsNotAccepted => "This account has not accepted the Xbox terms of use yet. Sign in once at xbox.com, accept them, then try again.".into(),
            Self::Other(code) => format!("Xbox Live refused the sign-in (error {code})."),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    /// This build has no client ID of its own (see `AuthConfig::detect`).
    #[error("Microsoft login isn't available in this build yet.")]
    NotAvailable,
    /// The player said no: closed the consent page with "No", or declined the device code on
    /// another device. Neutral wording, since it comes from either path.
    #[error("The sign-in was declined.")]
    Declined,
    /// Neither the browser nor the device code finished in time.
    #[error("The sign-in took too long and timed out. Start it again when you are ready.")]
    TimedOut,
    /// The device code expired before anyone entered it (15 minutes).
    #[error("The code expired before it was used. Start the sign-in again for a new one.")]
    DeviceCodeExpired,
    /// The browser came back with a `state` that isn't ours: a stale tab, or another program
    /// trying to slip its own code into our login. The code is not used, and the listener keeps
    /// waiting for the real redirect; this is reported only when the browser path times out with
    /// nothing better.
    #[error(
        "The browser returned a sign-in that this launcher did not start. Start the sign-in again."
    )]
    StateMismatch,
    /// Our Entra app registration is wrong: public client flows off (AADSTS70002/7000218), an
    /// unknown client ID (AADSTS700016), a redirect URI that isn't registered. Only the developer
    /// can fix it.
    #[error(
        "The launcher's Microsoft app registration is misconfigured ({0}). This is a bug in the build, not your account."
    )]
    AppMisconfigured(String),
    /// The saved sign-in can't be renewed (refresh token revoked or expired, password changed,
    /// the build's client ID changed, or the saved tokens were lost). Log in again.
    #[error("Your Microsoft sign-in has expired ({0}). Log in again.")]
    ReloginRequired(String),
    #[error("{}", .0.message())]
    Xbox(XboxProblem),
    /// Minecraft services answered 403 "Invalid app registration": Mojang hasn't allow-listed our
    /// client ID (yet). Never fall back to offline silently.
    #[error(
        "Mojang hasn't approved this launcher for Microsoft login yet, so Minecraft refused the sign-in."
    )]
    ClientNotApproved,
    /// No profile and no entitlement: this account doesn't own Minecraft: Java Edition.
    #[error(
        "This Microsoft account doesn't own Minecraft: Java Edition. If you play through PC Game Pass, start the game once in the official Minecraft Launcher, then try again."
    )]
    NotOwned,
    /// A Game Pass entitlement but no profile: the player has to pick a name first.
    #[error(
        "This account has Minecraft through PC Game Pass but no Minecraft profile yet. Start the game once in the official Minecraft Launcher (or pick a name at minecraft.net), then try again."
    )]
    GamePassNoProfile,
    /// Owns the game but has no profile (never picked a name).
    #[error(
        "This account owns Minecraft but has no profile name yet. Pick one at minecraft.net, then try again."
    )]
    NoProfile,
    #[error("Too many sign-in requests. Wait a moment and try again.")]
    RateLimited { retry_after: Option<Duration> },
    #[error("Couldn't reach {0}. Check your internet connection.")]
    Network(String),
    /// Any other answer we don't understand. `detail` is clipped and never holds a token.
    #[error("{service} answered with an error ({status}): {detail}")]
    Service {
        service: &'static str,
        status: u16,
        detail: String,
    },
    /// The OS keystore (or the fallback key file) can't be used right now: a locked keyring, a
    /// Secret Service that doesn't answer, an unreadable key file. The key may still be there, so
    /// even a login fails with this rather than replace it (which would sign every other account
    /// out). If the keyring is gone for good, logging out of every account resets the key.
    #[error("Couldn't open the saved sign-ins: {0}")]
    Keystore(String),
    #[error("Couldn't save or read the account list: {0}")]
    Storage(String),
    #[error("This account isn't in the launcher any more.")]
    UnknownAccount,
}

impl AuthError {
    /// The copy key the UI translates.
    pub fn copy_key(&self) -> &'static str {
        match self {
            Self::NotAvailable => "auth.error.not_available",
            Self::Declined => "auth.error.declined",
            Self::TimedOut => "auth.error.timed_out",
            Self::DeviceCodeExpired => "auth.error.device_code_expired",
            Self::StateMismatch => "auth.error.state_mismatch",
            Self::AppMisconfigured(_) => "auth.error.app_misconfigured",
            Self::ReloginRequired(_) => "auth.error.relogin_required",
            Self::Xbox(p) => p.copy_key(),
            Self::ClientNotApproved => "auth.error.client_not_approved",
            Self::NotOwned => "auth.error.not_owned",
            Self::GamePassNoProfile => "auth.error.game_pass_no_profile",
            Self::NoProfile => "auth.error.no_profile",
            Self::RateLimited { .. } => "auth.error.rate_limited",
            Self::Network(_) => "auth.error.network",
            Self::Service { .. } => "auth.error.service",
            Self::Keystore(_) => "auth.error.keystore",
            Self::Storage(_) => "auth.error.storage",
            Self::UnknownAccount => "auth.error.unknown_account",
        }
    }

    /// A page that helps, for a button next to the message.
    pub fn help_url(&self) -> Option<&'static str> {
        match self {
            Self::Xbox(p) => p.help_url(),
            Self::NotOwned => {
                Some("https://www.minecraft.net/store/minecraft-java-bedrock-edition-pc")
            }
            Self::NoProfile | Self::GamePassNoProfile => {
                Some("https://www.minecraft.net/msaprofile/mygames/editprofile")
            }
            _ => None,
        }
    }

    /// Worth trying again later without the player doing anything (a background refresh keeps
    /// the account and its tokens).
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Network(_) | Self::RateLimited { .. } => true,
            Self::Service { status, .. } => *status >= 500,
            _ => false,
        }
    }

    /// The saved sign-in is gone for good: the account needs an interactive login.
    pub fn needs_relogin(&self) -> bool {
        matches!(self, Self::ReloginRequired(_))
    }
}

/// A network failure, without the URL's query (which may hold a code) and without the body.
pub(crate) fn network(service: &'static str, err: &reqwest::Error) -> AuthError {
    if err.is_timeout() {
        AuthError::Network(format!("{service} (timed out)"))
    } else {
        AuthError::Network(service.to_string())
    }
}

/// Server text for `AuthError::Service`: one line, at most 300 characters.
pub(crate) fn clip(text: &str) -> String {
    let line: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let line = line.trim();
    match line.char_indices().nth(300) {
        Some((i, _)) => format!("{}...", &line[..i]),
        None => line.to_string(),
    }
}

/// `Retry-After` in seconds (the HTTP-date form is not used by these services).
pub(crate) fn retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
        .map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xerr_codes_map_to_reasons() {
        let cases = [
            (2148916233, XboxProblem::NoXboxAccount),
            (2148916235, XboxProblem::CountryUnavailable),
            (2148916236, XboxProblem::AgeVerification),
            (2148916237, XboxProblem::AgeVerification),
            (2148916238, XboxProblem::ChildAccount),
            (2148916227, XboxProblem::Banned),
            (2148916229, XboxProblem::GuardianBlocked),
            (2148916234, XboxProblem::TermsNotAccepted),
            (2148916262, XboxProblem::Other(2148916262)),
        ];
        for (code, want) in cases {
            assert_eq!(XboxProblem::from_xerr(code), want, "{code}");
        }
        let child = AuthError::Xbox(XboxProblem::ChildAccount);
        assert!(child.to_string().contains("family"));
        assert_eq!(child.copy_key(), "auth.error.xbox_child_account");
        assert!(child.help_url().is_some());
        assert!(
            AuthError::Xbox(XboxProblem::Other(7))
                .to_string()
                .contains("error 7")
        );
    }

    #[test]
    fn clip_keeps_one_short_line() {
        assert_eq!(clip(" a\nb "), "a b");
        let long = "x".repeat(400);
        assert_eq!(clip(&long).len(), 303);
    }

    #[test]
    fn transient_and_relogin() {
        assert!(AuthError::Network("x".into()).is_transient());
        assert!(
            AuthError::Service {
                service: "s",
                status: 503,
                detail: String::new()
            }
            .is_transient()
        );
        assert!(!AuthError::NotOwned.is_transient());
        assert!(AuthError::ReloginRequired("x".into()).needs_relogin());
    }

    /// Every error has its text in both copy files, so the UI never shows a bare key.
    #[test]
    fn every_error_has_copy_in_both_languages() {
        let s = String::new;
        let mut all = vec![
            AuthError::NotAvailable,
            AuthError::Declined,
            AuthError::TimedOut,
            AuthError::DeviceCodeExpired,
            AuthError::StateMismatch,
            AuthError::AppMisconfigured(s()),
            AuthError::ReloginRequired(s()),
            AuthError::ClientNotApproved,
            AuthError::NotOwned,
            AuthError::GamePassNoProfile,
            AuthError::NoProfile,
            AuthError::RateLimited { retry_after: None },
            AuthError::Network(s()),
            AuthError::Service {
                service: "s",
                status: 500,
                detail: s(),
            },
            AuthError::Keystore(s()),
            AuthError::Storage(s()),
            AuthError::UnknownAccount,
        ];
        for code in [
            2148916233, 2148916235, 2148916236, 2148916238, 2148916227, 2148916229, 2148916234, 1,
        ] {
            all.push(AuthError::Xbox(XboxProblem::from_xerr(code)));
        }
        for (name, json) in [
            ("en", include_str!("../copy/en.json")),
            ("pl", include_str!("../copy/pl.json")),
        ] {
            let table: std::collections::HashMap<String, String> =
                serde_json::from_str(json).unwrap();
            for e in &all {
                assert!(table.contains_key(e.copy_key()), "{name}: {}", e.copy_key());
            }
        }
    }
}
