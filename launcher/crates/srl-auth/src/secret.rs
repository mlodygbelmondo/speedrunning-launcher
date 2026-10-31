//! A string that must never reach a log, a panic message or the UI: tokens, codes, PKCE verifiers.
//! `Debug` and `Display` print a placeholder, the memory is wiped on drop, and the only way to the
//! value is the explicit `expose()`, so a leak shows up in review as a call to it.
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The real value. Call it only where the value leaves for its one legitimate destination (an
    /// HTTP request body or header, the game's command line, the encrypted token file).
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Secret {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_printed() {
        let s = Secret::new("eyJhbGciOi.super-secret");
        assert!(!format!("{s:?}").contains("super-secret"));
        assert!(!format!("{s}").contains("super-secret"));
        assert_eq!(s.expose(), "eyJhbGciOi.super-secret");
        // Serialised on purpose: only the encrypted token file holds this form.
        assert_eq!(
            serde_json::to_string(&s).unwrap(),
            "\"eyJhbGciOi.super-secret\""
        );
    }
}
