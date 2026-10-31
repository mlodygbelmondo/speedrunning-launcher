//! Accounts on disk.
//!
//! ```text
//! <root>/accounts.json            non-secret metadata: names, UUIDs, skins, the active account,
//!                                 where the token key lives (never a token)
//! <root>/auth/key-<key_id>        the token key, only when there is no OS keystore (0600 / DPAPI)
//! <root>/auth/tokens/<uuid>.bin   each account's tokens, XChaCha20-Poly1305 with the token key
//! ```
//!
//! Several accounts work side by side; one is active (the one Play uses).
use std::path::{Path, PathBuf};

use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    error::AuthError,
    keystore::{Key, Keystore, KeystoreKind, Loaded, create_private_dir, write_private},
    pkce::random_bytes,
    tokens::Tokens,
    xbox::Skin,
};

/// How the account has the game, as far as the entitlements say. For display only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ownership {
    Purchased,
    GamePass,
    /// The profile exists (so the game can be played) but the entitlements didn't say how.
    #[default]
    Unknown,
}

/// One account as the UI shows it. Nothing here is secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountInfo {
    pub uuid: Uuid,
    pub name: String,
    #[serde(default)]
    pub skin: Option<Skin>,
    #[serde(default)]
    pub ownership: Ownership,
    pub added_at: DateTime<Utc>,
    /// The last time the token chain was renewed.
    #[serde(default)]
    pub refreshed_at: Option<DateTime<Utc>>,
    /// Set when the saved sign-in can't be renewed any more (the reason, in English): the account
    /// stays in the list with a "log in again" badge, its tokens are gone.
    #[serde(default)]
    pub needs_login: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct AccountsFile {
    #[serde(default = "version")]
    pub version: u32,
    #[serde(default)]
    pub active: Option<Uuid>,
    /// An anonymous ID of this installation, passed to the game as `--clientId` like the official
    /// launcher's (the game uses it for telemetry). Not the Entra client ID.
    #[serde(default)]
    pub install_id: String,
    #[serde(default)]
    pub key_id: Option<String>,
    #[serde(default)]
    pub key_store: Option<KeystoreKind>,
    #[serde(default)]
    pub accounts: Vec<AccountInfo>,
    /// A login has finished here at least once, with an account that has a Minecraft profile.
    /// Logging out doesn't clear it: the app's offline policy (`play_as` in srl-app) lets a
    /// build with Microsoft login play offline only after that.
    #[serde(default)]
    pub signed_in_once: bool,
}

fn version() -> u32 {
    1
}

const MAGIC: &[u8; 4] = b"SRLT";
const FORMAT: u8 = 1;
const NONCE_LEN: usize = 24;

pub(crate) struct Store {
    root: PathBuf,
    keystore: Keystore,
    key: Option<Key>,
}

impl Store {
    pub fn new(root: &Path, keystore_choice: crate::keystore::KeystoreChoice) -> Self {
        Self {
            root: root.to_path_buf(),
            keystore: Keystore::new(keystore_choice, &root.join("auth")),
            key: None,
        }
    }

    pub fn accounts_path(root: &Path) -> PathBuf {
        root.join("accounts.json")
    }

    fn tokens_path(&self, uuid: Uuid) -> PathBuf {
        self.root
            .join("auth")
            .join("tokens")
            .join(format!("{}.bin", uuid.as_hyphenated()))
    }

    /// `accounts.json`, or an empty list if there is none yet. A damaged file is an error rather
    /// than an empty list, so a bad write never silently signs everyone out.
    pub fn load(root: &Path) -> Result<AccountsFile, AuthError> {
        let path = Self::accounts_path(root);
        let mut file = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<AccountsFile>(&bytes)
                .map_err(|e| AuthError::Storage(format!("{}: {e}", path.display())))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AccountsFile::default(),
            Err(e) => return Err(AuthError::Storage(format!("{}: {e}", path.display()))),
        };
        if file.install_id.is_empty() {
            file.install_id = Uuid::new_v4().simple().to_string();
        }
        file.version = 1;
        Ok(file)
    }

    pub fn save(&self, file: &AccountsFile) -> Result<(), AuthError> {
        let json =
            serde_json::to_vec_pretty(file).map_err(|e| AuthError::Storage(e.to_string()))?;
        write_private(&Self::accounts_path(&self.root), &json)
            .map_err(|e| AuthError::Storage(format!("accounts.json: {e}")))
    }

    /// The token key. `create`: make one if there is none (a login); otherwise a missing key
    /// means the saved sign-ins are lost.
    ///
    /// A new key replaces the old one only when the old one is gone or unusable for good. A
    /// keystore that can't be reached right now (a locked keyring whose prompt was dismissed, a
    /// Secret Service that is slow to start, `SRL_KEYSTORE=file` against an OS key) is an error
    /// even for a login: replacing the key would sign every other account out for good and could
    /// quietly move the key from the keyring to a file.
    async fn key(&mut self, file: &mut AccountsFile, create: bool) -> Result<Key, AuthError> {
        if let Some(k) = &self.key {
            return Ok(k.clone());
        }
        if let (Some(id), Some(kind)) = (file.key_id.clone(), file.key_store) {
            match self.keystore.load(kind, &id).await? {
                Loaded::Found(k) => {
                    self.key = Some(k.clone());
                    return Ok(k);
                }
                Loaded::Unusable(reason) if !create => {
                    return Err(AuthError::ReloginRequired(reason));
                }
                Loaded::Unusable(_) => {
                    // Whatever is left of it (a damaged entry, an exposed file) goes.
                    self.keystore.delete(kind, &id).await;
                }
            }
        }
        if !create {
            return Err(AuthError::ReloginRequired("no saved sign-in".into()));
        }
        let id = hex::encode(random_bytes::<8>());
        let (key, kind) = self.keystore.create(&id).await?;
        file.key_id = Some(id);
        file.key_store = Some(kind);
        // Every account's tokens were sealed with the old key: they are gone.
        for a in &mut file.accounts {
            if a.needs_login.is_none() {
                a.needs_login = Some("the saved sign-in key was replaced".into());
            }
        }
        self.save(file)?;
        self.key = Some(key.clone());
        Ok(key)
    }

    pub async fn read_tokens(
        &mut self,
        file: &mut AccountsFile,
        uuid: Uuid,
    ) -> Result<Tokens, AuthError> {
        let path = self.tokens_path(uuid);
        let sealed = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(AuthError::ReloginRequired("no saved sign-in".into()));
            }
            Err(e) => return Err(AuthError::Storage(format!("{}: {e}", path.display()))),
        };
        let key = self.key(file, false).await?;
        let plain = open(&key, uuid, &sealed).ok_or_else(|| {
            AuthError::ReloginRequired("the saved sign-in can't be decrypted".into())
        })?;
        serde_json::from_slice(&plain)
            .map_err(|_| AuthError::ReloginRequired("the saved sign-in is damaged".into()))
    }

    pub async fn write_tokens(
        &mut self,
        file: &mut AccountsFile,
        uuid: Uuid,
        tokens: &Tokens,
    ) -> Result<(), AuthError> {
        let key = self.key(file, true).await?;
        let plain = Zeroizing::new(
            serde_json::to_vec(tokens).map_err(|e| AuthError::Storage(e.to_string()))?,
        );
        let sealed = seal(&key, uuid, &plain);
        let path = self.tokens_path(uuid);
        create_private_dir(path.parent().expect("tokens path has a parent"))
            .and_then(|_| write_private(&path, &sealed))
            .map_err(|e| AuthError::Storage(format!("token file: {e}")))
    }

    pub fn delete_tokens(&self, uuid: Uuid) {
        let _ = std::fs::remove_file(self.tokens_path(uuid));
    }

    /// No accounts left: drop the key too, so nothing of ours stays in the keystore.
    pub async fn forget_key(&mut self, file: &mut AccountsFile) {
        if let (Some(id), Some(kind)) = (file.key_id.take(), file.key_store.take()) {
            self.keystore.delete(kind, &id).await;
        }
        self.key = None;
    }
}

fn aad(uuid: Uuid) -> Vec<u8> {
    format!("srl-auth/tokens/v1/{}", uuid.as_hyphenated()).into_bytes()
}

/// `SRLT | 1 | nonce (24) | ciphertext`. The account UUID is the associated data, so a token file
/// copied onto another account's name doesn't open.
fn seal(key: &Key, uuid: Uuid, plain: &[u8]) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new(key.as_slice().into());
    let nonce = random_bytes::<NONCE_LEN>();
    let ct = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plain,
                aad: &aad(uuid),
            },
        )
        .expect("XChaCha20-Poly1305 encryption does not fail for in-memory buffers");
    let mut out = Vec::with_capacity(MAGIC.len() + 1 + NONCE_LEN + ct.len());
    out.extend_from_slice(MAGIC);
    out.push(FORMAT);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    out
}

fn open(key: &Key, uuid: Uuid, sealed: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    let rest = sealed.strip_prefix(MAGIC.as_slice())?;
    let (&format, rest) = rest.split_first()?;
    if format != FORMAT || rest.len() < NONCE_LEN {
        return None;
    }
    let (nonce, ct) = rest.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new(key.as_slice().into());
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ct,
                aad: &aad(uuid),
            },
        )
        .ok()
        .map(Zeroizing::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{keystore::KeystoreChoice, secret::Secret};

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("srl-auth-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn tokens(refresh: &str) -> Tokens {
        Tokens {
            client_id: "c".into(),
            msa_refresh: Secret::new(refresh),
            msa_access: None,
            xbl: None,
            xsts: None,
            minecraft: None,
        }
    }

    #[test]
    fn sealed_files_open_only_for_their_account() {
        let key: Key = Zeroizing::new([7u8; 32]);
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let sealed = seal(&key, a, b"hello");
        assert_eq!(open(&key, a, &sealed).unwrap().as_slice(), b"hello");
        assert!(open(&key, b, &sealed).is_none());
        let other: Key = Zeroizing::new([8u8; 32]);
        assert!(open(&other, a, &sealed).is_none());
        let mut tampered = sealed.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(open(&key, a, &tampered).is_none());
    }

    #[tokio::test]
    async fn tokens_round_trip_and_never_appear_in_plain_text() {
        let root = dir("roundtrip");
        let mut store = Store::new(&root, KeystoreChoice::FileOnly);
        let mut file = Store::load(&root).unwrap();
        let uuid = Uuid::new_v4();
        store
            .write_tokens(&mut file, uuid, &tokens("REFRESH-TOKEN-VALUE"))
            .await
            .unwrap();
        assert_eq!(file.key_store, Some(KeystoreKind::File));
        // A new process: the key comes back from the key file named in accounts.json.
        let mut file = Store::load(&root).unwrap();
        let mut store = Store::new(&root, KeystoreChoice::FileOnly);
        let t = store.read_tokens(&mut file, uuid).await.unwrap();
        assert_eq!(t.msa_refresh.expose(), "REFRESH-TOKEN-VALUE");
        // Nothing on disk holds the token in the clear.
        for entry in walk(&root) {
            let bytes = std::fs::read(&entry).unwrap();
            assert!(
                !bytes
                    .windows(b"REFRESH-TOKEN-VALUE".len())
                    .any(|w| w == b"REFRESH-TOKEN-VALUE"),
                "{} holds the token",
                entry.display()
            );
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn a_lost_key_means_log_in_again() {
        let root = dir("lostkey");
        let mut store = Store::new(&root, KeystoreChoice::FileOnly);
        let mut file = Store::load(&root).unwrap();
        let uuid = Uuid::new_v4();
        store
            .write_tokens(&mut file, uuid, &tokens("r"))
            .await
            .unwrap();
        let id = file.key_id.clone().unwrap();
        std::fs::remove_file(root.join("auth").join(format!("key-{id}"))).unwrap();
        let mut store = Store::new(&root, KeystoreChoice::FileOnly);
        let mut file = Store::load(&root).unwrap();
        assert!(
            store
                .read_tokens(&mut file, uuid)
                .await
                .unwrap_err()
                .needs_relogin()
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A keystore that can't be reached right now must not make a login replace the key: that
    /// would sign every other account out for good.
    #[tokio::test]
    async fn a_keystore_error_during_a_login_keeps_the_old_key() {
        let root = dir("noreplace");
        let mut store = Store::new(&root, KeystoreChoice::FileOnly);
        let mut file = Store::load(&root).unwrap();
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        store
            .write_tokens(&mut file, a, &tokens("A"))
            .await
            .unwrap();
        file.accounts.push(account(a));
        store.save(&file).unwrap();
        let id = file.key_id.clone().unwrap();

        // 1. The key is in the "OS keystore", which SRL_KEYSTORE=file hides: B's login fails.
        let mut os_file = Store::load(&root).unwrap();
        os_file.key_store = Some(KeystoreKind::Os);
        let mut store = Store::new(&root, KeystoreChoice::FileOnly);
        let err = store
            .write_tokens(&mut os_file, b, &tokens("B"))
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::Keystore(_)), "{err:?}");
        assert_eq!(os_file.key_id.as_deref(), Some(id.as_str()));

        // 2. The key file can't be read (an I/O error, not a missing file): the same.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let key_file = root.join("auth").join(format!("key-{id}"));
            let saved = std::fs::read(&key_file).unwrap();
            std::fs::remove_file(&key_file).unwrap();
            std::fs::create_dir(&key_file).unwrap();
            let mut f = Store::load(&root).unwrap();
            let mut store = Store::new(&root, KeystoreChoice::FileOnly);
            let err = store
                .write_tokens(&mut f, b, &tokens("B"))
                .await
                .unwrap_err();
            assert!(matches!(err, AuthError::Keystore(_)), "{err:?}");
            std::fs::remove_dir(&key_file).unwrap();
            std::fs::write(&key_file, saved).unwrap();
            std::fs::set_permissions(&key_file, std::fs::Permissions::from_mode(0o600)).unwrap();
        }

        // Nothing was replaced: A is not flagged and its tokens still open.
        let mut file = Store::load(&root).unwrap();
        assert_eq!(file.key_id.as_deref(), Some(id.as_str()));
        assert!(file.accounts[0].needs_login.is_none());
        let mut store = Store::new(&root, KeystoreChoice::FileOnly);
        let t = store.read_tokens(&mut file, a).await.unwrap();
        assert_eq!(t.msa_refresh.expose(), "A");
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The review's scenario on a real machine: the key is in the OS keystore, which doesn't
    /// answer (`SRL_AUTH_OS_TEST=absent`, e.g. `DBUS_SESSION_BUS_ADDRESS` pointing nowhere). A
    /// login must fail with `Keystore`, not write a key file and sign the other account out.
    #[tokio::test]
    #[ignore = "needs a machine without a keystore; see keystore::tests::os_test_mode"]
    async fn auto_mode_without_a_keystore_never_replaces_an_os_key() {
        if std::env::var("SRL_AUTH_OS_TEST").as_deref() != Ok("absent") {
            eprintln!("skipped: SRL_AUTH_OS_TEST is not `absent`");
            return;
        }
        let root = dir("absent-os");
        let mut file = Store::load(&root).unwrap();
        file.key_id = Some("0123456789abcdef".into());
        file.key_store = Some(KeystoreKind::Os);
        file.accounts.push(account(Uuid::new_v4()));
        let mut store = Store::new(&root, KeystoreChoice::Auto);
        store.save(&file).unwrap();
        let started = std::time::Instant::now();
        let err = store
            .write_tokens(&mut file, Uuid::new_v4(), &tokens("B"))
            .await
            .unwrap_err();
        eprintln!("failed after {:?}: {err}", started.elapsed());
        assert!(matches!(err, AuthError::Keystore(_)), "{err:?}");
        let file = Store::load(&root).unwrap();
        assert_eq!(file.key_store, Some(KeystoreKind::Os));
        assert!(file.accounts[0].needs_login.is_none());
        assert!(!walk(&root).iter().any(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("key-"))
        }));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A key that is gone or exposed for good is replaced by a login; the others are flagged and
    /// the exposed file is deleted.
    #[cfg(unix)]
    #[tokio::test]
    async fn an_exposed_key_file_is_replaced_by_a_login() {
        use std::os::unix::fs::PermissionsExt;
        let root = dir("exposed");
        let mut store = Store::new(&root, KeystoreChoice::FileOnly);
        let mut file = Store::load(&root).unwrap();
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        store
            .write_tokens(&mut file, a, &tokens("A"))
            .await
            .unwrap();
        file.accounts.push(account(a));
        store.save(&file).unwrap();
        let old = file.key_id.clone().unwrap();
        let old_file = root.join("auth").join(format!("key-{old}"));
        std::fs::set_permissions(&old_file, std::fs::Permissions::from_mode(0o644)).unwrap();

        // A launch with the exposed key: log in again.
        let mut store = Store::new(&root, KeystoreChoice::FileOnly);
        let mut file = Store::load(&root).unwrap();
        assert!(
            store
                .read_tokens(&mut file, a)
                .await
                .unwrap_err()
                .needs_relogin()
        );
        // A login: a new key, the old file gone, A flagged.
        store
            .write_tokens(&mut file, b, &tokens("B"))
            .await
            .unwrap();
        assert_ne!(file.key_id.as_deref(), Some(old.as_str()));
        assert!(!old_file.exists());
        assert!(file.accounts[0].needs_login.is_some());
        std::fs::remove_dir_all(&root).unwrap();
    }

    fn account(uuid: Uuid) -> AccountInfo {
        AccountInfo {
            uuid,
            name: "A".into(),
            skin: None,
            ownership: Ownership::Unknown,
            added_at: Utc::now(),
            refreshed_at: None,
            needs_login: None,
        }
    }

    fn walk(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
        out
    }
}
