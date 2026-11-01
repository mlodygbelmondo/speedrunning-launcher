//! Home (REDESIGN.md "Home"): one layout per selected mode over the mode's key art. Every number is
//! a position in the 1280×800 window. Picking a mode tile cross-fades the key art and lets the
//! hero, the numbers and the band rise in (`Launcher::mode_fade`, a few hundred ms of frames, then
//! idle again).
//!
//! Without `SRL_DEMO` the numbers come from the stats the launcher has (`Launcher::stats`): a run
//! mode's PB, sum of best, runs this month and the PB's split strip from the SpeedRunIGT records;
//! Ranked's elo, season record, best time and recent matches from the MCSR Ranked cache (read at
//! start, no network; aged data says how old it is). What it doesn't have is left out (players
//! online, the space shared modes save), and the hero, the status, Play and the mode strip carry
//! the page on their own over the key art.
use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, ObjectFit, ParentElement, SharedString,
    Styled, div, img, prelude::*, px, relative,
};
use srl_core::Mode;
use srl_stats::SplitKind;

use crate::{
    app::{Launcher, Page},
    assets::{SHADE_HERO, card_path, embedded},
    catalog,
    copy::{t, tf, tn},
    demo,
    v3::*,
};

use super::stats::{
    fmt_date, fmt_igt, fmt_split, has_ranked_match, pb_category_sob, ranked_age, ranked_cells,
    ranked_season, split_name, tier_id, today_and_month_ms,
};

/// Baseline of `size` px text in a `line` px box (CSS half-leading), for a font whose ascent and
/// descent are `asc`/`desc` em.
fn baseline(size: f32, line: f32, asc: f32, desc: f32) -> f32 {
    (line - (asc + desc) * size) / 2. + asc * size
}
const PLEX_ASC: f32 = 1.025;
const PLEX_DESC: f32 = 0.275;

impl Launcher {
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
        inter(12., FontWeight::MEDIUM, 1.)
            .whitespace_nowrap()
            .text_color(muted3())
            .child(SharedString::from(s.to_string()))
    }

    /// The right column: Ranked's elo and season, or a run mode's PB, from the stats (the demo's
    /// sample with `SRL_DEMO`).
    fn home_right(&self, mode: Mode) -> Option<Div> {
        let Some(d) = demo::get() else {
            return self.home_right_real(mode);
        };
        let col = div()
            .absolute()
            .right(px(48.))
            .top(px(116.))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(22.));
        if mode == Mode::Ranked {
            let r = &d.ranked;
            let season = d.season.to_string();
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
        } else {
            let run = d.runs.get(mode.id())?;
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
        }
    }

    /// The right column from the real stats: Ranked from the MCSR Ranked cache, a run mode from
    /// its records. Nothing for Draftout (the design has no column for it) or without data.
    fn home_right_real(&self, mode: Mode) -> Option<Div> {
        let col = div()
            .absolute()
            .right(px(48.))
            .top(px(116.))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(22.));
        match mode {
            Mode::Draftout => None,
            Mode::Ranked => {
                let l = self.stats.ranked.as_ref()?;
                let o = l.value.as_ref()?;
                let p = &o.profile.value;
                let season = ranked_season(o);
                let delta = o
                    .matches
                    .as_ref()
                    .and_then(|m| m.value.first())
                    .and_then(|m| m.elo_change);
                let lift = baseline(26., 26., PLEX_ASC, PLEX_DESC)
                    - baseline(13., 13., PLEX_ASC, PLEX_DESC);
                let elo = p.elo.map(|elo| {
                    let value = div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .child(Self::form_value(elo.to_string(), text3()))
                        .children(delta.map(|d| {
                            spaced(
                                &arrow(&format!("{d:+}")),
                                PLEX,
                                FontWeight::SEMIBOLD,
                                13.,
                                -0.01,
                                1.,
                                if d < 0 { behind3() } else { ahead3() },
                            )
                            .ml(px(6.))
                            .mt(px(lift))
                        }));
                    let label = div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .children(p.tier.map(|tier| {
                            div()
                                .flex_none()
                                .size(px(7.))
                                .mr(px(6.))
                                .bg(tier_color(&tier_id(tier)))
                        }))
                        .child(Self::form_label(&match p.tier {
                            Some(tier) => tf(
                                "v3.r.elo",
                                &[("tier", &t(&format!("tier.{}", tier_id(tier))))],
                            ),
                            None => t("stats.elo"),
                        }));
                    Self::form_item(value, label)
                });
                let season_label = match season {
                    Some(s) => tf("v3.r.season", &[("season", &s.to_string())]),
                    None => t("stats.season_record"),
                };
                let best = p.season.best_time_ms.map(|ms| {
                    Self::form_item(
                        Self::form_value(fmt_igt(ms), text3()),
                        Self::form_label(&t("v3.r.best")),
                    )
                });
                // Like Stats: no season record without a ranked match this season.
                let record = has_ranked_match(o).then(|| {
                    Self::form_item(
                        Self::form_value(
                            format!("{} – {}", p.season.wins, p.season.losses),
                            text3(),
                        ),
                        Self::form_label(&season_label),
                    )
                });
                Some(
                    col.children(elo)
                        .children(record)
                        .children(best)
                        .children(ranked_age(l)),
                )
            }
            _ => {
                let h = self.stats.local.as_ref()?.mode(mode);
                if h.run_count == 0 {
                    return None;
                }
                let pb = h.headline_pb();
                let (_, month) = today_and_month_ms();
                let month_runs = h.runs_since(month).count();
                Some(
                    col.children(pb.map(|pb| {
                        Self::form_item(
                            Self::form_value(fmt_igt(pb.record.igt_ms), text3()),
                            Self::form_label(&tf(
                                "v3.pb",
                                &[("date", &fmt_date(pb.record.date_ms))],
                            )),
                        )
                    }))
                    .children(pb.and_then(|pb| pb_category_sob(h, pb)).map(|ms| {
                        Self::form_item(
                            Self::form_value(fmt_igt(ms), gold()),
                            Self::form_label(&t("v3.sob")),
                        )
                    }))
                    .child(Self::form_item(
                        Self::form_value(month_runs.to_string(), text3()),
                        Self::form_label(&t("v3.runs_month")),
                    )),
                )
            }
        }
    }

    /// Band header: a section head on the left and (optionally) its source on the right.
    fn band_head(left: &str, right: Option<&str>) -> Div {
        div()
            .flex()
            .flex_row()
            .justify_between()
            .child(section_head(left))
            .when_some(right, |d, r| d.child(label(r)))
    }

    /// Ranked's recent matches and the Weekly Race: the demo's, else from the MCSR Ranked cache
    /// (only when there are matches).
    fn home_matches(&self) -> Option<Div> {
        let Some(d) = demo::get() else {
            let o = self.stats.ranked.as_ref()?.value.as_ref()?;
            o.matches.as_ref().filter(|m| !m.value.is_empty())?;
            return Some(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(Self::band_head(
                        &t("v3.matches"),
                        Some(&t("v3.matches_src")),
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(14.))
                            .children(ranked_cells(o, 3, true)),
                    ),
            );
        };
        let r = &d.ranked;
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
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(Self::band_head(
                    &t("v3.matches"),
                    Some(&t("v3.matches_src")),
                ))
                .child(div().flex().flex_row().gap(px(14.)).children(cells)),
        )
    }

    /// The PB split strip of a run mode (from direction B), from the records: each segment of the
    /// PB, the best segment of its category in gold. Only for a PB with every route point.
    fn home_strip_real(&self, mode: Mode) -> Option<Vec<Div>> {
        let h = self.stats.local.as_ref()?.mode(mode);
        let pb = h.headline_pb()?;
        let rec = &pb.record;
        let times = srl_stats::segment_times(rec);
        if times.iter().any(Option::is_none) {
            return None;
        }
        let best = h
            .category(&rec.category, &rec.run_type)
            .map(|c| c.best_segments.as_slice())
            .unwrap_or_default();
        let (first, second) = if rec.split(SplitKind::Bastion) <= rec.split(SplitKind::Fortress) {
            (SplitKind::Bastion, SplitKind::Fortress)
        } else {
            (SplitKind::Fortress, SplitKind::Bastion)
        };
        let art = |k: SplitKind| match k {
            SplitKind::Bastion => "bastion",
            _ => "fortress",
        };
        let parts: [(SplitKind, &str); 7] = [
            (SplitKind::Nether, "nether-portal"),
            (first, art(first)),
            (second, art(second)),
            (SplitKind::Blind, "overworld"),
            (SplitKind::Stronghold, "stronghold"),
            (SplitKind::End, "the-end"),
            (SplitKind::Dragon, "the-end"),
        ];
        Some(
            parts
                .iter()
                .enumerate()
                .map(|(i, (kind, art))| {
                    let ms = times[i].unwrap_or_default();
                    let gold_ms = best
                        .iter()
                        .find(|b| b.segment == srl_stats::Segment::ALL[i])
                        .map(|b| b.ms);
                    Self::strip_segment(
                        &split_name(*kind),
                        art,
                        (ms / 1000).max(1) as f32,
                        fmt_split(ms),
                        gold_ms.map(fmt_split).unwrap_or_default(),
                    )
                })
                .collect(),
        )
    }

    /// One segment of the split strip: `flex: secs`, the art under a bottom gradient, the name,
    /// the time and the best segment in gold.
    fn strip_segment(name: &str, art: &str, secs: f32, time: String, best: String) -> Div {
        div()
            .relative()
            .h_full()
            .min_w(px(0.))
            .flex_basis(px(0.))
            .flex_shrink(1.)
            .overflow_hidden()
            .bg(gpui::rgb(0x1A1622))
            .flex_grow(secs)
            .child(art_fill(card_path(art)))
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
                    .child(
                        inter(12., FontWeight::SEMIBOLD, 1.)
                            .whitespace_nowrap()
                            .text_color(text3())
                            .child(SharedString::from(name.to_string())),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .child(
                                plex(13., FontWeight::MEDIUM)
                                    .text_color(text3())
                                    .child(time),
                            )
                            .child(
                                plex(11., FontWeight::MEDIUM)
                                    .ml(px(6.))
                                    .mt(px(baseline(13., 13., PLEX_ASC, PLEX_DESC)
                                        - baseline(11., 11., PLEX_ASC, PLEX_DESC)))
                                    .text_color(gold())
                                    .child(best),
                            ),
                    ),
            )
    }

    /// The PB split strip of a run mode (from direction B): the demo's, else from the records.
    fn home_strip(&self, mode: Mode) -> Option<Div> {
        let segs: Vec<Div> = match demo::get() {
            Some(d) => d
                .runs
                .get(mode.id())?
                .splits
                .iter()
                .map(|sp| {
                    Self::strip_segment(
                        &sp.name,
                        &sp.art,
                        sp.secs as f32,
                        sp.time.clone(),
                        sp.best.clone(),
                    )
                })
                .collect(),
            None => self.home_strip_real(mode)?,
        };
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

    /// What installing the mode adds: its layers, new ones first, then the ones an installed mode
    /// already brought (they're shared on disk), and only those carry a tag: when every layer is
    /// new, "new" under each says nothing. Real data: the catalogue plus the mode states.
    fn home_install(&self, mi: usize) -> Div {
        let mode = self.mode(mi);
        let have_layer = |l: &str| {
            catalog::sharing(mode, l)
                .iter()
                .any(|d| self.installed(crate::backend::index(d.mode)))
        };
        let layers = catalog::def(mode).layers;
        let chip = |layer: &str, tag: Option<SharedString>| {
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
                        .text_color(if tag.is_some() { muted3() } else { text3() })
                        .whitespace_nowrap()
                        .child(t(&format!("layer.{layer}"))),
                )
                .children(tag.map(|tag| {
                    inter(11., FontWeight::MEDIUM, 1.)
                        .whitespace_nowrap()
                        .text_color(faint3())
                        .child(tag)
                }))
        };
        let have_tag = t("v3.install_have");
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
                    .children(
                        layers
                            .iter()
                            .filter(|l| !have_layer(l))
                            .map(|l| chip(l, None)),
                    )
                    .children(
                        layers
                            .iter()
                            .filter(|l| have_layer(l))
                            .map(|l| chip(l, Some(have_tag.clone()))),
                    ),
            )
    }

    /// The status word at a tile's top right.
    fn tile_badge(label: &str, color: gpui::Hsla) -> Div {
        div()
            .absolute()
            .top(px(10.))
            .right(px(10.))
            .px(px(7.))
            .py(px(5.))
            .bg(bg3a(0.75))
            .child(status_text(label, color))
    }

    fn home_modes(&self, cx: &mut Context<Self>) -> Div {
        let customs = self.home_customs(cx);
        let tiles =
            catalog::MODES.iter().enumerate().map(|(i, d)| {
                let on = i == self.mode_sel;
                let status = self.status(i);
                // Only what needs a look: not installed is the normal start, and installing has its
                // bar.
                let badge = match status {
                    Status::Update | Status::Running | Status::Failed => {
                        Some(Self::tile_badge(&t(status.key()), status.color()))
                    }
                    _ => None,
                };
                let bar = match self.state(i) {
                    crate::backend::ModeState::Installing(p) => Some(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .child(thin_progress(
                                self.eased_bar(format!("tile-{i}"), super::play::bar_fraction(p)),
                                2.,
                            )),
                    ),
                    _ => None,
                };
                div()
                    .id(("home-mode", i))
                    .group("home-tile")
                    .relative()
                    .flex_1()
                    .min_w(px(0.))
                    .h(px(116.))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.select_mode(i, cx)))
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        this.hover_tile(i, *hovered, cx)
                    }))
                    .child(overlay(tile_bg()))
                    // The art zooms inside its own clip; the selection ring sits outside the tile.
                    .child({
                        let z = self.tile_zoom(i);
                        div().absolute().size_full().overflow_hidden().child(
                            art_fill(card_path(d.art))
                                .left(relative(-z / 2.))
                                .top(relative(-z / 2.))
                                .w(relative(1. + z))
                                .h(relative(1. + z)),
                        )
                    })
                    .when(!on, |d| d.child(overlay(bg3a(0.5))))
                    .child(shade_up(0.9, 0.0, 0.7))
                    .children(badge)
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
                                    .child(d.short),
                            )
                            .child(
                                plex(11., FontWeight::MEDIUM)
                                    .mt(px(6.))
                                    .text_color(muted3())
                                    .child(d.mode.minecraft_version()),
                            ),
                    )
                    .children(bar)
                    .child(if on { ring() } else { hover_ring("home-tile") })
            });
        let shared = demo::get().map(|d| tf("v3.modes_shared", &[("size", &d.disk_saved)]));
        div()
            .absolute()
            .left(px(64.))
            .right(px(48.))
            .bottom(px(36.))
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(match customs {
                Some(links) => div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_end()
                    .child(section_head(&t("v3.modes")))
                    .child(links),
                None => Self::band_head(&t("v3.modes"), shared.as_deref()),
            })
            .child(div().flex().flex_row().gap(px(14.)).children(tiles))
    }

    /// "Your instances": a link to each custom instance (up to three, then "+N"), at the right
    /// of the Modes head. `None` without custom instances.
    fn home_customs(&self, cx: &mut Context<Self>) -> Option<Div> {
        let list = &self.settings.custom_instances;
        if list.is_empty() {
            return None;
        }
        const SHOWN: usize = 3;
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(18.))
            .child(label(&t("home.customs")));
        for c in list.iter().take(SHOWN) {
            let id = c.id;
            row = row.child(
                tertiary_in(("home-custom", id.0 as usize), &c.name, muted3()).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.select_instance(super::instances::InstKey::Custom(id), cx);
                        this.navigate(Page::Instances, cx);
                    }),
                ),
            );
        }
        if list.len() > SHOWN {
            row = row.child(
                tertiary_in(
                    "home-custom-more",
                    &format!("+{}", list.len() - SHOWN),
                    muted3(),
                )
                .on_click(cx.listener(|this, _, _, cx| this.navigate(Page::Instances, cx))),
            );
        }
        Some(row)
    }

    /// The status row under the description: the status, the mod count when known, and one
    /// mode-specific extra. The version is in the eyebrow and the download size under the
    /// button, so neither repeats here.
    fn status_row(&self, mi: usize) -> Div {
        let mode = self.mode(mi);
        let status = self.status(mi);
        let info = &self.modes[mi].info;
        let demo = demo::get();
        let mods: Option<SharedString> = info
            .mods
            .filter(|_| self.installed(mi))
            .map(|n| tn("v3.mods_checked", u64::from(n)));
        // Another 1.16.1 mode is installed: this one isn't a fresh download.
        let (label, color) = if self.shares_install(mi) {
            (t("v3.status.shared"), status.color())
        } else {
            (t(status.key()), status.color())
        };
        let extra: Option<SharedString> = match status {
            Status::Installing | Status::Failed => None,
            _ => match (mode, demo, self.account.active_name()) {
                // Who plays: a Microsoft account plays every mode (`account::play_as`).
                (_, None, Some(name)) => Some(tf("v3.plays_as", &[("name", name)])),
                (Mode::Ranked, Some(d), _) => Some(tf("v3.online", &[("n", &d.online)])),
                (Mode::Ranked | Mode::Draftout, None, None) => Some(t("v3.online_needs_ms")),
                (_, Some(d), _) if status != Status::Missing => {
                    d.key_tools.get(mode.id()).map(|s| s.clone().into())
                }
                _ => None,
            },
        };
        div()
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
                    .child(div().flex_none().size(px(6.)).mr(px(8.)).bg(color))
                    .child(status_text(&label, color)),
            )
            .children(mods.map(|m| {
                inter(12., FontWeight::MEDIUM, 1.)
                    .text_color(faint3())
                    .child(m)
            }))
            .children(extra.map(|e| {
                inter(12., FontWeight::MEDIUM, 1.)
                    .text_color(faint3())
                    .child(e)
            }))
    }

    /// The eyebrow over a mode's title. Ranked names its season only when we know it (demo).
    pub fn eyebrow(&self, mode: Mode) -> SharedString {
        let mc = mode.minecraft_version();
        match (mode, demo::get()) {
            (Mode::Ranked, Some(d)) => tf(
                "v3.eyebrow.ranked",
                &[("season", &d.season.to_string()), ("mc", mc)],
            ),
            (Mode::Ranked, None) => tf("v3.eyebrow.ranked_offline", &[("mc", mc)]),
            _ => tf(&format!("v3.eyebrow.{}", mode.id()), &[("mc", mc)]),
        }
    }

    pub fn render_home(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mi = self.mode_sel;
        let mode = self.mode(mi);
        let d = catalog::def(mode);
        let status = self.status(mi);
        let fade = self.mode_fade();

        let hero = div()
            .absolute()
            .left(px(64.))
            .top(px(116. + fade.rise))
            .when(fade.text < 1.0, |d| d.opacity(fade.text))
            .w(px(560.))
            .flex()
            .flex_col()
            .child(eyebrow(&self.eyebrow(mode), accent2()))
            .child(tight(d.short, 112., FontWeight::EXTRA_BOLD, -0.03, 0.86, text3()).mt(px(16.)))
            .child(
                inter(16., FontWeight::NORMAL, 1.5)
                    .mt(px(20.))
                    .whitespace_nowrap()
                    .text_color(muted3())
                    .child(wrap(
                        &super::play::mode_desc(mode),
                        INTER,
                        FontWeight::NORMAL,
                        16.,
                        440.,
                    )),
            )
            .child(self.status_row(mi))
            .child(
                div()
                    .mt(px(30.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(28.))
                    .child(self.mode_play(mi, "home-play", 288., 64., 19., cx))
                    .children(self.play_ghosts(mi, "home", false, cx)),
            )
            .children(self.fail_line(mi, 560., cx))
            .children(self.mods_line(mi, 560.));

        let band: Option<Div> = match status {
            Status::Missing | Status::Installing => Some(self.home_install(mi)),
            _ if mode == Mode::Ranked => self.home_matches(),
            _ => self.home_strip(mode),
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
            .children(
                self.home_right(mode)
                    .filter(|_| !matches!(status, Status::Missing | Status::Installing))
                    .map(|r| {
                        // The numbers follow the hero a beat later.
                        let late = (fade.text * 1.25 - 0.25).max(0.0);
                        div()
                            .absolute()
                            .top(px(fade.rise * 0.6))
                            .size_full()
                            .when(late < 1.0, |d| d.opacity(late))
                            .child(r)
                    }),
            )
            .children(band.map(|b| {
                div()
                    .absolute()
                    .left(px(64.))
                    .right(px(48.))
                    .top(px(494. + fade.rise * 0.6))
                    .when(fade.text < 1.0, |d| d.opacity(fade.text))
                    .child(b)
            }))
            .child(self.home_modes(cx))
            .into_any_element()
    }
}
