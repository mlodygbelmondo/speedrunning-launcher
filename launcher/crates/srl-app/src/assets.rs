//! Embedded assets from `launcher/assets`: images, icons, fonts and the JSON files. Nothing is read
//! from disk at runtime, so the binary works from any directory.
use std::borrow::Cow;

use gpui::{AssetSource, ImageSource, Resource, SharedString};

mod files {
    include!(concat!(env!("OUT_DIR"), "/files.rs"));
}

pub const COPY_JSON: &str = include_str!("../../../assets/copy.json");
pub const COPY_PL_JSON: &str = include_str!("../../../assets/copy.pl.json");
pub const PRACTICE_JSON: &str = include_str!("../../../assets/practice.json");
pub const LEARN_JSON: &str = include_str!("../../../assets/learn.json");
pub const DEMO_JSON: &str = include_str!("../../../assets/demo/demo.json");
pub const DEMO_PL_JSON: &str = include_str!("../../../assets/demo/demo.pl.json");
/// Settings' credits for the player: the pictures, fonts and icons, with their authors and
/// licences (English, then Polish). The developer notes are `assets/art/CREDITS.md`.
pub const CREDITS_MD: &str = include_str!("../../../assets/credits.md");
pub const CREDITS_PL_MD: &str = include_str!("../../../assets/credits.pl.md");
/// Settings' credits: the licences of adapted code (Pandora Launcher, EasyInjectBundled).
pub const NOTICES_MD: &str = include_str!("../../../THIRD_PARTY_NOTICES.md");

/// Every font the v3 screens use (REDESIGN.md "Fonts"): static Archivo at width 112 ("Archivo SRL"),
/// IBM Plex Mono and Inter.
pub const FONTS: [&[u8]; 10] = [
    include_bytes!("../../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Bold.ttf"),
    include_bytes!("../../../assets/fonts/ArchivoSRL-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/ArchivoSRL-Bold.ttf"),
    include_bytes!("../../../assets/fonts/ArchivoSRL-ExtraBold.ttf"),
    include_bytes!("../../../assets/fonts/IBMPlexMono-Regular.ttf"),
    include_bytes!("../../../assets/fonts/IBMPlexMono-Medium.ttf"),
    include_bytes!("../../../assets/fonts/IBMPlexMono-SemiBold.ttf"),
];

/// The bytes of an embedded file by its path under `launcher/assets`.
pub fn bytes(path: &str) -> Option<&'static [u8]> {
    files::FILES
        .binary_search_by(|(n, _)| (*n).cmp(path))
        .ok()
        .map(|i| files::FILES[i].1)
}

/// Serves the embedded files to `img()` and `svg().path()`. GPUI decodes images off the main thread
/// and caches them; SVGs are rasterised per size into the sprite atlas.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(bytes(path).map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

pub fn embedded(path: &str) -> ImageSource {
    ImageSource::Resource(Resource::Embedded(SharedString::from(path.to_string())))
}

/// Asset path of a card image (`card/<id>.jpg`, 640×360).
pub fn card_path(id: &str) -> SharedString {
    SharedString::from(format!("art/card/{id}.jpg"))
}

/// Asset path of a full-window hero image (`hero/<id>.jpg`, 1280×800).
pub fn hero_path(id: &str) -> SharedString {
    SharedString::from(format!("art/hero/{id}.jpg"))
}

pub const SHADE_HERO: &str = "art/shade-hero.png";

/// Asset path of an icon (`logo`, or a Lucide icon).
pub fn icon_path(name: &str) -> SharedString {
    SharedString::from(format!("icons/{name}.svg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_and_practice_art_is_embedded() {
        for m in srl_core::Mode::ALL {
            let art = crate::catalog::def(m).art;
            assert!(bytes(&hero_path(art)).is_some(), "hero {art}");
            assert!(bytes(&card_path(art)).is_some(), "card {art}");
        }
        for c in &crate::catalog::practice().categories {
            assert!(bytes(&card_path(&c.art)).is_some(), "card {}", c.art);
        }
        for topic in &crate::catalog::learn().topics {
            assert!(
                bytes(&card_path(&topic.art)).is_some(),
                "card {}",
                topic.art
            );
        }
        assert!(bytes(SHADE_HERO).is_some());
        assert!(bytes(&icon_path("logo")).is_some());
    }
}
