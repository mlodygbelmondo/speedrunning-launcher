//! What the Tools page shows for each tool.
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    /// The tool doesn't run on this system (Toolscreen off Windows, waywall off Linux), or this
    /// CPU has no legal build of it.
    NotAvailable,
    /// Not downloaded yet; for waywall: not found on `PATH`.
    NotInstalled,
    Installed(InstalledTool),
    /// An older version is on disk; `install` replaces it with `latest`.
    UpdateAvailable {
        installed: String,
        latest: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledTool {
    /// `None` when the tool doesn't say (waywall has no `--version`).
    pub version: Option<String>,
    /// The jar, the DLL folder or the executable.
    pub path: PathBuf,
    /// True for tools the launcher downloaded and can update; false for waywall.
    pub managed: bool,
}
