//! Scene v3 ("Plansza", REDESIGN.md): tokens, fonts and the primitives the v3 screens share.
//!
//! GPUI gaps worked around here (REDESIGN.md "Parity rules for GPUI"):
//! - **Letter-spacing:** GPUI has none. `spaced()` shapes the whole string once (so kerning stays,
//!   like CSS), then paints every glyph itself, moved right by `track` em per preceding character,
//!   and sizes the box like CSS does (the tracking after the last letter included). `caps()`,
//!   `tight()` and the tracked Plex text all go through it. A spaced label is painted, not laid
//!   out as text, so it never wraps; hover colours come from a second copy (`caps_hover()`).
//! - **Outline with offset:** `ring()`, a 2 px border on an overlay 5 px outside the tile.
//! - **Pixel corners:** `play_button()` draws two overlapping rects.
//! - **Nearest-neighbour avatars:** `pixel_avatar()` decodes the 64×64 PNG once, keeps its 8×8
//!   cells and paints them as whole-pixel quads.
use std::{
    cell::RefCell,
    collections::HashMap,
    io::Cursor,
    rc::Rc,
    sync::{Arc, Mutex, OnceLock},
};

use gpui::{
    App, Div, FontId, FontWeight, GlyphId, Hsla, ImageSource, ObjectFit, ParentElement,
    SharedString, Styled, TextRun, WindowTextSystem, canvas, div, fill, font, img,
    linear_color_stop, linear_gradient, point, prelude::*, px, size,
};

use crate::{
    assets::{avatar_bytes, embedded},
    theme::c,
};

pub const ARCHIVO: &str = "Archivo SRL";
pub const PLEX: &str = "IBM Plex Mono";
pub use crate::theme::INTER;

// ---------------------------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------------------------

pub fn bg3() -> Hsla {
    c(0x0A0810)
}
pub fn bg3a(a: f32) -> Hsla {
    Hsla { a, ..bg3() }
}
pub fn text3() -> Hsla {
    c(0xF1EEF8)
}
pub fn text3a(a: f32) -> Hsla {
    Hsla { a, ..text3() }
}
pub fn muted3() -> Hsla {
    text3a(0.64)
}
pub fn faint3() -> Hsla {
    text3a(0.40)
}
pub fn line3() -> Hsla {
    text3a(0.09)
}
pub fn accent3a(a: f32) -> Hsla {
    Hsla { a, ..c(0x8B5CF6) }
}
pub fn accent2() -> Hsla {
    c(0xB79CFF)
}
pub fn ok3() -> Hsla {
    c(0x4ADE80)
}
pub fn gold() -> Hsla {
    c(0xD8AF1F)
}
pub fn ahead3() -> Hsla {
    c(0x29CC54)
}
pub fn behind3() -> Hsla {
    c(0xE0493B)
}
/// Text on the light Play button.
pub fn ink() -> Hsla {
    c(0x120D1C)
}
pub fn tile_bg() -> Hsla {
    c(0x1A1424)
}

/// MCSR Ranked tier colours.
pub fn tier_color(tier: &str) -> Hsla {
    c(match tier {
        "coal" => 0xAAAAAA,
        "iron" => 0xFFFFFF,
        "gold" => 0xEAB308,
        "emerald" => 0x21A83B,
        "diamond" => 0x2CE0D8,
        _ => 0x7C5CBF, // netherite
    })
}

/// `modes[].status` colour: ready → ok, update → gold, missing → accent-2.
pub fn status_color(status: &str) -> Hsla {
    match status {
        "ready" => ok3(),
        "update" => gold(),
        _ => accent2(),
    }
}

// ---------------------------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------------------------

/// A one-line text box in Archivo SRL with line height 1 (the mockups use `/1` everywhere).
pub fn arch(size_px: f32, weight: FontWeight) -> Div {
    div()
        .font_family(ARCHIVO)
        .font_weight(weight)
        .text_size(px(size_px))
        .line_height(px(size_px))
        .whitespace_nowrap()
}

/// A text box in IBM Plex Mono with line height 1.
pub fn plex(size_px: f32, weight: FontWeight) -> Div {
    div()
        .font_family(PLEX)
        .font_weight(weight)
        .text_size(px(size_px))
        .line_height(px(size_px))
        .whitespace_nowrap()
}

/// Inter at `size` with a CSS-style unitless line height.
pub fn inter(size_px: f32, weight: FontWeight, line: f32) -> Div {
    div()
        .font_family(INTER)
        .font_weight(weight)
        .text_size(px(size_px))
        .line_height(px(size_px * line))
}

/// One shaped label: where every glyph goes, with the tracking already added.
struct Shaped {
    width: f32,
    ascent: f32,
    descent: f32,
    glyphs: Vec<(FontId, GlyphId, f32, bool)>,
}

type ShapeKey = (String, &'static str, u32, u32, u32);

thread_local! {
    /// A line shaper on the app's text system (font ids are shared with every window).
    static SHAPER: RefCell<Option<(WindowTextSystem, Arc<gpui::TextSystem>)>> =
        const { RefCell::new(None) };
    /// Every label is shaped once and reused on later frames.
    static SHAPED: RefCell<HashMap<ShapeKey, Rc<Shaped>>> = RefCell::new(HashMap::new());
}

/// Call once, after the fonts are added.
pub fn init_text(cx: &App) {
    let ts = cx.text_system().clone();
    SHAPER.with(|s| *s.borrow_mut() = Some((WindowTextSystem::new(ts.clone()), ts)));
}

fn shape(
    s: &str,
    family: &'static str,
    weight: FontWeight,
    size_px: f32,
    track: f32,
) -> Rc<Shaped> {
    let key = (
        s.to_string(),
        family,
        weight.0.to_bits(),
        size_px.to_bits(),
        track.to_bits(),
    );
    if let Some(hit) = SHAPED.with(|c| c.borrow().get(&key).cloned()) {
        return hit;
    }
    let (layout, ascent, descent) = SHAPER.with(|sh| {
        let sh = sh.borrow();
        let (shaper, ts) = sh.as_ref().expect("v3::init_text not called");
        let f = gpui::Font {
            weight,
            // Characters the family lacks (▲ ▼ in Plex Mono) come from what the web build's
            // generic family resolves to on Linux (`monospace` / `sans-serif` → DejaVu), when
            // it is installed.
            fallbacks: Some(gpui::FontFallbacks::from_fonts(vec![
                if family == PLEX { "DejaVu Sans Mono" } else { "DejaVu Sans" }.into(),
            ])),
            ..font(family)
        };
        // The baseline comes from the label's own font, not from a fallback glyph (CSS strut).
        let id = ts.resolve_font(&f);
        let layout = shaper.layout_line(
            s,
            px(size_px),
            &[TextRun {
                len: s.len(),
                font: f,
                color: gpui::black(),
                ..Default::default()
            }],
            None,
        );
        (
            layout,
            f32::from(ts.ascent(id, px(size_px))),
            f32::from(ts.descent(id, px(size_px))).abs(),
        )
    });
    // CSS letter-spacing: `track` em after every character (grapheme clusters are single chars
    // in this UI), so a glyph moves right by the tracking of all characters before it.
    let t = track * size_px;
    let mut char_ix = vec![0usize; s.len() + 1];
    let mut n = 0;
    for (b, _) in s.char_indices() {
        char_ix[b] = n;
        n += 1;
    }
    let char_ix = &char_ix;
    let glyphs = layout
        .runs
        .iter()
        .flat_map(|run| {
            run.glyphs.iter().map(move |g| {
                (
                    run.font_id,
                    g.id,
                    f32::from(g.position.x) + t * char_ix[g.index] as f32,
                    g.is_emoji,
                )
            })
        })
        .collect();
    let shaped = Rc::new(Shaped {
        width: f32::from(layout.width) + t * n as f32,
        ascent,
        descent,
        glyphs,
    });
    SHAPED.with(|c| c.borrow_mut().insert(key, shaped.clone()));
    shaped
}

/// One line of `family` text with CSS letter-spacing `track` em (negative allowed) in a box
/// `line` × size tall, as wide as CSS makes it.
pub fn spaced(
    s: &str,
    family: &'static str,
    weight: FontWeight,
    size_px: f32,
    track: f32,
    line: f32,
    color: Hsla,
) -> Div {
    let sh = shape(s, family, weight, size_px, track);
    let line_h = size_px * line;
    let w = sh.width;
    div().flex_none().w(px(w)).h(px(line_h)).child(
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                // GPUI's own text baseline: half the leading above the ascent.
                let base = bounds.origin.y + px((line_h - sh.ascent - sh.descent) / 2. + sh.ascent);
                for &(font_id, glyph, x, emoji) in &sh.glyphs {
                    let at = point(bounds.origin.x + px(x), base);
                    let _ = if emoji {
                        window.paint_emoji(at, font_id, glyph, px(size_px))
                    } else {
                        window.paint_glyph(at, font_id, glyph, px(size_px), color)
                    };
                }
            },
        )
        .size_full(),
    )
}

/// `s` with a line break wherever the browser would wrap it at `max_w` px: greedy, at spaces,
/// measured on the shaped line (kerning included, no trailing space). GPUI's own wrapper counts
/// the space after the last word and also breaks before "·", so its lines come out shorter than
/// the web build's. Wrapped once per string and reused.
pub fn wrap(s: &str, family: &'static str, weight: FontWeight, size_px: f32, max_w: f32) -> SharedString {
    thread_local! {
        static WRAPPED: RefCell<HashMap<ShapeKey, SharedString>> = RefCell::new(HashMap::new());
    }
    let key = (s.to_string(), family, weight.0.to_bits(), size_px.to_bits(), max_w.to_bits());
    if let Some(hit) = WRAPPED.with(|c| c.borrow().get(&key).cloned()) {
        return hit;
    }
    let measure = |line: &str| -> f32 {
        SHAPER.with(|sh| {
            let sh = sh.borrow();
            let (shaper, _) = sh.as_ref().expect("v3::init_text not called");
            let f = gpui::Font {
                weight,
                ..font(family)
            };
            f32::from(
                shaper
                    .layout_line(
                        line,
                        px(size_px),
                        &[TextRun {
                            len: line.len(),
                            font: f,
                            color: gpui::black(),
                            ..Default::default()
                        }],
                        None,
                    )
                    .width,
            )
        })
    };
    let mut out = String::with_capacity(s.len() + 8);
    for (pi, para) in s.split('\n').enumerate() {
        if pi > 0 {
            out.push('\n');
        }
        let mut line = String::new();
        for word in para.split(' ') {
            if line.is_empty() {
                line.push_str(word);
                continue;
            }
            let cand = format!("{line} {word}");
            if measure(&cand) <= max_w + 0.01 {
                line = cand;
            } else {
                out.push_str(&line);
                out.push('\n');
                line = word.to_string();
            }
        }
        out.push_str(&line);
    }
    let out = SharedString::from(out);
    WRAPPED.with(|c| c.borrow_mut().insert(key, out.clone()));
    out
}

/// An upper-case Archivo caps label (`size` px, line height 1, tracking `track` em).
pub fn caps(s: &str, size_px: f32, weight: FontWeight, track: f32, color: Hsla) -> Div {
    spaced(
        &s.to_uppercase(),
        ARCHIVO,
        weight,
        size_px,
        track,
        1.,
        color,
    )
}

/// `caps()` that turns `hover` while the pointer is over the group `group`.
pub fn caps_hover(
    s: &str,
    size_px: f32,
    weight: FontWeight,
    track: f32,
    color: Hsla,
    hover: Hsla,
    group: &'static str,
) -> Div {
    div()
        .flex_none()
        .relative()
        .child(caps(s, size_px, weight, track, color))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .opacity(0.)
                .group_hover(group, |st| st.opacity(1.))
                .child(caps(s, size_px, weight, track, hover)),
        )
}

/// The common caps label: Archivo SRL SemiBold 11, tracking 0.2 em, `faint`.
pub fn cap_label(s: &str) -> Div {
    caps(s, 11., FontWeight::SEMIBOLD, 0.2, faint3())
}

/// Display text with negative tracking (the big titles): Archivo SRL at `size`, `track` em,
/// line height `line`.
pub fn tight(s: &str, size_px: f32, weight: FontWeight, track: f32, line: f32, color: Hsla) -> Div {
    spaced(s, ARCHIVO, weight, size_px, track, line, color)
}

// ---------------------------------------------------------------------------------------------
// Art, overlays, outline
// ---------------------------------------------------------------------------------------------

/// An embedded image filling its (relative) parent, cover-fit and centred.
pub fn art_fill(path: SharedString) -> gpui::Img {
    img(ImageSource::from(embedded(&path)))
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .object_fit(ObjectFit::Cover)
}

/// A flat overlay rect filling the parent.
pub fn overlay(color: Hsla) -> Div {
    div().absolute().top_0().left_0().size_full().bg(color)
}

/// A bottom-up `bg` gradient: `bottom` alpha at the bottom edge to `top` alpha at `stop` (0..1 of
/// the height), constant above it.
pub fn shade_up(bottom: f32, top: f32, stop: f32) -> Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(linear_gradient(
            0.,
            linear_color_stop(bg3a(bottom), 0.),
            linear_color_stop(bg3a(top), stop),
        ))
}

/// A left-to-right `bg` gradient over `[x0, x1]` (fractions of the parent's width).
pub fn shade_right(x0: f32, x1: f32, from: f32, to: f32) -> Div {
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(gpui::relative(x0))
        .w(gpui::relative(x1 - x0))
        .bg(linear_gradient(
            90.,
            linear_color_stop(bg3a(from), 0.),
            linear_color_stop(bg3a(to), 1.),
        ))
}

/// CSS `outline: 2px solid text; outline-offset: 3px` as an overlay.
pub fn ring() -> Div {
    div()
        .absolute()
        .top(px(-5.))
        .left(px(-5.))
        .right(px(-5.))
        .bottom(px(-5.))
        .border_2()
        .border_color(text3())
}

/// The 1 px `faint` outline at offset 3 that an unselected tile shows while the pointer is over
/// the group `group`.
pub fn hover_ring(group: &'static str) -> Div {
    div()
        .absolute()
        .top(px(-4.))
        .left(px(-4.))
        .right(px(-4.))
        .bottom(px(-4.))
        .border_1()
        .border_color(gpui::transparent_black())
        .group_hover(group, |s| s.border_color(faint3()))
}

// ---------------------------------------------------------------------------------------------
// Play button
// ---------------------------------------------------------------------------------------------

/// The light Play/Download button with 4 px pixel corners (two overlapping rects): the label in
/// Archivo SRL ExtraBold, tracking 0.2 em, and the hint under it in Plex Mono 10.
pub fn play_button(
    id: impl Into<gpui::ElementId>,
    label: &str,
    hint: SharedString,
    w: f32,
    h: f32,
    label_size: f32,
) -> gpui::Stateful<Div> {
    div()
        .id(id.into())
        .relative()
        .flex_none()
        .w(px(w))
        .h(px(h))
        .cursor_pointer()
        .group("v3-play")
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(4.))
                .right(px(4.))
                .bg(text3())
                .group_hover("v3-play", |s| s.bg(gpui::white())),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(4.))
                .bottom(px(4.))
                .bg(text3())
                .group_hover("v3-play", |s| s.bg(gpui::white())),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                // `margin-right: -0.2em` centres the tracked word.
                .child(
                    caps(label, label_size, FontWeight::EXTRA_BOLD, 0.2, ink())
                        .mr(px(-0.2 * label_size)),
                )
                .child(
                    spaced(&hint, PLEX, FontWeight::MEDIUM, 10., 0.04, 1., Hsla { a: 0.55, ..ink() })
                        .mt(px(7.)),
                ),
        )
}

/// A ghost caps action (Archivo SRL SemiBold 12, tracking 0.16 em, `muted`).
pub fn ghost(id: impl Into<gpui::ElementId>, label: &str) -> gpui::Stateful<Div> {
    div()
        .id(id.into())
        .flex_none()
        .group("v3-ghost")
        .cursor_pointer()
        .child(caps_hover(
            label,
            12.,
            FontWeight::SEMIBOLD,
            0.16,
            muted3(),
            text3(),
            "v3-ghost",
        ))
}

/// An `h` px progress bar for the fake installs (`accent-2` on `line`).
pub fn thin_progress(f: f32, h: f32) -> Div {
    div().h(px(h)).w_full().bg(line3()).child(
        div()
            .h_full()
            .w(gpui::relative(f.clamp(0., 1.)))
            .bg(accent2()),
    )
}

// ---------------------------------------------------------------------------------------------
// Pixel avatars
// ---------------------------------------------------------------------------------------------

type Cells = Arc<[Hsla; 64]>;

/// The 8×8 cells of avatar `index` (64×64 PNG, 8 px per cell), decoded on first use.
fn avatar_cells(index: usize) -> Option<Cells> {
    static CACHE: OnceLock<Mutex<HashMap<usize, Option<Cells>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock().ok()?;
    map.entry(index)
        .or_insert_with(|| decode_cells(avatar_bytes(index)?))
        .clone()
}

fn decode_cells(bytes: &[u8]) -> Option<Cells> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => return None,
    };
    let (w, h) = (info.width as usize, info.height as usize);
    let mut cells = [Hsla::default(); 64];
    for r in 0..8 {
        for col in 0..8 {
            let x = (col * w + w / 2) / 8;
            let y = (r * h + h / 2) / 8;
            let i = (y * w + x) * channels;
            let p = &buf[i..i + channels];
            let (rr, gg, bb, aa) = match channels {
                4 => (p[0], p[1], p[2], p[3]),
                3 => (p[0], p[1], p[2], 255),
                2 => (p[0], p[0], p[0], p[1]),
                _ => (p[0], p[0], p[0], 255),
            };
            cells[r * 8 + col] = gpui::Rgba {
                r: rr as f32 / 255.,
                g: gg as f32 / 255.,
                b: bb as f32 / 255.,
                a: aa as f32 / 255.,
            }
            .into();
        }
    }
    Some(Arc::new(cells))
}

/// Avatar index from a data path like `avatars/042.png`.
pub fn avatar_index(path: &str) -> usize {
    path.trim_start_matches("avatars/")
        .trim_end_matches(".png")
        .parse()
        .unwrap_or(0)
}

/// A pixel-art avatar drawn nearest-neighbour: 64 quads with edges on whole pixels.
pub fn pixel_avatar(path: &str, size_px: f32) -> Div {
    let cells = avatar_cells(avatar_index(path));
    div().flex_none().size(px(size_px)).child(
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let Some(cells) = &cells else { return };
                let ox = bounds.origin.x.as_f32().round();
                let oy = bounds.origin.y.as_f32().round();
                let edge = |i: usize| (i as f32 * size_px / 8.).round();
                for r in 0..8 {
                    for col in 0..8 {
                        let (x0, x1, y0, y1) = (edge(col), edge(col + 1), edge(r), edge(r + 1));
                        window.paint_quad(fill(
                            gpui::Bounds {
                                origin: point(px(ox + x0), px(oy + y0)),
                                size: size(px(x1 - x0), px(y1 - y0)),
                            },
                            cells[r * 8 + col],
                        ));
                    }
                }
            },
        )
        .size_full(),
    )
}

/// A signed number with a real minus sign ("-18" → "−18"), like the web build.
pub fn minus(s: &str) -> String {
    match s.strip_prefix('-') {
        Some(rest) => format!("−{rest}"),
        None => s.to_string(),
    }
}

/// An Elo change as an arrow and the size of the change ("+24" → "▲ 24", "-8" → "▼ 8").
pub fn arrow(s: &str) -> String {
    let up = !s.starts_with('-');
    let n = s.trim_start_matches(['+', '-']);
    format!("{} {n}", if up { '▲' } else { '▼' })
}
