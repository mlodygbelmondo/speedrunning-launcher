//! Shared components from SCREENS.md ("Shared components"). Stateless pieces are free functions;
//! the ones with state or animation (toggle, segmented control, spinner) are `Scene` methods.
use std::{f32::consts::TAU, rc::Rc, time::Instant};

use gpui::{
    App, ClickEvent, Context, Div, ElementId, FontWeight, Hsla, IntoElement, ParentElement, Pixels,
    RenderOnce, SharedString, Stateful, Styled, Svg, Transformation, Window, canvas, div, fill,
    point, prelude::*, px, radians, relative, size, svg,
};

use crate::{app::Scene, assets::icon_path, theme::*};

/// Line height used for a font size (keeps text boxes predictable across screens).
pub fn lh(size: f32) -> Pixels {
    px(match size {
        s if s <= 11.0 => 14.0,
        s if s <= 12.0 => 16.0,
        s if s <= 13.0 => 18.0,
        s if s <= 15.0 => 20.0,
        s if s <= 17.0 => 22.0,
        s if s <= 20.0 => 26.0,
        s if s <= 22.0 => 28.0,
        s if s <= 26.0 => 32.0,
        s if s <= 28.0 => 36.0,
        s if s <= 30.0 => 38.0,
        s if s <= 32.0 => 40.0,
        s => (s * 1.25).round(),
    })
}

/// A text box in Inter at `size` px.
pub fn txt(size: f32) -> Div {
    div().text_size(px(size)).line_height(lh(size))
}

/// A text box in JetBrains Mono at `size` px.
pub fn mono(size: f32) -> Div {
    txt(size).font_family(MONO)
}

pub fn muted_txt(size: f32, s: impl Into<SharedString>) -> Div {
    txt(size).text_color(muted()).child(s.into())
}

pub fn icon(name: &str, size_px: f32, color: Hsla) -> Svg {
    svg()
        .path(icon_path(name))
        .flex_none()
        .size(px(size_px))
        .text_color(color)
}

/// 6×6 accent square, no rounding.
pub fn pixel_bullet() -> Div {
    div().flex_none().size(px(6.)).bg(accent())
}

pub fn card() -> Div {
    div()
        .bg(surface())
        .border_1()
        .border_color(border())
        .rounded(px(14.))
        .p(px(18.))
}

/// Card title: Inter SemiBold 15, 12 px above the content, optional pixel bullet.
pub fn card_title(s: impl Into<SharedString>, bullet: bool) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .mb(px(12.))
        .when(bullet, |d| d.child(pixel_bullet()))
        .child(
            txt(15.)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(text())
                .child(s.into()),
        )
}

pub fn hline() -> Div {
    div().flex_none().h(px(1.)).w_full().bg(border())
}

pub fn icon_tile(name: &str, tile: f32, icon_px: f32, radius: f32) -> Div {
    div()
        .flex_none()
        .size(px(tile))
        .rounded(px(radius))
        .bg(raised())
        .flex()
        .items_center()
        .justify_center()
        .child(icon(name, icon_px, accent()))
}

/// Lays `items` out in rows of `cols` equal columns (missing cells are empty spacers).
pub fn grid(cols: usize, gap: f32, items: Vec<gpui::AnyElement>) -> Div {
    let mut rows = Vec::new();
    let mut it = items.into_iter().peekable();
    while it.peek().is_some() {
        let mut row = div().flex().flex_row().gap(px(gap));
        for _ in 0..cols {
            let cell = div().flex_1().min_w(px(0.)).flex().flex_col();
            row = row.child(match it.next() {
                Some(el) => cell.child(el),
                None => cell,
            });
        }
        rows.push(row);
    }
    div().flex().flex_col().gap(px(gap)).children(rows)
}

// ---------------------------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    kind: Kind,
    small: bool,
    icon: Option<&'static str>,
    leading: Option<gpui::AnyElement>,
    trailing: Option<&'static str>,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, kind: Kind) -> Button {
    Button {
        id: id.into(),
        label: label.into(),
        kind,
        small: false,
        icon: None,
        leading: None,
        trailing: None,
        disabled: false,
        on_click: None,
    }
}

impl Button {
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
    pub fn icon(mut self, name: &'static str) -> Self {
        self.icon = Some(name);
        self
    }
    /// A custom leading element (e.g. a spinner) instead of a static icon.
    pub fn leading(mut self, el: impl IntoElement) -> Self {
        self.leading = Some(el.into_any_element());
        self
    }
    pub fn trailing(mut self, name: &'static str) -> Self {
        self.trailing = Some(name);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (h, radius, pad, font) = if self.small {
            (32., 8., 12., 13.)
        } else {
            (40., 10., 18., 14.)
        };
        let fg = if self.disabled {
            muted()
        } else {
            match self.kind {
                Kind::Primary | Kind::Secondary => text(),
                Kind::Ghost => muted(),
                Kind::Danger => behind(),
            }
        };
        let group: SharedString = "btn".into();
        let ghost = self.kind == Kind::Ghost && !self.disabled;
        let icon_el = |name: &'static str| {
            icon(name, 16., fg).when(ghost, |s| {
                s.group_hover(group.clone(), |s| s.text_color(text()))
            })
        };
        let mut el = div()
            .id(self.id)
            .group(group.clone())
            .flex_none()
            .h(px(h))
            .px(px(pad))
            .rounded(px(radius))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(font))
            .line_height(px(18.))
            .text_color(fg)
            .whitespace_nowrap();
        if self.disabled {
            el = el.bg(raised());
        } else {
            el = match self.kind {
                Kind::Primary => el.bg(accent()).hover(|s| s.bg(accent_hover())),
                Kind::Secondary | Kind::Danger => el
                    .bg(raised())
                    .border_1()
                    .border_color(border())
                    .hover(|s| s.border_color(accent())),
                Kind::Ghost => el.hover(|s| s.text_color(text())),
            }
            .cursor_pointer();
            if let Some(f) = self.on_click {
                el = el.on_click(f);
            }
        }
        el.when_some(self.icon, |d, name| d.child(icon_el(name)))
            .children(self.leading)
            .child(self.label)
            .when_some(self.trailing, |d, name| d.child(icon_el(name)))
    }
}

// ---------------------------------------------------------------------------------------------
// Chips, badges, checkbox, progress, status icon
// ---------------------------------------------------------------------------------------------

pub fn chip(id: impl Into<ElementId>, label: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(30.))
        .px(px(12.))
        .rounded(px(15.))
        .border_1()
        .flex()
        .items_center()
        .font_weight(FontWeight::MEDIUM)
        .text_size(px(13.))
        .line_height(px(18.))
        .cursor_pointer()
        .map(|d| {
            if on {
                d.bg(alpha(accent(), 0.18))
                    .border_color(accent())
                    .text_color(text())
            } else {
                d.bg(surface())
                    .border_color(border())
                    .text_color(muted())
                    .hover(|s| s.text_color(text()))
            }
        })
        .child(label.into())
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // every variant SCREENS.md defines, not all are used by a screen
pub enum Badge {
    Ok,
    Warn,
    Muted,
    Pb,
    Accent,
}

fn badge_colors(kind: Badge) -> (Hsla, Hsla) {
    match kind {
        Badge::Ok => (ahead(), alpha(ahead(), 0.12)),
        Badge::Warn => (warn(), alpha(warn(), 0.12)),
        Badge::Muted => (muted(), raised()),
        Badge::Pb => (pb(), alpha(pb(), 0.14)),
        Badge::Accent => (text(), alpha(accent(), 0.30)),
    }
}

pub fn badge(s: impl Into<SharedString>, kind: Badge) -> Div {
    let (fg, bgc) = badge_colors(kind);
    div()
        .flex_none()
        .h(px(22.))
        .px(px(8.))
        .rounded(px(6.))
        .bg(bgc)
        .text_color(fg)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(11.))
        .line_height(px(14.))
        .whitespace_nowrap()
        .child(s.into())
}

pub fn badge_icon(s: impl Into<SharedString>, kind: Badge, icon_name: &str) -> Div {
    let (fg, bgc) = badge_colors(kind);
    div()
        .flex_none()
        .h(px(22.))
        .px(px(8.))
        .rounded(px(6.))
        .bg(bgc)
        .text_color(fg)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(11.))
        .line_height(px(14.))
        .whitespace_nowrap()
        .child(icon(icon_name, 12., fg))
        .child(s.into())
}

/// Badge whose text is JetBrains Mono 11 (versions, lengths).
pub fn badge_mono(s: impl Into<SharedString>, kind: Badge) -> Div {
    badge(s, kind)
        .font_family(MONO)
        .font_weight(FontWeight::NORMAL)
}

pub fn checkbox(on: bool) -> Div {
    let d = div()
        .flex_none()
        .size(px(18.))
        .rounded(px(5.))
        .flex()
        .items_center()
        .justify_center();
    if on {
        d.bg(accent()).child(icon("check", 12., text()))
    } else {
        d.border(px(1.5)).border_color(border())
    }
}

/// Radio: an 18 px circle with a 1.5 px border, or an accent fill with an 8 px dot.
pub fn radio(on: bool) -> Div {
    let d = div()
        .flex_none()
        .size(px(18.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center();
    if on {
        d.bg(accent())
            .child(div().size(px(8.)).rounded_full().bg(text()))
    } else {
        d.border(px(1.5)).border_color(border())
    }
}

pub fn progress(frac: f32) -> Div {
    div().h(px(4.)).w_full().rounded(px(2.)).bg(border()).child(
        div()
            .h_full()
            .w(relative(frac.clamp(0.0, 1.0)))
            .rounded(px(2.))
            .bg(accent()),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // every variant SCREENS.md defines, not all are used by a screen
pub enum Status {
    Ok,
    Warn,
    Info,
    Error,
}

pub fn status_icon(status: Status, size_px: f32) -> Svg {
    match status {
        Status::Ok => icon("circle-check", size_px, ahead()),
        Status::Warn => icon("triangle-alert", size_px, warn()),
        Status::Info => icon("info", size_px, muted()),
        Status::Error => icon("circle-x", size_px, behind()),
    }
}

/// Setting row: label (Inter Medium 14) with an optional hint, and a control on the right.
pub fn setting_row(
    label: impl Into<SharedString>,
    hint: Option<SharedString>,
    control: impl IntoElement,
    first: bool,
) -> Div {
    let has_hint = hint.is_some();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.))
        .min_h(px(56.))
        .when(has_hint, |d| d.py(px(10.)))
        .when(!first, |d| d.border_t_1().border_color(border()))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(txt(14.).font_weight(FontWeight::MEDIUM).child(label.into()))
                .when_some(hint, |d, h| d.child(muted_txt(12., h))),
        )
        .child(control)
}

/// QR code from rows of '0'/'1': black modules on a white tile with a 4-module quiet zone.
/// Module edges are rounded to whole pixels so 4.5 px modules stay crisp.
pub fn qr_tile(rows: &'static [String], module: f32) -> Div {
    let n = rows.len();
    let quiet = 4usize;
    let total = ((n + 2 * quiet) as f32 * module).round();
    let edge = move |i: usize| (i as f32 * module).round();
    div()
        .flex_none()
        .size(px(total))
        .rounded(px(10.))
        .bg(white())
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let ox = bounds.origin.x.floor();
                    let oy = bounds.origin.y.floor();
                    let ink = c(0x0A090E);
                    for (r, row) in rows.iter().enumerate() {
                        let bytes = row.as_bytes();
                        let mut cidx = 0;
                        while cidx < bytes.len() {
                            if bytes[cidx] != b'1' {
                                cidx += 1;
                                continue;
                            }
                            let start = cidx;
                            while cidx < bytes.len() && bytes[cidx] == b'1' {
                                cidx += 1;
                            }
                            let x0 = edge(quiet + start);
                            let x1 = edge(quiet + cidx);
                            let y0 = edge(quiet + r);
                            let y1 = edge(quiet + r + 1);
                            window.paint_quad(fill(
                                gpui::Bounds {
                                    origin: point(ox + px(x0), oy + px(y0)),
                                    size: size(px(x1 - x0), px(y1 - y0)),
                                },
                                ink,
                            ));
                        }
                    }
                },
            )
            .size_full(),
        )
}

// ---------------------------------------------------------------------------------------------
// Stateful components
// ---------------------------------------------------------------------------------------------

pub const TOGGLE_MS: f32 = 120.0;
pub const SPIN_MS: f32 = 900.0;

impl Scene {
    pub fn toggle_on(&self, key: &str) -> bool {
        self.toggles.get(key).copied().unwrap_or(false)
    }

    /// Toggle: 36×20 track, 16 px knob sliding 120 ms ease-out.
    pub fn toggle(&self, key: &'static str, cx: &mut Context<Self>) -> Stateful<Div> {
        let on = self.toggle_on(key);
        let (off_x, on_x) = (2.0, 18.0);
        let mut x = if on { on_x } else { off_x };
        if let Some(start) = self.toggle_anim.get(key) {
            let t = ms_since(*start, Instant::now()) / TOGGLE_MS;
            if t < 1.0 {
                let e = ease_out(t);
                x = if on {
                    off_x + (on_x - off_x) * e
                } else {
                    on_x + (off_x - on_x) * e
                };
                self.want_frame.set(true);
            }
        }
        div()
            .id(SharedString::from(format!("toggle-{key}")))
            .flex_none()
            .relative()
            .w(px(36.))
            .h(px(20.))
            .rounded(px(10.))
            .bg(if on { accent() } else { border() })
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_toggle(key, !this.toggle_on(key), cx);
            }))
            .child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left(px(x))
                    .size(px(16.))
                    .rounded_full()
                    .bg(text()),
            )
    }

    pub fn set_toggle(&mut self, key: &'static str, on: bool, cx: &mut Context<Self>) {
        if self.toggle_on(key) != on {
            self.toggles.insert(key, on);
            self.toggle_anim.insert(key, Instant::now());
            cx.notify();
        }
    }

    /// Segmented control; `on_select` runs with the clicked index.
    pub fn segmented(
        &self,
        id: &'static str,
        items: Vec<SharedString>,
        selected: usize,
        cx: &mut Context<Self>,
        on_select: impl Fn(&mut Scene, usize, &mut Window, &mut Context<Scene>) + 'static,
    ) -> Div {
        let on_select = Rc::new(on_select);
        div()
            .flex_none()
            .flex()
            .flex_row()
            .p(px(3.))
            .rounded(px(10.))
            .bg(raised())
            .children(items.into_iter().enumerate().map(|(i, label)| {
                let sel = i == selected;
                let f = on_select.clone();
                div()
                    .id((id, i))
                    .h(px(30.))
                    .px(px(12.))
                    .rounded(px(8.))
                    .flex()
                    .items_center()
                    .font_weight(FontWeight::MEDIUM)
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .map(|d| {
                        if sel {
                            d.bg(accent()).text_color(text())
                        } else {
                            d.text_color(muted()).hover(|s| s.text_color(text()))
                        }
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        f(this, i, window, cx);
                        cx.notify();
                    }))
                    .child(label)
            }))
    }

    /// `refresh-cw` rotating once per 900 ms; requests frames while drawn.
    pub fn spinner(&self, size_px: f32, color: Hsla) -> Svg {
        self.want_frame.set(true);
        let ms = ms_since(self.epoch, Instant::now()) % SPIN_MS;
        icon("refresh-cw", size_px, color)
            .with_transformation(Transformation::rotate(radians(ms / SPIN_MS * TAU)))
    }
}
