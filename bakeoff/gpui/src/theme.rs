//! Colour tokens (SPEC.md + SCREENS.md), fonts and CSS-matching easing curves.
use std::time::Instant;

use gpui::{Hsla, rgb};

pub fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}
/// `token @ NN%`: the colour with its alpha replaced.
pub fn alpha(color: Hsla, a: f32) -> Hsla {
    Hsla { a, ..color }
}

/// The v3 window bg (REDESIGN.md); v2 was #0A090E. The web build changed it everywhere too.
pub fn bg() -> Hsla {
    c(0x0A0810)
}
pub fn surface() -> Hsla {
    c(0x15131D)
}
pub fn raised() -> Hsla {
    c(0x1B1327)
}
pub fn border() -> Hsla {
    c(0x26222F)
}
pub fn accent() -> Hsla {
    c(0x8B5CF6)
}
pub fn accent_hover() -> Hsla {
    c(0x9D74F8)
}
pub fn text() -> Hsla {
    c(0xECEAF2)
}
pub fn muted() -> Hsla {
    c(0x8A8796)
}
pub fn ahead() -> Hsla {
    c(0x4ADE80)
}
pub fn behind() -> Hsla {
    c(0xF87171)
}
pub fn pb() -> Hsla {
    c(0xC084FC)
}
pub fn warn() -> Hsla {
    c(0xFBBF24)
}
pub fn black() -> Hsla {
    c(0x000000)
}
pub fn white() -> Hsla {
    c(0xFFFFFF)
}

pub const INTER: &str = "Inter";
pub const MONO: &str = "JetBrains Mono";

// ---------------------------------------------------------------------------------------------
// Easing (CSS cubic-bezier, so timing matches the web scene exactly)
// ---------------------------------------------------------------------------------------------

fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let bez = |a: f32, b: f32, s: f32| {
        let u = 1.0 - s;
        3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
    };
    // Solve x(s) = t by bisection (monotonic for CSS curves), then evaluate y(s).
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    let mut s = t;
    for _ in 0..24 {
        s = 0.5 * (lo + hi);
        if bez(x1, x2, s) < t {
            lo = s;
        } else {
            hi = s;
        }
    }
    bez(y1, y2, s)
}
pub fn ease_in(t: f32) -> f32 {
    cubic_bezier(0.42, 0.0, 1.0, 1.0, t)
}
pub fn ease_out(t: f32) -> f32 {
    cubic_bezier(0.0, 0.0, 0.58, 1.0, t)
}
pub fn ease_in_out(t: f32) -> f32 {
    cubic_bezier(0.42, 0.0, 0.58, 1.0, t)
}

pub fn ms_since(start: Instant, now: Instant) -> f32 {
    now.saturating_duration_since(start).as_secs_f32() * 1000.0
}
