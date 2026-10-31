//! Linux: the Secret Service through oo7 (GNOME Keyring, KWallet; the Secret portal in Flatpak).
//! Adapted from Pandora Launcher's `crates/auth/src/secret.rs` (MIT): one item per key, found by
//! its attributes, `replace = true` on write.
use std::time::Duration;

use super::SERVICE;

/// Connecting needs no prompt: if the bus or the service isn't there, it fails fast, and if
/// it hangs (a service that is being activated and never comes up) we give up and use the file.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

async fn keyring() -> Result<oo7::Keyring, String> {
    let keyring = tokio::time::timeout(CONNECT_TIMEOUT, oo7::Keyring::new())
        .await
        .map_err(|_| "the Secret Service did not answer".to_string())?
        .map_err(|e| format!("no Secret Service: {e}"))?;
    // May show the keyring's own unlock prompt, on the player's desktop: that is the point.
    keyring
        .unlock()
        .await
        .map_err(|e| format!("the keyring stayed locked: {e}"))?;
    Ok(keyring)
}

fn attributes(name: &str) -> [(&'static str, &str); 2] {
    [("application", SERVICE), ("srl-entry", name)]
}

pub async fn get(name: &str) -> Result<Option<Vec<u8>>, String> {
    let keyring = keyring().await?;
    let items = keyring
        .search_items(&attributes(name))
        .await
        .map_err(|e| e.to_string())?;
    match items.first() {
        None => Ok(None),
        Some(item) => {
            let secret = item.secret().await.map_err(|e| e.to_string())?;
            Ok(Some(secret.as_bytes().to_vec()))
        }
    }
}

pub async fn set(name: &str, value: &[u8]) -> Result<(), String> {
    let keyring = keyring().await?;
    keyring
        .create_item(
            "Speedrunning Launcher sign-in key",
            &attributes(name),
            value,
            true,
        )
        .await
        .map_err(|e| e.to_string())
}

pub async fn delete(name: &str) -> Result<(), String> {
    let keyring = keyring().await?;
    keyring
        .delete(&attributes(name))
        .await
        .map_err(|e| e.to_string())
}
