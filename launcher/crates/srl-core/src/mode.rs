//! The five modes. The four 1.16.1 modes (Ranked, RSG, FSG, SSG) share one game directory,
//! `instances/mcsr`: the same options, configs, resource packs and worlds, and each mode is the
//! set of mods Play puts into its `mods/` (see `crate::shared`). Draftout runs another Minecraft
//! version and keeps its own directory.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Ranked,
    Rsg,
    Fsg,
    Ssg,
    Draftout,
}

impl Mode {
    /// In the order the UI shows them (the Home tiles and the Modes list).
    pub const ALL: [Mode; 5] = [
        Mode::Ranked,
        Mode::Rsg,
        Mode::Fsg,
        Mode::Ssg,
        Mode::Draftout,
    ];

    /// Stable id: directory names, settings and the ids in `assets/screens.json`.
    pub fn id(self) -> &'static str {
        match self {
            Mode::Ranked => "ranked",
            Mode::Rsg => "rsg",
            Mode::Fsg => "fsg",
            Mode::Ssg => "ssg",
            Mode::Draftout => "draftout",
        }
    }

    pub fn from_id(id: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.id() == id)
    }

    /// The modes that share the `instances/mcsr` game directory, in `ALL` order.
    pub const SHARED: [Mode; 4] = [Mode::Ranked, Mode::Rsg, Mode::Fsg, Mode::Ssg];

    /// Whether the mode plays in the shared 1.16.1 directory (`instances/mcsr`).
    pub fn is_shared(self) -> bool {
        self != Mode::Draftout
    }

    /// Minecraft version the mode runs on. Draftout is the only one on the newest release.
    pub fn minecraft_version(self) -> &'static str {
        match self {
            Mode::Draftout => "26.3",
            _ => "1.16.1",
        }
    }
}

/// A game directory the launcher installs into and starts: one of the five modes, or the practice
/// instance (`Paths::practice_instance`). The practice instance is not a mode on purpose: `Mode`
/// is matched exhaustively and sized into per-mode arrays all over the app, and practice has no
/// run rules or tiles of its own. srl-game and srl-mods key their files and guards by `Instance`,
/// so a practice game and a mode's game never share a log, a pid file or `mods/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Instance {
    Mode(Mode),
    Practice,
}

impl Instance {
    /// Stable id: the instance directory, `logs/<id>-latest.log`, `logs/<id>.pid` and srl-game's
    /// verification record. A mode's is its `Mode::id`; `practice` is no mode's id.
    pub fn id(self) -> &'static str {
        match self {
            Instance::Mode(m) => m.id(),
            Instance::Practice => "practice",
        }
    }

    pub fn mode(self) -> Option<Mode> {
        match self {
            Instance::Mode(m) => Some(m),
            Instance::Practice => None,
        }
    }

    /// The name of the game directory under `instances/`: `mcsr` for the four shared modes,
    /// else the instance id. Two instances with the same `dir_id` share `mods/` and can't run
    /// at the same time.
    pub fn dir_id(self) -> &'static str {
        match self {
            Instance::Mode(m) if m.is_shared() => crate::shared::DIR,
            other => other.id(),
        }
    }

    /// The instances that play in the same game directory, this one included.
    pub fn siblings(self) -> Vec<Instance> {
        match self {
            Instance::Mode(m) if m.is_shared() => {
                Mode::SHARED.into_iter().map(Instance::Mode).collect()
            }
            other => vec![other],
        }
    }
}

impl From<Mode> for Instance {
    fn from(mode: Mode) -> Self {
        Instance::Mode(mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_ids_never_collide_with_modes() {
        assert!(Mode::from_id(Instance::Practice.id()).is_none());
        for m in Mode::ALL {
            assert_eq!(Instance::from(m).id(), m.id());
            assert_eq!(Instance::from(m).mode(), Some(m));
        }
        assert_eq!(Instance::Practice.mode(), None);
    }

    #[test]
    fn the_shared_modes_have_one_directory() {
        for m in Mode::SHARED {
            assert_eq!(Instance::from(m).dir_id(), "mcsr");
            assert_eq!(Instance::from(m).siblings().len(), 4);
        }
        assert_eq!(Instance::from(Mode::Draftout).dir_id(), "draftout");
        assert_eq!(Instance::Practice.dir_id(), "practice");
        assert_eq!(Instance::Practice.siblings(), vec![Instance::Practice]);
        assert!(Mode::from_id("mcsr").is_none());
    }

    #[test]
    fn ids_round_trip() {
        for m in Mode::ALL {
            assert_eq!(Mode::from_id(m.id()), Some(m));
        }
        assert_eq!(Mode::from_id("aa"), None);
    }
}
