//! Errors, typed so the UI can pick the right message and action for each.
use std::fmt;

use crate::UpdatePlan;

#[derive(Debug)]
pub enum PracticeError {
    /// No such id in the catalogue.
    UnknownMap(String),
    /// The map has no download we may use; the UI shows its "open the author's page" link.
    LinkOnly(String),
    NotInstalled(String),
    /// Use update or reset instead.
    AlreadyInstalled(String),
    /// `update` was called without a choice while a world has been played. Carries the plan, so
    /// the UI can ask (it never decides silently).
    NeedsDecision(Box<UpdatePlan>),
    /// The author's file is gone (HTTP 404 or 410): they moved or removed the release. The
    /// catalogue needs an update; until then, the author's page is the way.
    SourceGone {
        url: String,
    },
    /// No connection, a timeout or a server error, after retries.
    Network(String),
    /// The download doesn't match the pinned hash or size, after retries. Nothing was installed.
    HashMismatch {
        expected: String,
        got: String,
    },
    /// Not a zip, or a broken one.
    BadArchive(String),
    /// The zip tried to write outside its folder, holds a symlink, or is far too big. Nothing
    /// was written.
    UnsafeArchive(String),
    /// The zip has no `level.dat`, so no world to install.
    NoWorld,
    /// A running game has this world open (it holds `session.lock`). Nothing was changed; quit
    /// the world or the game first.
    WorldInUse {
        folder: String,
    },
    /// Disk errors, and anything else.
    Other(anyhow::Error),
}

impl fmt::Display for PracticeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMap(id) => write!(f, "no practice map {id:?} in the catalogue"),
            Self::LinkOnly(id) => write!(f, "{id} can only be downloaded from the author's page"),
            Self::NotInstalled(id) => write!(f, "{id} is not installed"),
            Self::AlreadyInstalled(id) => write!(f, "{id} is already installed"),
            Self::NeedsDecision(_) => write!(f, "a played world would change: ask the player"),
            Self::SourceGone { url } => write!(f, "the author's file is gone: {url}"),
            Self::Network(e) => write!(f, "download failed: {e}"),
            Self::HashMismatch { expected, got } => {
                write!(
                    f,
                    "the download doesn't match: expected {expected}, got {got}"
                )
            }
            Self::BadArchive(e) => write!(f, "not a readable zip: {e}"),
            Self::UnsafeArchive(e) => write!(f, "refused an unsafe zip: {e}"),
            Self::NoWorld => write!(f, "the zip has no world (no level.dat)"),
            Self::WorldInUse { folder } => write!(f, "the game has saves/{folder} open"),
            Self::Other(e) => write!(f, "{e:#}"),
        }
    }
}

impl std::error::Error for PracticeError {}

impl From<std::io::Error> for PracticeError {
    fn from(e: std::io::Error) -> Self {
        Self::Other(e.into())
    }
}

impl From<anyhow::Error> for PracticeError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}
