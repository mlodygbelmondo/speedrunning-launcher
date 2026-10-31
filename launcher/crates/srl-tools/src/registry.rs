//! What each tool is, where it runs, and which modes have it on by default.
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use srl_core::Mode;
use srl_game::rules::Os;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolId {
    NinjabrainBot,
    Toolscreen,
    Waywall,
}

impl ToolId {
    pub const ALL: [ToolId; 3] = [ToolId::NinjabrainBot, ToolId::Toolscreen, ToolId::Waywall];

    /// Stable id: directory names under `tools/`, settings, copy keys (`tool.<id>.what`).
    pub fn id(self) -> &'static str {
        match self {
            ToolId::NinjabrainBot => "ninjabrain-bot",
            ToolId::Toolscreen => "toolscreen",
            ToolId::Waywall => "waywall",
        }
    }

    pub fn from_id(id: &str) -> Option<ToolId> {
        ToolId::ALL.into_iter().find(|t| t.id() == id)
    }

    pub fn info(self) -> &'static ToolInfo {
        TOOLS
            .iter()
            .find(|t| t.id == self)
            .expect("every tool has an entry")
    }
}

/// Who puts the tool on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The launcher downloads the pinned version from GitHub releases and checks its hash.
    Download,
    /// The player installs it with the system's package manager; the launcher only finds it on
    /// `PATH` (waywall: its packages need root and a patched GLFW).
    System,
}

/// One tool, for the Tools page and the credits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolInfo {
    pub id: ToolId,
    pub name: &'static str,
    /// What it does, in English (logs, the CLI). The UI uses `copy_key` in `assets/copy.json`.
    pub what: &'static str,
    pub copy_key: &'static str,
    /// The systems it runs on.
    pub os: &'static [Os],
    /// The version the launcher installs. `None` for tools the system provides.
    pub pinned: Option<&'static str>,
    /// Why that version, in one line.
    pub pin_reason: &'static str,
    pub source: Source,
    /// SPDX id.
    pub licence: &'static str,
    pub homepage: &'static str,
    /// The authors, for the credits.
    pub credit: &'static str,
}

impl ToolInfo {
    pub fn runs_on(&self, os: Os) -> bool {
        self.os.contains(&os)
    }
}

pub static TOOLS: [ToolInfo; 3] = [
    ToolInfo {
        id: ToolId::NinjabrainBot,
        name: "Ninjabrain Bot",
        what: "Finds the stronghold from your Eye of Ender throws (F3+C).",
        copy_key: "tool.ninjabrain-bot.what",
        os: &[Os::Windows, Os::Linux, Os::MacOs],
        pinned: Some(crate::ninjabrain::VERSION),
        pin_reason: "speedrun.com rules v8, A.11.7.a: NinjabrainBot up to v1.5.2",
        source: Source::Download,
        licence: "GPL-3.0",
        homepage: "https://github.com/Ninjabrain1/Ninjabrain-Bot",
        credit: "Ninjabrain1",
    },
    ToolInfo {
        id: ToolId::Toolscreen,
        name: "Toolscreen",
        what: "Instant resizing (Thin, Wide, EyeZoom) and overlays inside the game window.",
        copy_key: "tool.toolscreen.what",
        os: &[Os::Windows],
        pinned: Some(crate::toolscreen::STABLE_VERSION),
        pin_reason: "the newest release whose Toolscreen.dll and liblogger are in legal-dlls.csv",
        source: Source::Download,
        licence: "MIT",
        homepage: "https://github.com/jojoe77777/Toolscreen",
        credit: "jojoe77777",
    },
    ToolInfo {
        id: ToolId::Waywall,
        name: "waywall",
        what: "Runs the game in its own Wayland window with resizing, mirrors and key remaps.",
        copy_key: "tool.waywall.what",
        os: &[Os::Linux],
        pinned: None,
        pin_reason: "installed by the player; the launcher finds it on PATH",
        source: Source::System,
        licence: "GPL-3.0",
        homepage: "https://github.com/tesselslate/waywall",
        credit: "tesselslate (Woofdoggo)",
    },
];

/// Why a tool can't be used in a mode, on this system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unavailable {
    /// The tool doesn't run on this OS.
    Os,
    /// The mode's rules forbid it (Draftout bans Ninjabrain Bot).
    Banned,
    /// Not supported in this mode yet (Toolscreen on 26.3 needs its Vulkan build, not pinned).
    NotSupported,
}

/// Whether `tool` may be turned on for `mode` on `os`.
pub fn availability(tool: ToolId, mode: Mode, os: Os) -> Result<(), Unavailable> {
    if !tool.info().runs_on(os) {
        return Err(Unavailable::Os);
    }
    match (tool, mode) {
        // draftoutmc.com: "Ninjabrain Bot is banned" (research/02 §1.2).
        (ToolId::NinjabrainBot, Mode::Draftout) => Err(Unavailable::Banned),
        // 26.3 renders through Vulkan, which Toolscreen supports from 1.5.0 with a Vulkan layer
        // the launcher doesn't set up yet.
        (ToolId::Toolscreen, Mode::Draftout) => Err(Unavailable::NotSupported),
        _ => Ok(()),
    }
}

/// The tools turned on for a mode. Kept by the app per mode; `defaults` is the first value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolSelection(pub BTreeSet<ToolId>);

impl ToolSelection {
    /// The defaults (research/02 §4, the mode × tool table):
    ///
    /// - Ninjabrain Bot on for Ranked, RSG, FSG and SSG; never for Draftout (banned there).
    /// - Toolscreen on for the 1.16.1 modes on Windows.
    /// - waywall off: it needs the player's own setup (a Wayland session, the patched GLFW and a
    ///   config), so the player turns it on.
    pub fn defaults(mode: Mode, os: Os) -> ToolSelection {
        let on = [ToolId::NinjabrainBot, ToolId::Toolscreen]
            .into_iter()
            .filter(|t| availability(*t, mode, os).is_ok());
        ToolSelection(on.collect())
    }

    pub fn has(&self, tool: ToolId) -> bool {
        self.0.contains(&tool)
    }

    pub fn set(&mut self, tool: ToolId, on: bool) {
        if on {
            self.0.insert(tool);
        } else {
            self.0.remove(&tool);
        }
    }

    /// The selection without the tools `mode` can't have on `os` (a selection saved on another
    /// system, or a rule that changed).
    pub fn usable(&self, mode: Mode, os: Os) -> ToolSelection {
        ToolSelection(
            self.0
                .iter()
                .copied()
                .filter(|t| availability(*t, mode, os).is_ok())
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for t in ToolId::ALL {
            assert_eq!(ToolId::from_id(t.id()), Some(t));
            assert_eq!(t.info().id, t);
            assert_eq!(t.info().copy_key, format!("tool.{}.what", t.id()));
        }
        assert_eq!(
            serde_json::to_string(&ToolId::NinjabrainBot).unwrap(),
            "\"ninjabrain-bot\""
        );
    }

    #[test]
    fn defaults_per_mode() {
        let ids = |s: ToolSelection| s.0.into_iter().collect::<Vec<_>>();
        for mode in [Mode::Ranked, Mode::Rsg, Mode::Fsg, Mode::Ssg] {
            assert_eq!(
                ids(ToolSelection::defaults(mode, Os::Windows)),
                [ToolId::NinjabrainBot, ToolId::Toolscreen]
            );
            assert_eq!(
                ids(ToolSelection::defaults(mode, Os::Linux)),
                [ToolId::NinjabrainBot]
            );
            assert_eq!(
                ids(ToolSelection::defaults(mode, Os::MacOs)),
                [ToolId::NinjabrainBot]
            );
        }
        assert!(
            ToolSelection::defaults(Mode::Draftout, Os::Windows)
                .0
                .is_empty()
        );
        assert!(
            ToolSelection::defaults(Mode::Draftout, Os::Linux)
                .0
                .is_empty()
        );
    }

    #[test]
    fn rules() {
        assert_eq!(
            availability(ToolId::NinjabrainBot, Mode::Draftout, Os::Linux),
            Err(Unavailable::Banned)
        );
        assert_eq!(
            availability(ToolId::Toolscreen, Mode::Rsg, Os::Linux),
            Err(Unavailable::Os)
        );
        assert_eq!(
            availability(ToolId::Waywall, Mode::Rsg, Os::Windows),
            Err(Unavailable::Os)
        );
        assert_eq!(
            availability(ToolId::Waywall, Mode::Draftout, Os::Linux),
            Ok(())
        );
        let mut s = ToolSelection::default();
        s.set(ToolId::NinjabrainBot, true);
        s.set(ToolId::Toolscreen, true);
        assert_eq!(
            s.usable(Mode::Draftout, Os::Windows),
            ToolSelection::default()
        );
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#"["ninjabrain-bot","toolscreen"]"#);
        assert_eq!(serde_json::from_str::<ToolSelection>(&json).unwrap(), s);
    }
}
