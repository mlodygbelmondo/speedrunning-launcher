//! Speedrunluncher bake-off: GPUI contender. See ../SPEC.md (measured scene) and ../SCREENS.md
//! (clickable prototype screens).
//!
//! - `app`: root view, navigation + page transition, rail v2, page header v2, toast.
//! - `screens/*`: one file per screen. `ui`: shared components. `scenario`: the measured run.
//! - `copy`: UI strings and the current language (copy.json, copy.pl.json). `data`: data.json and
//!   screens.json (+ the screens.pl.json overlay). `assets`: embedded files.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod assets;
mod copy;
mod data;
mod report;
mod scenario;
mod screens;
mod shell;
mod theme;
mod ui;
mod v3;

use std::borrow::Cow;

use gpui::{
    App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions, px,
    size,
};

use app::Scene;
use assets::{Assets, FONTS};

fn main() {
    let phase_ms = report::init();
    // Parse the embedded JSON (both languages) up front so a broken file fails at startup.
    copy::init();
    data::init();
    // Deep link (SCREENS.md "Screen ids") and start language (SCREENS.md "Language"). Both are
    // ignored in scenario mode, which always runs the English Home/Library scene.
    let deep_link = std::env::var("BAKEOFF_SCREEN")
        .ok()
        .filter(|s| !s.is_empty() && phase_ms.is_none());
    if phase_ms.is_none()
        && let Ok(code) = std::env::var("BAKEOFF_LANG")
        && !code.is_empty()
    {
        match copy::Lang::from_code(&code) {
            Some(lang) => copy::set_lang(lang),
            None => eprintln!("BAKEOFF_LANG: unknown language {code:?} (en|pl), using en"),
        }
    }

    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            cx.set_quit_mode(gpui::QuitMode::Explicit);
            cx.text_system()
                .add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect())
                .expect("bundled fonts");
            v3::init_text(cx);

            let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(copy::window_title()),
                    ..Default::default()
                }),
                window_decorations: Some(WindowDecorations::Server),
                app_id: Some("bakeoff-gpui".into()),
                // GPUI caps unfocused windows at ~30 fps by default. Browsers don't throttle a
                // visible-but-unfocused window, so turn it off to keep the phases comparable.
                inactive_frame_interval: None,
                ..Default::default()
            };
            let deep_link = deep_link.clone();
            cx.open_window(options, move |_, cx| {
                cx.new(|_| {
                    let mut scene = Scene::new(phase_ms);
                    if let Some(id) = &deep_link
                        && !scene.apply_deep_link(id)
                    {
                        eprintln!("BAKEOFF_SCREEN: unknown screen id {id:?}, starting on home");
                    }
                    scene
                })
            })
            .expect("open window");
            cx.activate(true);

            // Interactive mode: closing the window quits. Scenario mode closes it on purpose.
            if phase_ms.is_none() {
                cx.on_window_closed(|cx, _| {
                    if cx.windows().is_empty() {
                        cx.quit();
                    }
                })
                .detach();
            }
        });
}
