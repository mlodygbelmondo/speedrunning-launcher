//! Embedded assets: images, icons, fonts and the JSON files. Nothing is read from disk at runtime.
use std::borrow::Cow;

use gpui::{AssetSource, ImageSource, Resource, SharedString};

mod avatars {
    include!(concat!(env!("OUT_DIR"), "/avatars.rs"));
}
mod icons {
    include!(concat!(env!("OUT_DIR"), "/icons.rs"));
}
mod art {
    include!(concat!(env!("OUT_DIR"), "/art.rs"));
}

const HERO_JPG: &[u8] = include_bytes!("../../assets/hero.jpg");
const GLOW_PNG: &[u8] = include_bytes!("../../assets/glow.png");
pub const DATA_JSON: &str = include_str!("../../assets/data.json");
pub const COPY_JSON: &str = include_str!("../../assets/copy.json");
pub const COPY_PL_JSON: &str = include_str!("../../assets/copy.pl.json");
pub const SCREENS_JSON: &str = include_str!("../../assets/screens.json");
pub const SCREENS_PL_JSON: &str = include_str!("../../assets/screens.pl.json");
pub const FONTS: [&[u8]; 12] = [
    include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../../assets/fonts/Inter-Bold.ttf"),
    include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../../assets/fonts/JetBrainsMono-Bold.ttf"),
    // Scene v3 (REDESIGN.md): static Archivo at width 112 and IBM Plex Mono.
    include_bytes!("../../assets/fonts/ArchivoSRL-SemiBold.ttf"),
    include_bytes!("../../assets/fonts/ArchivoSRL-Bold.ttf"),
    include_bytes!("../../assets/fonts/ArchivoSRL-ExtraBold.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-Medium.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-SemiBold.ttf"),
];

/// Serves the embedded files to `img()` and `svg().path()`. GPUI decodes images off the main thread
/// and caches them; SVGs are rasterised per size into the sprite atlas.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        let bytes: Option<&'static [u8]> = match path {
            "hero.jpg" => Some(HERO_JPG),
            "glow.png" => Some(GLOW_PNG),
            p if p.starts_with("art/") => art::ART
                .binary_search_by(|(n, _)| (*n).cmp(p))
                .ok()
                .map(|i| art::ART[i].1),
            p if p.starts_with("icons/") => p
                .strip_prefix("icons/")
                .and_then(|s| s.strip_suffix(".svg"))
                .and_then(|name| {
                    icons::ICONS
                        .binary_search_by(|(n, _)| (*n).cmp(name))
                        .ok()
                        .map(|i| icons::ICONS[i].1)
                }),
            p => p
                .strip_prefix("avatars/")
                .and_then(|s| s.strip_suffix(".png"))
                .and_then(|s| s.parse::<usize>().ok())
                .and_then(|i| avatars::AVATARS.get(i).copied()),
        };
        Ok(bytes.map(Cow::Borrowed))
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

/// Raw bytes of an embedded avatar (`avatars/NNN.png`).
pub fn avatar_bytes(index: usize) -> Option<&'static [u8]> {
    avatars::AVATARS.get(index).copied()
}

/// Asset path of a Lucide icon (or `logo`).
pub fn icon_path(name: &str) -> SharedString {
    SharedString::from(format!("icons/{name}.svg"))
}
