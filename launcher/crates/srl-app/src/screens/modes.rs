//! Modes (REDESIGN.md "Modes"): the mode list on the left, the selected mode's detail with its
//! layer stack on the right, over the mode's key art at 16 %.
use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::*, px,
};

use crate::{
    app::Launcher,
    assets::card_path,
    catalog,
    copy::{size, t, tf, tn},
    demo,
    v3::*,
};

impl Launcher {
    fn mode_row(&self, mi: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let d = &catalog::MODES[mi];
        let on = mi == self.mode_sel;
        let status = self.status(mi);
        let bar = match self.state(mi) {
            crate::backend::ModeState::Installing(p) => Some(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .child(thin_progress(super::play::bar_fraction(p), 2.)),
            ),
            _ => None,
        };
        div()
            .id(("mode-row", mi))
            .group("mode-row")
            .relative()
            .flex_none()
            .h(px(104.))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.select_mode(mi, cx)))
            .child(overlay(tile_bg()))
            .child(art_fill(card_path(d.art)))
            // 90deg: .92 → .55 at 55 % → .25 (selected) or .95 → .78 at 60 % → .6.
            .map(|e| {
                if on {
                    e.child(shade_right(0., 0.55, 0.92, 0.55))
                        .child(shade_right(0.55, 1.0, 0.55, 0.25))
                } else {
                    e.child(shade_right(0., 0.6, 0.95, 0.78))
                        .child(shade_right(0.6, 1.0, 0.78, 0.6))
                }
            })
            .child(
                div()
                    .absolute()
                    .left(px(20.))
                    .top(px(26.))
                    .flex()
                    .flex_col()
                    .child(tight(
                        d.short,
                        30.,
                        FontWeight::EXTRA_BOLD,
                        -0.02,
                        1.0,
                        text3(),
                    ))
                    .child(
                        inter(12., FontWeight::MEDIUM, 1.)
                            .mt(px(8.))
                            .whitespace_nowrap()
                            .text_color(muted3())
                            .child(t(&format!("mode.{}.full", d.mode.id()))),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .right(px(18.))
                    .top(px(18.))
                    .flex()
                    .flex_col()
                    .items_end()
                    // Like Home's tiles: only the states that need a look.
                    .when(
                        matches!(
                            status,
                            Status::Installing | Status::Update | Status::Running | Status::Failed
                        ),
                        |d| d.child(status_text(&t(status.key()), status.color()).mb(px(8.))),
                    )
                    .child(
                        plex(11., FontWeight::MEDIUM)
                            .text_color(faint3())
                            .child(d.mode.minecraft_version()),
                    ),
            )
            .children(bar)
            .child(if on { ring() } else { hover_ring("mode-row") })
    }

    fn layer_row(&self, mi: usize, lid: &str) -> Div {
        let mode = self.mode(mi);
        let others: Vec<&str> = catalog::sharing(mode, lid)
            .iter()
            .map(|d| d.short)
            .collect();
        let own = others.is_empty();
        // What's in the layer comes from the mod lists (srl-mods) in a later step; the demo
        // shows the prototype's.
        let detail: Option<SharedString> = demo::get().and_then(|d| d.layers.get(lid)).map(|l| {
            match (&l.detail, l.mods.as_slice()) {
                (Some(d), _) => d.clone().into(),
                (None, [(_, ver)]) => format!("{} · {ver}", t("modes.mods_count_one")).into(),
                (None, mods) => tn("modes.mods_count", mods.len() as u64),
            }
        });
        let tag = if own {
            t("v3.only_this")
        } else {
            tf("v3.shared_with", &[("list", &others.join(", "))])
        };
        div()
            .relative()
            .h(px(40.))
            .px(px(16.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(14.))
            .bg(if own { accent3a(0.22) } else { text3a(0.05) })
            .when(own, |d| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(px(3.))
                        .bg(accent2()),
                )
            })
            .child(
                inter(14., FontWeight::SEMIBOLD, 1.0)
                    .flex_none()
                    .w(px(210.))
                    .text_color(text3())
                    .truncate()
                    .child(t(&format!("layer.{lid}"))),
            )
            .child(
                plex(12., FontWeight::NORMAL)
                    .flex_1()
                    .min_w(px(0.))
                    .text_color(muted3())
                    .truncate()
                    .children(detail),
            )
            .child(
                inter(12., FontWeight::MEDIUM, 1.)
                    .flex_none()
                    .text_color(if own { accent2() } else { faint3() })
                    .child(tag),
            )
    }

    fn mode_detail(&self, cx: &mut Context<Self>) -> Div {
        let mi = self.mode_sel;
        let mode = self.mode(mi);
        let d = catalog::def(mode);
        let info = &self.modes[mi].info;
        let id = mode.id();
        let saved = demo::get().map(|d| tf("v3.layers_saved", &[("size", &d.disk_saved)]));
        div()
            .absolute()
            .left(px(560.))
            .right(px(48.))
            .top(px(104.))
            .flex()
            .flex_col()
            .child(kicker(&self.eyebrow(mode), accent2()).mt(px(4.)))
            .child(tight(d.short, 72., FontWeight::EXTRA_BOLD, -0.03, 0.9, text3()).mt(px(12.)))
            .child(
                inter(15., FontWeight::NORMAL, 1.5)
                    .mt(px(14.))
                    .whitespace_nowrap()
                    .text_color(muted3())
                    .child(wrap(
                        &super::play::mode_desc(mode),
                        INTER,
                        FontWeight::NORMAL,
                        15.,
                        600.,
                    )),
            )
            .child(
                div()
                    .mt(px(22.))
                    .flex()
                    .flex_row()
                    .gap(px(36.))
                    .child(fact(&t("v3.f_mc"), mode.minecraft_version().into(), true))
                    .children(
                        info.mods
                            .map(|n| fact(&t("v3.f_mods"), n.to_string().into(), true)),
                    )
                    .children(
                        info.disk
                            .map(|b| fact(&t("v3.f_disk"), size(b).into(), true)),
                    )
                    .child(fact(
                        &t("v3.f_board"),
                        t(&format!("mode.{id}.board")),
                        false,
                    )),
            )
            .child(
                div()
                    .mt(px(30.))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .child(section_head(&t("v3.layers")))
                    .children(saved.map(|s| label(&s))),
            )
            .child(
                div()
                    .mt(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .children(d.layers.iter().map(|l| self.layer_row(mi, l))),
            )
            .child(
                div()
                    .mt(px(26.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(28.))
                    .child(self.mode_play(mi, "modes-play", 280., 58., 17., cx))
                    .children(self.play_ghosts(mi, "modes", true, cx))
                    .child(
                        secondary("modes-mods", &t("v3.mods_button"))
                            .on_click(cx.listener(|this, _, _, cx| this.open_mods(cx))),
                    ),
            )
            .children(self.fail_line(mi, 640., cx))
            .children(self.mods_line(mi, 640.))
            .child(
                inter(12., FontWeight::NORMAL, 1.45)
                    .mt(px(18.))
                    .whitespace_nowrap()
                    .text_color(faint3())
                    .child(wrap(
                        &t(&format!("v3.note.{id}")),
                        INTER,
                        FontWeight::NORMAL,
                        12.,
                        640.,
                    )),
            )
    }

    pub fn render_modes(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let list = div()
            .absolute()
            .left(px(64.))
            .top(px(104.))
            .w(px(432.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .children(
                (0..catalog::MODES.len())
                    .map(|i| self.mode_row(i, cx).into_any_element())
                    .collect::<Vec<_>>(),
            );
        div()
            .size_full()
            .relative()
            // Key art at 16 %: the image under a bg @ 84 % rect, then the 3-stop horizontal
            // gradient as two 2-stop halves and the bottom fade.
            .children(self.hero_images())
            .child(overlay(bg3a(0.84)))
            .child(shade_right(0., 0.5, 1.0, 0.7))
            .child(shade_right(0.5, 1.0, 0.7, 0.85))
            .child(shade_up(1.0, 0.0, 0.6))
            .child(list)
            .child(self.mode_detail(cx))
            .into_any_element()
    }
}
