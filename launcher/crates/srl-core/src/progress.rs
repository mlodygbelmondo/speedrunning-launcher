//! Progress of a long task (installing a mode, updating mods), reported to the UI.
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Queued behind another mode's install (modes share files, so one installs at a time).
    Waiting,
    /// Version manifests and mod lists.
    Metadata,
    Java,
    /// Client jar, libraries and natives.
    Game,
    Assets,
    Mods,
    Tools,
    /// Renewing the Microsoft sign-in before the game starts (srl-auth's `prepare_launch`),
    /// usually instant. Only with a Microsoft account.
    SigningIn,
    Launching,
}

/// What `Progress::done` and `Progress::total` count. Only the code that builds a download list
/// knows whether every file in it has a size, so it says so here and the UI can label the numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Bytes,
    /// Files, or steps such as metadata lists.
    Files,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub stage: Stage,
    pub done: u64,
    pub total: u64,
    pub unit: Unit,
}

impl Progress {
    /// 0.0..=1.0 within the stage. Nothing counted yet (`total == 0`) is 0: an empty bar, not a
    /// full one that looks nearly done.
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.done as f64 / self.total as f64).min(1.0) as f32
        }
    }
}

/// Called from any thread, possibly often: keep it cheap (store the value, notify the UI).
pub type ProgressFn = Arc<dyn Fn(Progress) + Send + Sync>;
