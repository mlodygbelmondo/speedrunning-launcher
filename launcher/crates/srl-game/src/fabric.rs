//! Fabric Loader: picking the loader version and fetching its profile JSON.
//!
//! The profile (`/v2/versions/loader/<game>/<loader>/profile/json`) is a version JSON that
//! inherits from vanilla. Fabric's own jars (`fabric-loader`, `intermediary`) come without a
//! sha1 there, so we fetch the Maven `.sha1` sidecar once and store the completed profile under
//! `versions/<profile id>/`, which makes later installs work offline.
use std::path::Path;

use anyhow::{Context, Result, bail};
use srl_core::Paths;

use crate::{
    meta::{MetaCache, read_json},
    schema::{FABRIC_META_URL, FabricLoaderEntry, VersionJson},
    util::{cmp_versions, maven_path, write_atomic},
};

/// The newest stable loader at or above `min`; if no stable one qualifies, the newest one that
/// does.
pub fn choose_loader(entries: &[FabricLoaderEntry], min: &str) -> Option<String> {
    let ok = |e: &&FabricLoaderEntry| cmp_versions(&e.loader.version, min).is_ge();
    let newest = |it: Vec<&FabricLoaderEntry>| {
        it.into_iter()
            .max_by(|a, b| cmp_versions(&a.loader.version, &b.loader.version))
            .map(|e| e.loader.version.clone())
    };
    newest(
        entries
            .iter()
            .filter(ok)
            .filter(|e| e.loader.stable)
            .collect(),
    )
    .or_else(|| newest(entries.iter().filter(ok).collect()))
}

pub fn profile_id(game: &str, loader: &str) -> String {
    format!("fabric-loader-{loader}-{game}")
}

/// Picks the loader for `game`. Uses the cached loader list when offline, and as a last resort
/// the newest profile already installed.
pub async fn resolve_loader(
    meta: &MetaCache<'_>,
    paths: &Paths,
    game: &str,
    min: &str,
) -> Result<String> {
    let url = format!("{FABRIC_META_URL}/versions/loader/{game}");
    let cache = paths
        .meta()
        .join("fabric")
        .join(format!("loaders-{game}.json"));
    match meta.list::<Vec<FabricLoaderEntry>>(&url, &cache).await {
        Ok(entries) => choose_loader(&entries, min)
            .with_context(|| format!("no Fabric Loader {min} or newer for Minecraft {game}")),
        Err(err) => match newest_installed(paths, game, min) {
            Some(v) => {
                meta.warn(format!(
                    "Fabric's loader list is unavailable ({err:#}); using the installed loader {v}"
                ));
                Ok(v)
            }
            None => Err(err),
        },
    }
}

/// The newest loader at or above `min` whose profile is already under `versions/`, so it needs
/// no network.
pub fn newest_installed(paths: &Paths, game: &str, min: &str) -> Option<String> {
    installed_loaders(&paths.versions(), game)
        .into_iter()
        .filter(|v| cmp_versions(v, min).is_ge())
        .max_by(|a, b| cmp_versions(a, b))
}

fn installed_loaders(versions: &Path, game: &str) -> Vec<String> {
    let suffix = format!("-{game}");
    let Ok(dir) = std::fs::read_dir(versions) else {
        return Vec::new();
    };
    dir.filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|name| {
            let loader = name.strip_prefix("fabric-loader-")?.strip_suffix(&suffix)?;
            let json = versions.join(&name).join(format!("{name}.json"));
            json.exists().then(|| loader.to_string())
        })
        .collect()
}

/// The loader profile, with a sha1 (and size, when the server tells) for every library.
pub async fn load_profile(
    meta: &MetaCache<'_>,
    paths: &Paths,
    game: &str,
    loader: &str,
) -> Result<VersionJson> {
    let id = profile_id(game, loader);
    // The loader version can come from the user (`with_fabric_loader`).
    if crate::util::safe_relative_path(&id).is_none_or(|p| p.components().count() != 1) {
        bail!("bad Fabric Loader version {loader:?}");
    }
    let path = paths.version_dir(&id).join(format!("{id}.json"));
    if let Ok(profile) = read_json::<VersionJson>(&path).await
        && profile.libraries.iter().all(|l| l.sha1.is_some())
    {
        return Ok(profile);
    }
    let url = format!("{FABRIC_META_URL}/versions/loader/{game}/{loader}/profile/json");
    let mut profile: VersionJson = meta.downloader.fetch_json(&url).await?;
    if profile.inherits_from.as_deref() != Some(game) {
        bail!("Fabric profile {url} does not inherit from {game}");
    }
    for lib in &mut profile.libraries {
        if lib.sha1.is_some() {
            continue;
        }
        let base = lib.url.as_deref().unwrap_or("https://maven.fabricmc.net/");
        let sep = if base.ends_with('/') { "" } else { "/" };
        let jar = format!("{base}{sep}{}", maven_path(&lib.name)?);
        let sidecar = meta.downloader.fetch_bytes(&format!("{jar}.sha1")).await?;
        let sha1 = String::from_utf8_lossy(&sidecar)
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if sha1.len() != 40 || !sha1.bytes().all(|b| b.is_ascii_hexdigit()) {
            bail!("bad sha1 sidecar for {jar}");
        }
        lib.sha1 = Some(sha1);
        // For byte-based progress. Optional: the download is verified by the sha1 either way.
        if lib.size.is_none()
            && let Ok(resp) = meta.downloader.client().head(&jar).send().await
        {
            // From the header: `content_length()` reports the (empty) HEAD body.
            lib.size = resp.error_for_status().ok().and_then(|r| {
                r.headers()
                    .get("content-length")?
                    .to_str()
                    .ok()?
                    .parse()
                    .ok()
            });
        }
    }
    write_atomic(&path, &serde_json::to_vec_pretty(&profile)?).await?;
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(list: &[(&str, bool)]) -> Vec<FabricLoaderEntry> {
        list.iter()
            .map(|(v, stable)| {
                serde_json::from_value(
                    serde_json::json!({"loader": {"version": v, "stable": stable}}),
                )
                .unwrap()
            })
            .collect()
    }

    #[test]
    fn newest_stable_at_or_above_min() {
        let e = entries(&[
            ("0.20.1", true),
            ("0.20.2", false),
            ("0.19.5", false),
            ("0.16.14", true),
            ("0.15.11", true),
        ]);
        assert_eq!(choose_loader(&e, "0.16.0").as_deref(), Some("0.20.1"));
        assert_eq!(choose_loader(&e, "0.20.2").as_deref(), Some("0.20.2"));
        assert_eq!(choose_loader(&e, "0.21.0"), None);
        let old = entries(&[("0.15.11", true), ("0.16.1", false)]);
        assert_eq!(choose_loader(&old, "0.16.0").as_deref(), Some("0.16.1"));
    }
}
