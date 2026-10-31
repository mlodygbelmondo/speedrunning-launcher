//! Home, scene v3 (REDESIGN.md "Home"): one layout per selected mode over the mode's key art.
//! Every number is a position in the 1280×800 window. Picking a mode tile switches instantly (no
//! transition, no animation), so idle stays idle.
use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, ObjectFit, ParentElement, SharedString,
    Styled, div, img, prelude::*, px,
};

use crate::{
    app::Scene,
    assets::{SHADE_HERO, card_path, embedded},
    copy::{t, tf},
    data::screens,
    v3::*,
};

/// Baseline of `size` px text in a `line` px box (CSS half-leading), for a font whose ascent and
/// descent are `asc`/`desc` em.
fn baseline(size: f32, line: f32, asc: f32, desc: f32) -> f32 {
    (line - (asc + desc) * size) / 2. + asc * size
}
const PLEX_ASC: f32 = 1.025;
const PLEX_DESC: f32 = 0.275;

impl Scene {
    /// One stat of the right column: a Plex Mono SemiBold 26 value over a caps label.
    fn form_item(value: impl IntoElement, label: Div) -> Div {
        div()
            .flex()
            .flex_col()
            .items_end()
            .child(value)
            .child(label.mt(px(7.)))
    }

    /// Plex Mono SemiBold 26, tracking −0.01 em.
    fn form_value(v: impl Into<SharedString>, color: gpui::Hsla) -> Div {
        let v: SharedString = v.into();
        spaced(&v, PLEX, FontWeight::SEMIBOLD, 26., -0.01, 1., color)
    }

    fn form_label(s: &str) -> Div {
        caps(s, 11., FontWeight::SEMIBOLD, 0.16, muted3())
    }

    fn home_right(&self, mi: usize) -> Option<Div> {
        let s = screens();
        let id = s.modes[mi].id.as_str();
        let col = div()
            .absolute()
            .right(px(48.))
            .top(px(116.))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(22.));
        if id == "ranked" {
            let r = &s.v3.ranked;
            let season = s.v3.season.to_string();
            let tier = t(&format!("tier.{}", r.tier));
            // "1642" + the delta at 13 px on the same baseline.
            let lift =
                baseline(26., 26., PLEX_ASC, PLEX_DESC) - baseline(13., 13., PLEX_ASC, PLEX_DESC);
            let elo = div()
                .flex()
                .flex_row()
                .items_start()
                .child(Self::form_value(r.elo.to_string(), text3()))
                .child(
                    spaced(
                        &arrow(&r.elo_delta),
                        PLEX,
                        FontWeight::SEMIBOLD,
                        13.,
                        -0.01,
                        1.,
                        ahead3(),
                    )
                    .ml(px(6.))
                    .mt(px(lift)),
                );
            let elo_label = div()
                .flex()
                .flex_row()
                .items_center()
                .child(
                    div()
                        .flex_none()
                        .size(px(7.))
                        .mr(px(6.))
                        .bg(tier_color(&r.tier)),
                )
                .child(Self::form_label(&tf("v3.r.elo", &[("tier", &tier)])));
            Some(
                col.child(Self::form_item(elo, elo_label))
                    .child(Self::form_item(
                        Self::form_value(format!("{} – {}", r.season_w, r.season_l), text3()),
                        Self::form_label(&tf("v3.r.season", &[("season", &season)])),
                    ))
                    .child(Self::form_item(
                        Self::form_value(r.best.clone(), text3()),
                        Self::form_label(&t("v3.r.best")),
                    )),
            )
        } else if let Some(run) = s.v3.runs.get(id) {
            Some(
                col.child(Self::form_item(
                    Self::form_value(run.pb.clone(), text3()),
                    Self::form_label(&tf("v3.pb", &[("date", &run.pb_date)])),
                ))
                .child(Self::form_item(
                    Self::form_value(run.sob.clone(), gold()),
                    Self::form_label(&t("v3.sob")),
                ))
                .child(Self::form_item(
                    Self::form_value(run.runs_month.to_string(), text3()),
                    Self::form_label(&t("v3.runs_month")),
                )),
            )
        } else {
            None
        }
    }

    /// Band header: a caps label on the left and (optionally) one on the right.
    fn band_head(left: &str, right: Option<&str>) -> Div {
        div()
            .flex()
            .flex_row()
            .justify_between()
            .child(cap_label(left))
            .when_some(right, |d, r| d.child(cap_label(r)))
    }

    fn home_matches(&self) -> Div {
        let r = &screens().v3.ranked;
        let cell = |border: gpui::Hsla| {
            div()
                .flex_1()
                .min_w(px(0.))
                .h(px(52.))
                .flex()
                .flex_row()
                .items_center()
                // 12/10 instead of the mockup's 14/12, like the web build, so Polish fits.
                .gap(px(10.))
                .pl(px(12.))
                .pr(px(12.))
                .bg(text3a(0.04))
                .border_l(px(3.))
                .border_color(border)
        };
        let who = |name: SharedString, sub: SharedString| {
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .child(
                    inter(13., FontWeight::SEMIBOLD, 1.2)
                        .text_color(text3())
                        .truncate()
                        .child(name),
                )
                .child(
                    plex(11., FontWeight::NORMAL)
                        .line_height(px(14.3))
                        .text_color(muted3())
                        .truncate()
                        .child(sub),
                )
        };
        let mut cells: Vec<Div> = r
            .matches
            .iter()
            .map(|m| {
                let (color, sub) = if m.won {
                    (ahead3(), tf("v3.match_won", &[("time", &m.time)]))
                } else {
                    (behind3(), tf("v3.match_lost", &[("time", &m.time)]))
                };
                cell(color)
                    .child(pixel_avatar(&m.avatar, 26.))
                    .child(who(m.opponent.clone().into(), sub))
                    .child(
                        plex(13., FontWeight::MEDIUM)
                            .flex_none()
                            .text_color(color)
                            .child(minus(&m.elo)),
                    )
            })
            .collect();
        let w = &r.weekly;
        cells.push(
            cell(gpui::transparent_black())
                .child(who(
                    tf("v3.weekly", &[("n", &w.n.to_string())]),
                    tf(
                        "v3.weekly_place",
                        &[("place", &w.place.to_string()), ("total", &w.total)],
                    ),
                ))
                .child(
                    plex(13., FontWeight::MEDIUM)
                        .flex_none()
                        .text_color(muted3())
                        .child(w.time.clone()),
                ),
        );
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(Self::band_head(
                &t("v3.matches"),
                Some(&t("v3.matches_src")),
            ))
            .child(div().flex().flex_row().gap(px(14.)).children(cells))
    }

    fn home_strip(&self, id: &str) -> Option<Div> {
        let run = screens().v3.runs.get(id)?;
        let segs = run.splits.iter().map(|sp| {
            // CSS `flex: <secs>`: grow by the segment time from a zero basis.
            div()
                .relative()
                .h_full()
                .min_w(px(0.))
                .flex_basis(px(0.))
                .flex_shrink(1.)
                .overflow_hidden()
                .bg(gpui::rgb(0x1A1622))
                .flex_grow(sp.secs as f32)
                .child(art_fill(card_path(&sp.art)))
                .child(shade_up(0.92, 0.25, 0.8))
                .child(
                    div()
                        .absolute()
                        .left(px(10.))
                        .right(px(8.))
                        .bottom(px(9.))
                        .flex()
                        .flex_col()
                        .gap(px(5.))
                        .child(caps(&sp.name, 11., FontWeight::SEMIBOLD, 0.14, text3()))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_start()
                                .child(
                                    plex(13., FontWeight::MEDIUM)
                                        .text_color(text3())
                                        .child(sp.time.clone()),
                                )
                                .child(
                                    plex(11., FontWeight::MEDIUM)
                                        .ml(px(6.))
                                        .mt(px(baseline(13., 13., PLEX_ASC, PLEX_DESC)
                                            - baseline(11., 11., PLEX_ASC, PLEX_DESC)))
                                        .text_color(gold())
                                        .child(sp.best.clone()),
                                ),
                        ),
                )
        });
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(Self::band_head(&t("v3.strip"), Some(&t("v3.strip_src"))))
                .child(
                    div()
                        .h(px(66.))
                        .flex()
                        .flex_row()
                        .gap(px(3.))
                        .children(segs),
                ),
        )
    }

    fn home_install(&self, id: &str) -> Option<Div> {
        let s = screens();
        let inst = s.v3.install.get(id)?;
        // A chip: the layer name over a 9 px caps tag.
        let chip = |layer: &str, tag: SharedString, color: gpui::Hsla| {
            div()
                .flex_none()
                .h(px(40.))
                .px(px(16.))
                .flex()
                .flex_col()
                .justify_center()
                .gap(px(4.))
                .bg(text3a(0.05))
                .child(
                    inter(13., FontWeight::SEMIBOLD, 1.)
                        .text_color(text3())
                        .whitespace_nowrap()
                        .child(s.layer(layer).name.clone()),
                )
                .child(caps(&tag, 9., FontWeight::SEMIBOLD, 0.16, color))
        };
        let new_tag = t("v3.install_new");
        let have_tag = t("v3.install_have");
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(Self::band_head(&t("v3.install_title"), None))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(3.))
                        .children(inst.new.iter().map(|l| chip(l, new_tag.clone(), accent2())))
                        .children(
                            inst.have
                                .iter()
                                .map(|l| chip(l, have_tag.clone(), faint3())),
                        ),
                ),
        )
    }

    fn home_modes(&self, cx: &mut Context<Self>) -> Div {
        let s = screens();
        let tiles = s.modes.iter().enumerate().map(|(i, m)| {
            let v = s.v3_mode(i);
            let on = i == self.mode_sel;
            let missing = self.mode_status(i) == "missing";
            div()
                .id(("home-mode", i))
                .group("home-tile")
                .relative()
                .flex_1()
                .min_w(px(0.))
                .h(px(116.))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| this.select_mode(i, cx)))
                .child(overlay(tile_bg()))
                .child(art_fill(card_path(&v.art)))
                .when(!on, |d| d.child(overlay(bg3a(0.5))))
                .child(shade_up(0.9, 0.0, 0.7))
                .when(missing, |d| {
                    d.child(
                        div()
                            .absolute()
                            .top(px(10.))
                            .right(px(10.))
                            .px(px(7.))
                            .py(px(5.))
                            .bg(bg3a(0.75))
                            .child(caps(
                                &t("v3.badge_download"),
                                10.,
                                FontWeight::SEMIBOLD,
                                0.14,
                                accent2(),
                            )),
                    )
                })
                .child(
                    div()
                        .absolute()
                        .left(px(14.))
                        .bottom(px(12.))
                        .flex()
                        .flex_col()
                        .child(
                            arch(17., FontWeight::BOLD)
                                .text_color(text3())
                                .child(v.short.clone()),
                        )
                        .child(
                            plex(11., FontWeight::MEDIUM)
                                .mt(px(6.))
                                .text_color(muted3())
                                .child(m.mc.clone()),
                        ),
                )
                .child(if on { ring() } else { hover_ring("home-tile") })
        });
        div()
            .absolute()
            .left(px(64.))
            .right(px(48.))
            .bottom(px(36.))
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(Self::band_head(
                &t("v3.modes"),
                Some(&tf("v3.modes_shared", &[("size", &s.disk_saved)])),
            ))
            .child(div().flex().flex_row().gap(px(14.)).children(tiles))
    }

    pub fn render_home(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let s = screens();
        let mi = self.mode_sel;
        let m = &s.modes[mi];
        let v = s.v3_mode(mi);
        let id = m.id.as_str();
        let status = self.mode_status(mi);

        let mut row = div()
            .mt(px(16.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(18.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .flex_none()
                            .size(px(6.))
                            .mr(px(8.))
                            .bg(status_color(status)),
                    )
                    .child(
                        plex(12., FontWeight::MEDIUM)
                            .text_color(status_color(status))
                            .child(t(&format!("v3.status.{status}"))),
                    ),
            );
        // "1.16.1 · 23 mods checked", or only the version while the mode isn't installed.
        row = row.child(
            plex(12., FontWeight::MEDIUM)
                .text_color(faint3())
                .child(if status == "missing" {
                    SharedString::from(m.mc.clone())
                } else {
                    format!(
                        "{} · {}",
                        m.mc,
                        tf("v3.mods_checked", &[("n", &v.mods_total.to_string())])
                    )
                    .into()
                }),
        );
        let extra: Option<SharedString> = if status == "missing" {
            Some(tf(
                "v3.download_hint",
                &[("size", v.download.as_deref().unwrap_or(&v.disk))],
            ))
        } else if id == "ranked" {
            Some(tf("v3.online", &[("n", &s.v3.online)]))
        } else {
            self.key_tools(mi)
        };
        if let Some(extra) = extra {
            row = row.child(
                plex(12., FontWeight::MEDIUM)
                    .text_color(faint3())
                    .child(extra),
            );
        }

        let hero = div()
            .absolute()
            .left(px(64.))
            .top(px(116.))
            .w(px(560.))
            .flex()
            .flex_col()
            .child(caps(
                &tf(
                    &format!("v3.eyebrow.{id}"),
                    &[("season", &s.v3.season.to_string()), ("mc", &m.mc)],
                ),
                12.,
                FontWeight::SEMIBOLD,
                0.2,
                accent2(),
            ))
            .child(tight(&v.short, 112., FontWeight::EXTRA_BOLD, -0.03, 0.86, text3()).mt(px(16.)))
            .child(
                inter(16., FontWeight::NORMAL, 1.5)
                    .mt(px(20.))
                    .whitespace_nowrap()
                    .text_color(muted3())
                    .child(wrap(
                        &t(&format!("v3.desc.{id}")),
                        INTER,
                        FontWeight::NORMAL,
                        16.,
                        440.,
                    )),
            )
            .child(row)
            .child(
                div()
                    .mt(px(30.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(28.))
                    .child(self.mode_play(mi, "home-play", 288., 64., 19., cx))
                    .when(status != "missing", |d| {
                        d.child(ghost("home-verify", &t("v3.verify")))
                    }),
            );

        let band: Option<Div> = match id {
            "ranked" => Some(self.home_matches()),
            // What gets installed stays listed after the fake install, like the web build.
            _ => self.home_strip(id).or_else(|| self.home_install(id)),
        };

        div()
            .size_full()
            .relative()
            .children(self.hero_images())
            .child(
                img(embedded(SHADE_HERO))
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            )
            .child(hero)
            .children(self.home_right(mi).filter(|_| status != "missing"))
            .children(band.map(|b| {
                div()
                    .absolute()
                    .left(px(64.))
                    .right(px(48.))
                    .top(px(494.))
                    .child(b)
            }))
            .child(self.home_modes(cx))
            .into_any_element()
    }
}
