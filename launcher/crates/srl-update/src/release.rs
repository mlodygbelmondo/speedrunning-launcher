//! A GitHub release as `releases/latest` describes it, what of it is an update, and the checksum
//! asset.
use semver::Version;
use serde::Deserialize;

use crate::{CHECKSUM_NAME, MAX_ZIP_BYTES, ZIP_NAME};

/// The fields of GitHub's release object the update needs.
#[derive(Clone, Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    /// The release's page, for a player who has to download it by hand.
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

/// A newer release with everything the update needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub version: Version,
    pub tag: String,
    pub page: String,
    pub zip: Asset,
    pub checksum: Asset,
}

/// The version a release tag names: `v1.2.3` or `1.2.3`. A pre-release version (`v1.3.0-rc.1`)
/// is never an update, and neither is a tag that isn't semver.
pub fn parse_tag(tag: &str) -> Option<Version> {
    let v = Version::parse(tag.trim().strip_prefix('v').unwrap_or(tag.trim())).ok()?;
    v.pre.is_empty().then_some(v)
}

/// The tag's version if it is newer than `current`, by semver precedence (build metadata such
/// as `+abc` doesn't count).
pub fn newer(tag: &str, current: &Version) -> Option<Version> {
    parse_tag(tag).filter(|v| v.cmp_precedence(current).is_gt())
}

/// The Windows zip and its checksum among a release's assets.
pub fn pick_assets(assets: &[Asset]) -> Result<(Asset, Asset), String> {
    let find = |name: &str| {
        assets
            .iter()
            .find(|a| a.name == name)
            .cloned()
            .ok_or_else(|| format!("the release has no {name}"))
    };
    Ok((find(ZIP_NAME)?, find(CHECKSUM_NAME)?))
}

impl Release {
    /// `Some` when this release is newer than `current` and has its assets. A newer release
    /// without them, or with a zip too big to trust, is an error: someone has to fix it.
    pub fn offer(&self, current: &Version) -> Result<Option<Offer>, String> {
        if self.draft || self.prerelease {
            return Ok(None);
        }
        let Some(version) = newer(&self.tag_name, current) else {
            return Ok(None);
        };
        let (zip, checksum) =
            pick_assets(&self.assets).map_err(|e| format!("{}: {e}", self.tag_name))?;
        if zip.size > MAX_ZIP_BYTES {
            return Err(format!(
                "{}: {ZIP_NAME} is {} bytes (the limit is {MAX_ZIP_BYTES})",
                self.tag_name, zip.size
            ));
        }
        Ok(Some(Offer {
            version,
            tag: self.tag_name.clone(),
            page: if self.html_url.is_empty() {
                crate::RELEASES_PAGE.to_string()
            } else {
                self.html_url.clone()
            },
            zip,
            checksum,
        }))
    }
}

/// The SHA-256 in a checksum file, lower-case. The format is `sha256sum`'s: the hash, then
/// optionally the file name (`*` before it in binary mode), which must then be `file`.
pub fn parse_checksum(text: &str, file: &str) -> Result<String, String> {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .ok_or("the checksum file is empty")?;
    let mut parts = line.split_whitespace();
    let hash = parts.next().unwrap_or_default();
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("not a SHA-256: {hash:?}"));
    }
    if let Some(name) = parts.next() {
        let name = name.trim_start_matches('*');
        if name != file {
            return Err(format!("the checksum is for {name:?}, not {file:?}"));
        }
    }
    Ok(hash.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn versions_compare_by_semver() {
        let current = v("0.9.0");
        assert_eq!(newer("v0.10.0", &current), Some(v("0.10.0")));
        assert_eq!(newer("1.0.0", &current), Some(v("1.0.0")));
        assert_eq!(newer("v0.9.0", &current), None);
        assert_eq!(newer("v0.8.12", &current), None);
        // Pre-releases and tags that aren't versions are never updates.
        assert_eq!(newer("v1.0.0-rc.1", &current), None);
        assert_eq!(newer("launcher-latest", &current), None);
        assert_eq!(newer("v1.0", &current), None);
        assert_eq!(newer("", &current), None);
        // Build metadata doesn't make a version newer.
        assert_eq!(newer("v0.9.0+abc", &current), None);
    }

    fn asset(name: &str, size: u64) -> Asset {
        Asset {
            name: name.into(),
            browser_download_url: format!("https://example.invalid/{name}"),
            size,
        }
    }

    fn release(tag: &str, assets: Vec<Asset>) -> Release {
        Release {
            tag_name: tag.into(),
            html_url: format!("https://github.com/x/y/releases/tag/{tag}"),
            draft: false,
            prerelease: false,
            assets,
        }
    }

    #[test]
    fn picks_the_windows_zip_and_its_checksum() {
        let assets = vec![
            asset("speedrunning-launcher-linux.tar.gz", 10),
            asset(CHECKSUM_NAME, 99),
            asset(ZIP_NAME, 25 << 20),
            asset("speedrunning-launcher-windows.zip.sig", 10),
        ];
        let (zip, sum) = pick_assets(&assets).unwrap();
        assert_eq!((zip.name.as_str(), zip.size), (ZIP_NAME, 25 << 20));
        assert_eq!(sum.name, CHECKSUM_NAME);

        let current = v("0.1.0");
        let offer = release("v0.2.0", assets.clone())
            .offer(&current)
            .unwrap()
            .unwrap();
        assert_eq!(offer.version, v("0.2.0"));
        assert_eq!(offer.page, "https://github.com/x/y/releases/tag/v0.2.0");
        // The same version: nothing, even with the assets.
        assert_eq!(release("v0.1.0", assets.clone()).offer(&current), Ok(None));
        // A newer release without its checksum, or with a zip too big, is an error.
        let no_sum = release("v0.2.0", vec![asset(ZIP_NAME, 1)]);
        assert!(no_sum.offer(&current).unwrap_err().contains(CHECKSUM_NAME));
        let huge = release(
            "v0.2.0",
            vec![asset(ZIP_NAME, MAX_ZIP_BYTES + 1), asset(CHECKSUM_NAME, 1)],
        );
        assert!(huge.offer(&current).is_err());
        // A prerelease is never offered, whatever its tag says.
        let mut pre = release("v0.2.0", assets);
        pre.prerelease = true;
        assert_eq!(pre.offer(&current), Ok(None));
    }

    #[test]
    fn github_release_json_parses() {
        let json = r#"{"url":"https://api.github.com/x","tag_name":"v0.2.0","name":"0.2.0",
            "draft":false,"prerelease":false,"html_url":"https://github.com/r/releases/tag/v0.2.0",
            "assets":[{"name":"speedrunning-launcher-windows.zip","size":26214400,
            "browser_download_url":"https://github.com/r/releases/download/v0.2.0/speedrunning-launcher-windows.zip",
            "digest":"sha256:00","content_type":"application/zip"}]}"#;
        let r: Release = serde_json::from_str(json).unwrap();
        assert_eq!(r.tag_name, "v0.2.0");
        assert_eq!(r.assets[0].size, 26_214_400);
    }

    #[test]
    fn checksum_files() {
        let h = "A".repeat(64);
        let want = "a".repeat(64);
        assert_eq!(
            parse_checksum(&format!("{h}  {ZIP_NAME}\n"), ZIP_NAME),
            Ok(want.clone())
        );
        assert_eq!(
            parse_checksum(&format!("{h} *{ZIP_NAME}"), ZIP_NAME),
            Ok(want.clone())
        );
        assert_eq!(parse_checksum(&format!("\n{h}\n"), ZIP_NAME), Ok(want));
        assert!(parse_checksum(&format!("{h}  other.zip"), ZIP_NAME).is_err());
        assert!(parse_checksum("abc123  x.zip", ZIP_NAME).is_err());
        assert!(parse_checksum(&format!("{}  {ZIP_NAME}", "g".repeat(64)), ZIP_NAME).is_err());
        assert!(parse_checksum("  \n", ZIP_NAME).is_err());
    }
}
