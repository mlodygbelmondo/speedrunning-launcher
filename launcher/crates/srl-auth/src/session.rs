//! What the game needs from a Microsoft account to start online-capable.
//!
//! ## What srl-game passes
//!
//! `srl-game::launch::build_command` fills the version JSON's `${...}` placeholders. For a Microsoft
//! account they become (`MinecraftSession::placeholders` returns exactly this list):
//!
//! | Placeholder | Value | Game argument (version JSON) |
//! |---|---|---|
//! | `auth_player_name` | `name` | `--username` |
//! | `auth_uuid` | `uuid`, 32 hex digits without dashes | `--uuid` |
//! | `auth_access_token` | `access_token` | `--accessToken` |
//! | `auth_session` | `token:<access_token>:<uuid>` (pre-1.7 versions only) | `--session` |
//! | `user_type` | `msa` | `--userType` |
//! | `auth_xuid` | `xuid` (may be empty) | `--xuid` |
//! | `clientid` | `client_id` | `--clientId` |
//! | `user_properties` | `{}` | `--userProperties` (1.7-1.12 only) |
//!
//! 1.16.1 (RSG, Ranked) has `--username --version --gameDir --assetsDir --assetIndex --uuid
//! --accessToken --userType --versionType` in its `minecraftArguments`/`arguments.game`; the
//! newer versions add `--clientId` and `--xuid`. Whatever the version JSON doesn't ask for is not
//! passed. `user_type` is `msa` for every version: Prism and MultiMC pass it to 1.16.1 too.
//!
//! The access token goes on the command line because that is how Minecraft takes it; any local
//! process of the same user can read it there for its 24 hours. The launcher must mask it in any
//! log or command it shows (`MinecraftSession::redact`).
use uuid::Uuid;

use crate::secret::Secret;

/// Not `Serialize` on purpose: a session is built for one launch and never saved as is.
#[derive(Clone)]
pub struct MinecraftSession {
    pub name: String,
    pub uuid: Uuid,
    pub access_token: Secret,
    /// The Xbox user ID (the `xuid` claim of the access token), or empty if the token has none.
    /// The game only uses it for telemetry.
    pub xuid: String,
    /// The anonymous installation ID the game reports as `--clientId` (telemetry), like the
    /// official launcher's. Not our Entra client ID.
    pub client_id: String,
    /// Always `msa`.
    pub user_type: String,
    /// When `access_token` stops working. Not secret.
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

impl std::fmt::Debug for MinecraftSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MinecraftSession")
            .field("name", &self.name)
            .field("uuid", &self.uuid)
            .field("access_token", &self.access_token)
            .field("xuid", &self.xuid)
            .field("client_id", &self.client_id)
            .field("user_type", &self.user_type)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl MinecraftSession {
    /// The version JSON placeholders this session fills, as in the table above.
    pub fn placeholders(&self) -> Vec<(&'static str, String)> {
        let uuid = self.uuid.simple().to_string();
        let token = self.access_token.expose().to_string();
        vec![
            ("auth_player_name", self.name.clone()),
            ("auth_uuid", uuid.clone()),
            ("auth_session", format!("token:{token}:{uuid}")),
            ("auth_access_token", token),
            ("user_type", self.user_type.clone()),
            ("auth_xuid", self.xuid.clone()),
            ("clientid", self.client_id.clone()),
            ("user_properties", "{}".to_string()),
        ]
    }

    /// `text` with the access token masked: for the game's command line in logs and in the UI.
    pub fn redact(&self, text: &str) -> String {
        let token = self.access_token.expose();
        if token.is_empty() {
            text.to_string()
        } else {
            text.replace(token, "<access token>")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> MinecraftSession {
        MinecraftSession {
            name: "Violet_Runner".into(),
            uuid: Uuid::parse_str("069a79f4-44e9-4726-a5be-fca90e38aaf5").unwrap(),
            access_token: Secret::new("MC-ACCESS-TOKEN"),
            xuid: "2535400000000001".into(),
            client_id: "f00".into(),
            user_type: "msa".into(),
            expires_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn placeholders_for_the_game() {
        let s = session();
        let p: std::collections::HashMap<_, _> = s.placeholders().into_iter().collect();
        assert_eq!(p["auth_uuid"], "069a79f444e94726a5befca90e38aaf5");
        assert_eq!(p["auth_access_token"], "MC-ACCESS-TOKEN");
        assert_eq!(
            p["auth_session"],
            "token:MC-ACCESS-TOKEN:069a79f444e94726a5befca90e38aaf5"
        );
        assert_eq!(p["user_type"], "msa");
        assert_eq!(p["auth_xuid"], "2535400000000001");
        assert_eq!(p["clientid"], "f00");
    }

    #[test]
    fn debug_and_redact_hide_the_token() {
        let s = session();
        assert!(!format!("{s:?}").contains("MC-ACCESS-TOKEN"));
        assert_eq!(
            s.redact("java ... --accessToken MC-ACCESS-TOKEN --userType msa"),
            "java ... --accessToken <access token> --userType msa"
        );
    }
}
