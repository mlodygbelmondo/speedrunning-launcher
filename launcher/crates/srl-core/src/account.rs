//! Accounts. Only offline accounts for now: Microsoft login needs our own client ID approved by
//! Mojang, which Piotr decided to postpone (WORKLOG.md, 2026-10-26). Offline accounts play
//! singleplayer (RSG, FSG, SSG); MCSR Ranked and Draftout need a real Microsoft account.
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineAccount {
    pub name: String,
    pub uuid: Uuid,
}

impl OfflineAccount {
    /// The UUID vanilla servers give an offline player: a version 3 UUID of the MD5 of
    /// `OfflinePlayer:<name>`.
    pub fn new(name: &str) -> Self {
        let mut bytes: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
        bytes[6] = (bytes[6] & 0x0f) | 0x30;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self {
            name: name.to_string(),
            uuid: Uuid::from_bytes(bytes),
        }
    }

    /// A valid Minecraft name: 3 to 16 characters from `A-Z a-z 0-9 _`.
    pub fn is_valid_name(name: &str) -> bool {
        (3..=16).contains(&name.len())
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_vanilla() {
        // Known value: the offline UUID of "Notch".
        assert_eq!(
            OfflineAccount::new("Notch").uuid.to_string(),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
        assert!(OfflineAccount::is_valid_name("Violet_Runner"));
        assert!(!OfflineAccount::is_valid_name("no spaces"));
    }
}
