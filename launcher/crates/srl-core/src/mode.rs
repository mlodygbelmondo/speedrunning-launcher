//! The five modes. The four 1.16.1 modes (Ranked, RSG, FSG, SSG) share one game directory,
//! `instances/mcsr`: the same options, configs, resource packs and worlds, and each mode is the
//! set of mods Play puts into its `mods/` (see `crate::shared`). Draftout runs another Minecraft
//! version and keeps its own directory.
use std::borrow::Cow;

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

/// A custom instance's number: its directory is `instances/custom-<n>` and its id `custom-<n>`.
/// The definition (name, base mode, memory, mods) lives in the app's settings; srl-core only
/// needs the number to key files, guards and directories, so `Instance` stays `Copy`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CustomId(pub u32);

impl CustomId {
    /// The prefix of every custom instance's id. No mode's id and no other instance's starts
    /// with it, so a custom id can't collide with them.
    pub const PREFIX: &'static str = "custom-";

    /// `custom-<n>`: the directory under `instances/` and the key of its files.
    pub fn id(self) -> String {
        format!("{}{}", Self::PREFIX, self.0)
    }

    /// The custom id in `custom-<n>` (the exact form `id` writes), else `None`.
    pub fn from_id(id: &str) -> Option<CustomId> {
        let n = id.strip_prefix(Self::PREFIX)?;
        // "custom-007" or "custom-+7" would name the same number as "custom-7" but another
        // directory: only the canonical form counts.
        let parsed: u32 = n.parse().ok()?;
        (parsed.to_string() == n).then_some(CustomId(parsed))
    }
}

impl std::fmt::Display for CustomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.0)
    }
}

/// A game directory the launcher installs into and starts: one of the five modes, the practice
/// instance (`Paths::practice_instance`) or a custom instance the player created. The practice
/// instance is not a mode on purpose: `Mode` is matched exhaustively and sized into per-mode
/// arrays all over the app, and practice has no run rules or tiles of its own. A custom instance
/// has a base mode (its game, Java and legal mods), but that lives in the app's settings: here
/// it is only its own directory. srl-game and srl-mods key their files and guards by `Instance`,
/// so two instances never share a log, a pid file or `mods/` unless they share a directory.
///
/// Serialized as `{"mode":"rsg"}`, `"practice"` or `{"custom":3}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Instance {
    Mode(Mode),
    Practice,
    Custom(CustomId),
}

impl Instance {
    /// Stable id: the instance directory, `logs/<id>-latest.log`, `logs/<id>.pid` and srl-game's
    /// verification record. A mode's is its `Mode::id`; `practice` and `custom-<n>` are no
    /// mode's id. Borrowed for the built-in instances, owned for a custom one.
    pub fn id(self) -> Cow<'static, str> {
        match self {
            Instance::Mode(m) => Cow::Borrowed(m.id()),
            Instance::Practice => Cow::Borrowed("practice"),
            Instance::Custom(c) => Cow::Owned(c.id()),
        }
    }

    /// The instance with this `id`, the inverse of [`Instance::id`].
    pub fn from_id(id: &str) -> Option<Instance> {
        match id {
            "practice" => Some(Instance::Practice),
            _ => Mode::from_id(id)
                .map(Instance::Mode)
                .or_else(|| CustomId::from_id(id).map(Instance::Custom)),
        }
    }

    pub fn mode(self) -> Option<Mode> {
        match self {
            Instance::Mode(m) => Some(m),
            Instance::Practice | Instance::Custom(_) => None,
        }
    }

    /// The custom instance's number, `None` for a mode or practice.
    pub fn custom(self) -> Option<CustomId> {
        match self {
            Instance::Custom(c) => Some(c),
            _ => None,
        }
    }

    /// The name of the game directory under `instances/`: `mcsr` for the four shared modes,
    /// else the instance id. Two instances with the same `dir_id` share `mods/` and can't run
    /// at the same time.
    pub fn dir_id(self) -> Cow<'static, str> {
        match self {
            Instance::Mode(m) if m.is_shared() => Cow::Borrowed(crate::shared::DIR),
            other => other.id(),
        }
    }

    /// The instances that play in the same game directory, this one included. A custom
    /// instance has its directory to itself.
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
        assert!(Mode::from_id(&Instance::Practice.id()).is_none());
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
    fn custom_instances_have_their_own_directory() {
        let c = Instance::Custom(CustomId(7));
        assert_eq!(c.id(), "custom-7");
        assert_eq!(c.dir_id(), "custom-7");
        assert_eq!(c.siblings(), vec![c]);
        assert_eq!(c.mode(), None);
        assert_eq!(c.custom(), Some(CustomId(7)));
        assert_eq!(Instance::from_id("custom-7"), Some(c));
        for bad in ["custom-", "custom-07", "custom-+7", "custom--1", "custom-x"] {
            assert_eq!(Instance::from_id(bad), None, "{bad}");
        }
        for i in [Instance::Practice, Instance::Mode(Mode::Fsg)] {
            assert_eq!(Instance::from_id(&i.id()), Some(i));
        }
        assert!(Mode::from_id(&c.id()).is_none());
    }

    /// Values saved before custom instances keep their form; a custom one is `{"custom":n}`.
    #[test]
    fn instance_serde_is_backward_compatible() {
        let text = serde_json::to_string(&[
            Instance::Mode(Mode::Rsg),
            Instance::Practice,
            Instance::Custom(CustomId(3)),
        ])
        .unwrap();
        assert_eq!(text, r#"[{"mode":"rsg"},"practice",{"custom":3}]"#);
        let back: Vec<Instance> = serde_json::from_str(&text).unwrap();
        assert_eq!(back[2], Instance::Custom(CustomId(3)));
    }

    #[test]
    fn ids_round_trip() {
        for m in Mode::ALL {
            assert_eq!(Mode::from_id(m.id()), Some(m));
        }
        assert_eq!(Mode::from_id("aa"), None);
    }
}
