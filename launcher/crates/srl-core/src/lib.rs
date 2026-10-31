//! Shared types for every Speedrunning Launcher crate. The contracts between `srl-game`, `srl-mods` and
//! `srl-app` live here, so the crates can grow in parallel without depending on each other.
pub mod account;
pub mod download;
pub mod mode;
pub mod paths;
pub mod progress;
pub mod shared;

pub use account::OfflineAccount;
pub use download::{Download, Downloader, Hash, HashMismatch};
pub use mode::{Instance, Mode};
pub use paths::Paths;
pub use progress::{Progress, ProgressFn, Stage, Unit};

/// Sent as the User-Agent on every request, as the Mojang, Modrinth and MCSR APIs ask.
pub const USER_AGENT: &str = concat!("SpeedrunningLauncher/", env!("CARGO_PKG_VERSION"));
