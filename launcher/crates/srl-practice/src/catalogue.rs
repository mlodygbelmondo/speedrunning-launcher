//! The install catalogue (`data/catalogue.json`, built into the binary): for each practice map, the
//! author's own download pinned by sha512 and size, the credit, the licence status and what the
//! map needs. The app's Practice page shows these entries as they are (its `assets/practice.json`
//! adds only the categories' art and the practice tools).
//!
//! The catalogue is embedded rather than fetched: a map download runs nothing but still lands in
//! the player's saves, so what we install changes only with a reviewed launcher build. A hosted,
//! signed catalogue (research/04 §4.1) can replace `Catalogue::builtin` later without changing
//! the types.
use std::collections::HashSet;

use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

/// Minecraft 1.16.1's world `DataVersion`. A world saved by a newer game (a higher number) makes
/// 1.16.1 warn before opening it, and may not load at all.
pub const DATA_VERSION_1_16_1: i32 = 2567;

/// The Minecraft version the practice instance runs (`crate::PracticeProfile`).
pub const PRACTICE_MINECRAFT: &str = "1.16.1";

/// The categories `practice.json` uses (`practice.cat.<id>` in copy.json).
pub const CATEGORIES: [&str; 6] = [
    "all-in-one",
    "overworld",
    "bastion",
    "blaze",
    "stronghold",
    "end",
];

const BUILTIN: &str = include_str!("../data/catalogue.json");

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Catalogue {
    pub schema: u32,
    /// When the downloads were last checked by `tools/refresh_catalogue.py` (ISO date).
    pub checked: String,
    pub maps: Vec<MapEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapEntry {
    /// Stable id: the app's copy key `map.<id>`, install records, deep links.
    pub id: String,
    pub name: String,
    /// As shown on the card, e.g. `Semperzz & ItzToxic`.
    pub authors: String,
    /// One of [`CATEGORIES`].
    pub category: String,
    /// The Minecraft version the map is made for.
    pub minecraft: String,
    /// Full credit, shown on the map's page: authors, bundled sections, contributors.
    pub credit: String,
    /// The author's page (GitHub repo, or the only page there is). Always shown, with or without
    /// a download.
    pub source_page: String,
    #[serde(default)]
    pub video: Option<String>,
    #[serde(default)]
    pub support: Vec<SupportLink>,
    pub licence: Licence,
    #[serde(default)]
    pub requirements: Vec<Requirement>,
    /// Player data the author says to copy from an old version of the map into a new one.
    #[serde(default)]
    pub carry_over: Vec<CarryOver>,
    /// Exactly one of `download` and `link_only` is set.
    #[serde(default)]
    pub download: Option<MapDownload>,
    #[serde(default)]
    pub link_only: Option<LinkOnly>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportLink {
    pub label: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Licence {
    /// SPDX id; `None` when the author states no licence (all rights reserved).
    pub spdx: Option<String>,
    pub redistribution: Redistribution,
    pub note: String,
}

/// What research/04 §3 allows for the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Redistribution {
    /// An open licence: mirroring would be allowed with the notice. We still download from the
    /// author.
    OpenLicence,
    /// The author allowed mirroring in writing.
    PermissionGranted,
    /// No licence: download only from the author's own release URL, never re-host, credit.
    LinkOnly,
    /// Link only, no automatic download.
    Proprietary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requirement {
    /// Stable id, for a copy key (`practice.req.<id>`) once the UI translates these.
    pub id: String,
    pub kind: RequirementKind,
    /// The map works without it.
    pub optional: bool,
    /// The launcher takes care of it (installs it, or a launcher feature covers it).
    pub provided: bool,
    /// English text for the UI until there is a copy key.
    pub note: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequirementKind {
    Mod,
    Datapack,
    Setting,
    Note,
}

/// A file to copy from the old world into the new one on an update, as the author documents it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarryOver {
    /// Relative to the world folder, `/`-separated.
    pub from: String,
    pub to: String,
    /// What it holds, for the UI: "loadouts", "loadouts, PBs and settings".
    pub what: String,
    /// A command the player runs in the new world afterwards (the map imports the file).
    #[serde(default)]
    pub then_run: Option<String>,
    /// Only the same version of the map can read the file (its layout changes between versions,
    /// and the author documents another way across versions), so it is carried on a Reset to the
    /// same version and never on an update.
    #[serde(default)]
    pub same_version_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapDownload {
    pub version: String,
    /// The release tag. Some authors move one tag between versions (`latest`), which is why the
    /// file is pinned by hash and not trusted by URL.
    pub tag: String,
    pub released: String,
    /// `owner/name` on GitHub, for the refresh script.
    pub repo: String,
    /// Regex for the asset name in a newer release, for the refresh script.
    pub asset_pattern: String,
    /// The author's own release asset.
    pub url: String,
    /// Lower-case hex.
    pub sha512: String,
    pub size: u64,
    /// The world folders inside the zip (`""` is a world at the zip's root), as the refresh
    /// script found them.
    pub worlds: Vec<String>,
    /// The highest level.dat `DataVersion` among those worlds.
    pub data_version: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkOnly {
    #[serde(default)]
    pub version: Option<String>,
    pub reason: LinkOnlyReason,
    /// English text for the UI until there is a copy key.
    pub note: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkOnlyReason {
    /// The author's page needs a browser (Mediafire and similar): no stable URL to pin.
    NoDirectDownload,
    /// Made for a Minecraft version the practice instance doesn't run.
    OtherMinecraftVersion,
    /// The author asked not to be auto-downloaded.
    AuthorOptOut,
}

impl MapEntry {
    pub fn is_installable(&self) -> bool {
        self.download.is_some()
    }

    /// The version to show on the card.
    pub fn version(&self) -> Option<&str> {
        match (&self.download, &self.link_only) {
            (Some(d), _) => Some(&d.version),
            (None, Some(l)) => l.version.as_deref(),
            (None, None) => None,
        }
    }
}

impl Catalogue {
    /// The catalogue built into this launcher. Validated by the tests, so this can't fail in a
    /// released build; it still returns an error rather than panic.
    pub fn builtin() -> Result<Self> {
        Self::parse(BUILTIN)
    }

    pub fn parse(json: &str) -> Result<Self> {
        let c: Catalogue = serde_json::from_str(json)?;
        c.validate()?;
        Ok(c)
    }

    pub fn get(&self, id: &str) -> Option<&MapEntry> {
        self.maps.iter().find(|m| m.id == id)
    }

    /// The rules every entry must follow. They encode research/04 §3: downloads only from the
    /// author's own GitHub release, pinned, credited, and only 1.16.1 maps installable.
    pub fn validate(&self) -> Result<()> {
        ensure!(self.schema == 1, "unknown catalogue schema {}", self.schema);
        let mut ids = HashSet::new();
        for m in &self.maps {
            let ctx = |msg: &str| format!("map {:?}: {msg}", m.id);
            ensure!(ids.insert(m.id.as_str()), ctx("duplicate id"));
            ensure!(
                !m.id.is_empty()
                    && m.id
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                ctx("id must be [a-z0-9-]+")
            );
            ensure!(!m.name.trim().is_empty(), ctx("no name"));
            ensure!(!m.authors.trim().is_empty(), ctx("no authors"));
            ensure!(!m.credit.trim().is_empty(), ctx("no credit"));
            ensure!(
                CATEGORIES.contains(&m.category.as_str()),
                ctx("unknown category")
            );
            ensure!(m.source_page.starts_with("https://"), ctx("source page"));
            for link in m.video.iter().chain(m.support.iter().map(|s| &s.url)) {
                ensure!(link.starts_with("https://"), ctx("links must be https"));
            }
            for c in &m.carry_over {
                ensure!(
                    crate::archive::safe_relative(&c.from).is_some()
                        && crate::archive::safe_relative(&c.to).is_some(),
                    ctx("carry_over paths must be relative and inside the world")
                );
            }
            match (&m.download, &m.link_only) {
                (Some(d), None) => {
                    validate_download(m, d).map_err(|e| anyhow::anyhow!(ctx(&e.to_string())))?
                }
                (None, Some(_)) => {}
                _ => bail!(ctx("needs exactly one of download and link_only")),
            }
        }
        Ok(())
    }
}

fn validate_download(m: &MapEntry, d: &MapDownload) -> Result<()> {
    ensure!(
        m.minecraft == PRACTICE_MINECRAFT,
        "only {PRACTICE_MINECRAFT} maps are installable; list others as link_only"
    );
    ensure!(
        m.licence.redistribution != Redistribution::Proprietary,
        "a proprietary map is link only"
    );
    ensure!(
        is_sha512_hex(&d.sha512),
        "sha512 must be 128 lower-case hex digits (run tools/refresh_catalogue.py --write)"
    );
    ensure!(d.size > 0, "size");
    ensure!(!d.version.trim().is_empty(), "version");
    // The author's own GitHub release, never a mirror: research/04 §3 and §4.2.
    let (owner, name) = d.repo.split_once('/').unwrap_or_default();
    ensure!(
        !owner.is_empty() && !name.is_empty() && !name.contains('/'),
        "repo must be owner/name"
    );
    ensure!(
        is_release_asset(&d.url, owner, name, &d.tag),
        "url must be a release asset of {} under tag {:?}",
        d.repo,
        d.tag
    );
    let repo_page = format!("https://github.com/{}", d.repo);
    ensure!(
        m.source_page == repo_page || m.source_page.starts_with(&format!("{repo_page}/")),
        "the source page must be the repo the download comes from"
    );
    ensure!(!d.worlds.is_empty(), "no worlds recorded");
    if let Some(dv) = d.data_version {
        ensure!(
            dv <= DATA_VERSION_1_16_1,
            "DataVersion {dv} is newer than 1.16.1 ({DATA_VERSION_1_16_1})"
        );
    }
    Ok(())
}

/// 128 lower-case hex digits. A sha512 becomes a path in the content store, so nothing else may
/// pass (a record's `../x` would point outside it).
pub fn is_sha512_hex(s: &str) -> bool {
    s.len() == 128
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `https://github.com/<owner>/<name>/releases/download/<tag>/<asset>`, exactly. A string prefix
/// is not enough: `../` segments (or `%2e%2e`) after the prefix would resolve to another repo.
/// The URL must already be in normal form (parsing it changes nothing), so what is checked is
/// what gets requested.
fn is_release_asset(url: &str, owner: &str, name: &str, tag: &str) -> bool {
    let Ok(u) = reqwest::Url::parse(url) else {
        return false;
    };
    let lower = url.to_ascii_lowercase();
    if u.as_str() != url
        || ["%2e", "%2f", "%5c", "\\"]
            .iter()
            .any(|s| lower.contains(s))
    {
        return false;
    }
    if u.scheme() != "https"
        || u.host_str() != Some("github.com")
        || u.port().is_some()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return false;
    }
    let Some(segments) = u.path_segments() else {
        return false;
    };
    let segments: Vec<&str> = segments.collect();
    matches!(
        segments[..],
        [o, n, "releases", "download", t, asset]
            if o == owner && n == name && t == tag && !asset.is_empty() && asset != "." && asset != ".."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builtin() -> Catalogue {
        Catalogue::builtin().expect("the built-in catalogue is valid")
    }

    #[test]
    fn builtin_is_valid_and_covers_the_core_set() {
        let c = builtin();
        // research/04 §1.1, M1 to M14.
        for id in [
            "the-mcsr-practice-map-dibedy",
            "llama-s-bastion-practice-llamapag",
            "end-practice-ryguy2k4",
            "zero-practice-mescht",
            "blaze-practice-mescht",
            "blaze-practice-semperzz",
            "portal-practice-v2-semperzz",
            "portal-practice-mescht",
            "ow-practice-7rowl",
            "crafting-practice-v2-semperzz",
            "zero-sorting-practice-semperzz",
            "bt-practice-mescht",
            "end-portal-fill-v2-cylorun",
            "vine-ssg-practice-bdamja",
        ] {
            assert!(c.get(id).is_some(), "{id} missing");
        }
        let vine = c.get("vine-ssg-practice-bdamja").unwrap();
        assert!(!vine.is_installable());
        assert_eq!(
            vine.link_only.as_ref().unwrap().reason,
            LinkOnlyReason::OtherMinecraftVersion
        );
    }

    /// The app's cards come from this catalogue (one source, so a download button never installs
    /// something other than the card says); `assets/practice.json` only adds the categories' art,
    /// and must have exactly these categories.
    #[test]
    fn matches_the_ui_categories() {
        #[derive(Deserialize)]
        struct Ui {
            categories: Vec<UiCategory>,
            #[serde(default)]
            maps: Option<serde_json::Value>,
        }
        #[derive(Deserialize)]
        struct UiCategory {
            id: String,
        }
        let ui: Ui = serde_json::from_str(include_str!("../../../assets/practice.json")).unwrap();
        let ids: Vec<&str> = ui.categories.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, CATEGORIES);
        assert!(ui.maps.is_none(), "maps belong in data/catalogue.json only");
    }

    fn entry_json() -> serde_json::Value {
        serde_json::to_value(builtin().get("zero-practice-mescht").unwrap()).unwrap()
    }

    fn check(map: serde_json::Value) -> Result<Catalogue> {
        let c = serde_json::json!({ "schema": 1, "checked": "2026-10-26", "maps": [map] });
        Catalogue::parse(&c.to_string())
    }

    #[test]
    fn validation_rejects_bad_entries() {
        assert!(check(entry_json()).is_ok());

        let mut m = entry_json();
        m["download"]["url"] = "https://example.com/mirror/Zero.zip".into();
        assert!(check(m).is_err(), "a mirror is not the author's release");

        let mut m = entry_json();
        m["download"]["url"] =
            "https://github.com/someone-else/Zero-Practice/releases/download/v1/Zero.zip".into();
        assert!(check(m).is_err(), "another repo's asset");

        // Dot segments after the right prefix resolve to another repo.
        for url in [
            "https://github.com/Mescht/Zero-Practice/releases/download/v1.2.2/../../../../someone-else/mirror/releases/download/v1/Zero.zip",
            "https://github.com/Mescht/Zero-Practice/releases/download/v1.2.2/%2e%2e/%2E%2E/x.zip",
            "https://github.com/Mescht/Zero-Practice/releases/download/v1.2.2/a%2fb.zip",
            "https://github.com/Mescht/Zero-Practice/releases/download/v1.2.2/./Zero.zip",
            "https://github.com/Mescht/Zero-Practice/releases/download/v1.2.2/Zero.zip?x=1",
            "https://github.com:8443/Mescht/Zero-Practice/releases/download/v1.2.2/Zero.zip",
            "https://user@github.com/Mescht/Zero-Practice/releases/download/v1.2.2/Zero.zip",
            "http://github.com/Mescht/Zero-Practice/releases/download/v1.2.2/Zero.zip",
            "https://github.com/Mescht/Zero-Practice/releases/download/v9/Zero.zip",
            "https://github.com/Mescht/Zero-Practice/releases/download/v1.2.2/",
            "https://github.com/Mescht/Zero-Practice/releases/download/v1.2.2/x/Zero.zip",
        ] {
            let mut m = entry_json();
            m["download"]["url"] = url.into();
            assert!(check(m).is_err(), "{url}");
        }

        let mut m = entry_json();
        m["source_page"] = "https://github.com/Mescht/Zero-Practice-mirror".into();
        assert!(check(m).is_err(), "a look-alike source page");

        let mut m = entry_json();
        m["download"]["sha512"] = "ABC".into();
        assert!(check(m).is_err(), "bad hash");

        let mut m = entry_json();
        m["download"]["sha512"] = m["download"]["sha512"]
            .as_str()
            .unwrap()
            .to_uppercase()
            .into();
        assert!(check(m).is_err(), "upper-case hash");

        let mut m = entry_json();
        m["minecraft"] = "1.16.5".into();
        assert!(check(m).is_err(), "only 1.16.1 is installable");

        let mut m = entry_json();
        m["download"]["data_version"] = 2586.into();
        assert!(check(m).is_err(), "a world saved by 1.16.5");

        let mut m = entry_json();
        m["category"] = "nether".into();
        assert!(check(m).is_err(), "unknown category");

        let mut m = entry_json();
        m["credit"] = " ".into();
        assert!(check(m).is_err(), "no credit");

        let mut m = entry_json();
        m["carry_over"][0]["from"] = "../../options.txt".into();
        assert!(check(m).is_err(), "carry-over outside the world");

        let mut m = entry_json();
        m["link_only"] = serde_json::json!({ "reason": "no-direct-download", "note": "x" });
        assert!(check(m).is_err(), "both download and link_only");

        let mut m = entry_json();
        m["download"] = serde_json::Value::Null;
        assert!(check(m).is_err(), "neither");

        let m = entry_json();
        let c = serde_json::json!({ "schema": 1, "checked": "x", "maps": [m.clone(), m] });
        assert!(Catalogue::parse(&c.to_string()).is_err(), "duplicate id");
    }
}
