//! PKCE (RFC 7636, method S256) and the `state` value of the browser flow.
//!
//! A public client has no secret, so PKCE is what stops a program that sees the redirect (another
//! app listening on loopback, a browser extension) from trading our code for tokens: the code is
//! worthless without the verifier, which never leaves this process.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};

use crate::secret::Secret;

pub(crate) fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).expect("the OS random number generator works");
    buf
}

/// A verifier and its S256 challenge.
#[derive(Debug)]
pub struct Pkce {
    pub verifier: Secret,
    pub challenge: String,
}

impl Pkce {
    /// 32 random bytes, base64url: a 43-character verifier, the shortest RFC 7636 allows, with
    /// 256 bits of entropy.
    pub fn new() -> Self {
        Self::from_verifier(URL_SAFE_NO_PAD.encode(random_bytes::<32>()))
    }

    pub fn from_verifier(verifier: String) -> Self {
        let challenge = challenge_s256(&verifier);
        Self {
            verifier: Secret::new(verifier),
            challenge,
        }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

/// `BASE64URL(SHA256(ASCII(verifier)))`, RFC 7636 section 4.2.
pub fn challenge_s256(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// An unguessable `state` for one login (128 bits).
pub fn new_state() -> String {
    URL_SAFE_NO_PAD.encode(random_bytes::<16>())
}

/// Compares without an early exit, so the time taken says nothing about how much matched.
pub(crate) fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7636_appendix_b_vector() {
        let p = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into());
        assert_eq!(p.challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn fresh_values_are_long_and_distinct() {
        let (a, b) = (Pkce::new(), Pkce::new());
        assert_eq!(a.verifier.expose().len(), 43);
        assert_ne!(a.verifier.expose(), b.verifier.expose());
        assert_eq!(a.challenge, challenge_s256(a.verifier.expose()));
        assert_ne!(new_state(), new_state());
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "abcd"));
    }
}
