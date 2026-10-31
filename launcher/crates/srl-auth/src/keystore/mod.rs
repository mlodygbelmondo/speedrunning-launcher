//! Where the key that encrypts the token files lives.
//!
//! Envelope encryption (research 1.6): the tokens themselves are too big for Windows Credential
//! Manager (2560 bytes per credential, and a Minecraft token alone is 1-2 KB), so each account's
//! tokens are encrypted on disk with one random 256-bit key per data folder, and only that key goes
//! to the OS keystore:
//!
//! - Windows: Credential Manager (generic credential, local machine persistence);
//! - macOS: the login Keychain (generic password);
//! - Linux: the Secret Service (GNOME Keyring, KWallet) through oo7, which inside Flatpak uses the
//!   Secret portal by itself.
//!
//! When there is no keystore (headless Linux, no keyring daemon, the user dismissed the unlock
//! prompt) the key goes to a file next to the tokens instead: mode 0600 on Unix, DPAPI-protected
//! for the current user on Windows. The UI says so (`KeystoreKind::File`), since on Unix that file is
//! protected only by its permissions. `SRL_KEYSTORE=file` forces the file, for agents and tests,
//! which must never touch the keyring of the desktop they run on.
use std::{
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::AuthError;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as os;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as os;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as os;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod os {
    pub async fn get(_: &str) -> Result<Option<Vec<u8>>, String> {
        Err("no keystore on this OS".into())
    }
    pub async fn set(_: &str, _: &[u8]) -> Result<(), String> {
        Err("no keystore on this OS".into())
    }
    pub async fn delete(_: &str) -> Result<(), String> {
        Ok(())
    }
}

pub const KEYSTORE_ENV: &str = "SRL_KEYSTORE";

/// The name the key is filed under in every keystore. One entry per data folder (`key_id`), so a
/// dev folder never overwrites the key of the real one.
pub(crate) const SERVICE: &str = "SpeedrunningLauncher";

/// A keystore call that needs no prompt answers within seconds; one that shows an unlock prompt
/// waits for the player, but not forever.
pub(crate) const OS_TIMEOUT: Duration = Duration::from_secs(120);

pub type Key = Zeroizing<[u8; 32]>;

/// What `Keystore::load` found.
pub(crate) enum Loaded {
    Found(Key),
    /// Gone (keyring reset, file deleted), damaged, or exposed (a key file other users could
    /// read): the tokens it sealed are lost for good. The reason, in English.
    Unusable(String),
}

enum KeyFileError {
    /// Readable by other users: the key may have been copied, so it is not used again. Unix
    /// only: on Windows the file is DPAPI-sealed for the user, so other users can't use it.
    #[cfg_attr(not(unix), allow(dead_code))]
    Exposed(String),
    Io(io::Error),
}

impl Loaded {
    #[cfg(test)]
    pub fn found(self) -> Option<Key> {
        match self {
            Self::Found(k) => Some(k),
            Self::Unusable(_) => None,
        }
    }
}

impl From<io::Error> for KeyFileError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Where a key was put. Saved (not secret) in `accounts.json`, so a later start looks in the
/// same place and never falls back to a file just because the keyring is slow to start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeystoreKind {
    /// The OS keystore.
    Os,
    /// The fallback key file in `<root>/auth/`.
    File,
}

/// Which keystores may be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeystoreChoice {
    /// The OS keystore, the file when there is none.
    Auto,
    /// Only the file. Tests, agents and `SRL_KEYSTORE=file`.
    FileOnly,
}

impl KeystoreChoice {
    pub fn from_env() -> Self {
        match std::env::var(KEYSTORE_ENV) {
            Ok(v) if v.eq_ignore_ascii_case("file") => Self::FileOnly,
            _ => Self::Auto,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Keystore {
    choice: KeystoreChoice,
    dir: PathBuf,
}

impl Keystore {
    pub fn new(choice: KeystoreChoice, auth_dir: &Path) -> Self {
        Self {
            choice,
            dir: auth_dir.to_path_buf(),
        }
    }

    fn key_file(&self, key_id: &str) -> PathBuf {
        self.dir.join(format!("key-{key_id}"))
    }

    /// The key saved earlier in `kind`.
    ///
    /// `Err` means the keystore can't be used *right now* (the keyring is locked or slow, the
    /// Secret Service isn't up yet, `SRL_KEYSTORE=file` hides it, an I/O error): the key may still
    /// be there, so nobody may replace it. `Ok(Unusable)` means it is gone or must not be used any
    /// more, and only then may a login make a new one.
    pub async fn load(&self, kind: KeystoreKind, key_id: &str) -> Result<Loaded, AuthError> {
        let raw = match kind {
            KeystoreKind::Os => {
                if self.choice == KeystoreChoice::FileOnly {
                    return Err(AuthError::Keystore(
                        "the sign-ins are in the OS keystore, which SRL_KEYSTORE=file turns off"
                            .into(),
                    ));
                }
                with_timeout(os::get(&entry_name(key_id))).await?
            }
            KeystoreKind::File => match read_key_file(&self.key_file(key_id)) {
                Ok(raw) => raw,
                Err(KeyFileError::Exposed(why)) => return Ok(Loaded::Unusable(why)),
                Err(KeyFileError::Io(e)) => {
                    return Err(AuthError::Keystore(format!("key file: {e}")));
                }
            },
        };
        Ok(match raw {
            None => Loaded::Unusable("the saved sign-in key is gone from the keystore".into()),
            Some(bytes) => match to_key(&bytes) {
                Some(key) => Loaded::Found(key),
                None => Loaded::Unusable("the saved sign-in key is damaged".into()),
            },
        })
    }

    /// A new random key, saved in the OS keystore if there is one, else in the file.
    pub async fn create(&self, key_id: &str) -> Result<(Key, KeystoreKind), AuthError> {
        let key: Key = Zeroizing::new(crate::pkce::random_bytes::<32>());
        if self.choice == KeystoreChoice::Auto
            && with_timeout(os::set(&entry_name(key_id), key.as_slice()))
                .await
                .is_ok()
        {
            return Ok((key, KeystoreKind::Os));
        }
        write_key_file(&self.key_file(key_id), key.as_slice())
            .map_err(|e| AuthError::Keystore(format!("key file: {e}")))?;
        Ok((key, KeystoreKind::File))
    }

    /// Forgets the key (best effort: the token files it opened are deleted anyway).
    pub async fn delete(&self, kind: KeystoreKind, key_id: &str) {
        match kind {
            KeystoreKind::Os if self.choice == KeystoreChoice::Auto => {
                let _ = with_timeout(os::delete(&entry_name(key_id))).await;
            }
            KeystoreKind::Os => {}
            KeystoreKind::File => {
                let _ = std::fs::remove_file(self.key_file(key_id));
            }
        }
    }
}

fn entry_name(key_id: &str) -> String {
    format!("token-key-{key_id}")
}

async fn with_timeout<T>(
    f: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, AuthError> {
    match tokio::time::timeout(OS_TIMEOUT, f).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(AuthError::Keystore(e)),
        Err(_) => Err(AuthError::Keystore("the OS keystore did not answer".into())),
    }
}

/// `None`: the wrong size, so not a key of ours.
fn to_key(bytes: &[u8]) -> Option<Key> {
    let arr: [u8; 32] = bytes.try_into().ok()?;
    Some(Zeroizing::new(arr))
}

/// Writes a file only the current user can read, atomically (temp file, then rename).
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let dir = path.parent().unwrap_or(Path::new("."));
    create_private_dir(dir)?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, path)
}

/// `auth/` is 0700 on Unix. On Windows the per-user profile folder's ACL already keeps other
/// users out.
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_key_file(path: &Path, key: &[u8]) -> io::Result<()> {
    #[cfg(windows)]
    let protected = windows::dpapi_protect(key)?;
    #[cfg(not(windows))]
    let protected = key.to_vec();
    let protected = Zeroizing::new(protected);
    write_private(path, &protected)
}

fn read_key_file(path: &Path) -> Result<Option<Vec<u8>>, KeyFileError> {
    let bytes = match std::fs::read(path) {
        Ok(b) => Zeroizing::new(b),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    #[cfg(unix)]
    {
        // Someone loosened the permissions: the key may have been read. Refuse rather than keep
        // using it quietly; a new login makes a new key (and deletes this one).
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)?.permissions().mode();
        if mode & 0o077 != 0 {
            return Err(KeyFileError::Exposed(
                "the saved sign-in key file was readable by other users".into(),
            ));
        }
    }
    #[cfg(windows)]
    return Ok(Some(windows::dpapi_unprotect(&bytes)?));
    #[cfg(not(windows))]
    Ok(Some(bytes.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("srl-auth-ks-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[tokio::test]
    async fn file_fallback_round_trip() {
        let d = dir("roundtrip");
        let ks = Keystore::new(KeystoreChoice::FileOnly, &d);
        let (key, kind) = ks.create("abc").await.unwrap();
        assert_eq!(kind, KeystoreKind::File);
        let loaded = ks
            .load(KeystoreKind::File, "abc")
            .await
            .unwrap()
            .found()
            .unwrap();
        assert_eq!(*loaded, *key);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&d.join("key-abc")), 0o600);
            assert_eq!(mode(&d), 0o700);
        }
        // FileOnly never reaches for the OS keystore.
        assert!(ks.load(KeystoreKind::Os, "abc").await.is_err());
        ks.delete(KeystoreKind::File, "abc").await;
        assert!(
            ks.load(KeystoreKind::File, "abc")
                .await
                .unwrap()
                .found()
                .is_none()
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_world_readable_key_file_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let d = dir("perm");
        let ks = Keystore::new(KeystoreChoice::FileOnly, &d);
        ks.create("k").await.unwrap();
        std::fs::set_permissions(d.join("key-k"), std::fs::Permissions::from_mode(0o644)).unwrap();
        // Unusable for good (a login may replace it), not a passing error.
        assert!(matches!(
            ks.load(KeystoreKind::File, "k").await,
            Ok(Loaded::Unusable(_))
        ));
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// A key file that can't be read right now is an error, not "gone": a login must not
    /// replace a key that may still be there.
    #[cfg(unix)]
    #[tokio::test]
    async fn an_unreadable_key_file_is_an_error_not_a_missing_key() {
        let d = dir("unreadable");
        let ks = Keystore::new(KeystoreChoice::FileOnly, &d);
        std::fs::create_dir_all(d.join("key-k")).unwrap(); // reading a directory fails
        assert!(matches!(
            ks.load(KeystoreKind::File, "k").await,
            Err(AuthError::Keystore(_))
        ));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_round_trip() {
        let sealed = windows::dpapi_protect(b"0123456789abcdef").unwrap();
        assert_ne!(sealed.as_slice(), b"0123456789abcdef");
        assert_eq!(
            windows::dpapi_unprotect(&sealed).unwrap(),
            b"0123456789abcdef"
        );
    }

    /// What the OS keystore tests may do, from `SRL_AUTH_OS_TEST`. They are `#[ignore]`d and also
    /// do nothing unless told where they run, so `cargo test -- --ignored` on a desktop never
    /// writes into its owner's keyring:
    /// - `present`: a keystore made for the test (a private D-Bus session with its own
    ///   gnome-keyring, a throwaway Wine prefix, a CI runner);
    /// - `absent`: no keystore at all (e.g. `DBUS_SESSION_BUS_ADDRESS` pointing nowhere).
    fn os_test_mode() -> Option<String> {
        std::env::var("SRL_AUTH_OS_TEST").ok()
    }

    #[tokio::test]
    #[ignore = "touches the OS keystore; see os_test_mode"]
    async fn os_keystore_round_trip() {
        if os_test_mode().as_deref() != Some("present") {
            eprintln!("skipped: SRL_AUTH_OS_TEST is not `present`");
            return;
        }
        let name = format!("selftest-{}", std::process::id());
        os::set(&name, b"first").await.unwrap();
        os::set(&name, b"second").await.unwrap(); // replaces, never duplicates
        assert_eq!(
            os::get(&name).await.unwrap().as_deref(),
            Some(&b"second"[..])
        );
        os::delete(&name).await.unwrap();
        assert_eq!(os::get(&name).await.unwrap(), None);
        os::delete(&name).await.unwrap(); // deleting twice is fine

        let d = dir("os");
        let ks = Keystore::new(KeystoreChoice::Auto, &d);
        let (key, kind) = ks.create("selftest").await.unwrap();
        assert_eq!(kind, KeystoreKind::Os);
        assert!(!d.join("key-selftest").exists());
        let loaded = ks
            .load(KeystoreKind::Os, "selftest")
            .await
            .unwrap()
            .found()
            .unwrap();
        assert_eq!(*loaded, *key);
        ks.delete(KeystoreKind::Os, "selftest").await;
        assert!(
            ks.load(KeystoreKind::Os, "selftest")
                .await
                .unwrap()
                .found()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    #[ignore = "needs a machine without a keystore; see os_test_mode"]
    async fn auto_falls_back_to_the_file_without_a_keystore() {
        if os_test_mode().as_deref() != Some("absent") {
            eprintln!("skipped: SRL_AUTH_OS_TEST is not `absent`");
            return;
        }
        let d = dir("absent");
        let ks = Keystore::new(KeystoreChoice::Auto, &d);
        let started = std::time::Instant::now();
        let (key, kind) = ks.create("k").await.unwrap();
        eprintln!("fell back after {:?}", started.elapsed());
        assert_eq!(kind, KeystoreKind::File);
        assert!(d.join("key-k").exists());
        assert_eq!(
            *ks.load(KeystoreKind::File, "k")
                .await
                .unwrap()
                .found()
                .unwrap(),
            *key
        );
        std::fs::remove_dir_all(&d).unwrap();
    }
}
