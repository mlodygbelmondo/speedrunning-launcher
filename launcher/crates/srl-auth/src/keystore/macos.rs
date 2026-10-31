//! macOS: a generic password in the login Keychain. Adapted from Pandora Launcher's
//! `crates/auth/src/secret.rs` (MIT); uses the simpler `passwords` API of security-framework.
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

use super::SERVICE;

/// `errSecItemNotFound`.
const NOT_FOUND: i32 = -25300;

pub async fn get(name: &str) -> Result<Option<Vec<u8>>, String> {
    let name = name.to_string();
    blocking(move || match get_generic_password(SERVICE, &name) {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.code() == NOT_FOUND => Ok(None),
        Err(e) => Err(e.to_string()),
    })
    .await
}

pub async fn set(name: &str, value: &[u8]) -> Result<(), String> {
    let (name, value) = (name.to_string(), zeroize::Zeroizing::new(value.to_vec()));
    blocking(move || set_generic_password(SERVICE, &name, &value).map_err(|e| e.to_string())).await
}

pub async fn delete(name: &str) -> Result<(), String> {
    let name = name.to_string();
    blocking(move || match delete_generic_password(SERVICE, &name) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == NOT_FOUND => Ok(()),
        Err(e) => Err(e.to_string()),
    })
    .await
}

/// Keychain calls block (and may show a prompt): keep them off the async workers.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}
