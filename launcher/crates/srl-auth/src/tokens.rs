//! The token chain of one account, cached step by step, and the decision of what to renew.
//!
//! Prism re-runs the whole chain from the refresh token every time. We keep every step with its
//! expiry and restart from the latest step that is still good (research 1.5), which means fewer
//! calls to Minecraft's login endpoint, where rate limits bite in practice.
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    secret::Secret,
    xbox::{MinecraftToken, XboxToken},
};

/// Before a launch, the Minecraft token must have this much left. A grinding session (a wall of
/// many instances) must not start with a token that runs out mid-session, and MCSR Ranked asks
/// `player/certificates` with it while the game runs.
pub const LAUNCH_MARGIN: Duration = Duration::hours(8);

/// The background refresh at launcher start renews tokens with less than this left, so Play
/// rarely has to wait for the network.
pub const BACKGROUND_MARGIN: Duration = Duration::hours(12);

/// An intermediate token (MSA access, Xbox, XSTS) is reused only with at least this much left, so
/// it can't expire between our check and the next request.
pub const STEP_MARGIN: Duration = Duration::minutes(5);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MsaAccess {
    pub token: Secret,
    pub expires_at: DateTime<Utc>,
}

/// Every secret of one account. Lives only in the encrypted token file (see `store`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tokens {
    /// The Entra app that issued the refresh token. A refresh token only works with the client
    /// that got it, so a build with another ID needs a new login (Prism does the same).
    pub client_id: String,
    pub msa_refresh: Secret,
    #[serde(default)]
    pub msa_access: Option<MsaAccess>,
    #[serde(default)]
    pub xbl: Option<XboxToken>,
    #[serde(default)]
    pub xsts: Option<XboxToken>,
    #[serde(default)]
    pub minecraft: Option<MinecraftToken>,
}

/// Where the chain restarts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// The Minecraft token has the margin left: nothing to do.
    UseCached,
    /// XSTS still good: only `login_with_xbox`.
    MinecraftLogin,
    /// Xbox user token still good: XSTS, then login.
    Xsts,
    /// MSA access token still good: Xbox, XSTS, login.
    XboxUser,
    /// Everything else: refresh the MSA token (it rotates), then the whole chain.
    MsaRefresh,
}

impl Tokens {
    /// What to renew so the Minecraft token has `margin` left at `now`.
    pub fn plan(&self, now: DateTime<Utc>, margin: Duration) -> Plan {
        let left = |at: DateTime<Utc>| at - now;
        if self
            .minecraft
            .as_ref()
            .is_some_and(|t| left(t.expires_at) >= margin)
        {
            Plan::UseCached
        } else if self
            .xsts
            .as_ref()
            .is_some_and(|t| left(t.expires_at) >= STEP_MARGIN)
        {
            Plan::MinecraftLogin
        } else if self
            .xbl
            .as_ref()
            .is_some_and(|t| left(t.expires_at) >= STEP_MARGIN)
        {
            Plan::Xsts
        } else if self
            .msa_access
            .as_ref()
            .is_some_and(|t| left(t.expires_at) >= STEP_MARGIN)
        {
            Plan::XboxUser
        } else {
            Plan::MsaRefresh
        }
    }

    /// The Minecraft token, if it hasn't expired at `now` (it may be inside the margin).
    pub fn usable_minecraft(&self, now: DateTime<Utc>) -> Option<&MinecraftToken> {
        self.minecraft.as_ref().filter(|t| t.expires_at > now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xbox(at: DateTime<Utc>) -> XboxToken {
        XboxToken {
            token: Secret::new("x"),
            uhs: "u".into(),
            xid: None,
            expires_at: at,
        }
    }

    fn tokens(now: DateTime<Utc>, mc: i64, xsts: i64, xbl: i64, msa: i64) -> Tokens {
        let at = |mins: i64| now + Duration::minutes(mins);
        Tokens {
            client_id: "c".into(),
            msa_refresh: Secret::new("r"),
            msa_access: Some(MsaAccess {
                token: Secret::new("a"),
                expires_at: at(msa),
            }),
            xbl: Some(xbox(at(xbl))),
            xsts: Some(xbox(at(xsts))),
            minecraft: Some(MinecraftToken {
                token: Secret::new("m"),
                expires_at: at(mc),
            }),
        }
    }

    #[test]
    fn launch_needs_eight_hours_left() {
        let now = Utc::now();
        let h = 60;
        // 8 h 1 min left: launch as is. 7 h 59 min: renew from XSTS.
        assert_eq!(
            tokens(now, 8 * h + 1, 600, 600, 60).plan(now, LAUNCH_MARGIN),
            Plan::UseCached
        );
        assert_eq!(
            tokens(now, 8 * h, 600, 600, 60).plan(now, LAUNCH_MARGIN),
            Plan::UseCached
        );
        assert_eq!(
            tokens(now, 8 * h - 1, 600, 600, 60).plan(now, LAUNCH_MARGIN),
            Plan::MinecraftLogin
        );
        // The background refresh is more eager: 11 h left already renews.
        assert_eq!(
            tokens(now, 11 * h, 600, 600, 60).plan(now, BACKGROUND_MARGIN),
            Plan::MinecraftLogin
        );
        assert_eq!(
            tokens(now, 13 * h, 600, 600, 60).plan(now, BACKGROUND_MARGIN),
            Plan::UseCached
        );
    }

    #[test]
    fn restarts_from_the_latest_good_step() {
        let now = Utc::now();
        // XSTS with 4 min left is inside the 5 min step margin: start from the Xbox token.
        assert_eq!(
            tokens(now, 0, 4, 600, 60).plan(now, LAUNCH_MARGIN),
            Plan::Xsts
        );
        assert_eq!(
            tokens(now, 0, 0, 5, 60).plan(now, LAUNCH_MARGIN),
            Plan::Xsts
        );
        assert_eq!(
            tokens(now, 0, 0, 4, 60).plan(now, LAUNCH_MARGIN),
            Plan::XboxUser
        );
        assert_eq!(
            tokens(now, 0, 0, 0, 4).plan(now, LAUNCH_MARGIN),
            Plan::MsaRefresh
        );
        let mut empty = tokens(now, 0, 0, 0, 0);
        empty.msa_access = None;
        empty.xbl = None;
        empty.xsts = None;
        empty.minecraft = None;
        assert_eq!(empty.plan(now, LAUNCH_MARGIN), Plan::MsaRefresh);
    }

    #[test]
    fn usable_token_may_be_inside_the_margin() {
        let now = Utc::now();
        let t = tokens(now, 30, 0, 0, 0);
        assert!(t.usable_minecraft(now).is_some());
        assert!(t.usable_minecraft(now + Duration::minutes(31)).is_none());
    }

    #[test]
    fn debug_shows_no_token() {
        let now = Utc::now();
        let mut t = tokens(now, 1, 1, 1, 1);
        t.msa_refresh = Secret::new("REFRESH-SECRET");
        assert!(!format!("{t:?}").contains("REFRESH-SECRET"));
    }
}
