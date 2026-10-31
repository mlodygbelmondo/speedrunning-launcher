//! Modes, scene v3 (REDESIGN.md "Modes"): the mode list on the left, the selected mode's detail
//! with its layer stack on the right, over the mode's key art at 16 %.
use std::time::Instant;

use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::*, px,
};

use crate::{
    app::{Install, Item, Modal, Scene},
    assets::card_path,
    copy::{t, tf},
    data::screens,
    v3::*,
};

/// Key tools named in the Home status row of a run mode (REDESIGN.md "Status row"): the mode's
/// seed mod from its own layer, then the tools that start with the game on this OS.
const SEED_MOD: [(&str, &str, &str); 2] = [("rsg", "rsg", "SeedQueue"), ("fsg", "fsg", "FSG Mod")];
/// The run tools named in the Home status line.
const RUN_TOOLS: [&str; 2] = ["ninjabrain", "paceman"];

impl Scene {
    /// `ready`, `update` or `missing`; a finished fake install makes a missing mode ready.
    pub fn mode_status(&self, mi: usize) -> &'static str {
        if self.installed.contains(&Item::Mode(mi)) {
            return "ready";
        }
        match screens().modes[mi].status.as_str() {
            "ready" => "ready",
            "update" => "update",
            _ => "missing",
        }
    }

    pub fn open_check(&mut self, mi: usize, cx: &mut Context<Self>) {
        self.check_mode = mi;
        self.modal = Some(Modal {
            open: true,
            start: Instant::now(),
        });
        cx.notify();
    }

    /// "SeedQueue 1.8 · Ninjabrain · PaceMan" for RSG: the mode's key mod (if it has one), then
    /// the two run tools, by their first word, like the web build.
    pub fn key_tools(&self, mi: usize) -> Option<SharedString> {
        let s = screens();
        let id = s.modes[mi].id.as_str();
        let mut parts: Vec<String> = Vec::new();
        if let Some((_, layer, name)) = SEED_MOD.iter().find(|(m, _, _)| *m == id)
            && let Some((n, ver)) = s.layer(layer).mods.iter().find(|(n, _)| n == name)
        {
            if ver.starts_with(|c: char| c.is_ascii_digit()) {
                parts.push(format!("{n} {ver}"));
            } else {
                parts.push(n.clone());
            }
        }
        parts.extend(RUN_TOOLS.iter().filter_map(|tid| {
            let tl = s.tools.iter().find(|tl| tl.id == *tid)?;
            Some(tl.name.split(' ').next().unwrap_or(&tl.name).to_string())
        }));
        (!parts.is_empty()).then(|| parts.join(" · ").into())
    }

    /// The Play / Download button of mode `mi`, with the fake-install bar under it.
    pub fn mode_play(
        &self,
        mi: usize,
        id: &'static str,
        w: f32,
        h: f32,
        label_size: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        let s = screens();
        let v = s.v3_mode(mi);
        let status = self.mode_status(mi);
        let install = self.install_state(Item::Mode(mi), status != "missing");
        let (label, hint) = match (&install, status) {
            (Install::Running(_), _) | (_, "missing") => (
                if matches!(install, Install::Running(_)) {
                    t("common.installing")
                } else {
                    t("v3.download")
                },
                tf(
                    "v3.download_hint",
                    &[("size", v.download.as_deref().unwrap_or(&v.disk))],
                ),
            ),
            (_, "update") => (
                t("v3.play"),
                tf(
                    "v3.play_hint.update",
                    &[("mod", v.update_mod.as_deref().unwrap_or(&s.modes[mi].name))],
                ),
            ),
            _ if s.modes[mi].id == "ranked" => (t("v3.play"), t("v3.play_hint.ranked")),
            _ => (t("v3.play"), t("v3.play_hint.run")),
        };
        let btn = play_button(id, &label, hint, w, h, label_size).on_click(cx.listener(
            move |this, _, _, cx| {
                match this.install_state(Item::Mode(mi), this.mode_status(mi) != "missing") {
                    Install::Done => this.open_check(mi, cx),
                    Install::Running(_) => {}
                    Install::Not => this.start_install(Item::Mode(mi), cx),
                }
            },
        ));
        div().relative().flex_none().child(btn).when_some(
            match install {
                Install::Running(f) => Some(f),
                _ => None,
            },
            |d, f| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(h + 7.))
                        .child(thin_progress(f, 3.)),
                )
            },
        )
    }

    fn mode_row(&self, mi: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let s = screens();
        let m = &s.modes[mi];
        let v = s.v3_mode(mi);
        let on = mi == self.mode_sel;
        let status = self.mode_status(mi);
        div()
            .id(("mode-row", mi))
            .group("mode-row")
            .relative()
            .flex_none()
            .h(px(104.))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.select_mode(mi, cx)))
            .child(overlay(tile_bg()))
            .child(art_fill(card_path(&v.art)))
            // 90deg: .92 → .55 at 55 % → .25 (selected) or .95 → .78 at 60 % → .6.
            .map(|d| {
                if on {
                    d.child(shade_right(0., 0.55, 0.92, 0.55))
                        .child(shade_right(0.55, 1.0, 0.55, 0.25))
                } else {
                    d.child(shade_right(0., 0.6, 0.95, 0.78))
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
                        &v.short,
                        30.,
                        FontWeight::EXTRA_BOLD,
                        -0.02,
                        1.0,
                        text3(),
                    ))
                    .child(
                        plex(11., FontWeight::MEDIUM)
                            .mt(px(8.))
                            .text_color(muted3())
                            .child(m.full.clone()),
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
                    .child(caps(
                        &t(&format!("v3.status.{status}")),
                        10.,
                        FontWeight::SEMIBOLD,
                        0.16,
                        status_color(status),
                    ))
                    .child(
                        plex(11., FontWeight::MEDIUM)
                            .mt(px(8.))
                            .text_color(faint3())
                            .child(m.mc.clone()),
                    ),
            )
            .child(if on { ring() } else { hover_ring("mode-row") })
    }

    fn layer_row(&self, mi: usize, lid: &str) -> Div {
        let s = screens();
        let layer = s.layer(lid);
        let others: Vec<String> = s
            .modes
            .iter()
            .enumerate()
            .filter(|(j, o)| *j != mi && o.layers.iter().any(|l| l == lid))
            .map(|(j, _)| s.v3_mode(j).short.clone())
            .collect();
        let own = others.is_empty();
        let detail: SharedString = match (&layer.detail, layer.mods.as_slice()) {
            (Some(d), _) => d.clone().into(),
            (None, [(_, ver)]) => format!("{} · {ver}", t("modes.mods_count_one")).into(),
            (None, mods) => tf("modes.mods_count", &[("n", &mods.len().to_string())]),
        };
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
                    .child(layer.name.clone()),
            )
            .child(
                plex(12., FontWeight::NORMAL)
                    .flex_1()
                    .min_w(px(0.))
                    .text_color(muted3())
                    .truncate()
                    .child(detail),
            )
            .child(
                plex(11., FontWeight::MEDIUM)
                    .flex_none()
                    .text_color(if own { accent2() } else { faint3() })
                    .child(tag),
            )
    }

    fn mode_detail(&self, cx: &mut Context<Self>) -> Div {
        let s = screens();
        let mi = self.mode_sel;
        let m = &s.modes[mi];
        let v = s.v3_mode(mi);
        let id = m.id.as_str();
        let fact = |label: SharedString, value: SharedString| {
            div()
                .flex()
                .flex_col()
                .child(cap_label(&label).mb(px(8.)))
                .child(
                    plex(15., FontWeight::MEDIUM)
                        .text_color(text3())
                        .child(value),
                )
        };
        div()
            .absolute()
            .left(px(560.))
            .right(px(48.))
            .top(px(104.))
            .flex()
            .flex_col()
            .child(
                caps(
                    &tf(
                        &format!("v3.eyebrow.{id}"),
                        &[("season", &s.v3.season.to_string()), ("mc", &m.mc)],
                    ),
                    12.,
                    FontWeight::SEMIBOLD,
                    0.2,
                    accent2(),
                )
                .mt(px(4.)),
            )
            .child(tight(&v.short, 72., FontWeight::EXTRA_BOLD, -0.03, 0.9, text3()).mt(px(12.)))
            .child(
                inter(15., FontWeight::NORMAL, 1.5)
                    .mt(px(14.))
                    .whitespace_nowrap()
                    .text_color(muted3())
                    .child(wrap(
                        &t(&format!("v3.desc.{id}")),
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
                    .child(fact(t("v3.f_mc"), m.mc.clone().into()))
                    .child(fact(t("v3.f_mods"), v.mods_total.to_string().into()))
                    .child(fact(t("v3.f_disk"), v.disk.clone().into()))
                    .when(!m.leaderboard.is_empty(), |d| {
                        d.child(fact(t("v3.f_board"), m.leaderboard.clone().into()))
                    }),
            )
            .child(
                div()
                    .mt(px(30.))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .child(cap_label(&t("v3.layers")))
                    .child(cap_label(&tf(
                        "v3.layers_saved",
                        &[("size", &s.disk_saved)],
                    ))),
            )
            .child(
                div()
                    .mt(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .children(m.layers.iter().map(|l| self.layer_row(mi, l))),
            )
            .child(
                div()
                    .mt(px(26.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(28.))
                    .child(self.mode_play(mi, "modes-play", 280., 58., 17., cx))
                    // A mode that isn't installed has nothing to verify or open yet.
                    .when(self.mode_status(mi) != "missing", |d| {
                        d.child(ghost("modes-verify", &t("v3.verify")))
                            .child(ghost("modes-folder", &t("v3.open_folder")))
                    }),
            )
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
        let s = screens();
        let list = div()
            .absolute()
            .left(px(64.))
            .top(px(104.))
            .w(px(432.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .children(
                (0..s.modes.len())
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
