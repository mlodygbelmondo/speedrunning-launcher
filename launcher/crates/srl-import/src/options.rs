//! `options.txt`: one `key:value` per line, split at the first colon (as the game reads it). The
//! import merges the player's file into each instance's instead of replacing it: the instance's
//! lines keep their order and the lines it doesn't have are added at the end, so whatever the
//! launcher seeded there (Draftout's `onboardAccessibility:false`) survives.
use std::collections::HashMap;

/// The vanilla key bindings that exist in 1.16.1 and in today's releases under the same name
/// (compared on the options files the two versions write: 1.16.1 and Draftout's 26.3). Keys of
/// mods, and the ones only one version has (`key.socialInteractions`, the 26.x debug keys), are
/// not in it: an older or newer game would only drop them.
pub const SHARED_KEY_BINDINGS: [&str; 33] = [
    "key_key.advancements",
    "key_key.attack",
    "key_key.back",
    "key_key.chat",
    "key_key.command",
    "key_key.drop",
    "key_key.forward",
    "key_key.fullscreen",
    "key_key.hotbar.1",
    "key_key.hotbar.2",
    "key_key.hotbar.3",
    "key_key.hotbar.4",
    "key_key.hotbar.5",
    "key_key.hotbar.6",
    "key_key.hotbar.7",
    "key_key.hotbar.8",
    "key_key.hotbar.9",
    "key_key.inventory",
    "key_key.jump",
    "key_key.left",
    "key_key.loadToolbarActivator",
    "key_key.pickItem",
    "key_key.playerlist",
    "key_key.right",
    "key_key.saveToolbarActivator",
    "key_key.screenshot",
    "key_key.smoothCamera",
    "key_key.sneak",
    "key_key.spectatorOutlines",
    "key_key.sprint",
    "key_key.swapOffhand",
    "key_key.togglePerspective",
    "key_key.use",
];

/// Besides the key bindings, what carries over between versions: the same names and the same
/// scale in 1.16.1 and 26.3 (sensitivity 0..1 with 0.5 = 100 %, FOV stored as (fov - 70) / 40).
pub const SHARED_SETTINGS: [&str; 2] = ["mouseSensitivity", "fov"];

/// The resource pack lines: which packs are on, and which of them the game called incompatible
/// but the player turned on anyway. They come with the resource packs, not with the settings.
pub const RESOURCE_PACK_KEYS: [&str; 2] = ["resourcePacks", "incompatibleResourcePacks"];

/// The data version line. Every `options.txt` the game writes starts with it; the game upgrades
/// older files from it.
pub const VERSION_KEY: &str = "version";

#[derive(Clone, Debug, PartialEq, Eq)]
enum Line {
    Pair(String, String),
    /// A line without a colon: kept as it was in the instance's file, dropped from the player's.
    Other(String),
}

/// A parsed `options.txt`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    lines: Vec<Line>,
}

impl Options {
    pub fn parse(text: &str) -> Options {
        let lines = text
            .lines()
            .map(|l| l.trim_end_matches('\r'))
            .filter(|l| !l.is_empty())
            .map(|l| match l.split_once(':') {
                Some((k, v)) if !k.is_empty() => Line::Pair(k.to_string(), v.to_string()),
                _ => Line::Other(l.to_string()),
            })
            .collect();
        Options { lines }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().rev().find_map(|l| match l {
            Line::Pair(k, v) if k == key => Some(v.as_str()),
            _ => None,
        })
    }

    pub fn pairs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.lines.iter().filter_map(|l| match l {
            Line::Pair(k, v) => Some((k.as_str(), v.as_str())),
            Line::Other(_) => None,
        })
    }

    /// Sets `key` where it is (every copy of it), or adds it at the end.
    pub fn set(&mut self, key: &str, value: &str) {
        let mut found = false;
        for line in &mut self.lines {
            if let Line::Pair(k, v) = line
                && k == key
            {
                *v = value.to_string();
                found = true;
            }
        }
        if !found {
            self.lines
                .push(Line::Pair(key.to_string(), value.to_string()));
        }
    }

    /// The data version (`version:2567` is 1.16.1), if the file has a valid one.
    pub fn data_version(&self) -> Option<u32> {
        self.get(VERSION_KEY)?.trim().parse().ok()
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            match line {
                Line::Pair(k, v) => {
                    out.push_str(k);
                    out.push(':');
                    out.push_str(v);
                }
                Line::Other(s) => out.push_str(s),
            }
            out.push('\n');
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// What to take from the player's `options.txt`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// Everything but the resource pack lines (the same Minecraft version).
    AllSettings,
    /// Only the key bindings, mouse sensitivity and FOV (another version).
    Shared,
    /// Only the resource pack lines.
    ResourcePacks,
}

impl Pick {
    fn takes(self, key: &str, value: &str) -> bool {
        match self {
            Pick::AllSettings => !RESOURCE_PACK_KEYS.contains(&key),
            Pick::Shared => {
                SHARED_SETTINGS.contains(&key)
                    // Since 1.13 a binding names its key (`key.keyboard.w`); a number is the
                    // older format, which a newer game would misread.
                    || (SHARED_KEY_BINDINGS.contains(&key)
                        && (value.starts_with("key.") || value.starts_with("scancode.")))
            }
            Pick::ResourcePacks => RESOURCE_PACK_KEYS.contains(&key),
        }
    }
}

/// Merges the lines `picks` take from `source` into `target` (`None`: the instance has no
/// options.txt yet), then adds the `seed` lines the target lacks. A new file gets the source's
/// data version, so the game upgrades the merged lines the way it upgrades its own old files.
/// Returns the new text and how many settings it took from the source.
pub fn merge(
    target: Option<&Options>,
    source: &Options,
    picks: &[Pick],
    seed: &[(&str, &str)],
) -> (Options, usize) {
    let mut out = target.cloned().unwrap_or_default();
    let mut taken = 0;
    let mut seen = HashMap::new();
    if target.is_none()
        && let Some(v) = source.get(VERSION_KEY)
        && v.trim().parse::<u32>().is_ok()
    {
        out.set(VERSION_KEY, v);
    }
    for (key, value) in source.pairs() {
        // The version line is the target's own; settings, not their format, are imported.
        if key == VERSION_KEY || !picks.iter().any(|p| p.takes(key, value)) {
            continue;
        }
        seen.insert(key.to_string(), value.to_string());
    }
    // The last copy of a key wins, as when the game reads the file.
    let mut keys: Vec<_> = source
        .pairs()
        .map(|(k, _)| k)
        .filter(|k| seen.contains_key(*k))
        .collect();
    keys.dedup();
    let mut done = std::collections::HashSet::new();
    for key in keys {
        if done.insert(key) {
            out.set(key, &seen[key]);
            taken += 1;
        }
    }
    for (key, value) in seed {
        if out.get(key).is_none() {
            out.set(key, value);
        }
    }
    (out, taken)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MMC: &str = "version:2567\r\nautoJump:false\r\nfov:0.25\r\nmouseSensitivity:0.3\r\n\
        resourcePacks:[\"vanilla\",\"file/Dark.zip\"]\r\nincompatibleResourcePacks:[]\r\n\
        key_key.forward:key.keyboard.e\r\nkey_Create New World:key.keyboard.f6\r\n\
        lastServer:localhost:25565\r\nkey_key.attack:key.mouse.left\r\nbroken line\r\n";

    #[test]
    fn parses_like_the_game() {
        let o = Options::parse(MMC);
        assert_eq!(o.data_version(), Some(2567));
        assert_eq!(o.get("lastServer"), Some("localhost:25565"));
        assert_eq!(o.get("key_Create New World"), Some("key.keyboard.f6"));
        assert_eq!(o.get("broken line"), None);
        // The colon-less line is kept, the line endings become \n.
        assert!(o.render().contains("broken line\n"));
        assert!(!o.render().contains('\r'));
    }

    #[test]
    fn draftout_gets_keys_sensitivity_and_fov_only() {
        let draftout = Options::parse(
            "version:5023\nfov:0.0\nmouseSensitivity:0.5\nkey_key.forward:key.keyboard.w\n\
             key_key.socialInteractions:key.keyboard.p\nonboardAccessibility:false\nautoJump:true\n",
        );
        let (out, n) = merge(
            Some(&draftout),
            &Options::parse(MMC),
            &[Pick::Shared],
            &[("onboardAccessibility", "false")],
        );
        assert_eq!(n, 4);
        assert_eq!(out.data_version(), Some(5023));
        assert_eq!(out.get("fov"), Some("0.25"));
        assert_eq!(out.get("mouseSensitivity"), Some("0.3"));
        assert_eq!(out.get("key_key.forward"), Some("key.keyboard.e"));
        assert_eq!(out.get("key_key.attack"), Some("key.mouse.left"));
        // Not shared: a mod's key, video and sound settings, the packs.
        assert_eq!(out.get("key_Create New World"), None);
        assert_eq!(out.get("autoJump"), Some("true"));
        assert_eq!(out.get("resourcePacks"), None);
        assert_eq!(
            out.get("key_key.socialInteractions"),
            Some("key.keyboard.p")
        );
        assert_eq!(out.get("onboardAccessibility"), Some("false"));
        // The instance's order stays; nothing is duplicated.
        assert!(out.render().starts_with("version:5023\nfov:0.25\n"));
        assert_eq!(out.render().matches("fov:").count(), 1);
    }

    #[test]
    fn a_new_draftout_file_has_the_seed_and_a_version() {
        let (out, _) = merge(
            None,
            &Options::parse(MMC),
            &[Pick::Shared],
            &[("onboardAccessibility", "false")],
        );
        assert_eq!(out.data_version(), Some(2567));
        assert_eq!(out.get("onboardAccessibility"), Some("false"));
        assert_eq!(out.get("autoJump"), None);
    }

    #[test]
    fn numeric_key_codes_are_not_carried_to_another_version() {
        let old = Options::parse("version:1343\nkey_key.forward:17\nfov:0.5\n");
        let (out, n) = merge(None, &old, &[Pick::Shared], &[]);
        assert_eq!(n, 1);
        assert_eq!(out.get("key_key.forward"), None);
    }

    #[test]
    fn same_version_takes_everything_and_packs_only_when_asked() {
        let rsg = Options::parse("version:2567\nautoJump:true\nresourcePacks:[]\nchatScale:1.0\n");
        let (out, _) = merge(Some(&rsg), &Options::parse(MMC), &[Pick::AllSettings], &[]);
        assert_eq!(out.get("autoJump"), Some("false"));
        assert_eq!(out.get("key_Create New World"), Some("key.keyboard.f6"));
        assert_eq!(out.get("resourcePacks"), Some("[]"));
        assert_eq!(out.get("chatScale"), Some("1.0"));
        let (out, n) = merge(
            Some(&rsg),
            &Options::parse(MMC),
            &[Pick::ResourcePacks],
            &[],
        );
        assert_eq!(n, 2);
        assert_eq!(
            out.get("resourcePacks"),
            Some("[\"vanilla\",\"file/Dark.zip\"]")
        );
        assert_eq!(out.get("autoJump"), Some("true"));
    }

    #[test]
    fn the_last_copy_of_a_key_wins() {
        let src = Options::parse("fov:0.1\nfov:0.2\n");
        let (out, n) = merge(None, &src, &[Pick::Shared], &[]);
        assert_eq!((out.get("fov"), n), (Some("0.2"), 1));
    }
}
