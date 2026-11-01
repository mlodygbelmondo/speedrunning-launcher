//! Scene v3 ("Plansza", bakeoff/REDESIGN.md): tokens, fonts and the primitives every screen shares.
//! Ported from the prototype (`bakeoff/gpui/src/v3.rs`) with its GPUI workarounds unchanged, so the
//! app looks exactly like the prototype's screenshots.
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
    App, Div, FontId, FontWeight, GlyphId, Hsla, ObjectFit, ParentElement, SharedString, Styled,
    TextRun, WindowTextSystem, canvas, div, fill, font, img, linear_color_stop, linear_gradient,
    point, prelude::*, px, size,
};

use crate::{
    assets::{bytes, embedded},
    backend::ModeState,
};

pub const ARCHIVO: &str = "Archivo SRL";
pub const PLEX: &str = "IBM Plex Mono";
pub const INTER: &str = "Inter";

pub fn c(hex: u32) -> Hsla {
    gpui::rgb(hex).into()
}

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
    text3a(0.50)
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

/// A mode's status as the status square, caps label and tile badge show it (REDESIGN.md "Status
/// row"), one per `ModeState` variant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Missing,
    Installing,
    Ready,
    Update,
    Running,
    Failed,
}

impl Status {
    pub fn of(state: &ModeState) -> Status {
        match state {
            ModeState::NotInstalled => Status::Missing,
            ModeState::Installing(_) => Status::Installing,
            ModeState::Ready => Status::Ready,
            ModeState::UpdateAvailable => Status::Update,
            ModeState::Running => Status::Running,
            ModeState::Failed(_) => Status::Failed,
        }
    }

    /// ready → ok, update → gold, missing → accent-2 (REDESIGN.md); the states the prototype
    /// didn't have follow the same logic: running is ok, installing is still "on its way" like
    /// missing, failed is `behind`.
    pub fn color(self) -> Hsla {
        match self {
            Status::Ready | Status::Running => ok3(),
            Status::Update => gold(),
            Status::Missing | Status::Installing => accent2(),
            Status::Failed => behind3(),
        }
    }

    /// The copy key of the status label.
    pub fn key(self) -> &'static str {
        match self {
            Status::Missing => "v3.status.missing",
            Status::Installing => "v3.status.installing",
            Status::Ready => "v3.status.ready",
            Status::Update => "v3.status.update",
            Status::Running => "v3.status.running",
            Status::Failed => "v3.status.failed",
        }
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
                if family == PLEX {
                    "DejaVu Sans Mono"
                } else {
                    "DejaVu Sans"
                }
                .into(),
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
pub fn wrap(
    s: &str,
    family: &'static str,
    weight: FontWeight,
    size_px: f32,
    max_w: f32,
) -> SharedString {
    thread_local! {
        static WRAPPED: RefCell<HashMap<ShapeKey, SharedString>> = RefCell::new(HashMap::new());
    }
    let key = (
        s.to_string(),
        family,
        weight.0.to_bits(),
        size_px.to_bits(),
        max_w.to_bits(),
    );
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

/// `wrap` with its lines evened out: as many lines as at `max_w`, but at the narrowest width that
/// still gives that many, so a two-line blurb doesn't end on one lone word.
pub fn wrap_even(
    s: &str,
    family: &'static str,
    weight: FontWeight,
    size_px: f32,
    max_w: f32,
) -> SharedString {
    thread_local! {
        static EVEN: RefCell<HashMap<ShapeKey, SharedString>> = RefCell::new(HashMap::new());
    }
    let key = (
        s.to_string(),
        family,
        weight.0.to_bits(),
        size_px.to_bits(),
        max_w.to_bits(),
    );
    if let Some(hit) = EVEN.with(|c| c.borrow().get(&key).cloned()) {
        return hit;
    }
    let lines = |w: f32| wrap(s, family, weight, size_px, w).split('\n').count();
    let n = lines(max_w);
    // Whole pixels, so the few widths tried stay in `wrap`'s cache. A pane is never wider than
    // a few thousand px; the bound keeps the halving finite on any input.
    let (mut lo, mut hi) = ((max_w / n as f32).floor(), max_w.ceil());
    let mut steps = 0;
    while n > 1 && hi - lo > 1. && steps < 16 {
        let mid = ((lo + hi) / 2.).floor();
        if lines(mid) > n {
            lo = mid;
        } else {
            hi = mid;
        }
        steps += 1;
    }
    let out = wrap(s, family, weight, size_px, hi.min(max_w));
    EVEN.with(|c| c.borrow_mut().insert(key, out.clone()));
    out
}

/// Where [`ellipsize`] cuts a line that doesn't fit.
#[derive(Clone, Copy)]
pub enum Cut {
    /// In the middle, keeping more of the end (a path, whose file name is at the end).
    Middle,
    /// At the end (a sentence, which reads from its start).
    End,
}

/// `s` on one line of at most `max_w` px: as it is when it fits, else cut with "…" where `cut`
/// says.
pub fn ellipsize(
    s: &str,
    family: &'static str,
    weight: FontWeight,
    size_px: f32,
    max_w: f32,
    at: Cut,
) -> SharedString {
    let fits = |t: &str| shape(t, family, weight, size_px, 0.).width <= max_w + 0.01;
    if fits(s) {
        return s.to_string().into();
    }
    let chars: Vec<char> = s.chars().collect();
    let cut = |n: usize| -> String {
        let head = match at {
            Cut::Middle => n * 2 / 5,
            Cut::End => n,
        };
        let tail = n - head;
        chars[..head]
            .iter()
            .chain(std::iter::once(&'…'))
            .chain(&chars[chars.len() - tail..])
            .collect()
    };
    // The most characters that still fit, by bisection (the width grows with `n`).
    let (mut lo, mut hi) = (0, chars.len() - 1);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if fits(&cut(mid)) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    cut(lo).into()
}

/// `wrap()` that may also break after a `/` or `\`, so a long path or URL never runs past `max_w`.
pub fn wrap_paths(
    s: &str,
    family: &'static str,
    weight: FontWeight,
    size_px: f32,
    max_w: f32,
) -> SharedString {
    // A hair space after each separator gives the wrapper a place to break; it's taken out again
    // (a break leaves it at the end of the line, where it's dropped too).
    const MARK: char = '\u{200A}';
    let marked: String = s
        .chars()
        .flat_map(|c| match c {
            '/' | '\\' => vec![c, ' ', MARK],
            _ => vec![c],
        })
        .collect();
    wrap(&marked, family, weight, size_px, max_w)
        .replace(&format!(" {MARK}"), "")
        .replace(&format!("\n{MARK}"), "\n")
        .into()
}

/// `s` in upper case, except versions ("Update to v1.2.2" → "UPDATE TO v1.2.2", also "(v1.2.2)"
/// and „v1.2.2”): a version is written as its author writes it. A version is a "v" right before a
/// digit, at the start of a word or after punctuation.
pub fn upper(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, &ch) in chars.iter().enumerate() {
        let starts_word = i == 0 || !chars[i - 1].is_alphanumeric();
        let before_digit = chars.get(i + 1).is_some_and(|d| d.is_ascii_digit());
        if ch == 'v' && starts_word && before_digit {
            out.push(ch);
        } else {
            out.extend(ch.to_uppercase());
        }
    }
    out
}

/// An upper-case Archivo caps label (`size` px, line height 1, tracking `track` em).
pub fn caps(s: &str, size_px: f32, weight: FontWeight, track: f32, color: Hsla) -> Div {
    spaced(&upper(s), ARCHIVO, weight, size_px, track, 1., color)
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

// ---------------------------------------------------------------------------------------------
// Labels: the type system
// ---------------------------------------------------------------------------------------------
//
// - Tracked caps (`caps`) only in the top nav, in one eyebrow per hero (`eyebrow`: Home, the game
//   screen, the full-window prompts) and on the primary button.
// - Everything else is sentence case: a section head (`section_head`), a small label over a value
//   or a column (`label`), a status word (`status_text`, `status_dot`).
// - Prose is Inter. IBM Plex Mono (`plex`) only for times, numbers, versions, sizes and paths.

/// The eyebrow over a hero's title: tracked caps, Archivo SRL SemiBold 12, 0.2 em.
pub fn eyebrow(s: &str, color: Hsla) -> Div {
    caps(s, 12., FontWeight::SEMIBOLD, 0.2, color)
}

/// The line over a page title that isn't a hero (Modes, Mods, Learn): what the eyebrow says, in
/// sentence case, Inter SemiBold 13.
pub fn kicker(s: &str, color: Hsla) -> Div {
    inter(13., FontWeight::SEMIBOLD, 1.)
        .flex_none()
        .whitespace_nowrap()
        .text_color(color)
        .child(SharedString::from(s.to_string()))
}

/// A section head ("Microsoft account", "Layers"): Inter SemiBold 13, `muted`, sentence case.
pub fn section_head(s: &str) -> Div {
    inter(13., FontWeight::SEMIBOLD, 1.)
        .flex_none()
        .whitespace_nowrap()
        .text_color(muted3())
        .child(SharedString::from(s.to_string()))
}

/// A small label over a value or a table column ("Minecraft", "Memory"): Inter Medium 12, `faint`.
pub fn label(s: &str) -> Div {
    inter(12., FontWeight::MEDIUM, 1.)
        .flex_none()
        .whitespace_nowrap()
        .text_color(faint3())
        .child(SharedString::from(s.to_string()))
}

/// A fact: its `label`, then the value at 15 px: Plex Mono for data (`mono`: a version, a size, a
/// count), Inter for words (a name, "Microsoft").
pub fn fact(l: &str, value_text: SharedString, mono: bool) -> Div {
    let value = if mono {
        plex(15., FontWeight::MEDIUM)
    } else {
        inter(15., FontWeight::MEDIUM, 1.)
    };
    div()
        .flex()
        .flex_col()
        .child(label(l).mb(px(9.)))
        .child(value.text_color(text3()).child(value_text))
}

/// A status word in its colour ("expired", "update"): Inter SemiBold 12.
pub fn status_text(s: &str, color: Hsla) -> Div {
    inter(12., FontWeight::SEMIBOLD, 1.)
        .flex_none()
        .whitespace_nowrap()
        .text_color(color)
        .child(SharedString::from(s.to_string()))
}

/// `status_text` after the 6 px status square.
pub fn status_dot(s: &str, color: Hsla) -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(7.))
        .child(div().flex_none().size(px(6.)).bg(color))
        .child(status_text(s, color))
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
    img(embedded(&path))
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

/// How a Play button reacts.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Clickable, white on hover.
    Live,
    /// Working (a mode installing, a game stopping): full colour, but no hover and no pointer.
    /// The label and hint say what it's doing.
    Busy,
    /// Not possible yet (an invalid name): the fill dimmed into the background.
    Off,
}

/// The light Play/Download button with 4 px pixel corners (two overlapping rects): the label in
/// Archivo SRL ExtraBold, tracking 0.2 em, and the hint under it in Plex Mono 10. The hint is for
/// data (a size, a version, progress); an empty one leaves the label alone, centred.
pub fn play_button(
    id: impl Into<gpui::ElementId>,
    label: &str,
    hint: SharedString,
    w: f32,
    h: f32,
    label_size: f32,
    press: Press,
) -> gpui::Stateful<Div> {
    // `Off` is an opaque colour (text @ 45 % over bg), not `opacity()`: GPUI applies opacity per
    // primitive, so the two overlapping rects would show their overlap brighter.
    let fill = if press == Press::Off {
        c(0x726F78)
    } else {
        text3()
    };
    let rect = move |d: Div| {
        let d = d.absolute().bg(fill);
        if press == Press::Live {
            d.group_hover("v3-play", |s| s.bg(gpui::white()))
        } else {
            d
        }
    };
    div()
        .id(id.into())
        .relative()
        .flex_none()
        .w(px(w))
        .h(px(h))
        .when(press == Press::Live, |d| d.cursor_pointer())
        .group("v3-play")
        .child(rect(div().top_0().bottom_0().left(px(4.)).right(px(4.))))
        .child(rect(div().left_0().right_0().top(px(4.)).bottom(px(4.))))
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
                .when(!hint.is_empty(), |d| {
                    d.child(
                        spaced(
                            &hint,
                            PLEX,
                            FontWeight::MEDIUM,
                            10.,
                            0.04,
                            1.,
                            Hsla { a: 0.55, ..ink() },
                        )
                        .mt(px(7.)),
                    )
                }),
        )
}

// ---------------------------------------------------------------------------------------------
// Buttons: primary, secondary, tertiary
// ---------------------------------------------------------------------------------------------
//
// One primary per view: `play_button` (Play, Stop, Import, a dialog's confirmation) or its small
// form `primary` (a section's or a dialog's main action). `secondary` is outlined, for actions
// that matter but aren't the one thing to do (Mods, Import…, Install on a row). `tertiary` is
// text with a hairline under it: Cancel, Back, Sign out, a link.

/// The height of `primary` and `secondary`.
pub const BUTTON_H: f32 = 36.;

/// The small primary button: the Play fill with its 4 px pixel corners, as wide as its label
/// (caps, Archivo SRL ExtraBold 12).
pub fn primary(id: impl Into<gpui::ElementId>, label: &str) -> gpui::Stateful<Div> {
    let rect = |d: Div| {
        d.absolute()
            .bg(text3())
            .group_hover("v3-primary", |s| s.bg(gpui::white()))
    };
    div()
        .id(id.into())
        .relative()
        .flex_none()
        .h(px(BUTTON_H))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .group("v3-primary")
        .child(rect(div().top_0().bottom_0().left(px(4.)).right(px(4.))))
        .child(rect(div().left_0().right_0().top(px(4.)).bottom(px(4.))))
        // The padding is on the label, not the button: the corner rects are placed inside the
        // button's padding, so they would leave it uncovered.
        .child(
            div()
                .px(px(20.))
                .child(caps(label, 12., FontWeight::EXTRA_BOLD, 0.16, ink()).mr(px(-0.16 * 12.))),
        )
}

/// The outlined secondary button: a 1 px light edge, the label in Inter SemiBold 13.
pub fn secondary(id: impl Into<gpui::ElementId>, label: &str) -> gpui::Stateful<Div> {
    div()
        .id(id.into())
        .flex_none()
        .h(px(BUTTON_H))
        .px(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(text3a(0.3))
        .cursor_pointer()
        .hover(|s| s.border_color(text3a(0.7)).bg(text3a(0.06)))
        .child(
            inter(13., FontWeight::SEMIBOLD, 1.)
                .whitespace_nowrap()
                .text_color(text3())
                .child(SharedString::from(label.to_string())),
        )
}

/// `secondary` in the losing red, for the one answer that can't be undone ("Delete 12 reset
/// worlds"): the edge and label in `behind3`, filled faintly on hover.
pub fn danger(id: impl Into<gpui::ElementId>, label: &str) -> gpui::Stateful<Div> {
    div()
        .id(id.into())
        .flex_none()
        .h(px(BUTTON_H))
        .px(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(Hsla {
            a: 0.6,
            ..behind3()
        })
        .cursor_pointer()
        .hover(|s| {
            s.border_color(behind3()).bg(Hsla {
                a: 0.12,
                ..behind3()
            })
        })
        .child(
            inter(13., FontWeight::SEMIBOLD, 1.)
                .whitespace_nowrap()
                .text_color(behind3())
                .child(SharedString::from(label.to_string())),
        )
}

/// `secondary` that has to wait (a change while the game runs): dim, no clicks.
pub fn secondary_off(label: &str) -> Div {
    div()
        .flex_none()
        .h(px(BUTTON_H))
        .px(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(text3a(0.1))
        .child(
            inter(13., FontWeight::SEMIBOLD, 1.)
                .whitespace_nowrap()
                .text_color(text3a(0.3))
                .child(SharedString::from(label.to_string())),
        )
}

/// A quiet link for a per-row removal ("Delete" on every world): `faint` with no hairline, so a
/// list of rows isn't a wall of underlines; red and underlined under the pointer.
pub fn tertiary_remove(id: impl Into<gpui::ElementId>, label: &str) -> gpui::Stateful<Div> {
    div()
        .id(id.into())
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .group("v3-remove")
        .cursor_pointer()
        .child(
            inter(13., FontWeight::MEDIUM, 1.)
                .whitespace_nowrap()
                .pb(px(3.))
                .border_b_1()
                .border_color(gpui::transparent_black())
                .text_color(faint3())
                .group_hover("v3-remove", |s| {
                    s.text_color(behind3()).border_color(Hsla {
                        a: 0.6,
                        ..behind3()
                    })
                })
                .child(SharedString::from(label.to_string())),
        )
}

/// The tertiary button: Inter Medium 13 in `color` over a hairline, white under the pointer.
pub fn tertiary_in(
    id: impl Into<gpui::ElementId>,
    label: &str,
    color: Hsla,
) -> gpui::Stateful<Div> {
    div()
        .id(id.into())
        .flex_none()
        // A row, so the label (and its hairline) is only as wide as the text, also where a
        // column stretches the button.
        .flex()
        .flex_row()
        .items_center()
        .group("v3-tertiary")
        .cursor_pointer()
        .child(
            inter(13., FontWeight::MEDIUM, 1.)
                .whitespace_nowrap()
                .pb(px(3.))
                .border_b_1()
                .border_color(Hsla { a: 0.35, ..color })
                .text_color(color)
                .group_hover("v3-tertiary", |s| {
                    s.text_color(text3()).border_color(text3a(0.6))
                })
                .child(SharedString::from(label.to_string())),
        )
}

/// `tertiary` that has to wait: dim, no hairline, no clicks.
pub fn tertiary_off(label: &str) -> Div {
    inter(13., FontWeight::MEDIUM, 1.)
        .flex_none()
        .whitespace_nowrap()
        .pb(px(4.))
        .text_color(text3a(0.26))
        .child(SharedString::from(label.to_string()))
}

/// The tertiary button in `muted` (Cancel, Back, Sign out).
pub fn tertiary(id: impl Into<gpui::ElementId>, label: &str) -> gpui::Stateful<Div> {
    tertiary_in(id, label, muted3())
}

/// A chip: the Mods page's mode tabs and the Tools page's "start with the game" toggles. On is the
/// accent fill over an accent underline; a toggle (`check`) also shows a tick when on, so on and
/// off read without comparing chips.
pub fn chip(
    id: impl Into<gpui::ElementId>,
    label: &str,
    on: bool,
    check: bool,
) -> gpui::Stateful<Div> {
    div()
        .id(id.into())
        .flex_none()
        .h(px(30.))
        .px(px(10.))
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(5.))
        .cursor_pointer()
        .bg(if on { accent3a(0.28) } else { text3a(0.06) })
        .border_b_2()
        .border_color(if on { accent2() } else { text3a(0.0) })
        .when(!on, |d| d.hover(|s| s.bg(text3a(0.1))))
        .when(check && on, |d| {
            d.child(spaced("✓", INTER, FontWeight::BOLD, 11., 0., 1., accent2()))
        })
        .child(
            inter(12., FontWeight::SEMIBOLD, 1.)
                .whitespace_nowrap()
                .text_color(if on { text3() } else { muted3() })
                .child(SharedString::from(label.to_string())),
        )
}

/// A chip that can't be picked (a mode whose rules ban the tool): struck through, no fill.
pub fn chip_off(label: &str) -> Div {
    div()
        .flex_none()
        .relative()
        .h(px(30.))
        .px(px(10.))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(line3())
        .child(
            inter(12., FontWeight::SEMIBOLD, 1.)
                .whitespace_nowrap()
                .text_color(text3a(0.26))
                .child(SharedString::from(label.to_string())),
        )
        .child(
            div()
                .absolute()
                .left(px(6.))
                .right(px(6.))
                .top(px(14.))
                .h(px(1.))
                .bg(text3a(0.3)),
        )
}

/// A disclosure line ("About this map"): a triangle that turns, then the label, `muted`.
pub fn disclosure(id: impl Into<gpui::ElementId>, label: &str, open: bool) -> gpui::Stateful<Div> {
    div()
        .id(id.into())
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(7.))
        .group("v3-disclosure")
        .cursor_pointer()
        .child(spaced(
            if open { "▾" } else { "▸" },
            INTER,
            FontWeight::MEDIUM,
            12.,
            0.,
            1.,
            muted3(),
        ))
        .child(
            inter(13., FontWeight::MEDIUM, 1.)
                .whitespace_nowrap()
                .text_color(muted3())
                .group_hover("v3-disclosure", |s| s.text_color(text3()))
                .child(SharedString::from(label.to_string())),
        )
}

/// A fade at the bottom of a scrolling list while rows are hidden below it (`absolute`, over the
/// list's last `h` px): the rows fade into the background instead of being cut by the edge.
pub fn fade_below(scroll: gpui::ScrollHandle, h: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let hidden = scroll.max_offset().y + scroll.offset().y;
            if hidden > px(1.) {
                window.paint_quad(fill(
                    bounds,
                    linear_gradient(
                        0.,
                        linear_color_stop(bg3a(1.0), 0.),
                        linear_color_stop(bg3a(0.0), 1.),
                    ),
                ));
            }
        },
    )
    .absolute()
    .left_0()
    .right_0()
    .bottom_0()
    .h(px(h))
}

/// The same at the top, while rows are scrolled away above (so a row never runs into the list's
/// header).
pub fn fade_above(scroll: gpui::ScrollHandle, h: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            if scroll.offset().y < px(-1.) {
                window.paint_quad(fill(
                    bounds,
                    linear_gradient(
                        180.,
                        linear_color_stop(bg3a(1.0), 0.),
                        linear_color_stop(bg3a(0.0), 1.),
                    ),
                ));
            }
        },
    )
    .absolute()
    .left_0()
    .right_0()
    .top_0()
    .h(px(h))
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

/// The 8×8 cells of demo avatar `id` (`demo/avatars/<id>.png`, 64×64, 8 px per cell), decoded on
/// first use.
fn avatar_cells(id: &str) -> Option<Cells> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Cells>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock().ok()?;
    map.entry(id.to_string())
        .or_insert_with(|| decode_cells(bytes(&format!("demo/avatars/{id}.png"))?))
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

/// A pixel-art demo avatar drawn nearest-neighbour: 64 quads with edges on whole pixels.
pub fn pixel_avatar(id: &str, size_px: f32) -> Div {
    match avatar_cells(id) {
        Some(cells) => pixel_grid(Arc::new(cells.to_vec()), 8, size_px),
        None => div().flex_none().size(px(size_px)),
    }
}

/// An account's face (`backend::account::Face`, 8x8 RGBA) drawn like `pixel_avatar`.
pub fn pixel_face(face: &crate::backend::account::Face, size_px: f32) -> Div {
    let cells: Vec<Hsla> = face
        .0
        .iter()
        .map(|&rgba| {
            gpui::Rgba {
                r: (rgba >> 24) as f32 / 255.,
                g: ((rgba >> 16) & 0xFF) as f32 / 255.,
                b: ((rgba >> 8) & 0xFF) as f32 / 255.,
                a: (rgba & 0xFF) as f32 / 255.,
            }
            .into()
        })
        .collect();
    pixel_grid(Arc::new(cells), 8, size_px)
}

/// A QR code (`srl_auth::QrMatrix`) with its 4-module quiet zone, dark on white, at the largest
/// whole number of pixels per module that fits in `max_px`, so every module is sharp and the
/// same size (a phone camera reads it from the screen). Returns the element and its side.
pub fn qr_code(m: &srl_auth::QrMatrix, max_px: f32) -> (Div, f32) {
    const QUIET: usize = 4;
    let n = m.width + 2 * QUIET;
    let module = (max_px / n as f32).floor().max(1.);
    let side = module * n as f32;
    let (dark, light) = (gpui::black(), gpui::white());
    let mut cells = vec![light; n * n];
    for y in 0..m.width {
        for x in 0..m.width {
            if m.dark[y * m.width + x] {
                cells[(y + QUIET) * n + x + QUIET] = dark;
            }
        }
    }
    (pixel_grid(Arc::new(cells), n, side), side)
}

/// `n` x `n` square cells, row by row, filling `size_px`, edges on whole pixels.
fn pixel_grid(cells: Arc<Vec<Hsla>>, n: usize, size_px: f32) -> Div {
    div().flex_none().size(px(size_px)).child(
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let ox = bounds.origin.x.as_f32().round();
                let oy = bounds.origin.y.as_f32().round();
                let edge = |i: usize| (i as f32 * size_px / n as f32).round();
                for r in 0..n {
                    for col in 0..n {
                        let (x0, x1, y0, y1) = (edge(col), edge(col + 1), edge(r), edge(r + 1));
                        window.paint_quad(fill(
                            gpui::Bounds {
                                origin: point(px(ox + x0), px(oy + y0)),
                                size: size(px(x1 - x0), px(y1 - y0)),
                            },
                            cells[r * n + col],
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

#[cfg(test)]
mod tests {
    use super::upper;

    #[test]
    fn upper_keeps_versions() {
        assert_eq!(upper("Update to v1.2.2"), "UPDATE TO v1.2.2");
        assert_eq!(upper("Reset to (v1.3)"), "RESET TO (v1.3)");
        assert_eq!(upper("Aktualizuj do „v2.0”"), "AKTUALIZUJ DO „v2.0”");
        // Not a version: a "v" inside a word, or not before a digit.
        assert_eq!(upper("dev2 vote"), "DEV2 VOTE");
        assert_eq!(upper("Minecraft 1.16.1"), "MINECRAFT 1.16.1");
    }
}
