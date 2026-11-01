//! Stats in the v3 layout, from the real data (`Launcher::stats`, a `StatsView`):
//! - the PB of every mode as a strip of Home's mode tiles: the run modes from SpeedRunIGT records
//!   (with the sum of best), Ranked's best time this season from the MCSR Ranked API (else the
//!   Ranked practice PB), Draftout's elo from the Draftout API. A tile with runs lists only that
//!   mode's runs when clicked;
//! - the recent runs that reached at least the Nether, with their splits against the PB of their
//!   category, and quiet notes for what is left out (resets before the Nether, runs with cheats
//!   or LAN, files that could not be read);
//! - the Ranked column: elo, tier, season record, recent matches and the weekly race.
//!
//! Every state is drawn in place, never as a modal: a skeleton while the first scan runs or the
//! first answer is awaited, the per-mode empty text, "updated 3 h ago" and "offline" on cached
//! API data, one line for each API error. The copy says only what srl-stats can back (its "For
//! the UI" section): runs from this launcher's instances, IGT as SpeedRunIGT has it.
use std::time::Duration;

use chrono::{Datelike, Local, TimeZone};
use gpui::{
    AnyElement, Context, Div, FontWeight, ParentElement, SharedString, Styled, Window, div,
    prelude::*, px,
};
use srl_core::Mode;
use srl_stats::{
    ApiError, Cached, LocalStats, MatchKind, Outcome, RankedOverview, Run, RunHistory, SplitKind,
};

use crate::{
    app::{Launcher, Page},
    assets::card_path,
    backend::stats::Lookup,
    catalog,
    copy::{t, tf, tn},
    v3::*,
};

use super::{field::Field, practice::page_title};

/// The run table's fixed columns; the splits share what is left.
const COL_DATE: f32 = 60.;
const COL_MODE: f32 = 70.;
const COL_RESULT: f32 = 150.;
const RUN_H: f32 = 44.;
/// Rows of recent runs that fit under the tiles.
const MAX_ROWS: usize = 7;
/// The Ranked column on the right, and the gap before it.
const RANKED_W: f32 = 300.;
const RANKED_GAP: f32 = 36.;
/// The split columns of the run table, in route order (bastion and fortress by name, whichever
/// came first).
const SPLITS: [SplitKind; 6] = [
    SplitKind::Nether,
    SplitKind::Bastion,
    SplitKind::Fortress,
    SplitKind::Blind,
    SplitKind::Stronghold,
    SplitKind::End,
];

// ---------------------------------------------------------------------------------------------
// Formatting, shared with Home
// ---------------------------------------------------------------------------------------------

/// A final time: "14:32.481", or "1:02:03.004" past an hour.
pub fn fmt_igt(ms: u64) -> String {
    let (h, m, s, milli) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}.{milli:03}")
    } else {
        format!("{m}:{s:02}.{milli:03}")
    }
}

/// A split or segment, to the second: "2:31".
pub fn fmt_split(ms: u64) -> String {
    let s = ms / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// A difference to the PB, to the second, with its sign: "+0:08", "−0:09" (a real minus).
pub fn fmt_delta(ms: i64) -> String {
    let sign = if ms < 0 { '−' } else { '+' };
    format!("{sign}{}", fmt_split(ms.unsigned_abs()))
}

/// A short local date: "Oct 24", "24 paź".
pub fn fmt_date(ms: i64) -> SharedString {
    match Local.timestamp_millis_opt(ms).earliest() {
        Some(d) => tf(
            "date.short",
            &[
                ("month", &t(&format!("date.m{}", d.month()))),
                ("day", &d.day().to_string()),
            ],
        ),
        None => "—".into(),
    }
}

/// Epoch ms of the local midnight that started today, and of the first of this month.
pub fn today_and_month_ms() -> (i64, i64) {
    let now = Local::now();
    let at = |d: chrono::NaiveDate| {
        d.and_hms_opt(0, 0, 0)
            .and_then(|t| Local.from_local_datetime(&t).earliest())
            .map(|t| t.timestamp_millis())
            .unwrap_or(0)
    };
    let today = now.date_naive();
    (at(today), at(today.with_day(1).unwrap_or(today)))
}

/// "Updated 3 h ago", or with `short` "3 h ago" (where a source or a title sits next to it).
/// Drawn when the page renders: an idle page draws nothing, so the label is as old as the last
/// thing the player did, like every other number on it.
pub fn fmt_age(age: Duration, short: bool) -> SharedString {
    let mins = age.as_secs() / 60;
    let key = |k: &str| format!("stats.{}.{k}", if short { "ago" } else { "age" });
    match mins {
        0 => t(&key("now")),
        1..60 => tf(&key("min"), &[("n", &mins.to_string())]),
        60..1440 => tf(&key("h"), &[("n", &(mins / 60).to_string())]),
        _ => tn(&key("days"), mins / 1440),
    }
}

/// What to say next to an age when refreshing failed: "offline", "try again in 40 s".
fn refresh_note(e: &ApiError) -> Option<(SharedString, gpui::Hsla)> {
    match e {
        ApiError::Offline(_) => Some((t("stats.offline"), behind3())),
        ApiError::RateLimited { retry_after_s } => Some((
            tf("stats.try_again", &[("n", &retry_after_s.to_string())]),
            muted3(),
        )),
        ApiError::Http(_) | ApiError::BadResponse(_) => Some((t("stats.api_error"), behind3())),
        ApiError::NotRefreshed | ApiError::NotFound | ApiError::InvalidName => None,
    }
}

/// The age line of cached API data, when it is past its cache time: "Updated 3 h ago" (`short`:
/// "3 h ago") and "offline" after it. `None` while the data is current.
pub fn age_line<T>(c: &Cached<T>, short: bool) -> Option<Div> {
    let e = c.refresh_error.as_ref()?;
    Some(
        div()
            .flex()
            .flex_row()
            .gap(px(6.))
            .child(
                inter(12., FontWeight::MEDIUM, 1.)
                    .whitespace_nowrap()
                    .text_color(muted3())
                    .child(fmt_age(c.age(), short)),
            )
            .children(refresh_note(e).map(|(s, color)| {
                inter(12., FontWeight::MEDIUM, 1.)
                    .whitespace_nowrap()
                    .text_color(color)
                    .child(s)
            })),
    )
}

/// The copy key of a standard split's name.
pub fn split_name(kind: SplitKind) -> SharedString {
    t(match kind {
        SplitKind::Nether => "stats.split.nether",
        SplitKind::Bastion => "stats.split.bastion",
        SplitKind::Fortress => "stats.split.fortress",
        SplitKind::Blind => "stats.split.blind",
        SplitKind::Stronghold => "stats.split.stronghold",
        SplitKind::End => "stats.split.end",
        SplitKind::Dragon => "stats.split.dragon",
    })
}

/// The tier's copy key and colour name ("diamond").
pub fn tier_id(tier: srl_stats::Tier) -> String {
    tier.name().to_ascii_lowercase()
}

/// The PB of the run's own category (category and run type), if it has one.
fn pb_of<'a>(h: &'a RunHistory, run: &Run) -> Option<&'a Run> {
    h.category(&run.record.category, &run.record.run_type)
        .and_then(|c| c.pb.as_ref())
}

/// A run worth a row: it reached a split (or finished). Resets in the overworld are most of
/// a random-seed player's runs and say nothing; they are counted in a note instead.
fn listed(run: &Run) -> bool {
    run.record.completed || SPLITS.iter().any(|k| run.record.split(*k).is_some())
}

/// The season of the newest ranked match, the only place the API names it.
pub fn ranked_season(o: &RankedOverview) -> Option<u32> {
    o.matches
        .as_ref()?
        .value
        .iter()
        .find(|m| m.kind == MatchKind::Ranked)?
        .season
}

/// A dim block standing in for text that is still loading.
fn bone(w: f32, h: f32) -> Div {
    div().flex_none().w(px(w)).h(px(h)).bg(text3a(0.06))
}

/// A band header: a section head on the left and its source on the right.
fn band_head(left: &str, right: &str) -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .justify_between()
        .child(section_head(left))
        .child(label(right))
}

/// A table header cell: Inter Medium 11, `faint` (smaller than `label`, so "Stronghold" fits a
/// split column with room to spare).
fn head_cell(s: &str) -> Div {
    inter(11., FontWeight::MEDIUM, 1.)
        .whitespace_nowrap()
        .text_color(faint3())
        .child(SharedString::from(s.to_string()))
}

/// A quiet note: Inter 12, `faint`.
fn note(s: &str, w: f32) -> Div {
    inter(12., FontWeight::NORMAL, 1.4)
        .whitespace_nowrap()
        .text_color(faint3())
        .child(wrap(s, INTER, FontWeight::NORMAL, 12., w))
}

/// What a PB tile shows at its bottom.
enum TileBody {
    /// The first scan or the first answer is still coming.
    Loading,
    /// A big value over a caps label.
    Value(String, SharedString),
    /// A big value from a cache that could not be refreshed, over its age line: the line is too
    /// wide for the tile's corner next to the title.
    Aged(String, Box<Div>),
    /// A line of text (no runs, ask for a name, an error).
    Text(SharedString),
}

impl Launcher {
    pub fn render_stats(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let tiles = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(section_head(&t("stats.pbs")))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(14.))
                    .children(Mode::ALL.into_iter().map(|m| self.pb_tile(m, cx))),
            );
        div()
            .size_full()
            .relative()
            .child(super::practice::art_band("the-end"))
            .child(
                div()
                    .absolute()
                    .left(px(64.))
                    .right(px(48.))
                    .top(px(104.))
                    .bottom(px(28.))
                    .flex()
                    .flex_col()
                    .child(page_title(&t("stats.title"), ""))
                    .child(tiles.mt(px(30.)))
                    .child(
                        div()
                            .mt(px(30.))
                            .flex_1()
                            .min_h(px(0.))
                            .flex()
                            .flex_row()
                            .gap(px(RANKED_GAP))
                            .child(self.runs_panel(cx))
                            .child(self.ranked_column(cx)),
                    ),
            )
            .into_any_element()
    }

    /// Opens Settings with the cursor in the Ranked or Draftout name field.
    pub fn edit_stats_name(&mut self, which: Field, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(Page::Settings, cx);
        let focus = match which {
            Field::Draftout => self.draftout_field.focus.clone(),
            _ => self.ranked_field.focus.clone(),
        };
        window.focus(&focus, cx);
    }

    // -----------------------------------------------------------------------------------------
    // PB tiles
    // -----------------------------------------------------------------------------------------

    /// One mode's tile: the PB (or Ranked's best time, Draftout's elo), the sum of best in gold
    /// at the top right. Clicking a mode with runs lists only its runs; Draftout without a name
    /// opens Settings.
    fn pb_tile(&self, mode: Mode, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
        let def = catalog::def(mode);
        let local = self.stats.local.as_deref();
        let history = local.map(|l| l.mode(mode));
        let pb_body = |h: &RunHistory| match h.headline_pb() {
            Some(pb) => TileBody::Value(
                fmt_igt(pb.record.igt_ms),
                tf("v3.pb", &[("date", &fmt_date(pb.record.date_ms))]),
            ),
            None if h.run_count > 0 => {
                TileBody::Text(tn("stats.runs_none_finished", u64::from(h.run_count)))
            }
            None => TileBody::Text(t("stats.no_runs")),
        };
        let mut corner: Option<Div> = None;
        let body = match mode {
            Mode::Draftout => self.draftout_body(&mut corner),
            Mode::Ranked => {
                let api_best = self
                    .stats
                    .ranked
                    .as_ref()
                    .and_then(|l| l.value.as_ref())
                    .and_then(|o| o.profile.value.season.best_time_ms);
                match (api_best, history) {
                    (Some(ms), _) => TileBody::Value(fmt_igt(ms), t("stats.best_match")),
                    (None, None) => TileBody::Loading,
                    (None, Some(h)) => match h.headline_pb() {
                        Some(pb) => TileBody::Value(
                            fmt_igt(pb.record.igt_ms),
                            tf(
                                "stats.practice_pb",
                                &[("date", &fmt_date(pb.record.date_ms))],
                            ),
                        ),
                        None => pb_body(h),
                    },
                }
            }
            _ => match history {
                None => TileBody::Loading,
                Some(h) => {
                    let sob = h.headline_pb().and_then(|pb| pb_category_sob(h, pb));
                    corner = sob.map(|ms| {
                        plex(11., FontWeight::MEDIUM)
                            .text_color(gold())
                            .child(tf("stats.sob_short", &[("time", &fmt_igt(ms))]))
                    });
                    pb_body(h)
                }
            },
        };
        let lit = matches!(body, TileBody::Value(..) | TileBody::Aged(..));
        // The Draftout tile asks for a name: its line is the action, in the link colour.
        let ask_name = mode == Mode::Draftout && self.stats.draftout.is_none();
        let bottom = match body {
            TileBody::Loading => div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(bone(110., 20.))
                .child(bone(140., 9.)),
            TileBody::Value(value, label) => div()
                .flex()
                .flex_col()
                .child(spaced(
                    &value,
                    PLEX,
                    FontWeight::SEMIBOLD,
                    22.,
                    -0.01,
                    1.,
                    text3(),
                ))
                .child(
                    inter(12., FontWeight::MEDIUM, 1.)
                        .mt(px(8.))
                        .whitespace_nowrap()
                        .text_color(muted3())
                        .child(label),
                ),
            TileBody::Aged(value, line) => div()
                .flex()
                .flex_col()
                .child(spaced(
                    &value,
                    PLEX,
                    FontWeight::SEMIBOLD,
                    22.,
                    -0.01,
                    1.,
                    text3(),
                ))
                .child((*line).mt(px(5.))),
            TileBody::Text(s) => inter(13., FontWeight::MEDIUM, 1.25)
                .whitespace_nowrap()
                .text_color(if ask_name { accent2() } else { muted3() })
                .child(wrap_even(&s, INTER, FontWeight::MEDIUM, 13., 190.)),
        };
        let has_runs = history.is_some_and(|h| h.run_count > 0);
        let selected = self.stats_mode == Some(mode);
        let clickable = (has_runs && mode != Mode::Draftout) || ask_name;
        let group: SharedString = format!("stats-tile-{}", mode.id()).into();
        div()
            .id(("stats-tile", crate::backend::index(mode)))
            .group(group.clone())
            .relative()
            .flex_1()
            .min_w(px(0.))
            .h(px(96.))
            .child(overlay(tile_bg()))
            .child(art_fill(card_path(def.art)))
            .child(overlay(bg3a(if lit { 0.2 } else { 0.6 })))
            .child(shade_up(0.92, 0.1, 0.85))
            .child(
                arch(15., FontWeight::BOLD)
                    .absolute()
                    .left(px(14.))
                    .top(px(12.))
                    .text_color(text3())
                    .child(def.short),
            )
            .children(corner.map(|c| div().absolute().right(px(12.)).top(px(13.)).child(c)))
            .child(div().absolute().left(px(14.)).bottom(px(12.)).child(bottom))
            .when(clickable, |d| {
                d.cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if ask_name {
                            this.edit_stats_name(Field::Draftout, window, cx);
                        } else {
                            this.stats_mode = if this.stats_mode == Some(mode) {
                                None
                            } else {
                                Some(mode)
                            };
                            cx.notify();
                        }
                    }))
                    .child(if selected { ring() } else { group_ring(group) })
            })
    }

    /// Draftout's tile from the Draftout API: elo and rank band, or why there is none. `corner`
    /// gets the source. Cached data that could not be refreshed shows its age under the elo.
    fn draftout_body(&self, corner: &mut Option<Div>) -> TileBody {
        let Some(l) = &self.stats.draftout else {
            return TileBody::Text(t("stats.draftout_set_name"));
        };
        match (&l.value, &l.error) {
            (Some(c), _) => {
                let p = &c.value.profile;
                *corner = Some(
                    plex(11., FontWeight::MEDIUM)
                        .text_color(faint3())
                        .child(t("stats.draftout_src")),
                );
                let Some(elo) = p.elo else {
                    return TileBody::Text(t("stats.draftout_no_elo"));
                };
                if let Some(line) = age_line(c, true) {
                    return TileBody::Aged(elo.to_string(), Box::new(line));
                }
                let label = if p.ranked {
                    tf("stats.draftout_elo", &[("rank", &p.rank_name)])
                } else {
                    t("stats.draftout_unranked")
                };
                TileBody::Value(elo.to_string(), label)
            }
            (None, Some(e)) => TileBody::Text(api_error_line(e, &l.name, false)),
            (None, None) => TileBody::Loading,
        }
    }

    // -----------------------------------------------------------------------------------------
    // Recent runs
    // -----------------------------------------------------------------------------------------

    fn runs_panel(&self, _cx: &mut Context<Self>) -> Div {
        let panel = div().flex_1().min_w(px(0.)).flex().flex_col();
        let scope = self.stats_mode;
        let left = match scope {
            Some(m) => tf("stats.recent_mode", &[("mode", catalog::def(m).short)]),
            None => t("stats.recent"),
        };
        let Some(local) = self.stats.local.as_deref() else {
            // The first scan: the table's shape, dimmed, and what is happening.
            return panel
                .child(band_head(&left, &t("stats.recent_src")))
                .child(self.runs_header().mt(px(6.)))
                .children((0..5).map(|i| {
                    div()
                        .flex_none()
                        .h(px(RUN_H))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(24.))
                        .border_t_1()
                        .border_color(line3())
                        .child(bone(44., 10.))
                        .child(bone(50., 12.))
                        .child(bone(90. + (i % 2) as f32 * 30., 12.))
                }))
                .child(note(&t("stats.scanning"), 560.).mt(px(14.)));
        };
        let modes: Vec<Mode> = match scope {
            Some(m) => vec![m],
            None => Mode::ALL.to_vec(),
        };
        let histories: Vec<(Mode, &RunHistory)> =
            modes.iter().map(|m| (*m, local.mode(*m))).collect();
        let total: u32 = histories.iter().map(|(_, h)| h.run_count).sum();
        // What the split columns mean, once there are rows to read it against.
        let right: SharedString = match scope {
            Some(m) => counts_line(local.mode(m)),
            None if total == 0 => SharedString::default(),
            None => t("stats.recent_src"),
        };
        let panel = panel.child(band_head(&left, &right));
        if total == 0 {
            let (title, body) = match scope {
                Some(_) => (t("stats.no_runs"), t("stats.empty_mode")),
                None => (t("stats.no_runs"), t("stats.empty_body")),
            };
            return panel
                .child(
                    inter(18., FontWeight::SEMIBOLD, 1.2)
                        .mt(px(26.))
                        .text_color(text3())
                        .child(title),
                )
                .child(
                    inter(14., FontWeight::NORMAL, 1.45)
                        .mt(px(8.))
                        .whitespace_nowrap()
                        .text_color(muted3())
                        .child(wrap(&body, INTER, FontWeight::NORMAL, 14., 520.)),
                )
                .children(self.local_notes(local, &histories).map(|(_, d)| d));
        }
        // The newest listed runs over the modes in scope (each mode's runs are newest first). Notes
        // past the second take a row's place, so the table never runs off the window.
        let notes = self.local_notes(local, &histories);
        let max_rows = MAX_ROWS - notes.as_ref().map_or(0, |(n, _)| n.saturating_sub(2));
        let mut rows: Vec<(&RunHistory, &Run)> = histories
            .iter()
            .flat_map(|(_, h)| {
                h.runs
                    .iter()
                    .filter(|r| listed(r))
                    .take(max_rows)
                    .map(move |r| (*h, r))
            })
            .collect();
        rows.sort_by_key(|(_, r)| std::cmp::Reverse(r.record.date_ms));
        rows.truncate(max_rows);
        let mut panel = panel.child(self.runs_header().mt(px(6.)));
        if rows.is_empty() {
            panel = panel.child(
                inter(14., FontWeight::NORMAL, 1.45)
                    .mt(px(14.))
                    .text_color(muted3())
                    .child(t("stats.no_split_runs")),
            );
        }
        panel
            .children(rows.into_iter().map(|(h, r)| Self::run_row(h, r)))
            .children(notes.map(|(_, d)| d))
    }

    fn runs_header(&self) -> Div {
        div()
            .flex_none()
            .h(px(28.))
            .flex()
            .flex_row()
            .items_center()
            .child(
                div()
                    .flex_none()
                    .w(px(COL_DATE))
                    .child(head_cell(&t("stats.col_date"))),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(COL_MODE))
                    .child(head_cell(&t("stats.col_mode"))),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(COL_RESULT))
                    .child(head_cell(&t("stats.col_result"))),
            )
            .children(SPLITS.iter().map(|k| {
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .justify_end()
                    .child(head_cell(&split_name(*k)))
            }))
    }

    /// One run: date, mode, the final time (PB badge, "not counted") or where it was reset, and
    /// each split with its difference to the PB of the run's category.
    fn run_row(h: &RunHistory, run: &Run) -> Div {
        let rec = &run.record;
        let pb = pb_of(h, run);
        let is_pb = pb.is_some_and(|p| p.id == run.id);
        let result: Div = if rec.completed {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(
                    plex(14., FontWeight::MEDIUM)
                        .text_color(text3())
                        .child(fmt_igt(rec.igt_ms)),
                )
                .when(is_pb, |d| {
                    d.child(
                        div().px(px(6.)).py(px(4.)).bg(accent3a(0.22)).child(
                            inter(11., FontWeight::BOLD, 1.)
                                .text_color(accent2())
                                .child(t("stats.pb")),
                        ),
                    )
                })
                .when(!rec.eligible(), |d| {
                    d.child(
                        inter(11., FontWeight::MEDIUM, 1.)
                            .whitespace_nowrap()
                            .text_color(faint3())
                            .child(t("stats.not_counted")),
                    )
                })
        } else {
            let furthest = SPLITS
                .iter()
                .filter_map(|k| rec.split(*k).map(|ms| (ms, *k)))
                .max()
                .map(|(_, k)| split_name(k))
                .unwrap_or_default();
            inter(13., FontWeight::NORMAL, 1.2)
                .text_color(muted3())
                .child(tf("stats.reset_at", &[("split", &furthest)]))
        };
        let pb_rec = pb.filter(|_| !is_pb).map(|p| &p.record);
        let split = |k: &SplitKind| {
            let cell = div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .items_end()
                .gap(px(4.));
            match rec.split(*k) {
                Some(ms) => {
                    let delta = pb_rec
                        .and_then(|p| p.split(*k))
                        .map(|p| ms as i64 - p as i64);
                    cell.child(
                        plex(13., FontWeight::MEDIUM)
                            .text_color(text3())
                            .child(fmt_split(ms)),
                    )
                    .children(delta.map(|d| {
                        plex(11., FontWeight::MEDIUM)
                            .text_color(if d < 0 { ahead3() } else { behind3() })
                            .child(fmt_delta(d))
                    }))
                }
                None => cell.child(
                    plex(13., FontWeight::MEDIUM)
                        .text_color(text3a(0.18))
                        .child("—"),
                ),
            }
        };
        div()
            .flex_none()
            .h(px(RUN_H))
            .flex()
            .flex_row()
            .items_center()
            .border_t_1()
            .border_color(line3())
            .child(
                plex(12., FontWeight::MEDIUM)
                    .flex_none()
                    .w(px(COL_DATE))
                    .text_color(faint3())
                    .child(fmt_date(rec.date_ms)),
            )
            .child(
                inter(13., FontWeight::SEMIBOLD, 1.2)
                    .flex_none()
                    .w(px(COL_MODE))
                    .text_color(text3())
                    .child(run.mode.map(|m| catalog::def(m).short).unwrap_or("—")),
            )
            .child(div().flex_none().w(px(COL_RESULT)).child(result))
            .children(SPLITS.iter().map(split))
    }

    /// The quiet notes under the table: resets not listed, finishes not counted, files and
    /// folders that could not be read, runs from elsewhere. With how many lines there are.
    fn local_notes(
        &self,
        local: &LocalStats,
        histories: &[(Mode, &RunHistory)],
    ) -> Option<(usize, Div)> {
        let mut lines: Vec<SharedString> = Vec::new();
        let hidden: usize = histories
            .iter()
            .map(|(_, h)| h.runs.iter().filter(|r| !listed(r)).count())
            .sum();
        if hidden > 0 {
            lines.push(tn("stats.hidden_resets", hidden as u64));
        }
        let excluded: u32 = histories
            .iter()
            .flat_map(|(_, h)| h.categories.iter())
            .map(|c| c.excluded_completions)
            .sum();
        if excluded > 0 {
            lines.push(tn("stats.excluded", u64::from(excluded)));
        }
        for (folder, _) in &local.unreadable {
            lines.push(tf(
                "stats.unreadable",
                &[("folder", &folder.display().to_string())],
            ));
        }
        if self.stats_mode.is_none() && local.unattributed.run_count > 0 {
            lines.push(tn(
                "stats.unattributed",
                u64::from(local.unattributed.run_count),
            ));
        }
        (!lines.is_empty()).then(|| {
            let n = lines.len();
            let d = div()
                .mt(px(12.))
                .pt(px(10.))
                .border_t_1()
                .border_color(line3())
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(lines.iter().map(|l| note(l, 800.)));
            (n, d)
        })
    }

    // -----------------------------------------------------------------------------------------
    // Ranked
    // -----------------------------------------------------------------------------------------

    fn ranked_column(&self, cx: &mut Context<Self>) -> Div {
        let col = div().flex_none().w(px(RANKED_W)).flex().flex_col();
        let settings_link = |id: &'static str, key: &str| {
            div()
                .mt(px(16.))
                .flex()
                .flex_row()
                .child(
                    tertiary(id, &t(key)).on_click(cx.listener(|this, _, window, cx| {
                        this.edit_stats_name(Field::Ranked, window, cx)
                    })),
                )
        };
        let Some(l) = &self.stats.ranked else {
            // No name: ask for it, and say which name it is.
            return col
                .child(section_head(&t("stats.ranked_plain")))
                .child(
                    inter(14., FontWeight::NORMAL, 1.45)
                        .mt(px(22.))
                        .whitespace_nowrap()
                        .text_color(muted3())
                        .child(wrap(
                            &t("stats.ranked_ask_body"),
                            INTER,
                            FontWeight::NORMAL,
                            14.,
                            RANKED_W,
                        )),
                )
                .child(settings_link("stats-ranked-set", "stats.to_settings"));
        };
        let Some(o) = &l.value else {
            let head = col.child(section_head(&t("stats.ranked_plain")));
            return match &l.error {
                Some(e) => {
                    let fix = matches!(e, ApiError::NotFound | ApiError::InvalidName);
                    head.child(
                        inter(14., FontWeight::NORMAL, 1.45)
                            .mt(px(22.))
                            .whitespace_nowrap()
                            .text_color(muted3())
                            .child(wrap(
                                &api_error_line(e, &l.name, true),
                                INTER,
                                FontWeight::NORMAL,
                                14.,
                                RANKED_W,
                            )),
                    )
                    .when(fix, |d| {
                        d.child(settings_link(
                            "stats-ranked-change",
                            "stats.change_in_settings",
                        ))
                    })
                }
                None => head
                    .child(
                        div()
                            .mt(px(18.))
                            .flex()
                            .flex_row()
                            .justify_between()
                            .child(bone(130., 40.))
                            .child(bone(80., 22.)),
                    )
                    .child(
                        div()
                            .mt(px(22.))
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .children((0..3).map(|_| bone(RANKED_W, 52.))),
                    )
                    .child(note(&t("stats.ranked_loading"), RANKED_W).mt(px(12.))),
            };
        };
        self.ranked_full(col, o)
    }

    /// The column with data: elo and tier, the season record, recent matches, the weekly race,
    /// then the source and, for cached data, its age.
    fn ranked_full(&self, col: Div, o: &RankedOverview) -> Div {
        let p = &o.profile.value;
        let season = ranked_season(o);
        let head = match season {
            Some(s) => tf("stats.ranked", &[("season", &s.to_string())]),
            None => t("stats.ranked_plain"),
        };
        let delta = o
            .matches
            .as_ref()
            .and_then(|m| m.value.iter().find(|m| m.kind == MatchKind::Ranked))
            .and_then(|m| m.elo_change);
        let (tier_label, tier_color) = match (p.elo, p.tier) {
            (Some(_), Some(tier)) => (
                tf(
                    "v3.r.elo",
                    &[("tier", &t(&format!("tier.{}", tier_id(tier))))],
                ),
                Some(tier_color(&tier_id(tier))),
            ),
            (Some(_), None) => (t("stats.elo"), None),
            (None, _) => (t("stats.placements"), None),
        };
        let elo = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_end()
                    .gap(px(8.))
                    .child(spaced(
                        &p.elo.map_or("—".to_string(), |e| e.to_string()),
                        PLEX,
                        FontWeight::SEMIBOLD,
                        40.,
                        -0.01,
                        1.,
                        text3(),
                    ))
                    .children(delta.filter(|_| p.elo.is_some()).map(|d| {
                        spaced(
                            &arrow(&format!("{d:+}")),
                            PLEX,
                            FontWeight::SEMIBOLD,
                            13.,
                            -0.01,
                            1.,
                            if d < 0 { behind3() } else { ahead3() },
                        )
                        .mb(px(4.))
                    })),
            )
            .child(
                div()
                    .mt(px(9.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .children(tier_color.map(|c| div().flex_none().size(px(7.)).mr(px(6.)).bg(c)))
                    .child(
                        inter(12., FontWeight::MEDIUM, 1.)
                            .whitespace_nowrap()
                            .text_color(muted3())
                            .child(tier_label),
                    ),
            );
        // No ranked match this season: the record would be last season's leftovers next to
        // "No ranked matches this season yet".
        let record = has_ranked_match(o).then(|| {
            div()
                .flex()
                .flex_col()
                .items_end()
                .child(spaced(
                    &format!("{} – {}", p.season.wins, p.season.losses),
                    PLEX,
                    FontWeight::SEMIBOLD,
                    22.,
                    -0.01,
                    1.,
                    text3(),
                ))
                .child(
                    inter(12., FontWeight::MEDIUM, 1.)
                        .mt(px(9.))
                        .whitespace_nowrap()
                        .text_color(muted3())
                        .child(match season {
                            Some(s) => tf("v3.r.season", &[("season", &s.to_string())]),
                            None => t("stats.season_record"),
                        }),
                )
        });
        let cells = ranked_cells(o, 3, false);
        // Stale data puts its age where the source goes: the column has no width for both, and
        // the age matters more once the numbers stop being live.
        let footer = div()
            .mt(px(12.))
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .child(age_line(&o.profile, true).unwrap_or_else(|| label(&t("v3.matches_src"))));
        col.child(section_head(&head))
            .child(
                div()
                    .mt(px(18.))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_end()
                    .child(elo)
                    .children(record),
            )
            .child(
                div()
                    .mt(px(22.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .children(cells),
            )
            .child(footer)
    }
}

/// Whether the cached matches have a ranked one: the test behind "No ranked matches this season
/// yet" (`ranked_cells`).
pub fn has_ranked_match(o: &RankedOverview) -> bool {
    o.matches
        .as_ref()
        .is_some_and(|m| m.value.iter().any(|m| m.kind == MatchKind::Ranked))
}

/// The sum of best of the PB's category, when every segment has a best.
pub fn pb_category_sob(h: &RunHistory, pb: &Run) -> Option<u64> {
    h.category(&pb.record.category, &pb.record.run_type)
        .and_then(|c| c.sum_of_best_ms)
}

/// "214 runs · 3 finished · 12 today" for one mode.
fn counts_line(h: &RunHistory) -> SharedString {
    let (today, _) = today_and_month_ms();
    let today_n = h.runs_since(today).count();
    let mut parts = vec![
        tn("stats.n_runs", u64::from(h.run_count)),
        tn("stats.n_finished", u64::from(h.completion_count)),
    ];
    if today_n > 0 {
        parts.push(tf("stats.n_today", &[("n", &today_n.to_string())]));
    }
    parts.join(" · ").into()
}

/// One line for an API error with no data to show. `ranked`: MCSR Ranked's wording, else
/// Draftout's (shorter, it goes into a tile).
pub fn api_error_line(e: &ApiError, name: &str, ranked: bool) -> SharedString {
    let key = match (e, ranked) {
        (ApiError::NotFound, true) => "stats.ranked_not_found",
        (ApiError::NotFound, false) => "stats.draftout_not_found",
        (ApiError::InvalidName, _) => "stats.invalid_name",
        (ApiError::Offline(_) | ApiError::NotRefreshed, true) => "stats.ranked_offline",
        (ApiError::Offline(_) | ApiError::NotRefreshed, false) => "stats.draftout_offline",
        (ApiError::RateLimited { retry_after_s }, _) => {
            return tf("stats.limited", &[("n", &retry_after_s.to_string())]);
        }
        (ApiError::Http(_) | ApiError::BadResponse(_), true) => "stats.ranked_error",
        (ApiError::Http(_) | ApiError::BadResponse(_), false) => "stats.draftout_error",
    };
    tf(key, &[("name", name)])
}

/// The match cells (Home's, stacked on Stats): up to `n` recent ranked matches, then the running
/// weekly race and the last one. `row`: Home's band (the cells share a row, the weekly race only
/// once).
pub fn ranked_cells(o: &RankedOverview, n: usize, row: bool) -> Vec<Div> {
    let cell = |border: gpui::Hsla| {
        let d = div()
            .h(px(52.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .px(px(12.))
            .bg(text3a(0.04))
            .border_l(px(3.))
            .border_color(border);
        if row {
            d.flex_1().min_w(px(0.))
        } else {
            d.flex_none()
        }
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
    let mut cells: Vec<Div> = Vec::new();
    let matches: Vec<_> = o
        .matches
        .as_ref()
        .map(|m| {
            m.value
                .iter()
                .filter(|m| m.kind == MatchKind::Ranked)
                .take(n)
                .collect()
        })
        .unwrap_or_default();
    for m in &matches {
        let time = fmt_igt(m.time_ms);
        let (color, sub) = match (m.outcome, m.forfeited) {
            (Outcome::Won, false) => (ahead3(), tf("v3.match_won", &[("time", &time)])),
            (Outcome::Won, true) => (ahead3(), t("stats.match_won_ff")),
            (Outcome::Lost, false) => (behind3(), tf("v3.match_lost", &[("time", &time)])),
            (Outcome::Lost, true) => (behind3(), t("stats.match_lost_ff")),
            (Outcome::Draw, _) => (faint3(), t("stats.match_draw")),
        };
        let name: SharedString = m
            .opponent
            .as_ref()
            .map(|p| p.nickname.clone())
            .unwrap_or_else(|| "—".into())
            .into();
        cells.push(
            cell(color).child(who(name, sub)).child(
                plex(13., FontWeight::MEDIUM)
                    .flex_none()
                    .text_color(color)
                    .child(
                        m.elo_change
                            .map_or("—".into(), |d| minus(&format!("{d:+}"))),
                    ),
            ),
        );
    }
    if matches.is_empty() && !row {
        cells.push(div().child(note(&t("stats.no_matches"), RANKED_W)));
    }
    if let Some(race) = &o.weekly_race {
        let top = race.value.leaderboard.len().to_string();
        let (sub, time) = match &o.my_weekly_race {
            Some(r) => (
                tf(
                    "stats.weekly_top",
                    &[("place", &r.rank.to_string()), ("n", &top)],
                ),
                Some(fmt_igt(r.time_ms)),
            ),
            None => (tf("stats.weekly_not_top", &[("n", &top)]), None),
        };
        cells.push(
            cell(gpui::transparent_black())
                .child(who(
                    tf("stats.weekly_now", &[("n", &race.value.id.to_string())]),
                    sub,
                ))
                .children(time.map(|t| {
                    plex(13., FontWeight::MEDIUM)
                        .flex_none()
                        .text_color(muted3())
                        .child(t)
                })),
        );
    }
    if let Some(last) = o.my_last_weekly_race.as_ref().filter(|_| !row) {
        cells.push(
            cell(gpui::transparent_black())
                .child(who(
                    tf("stats.weekly_last", &[("n", &last.race_id.to_string())]),
                    tf("stats.weekly_place", &[("place", &last.rank.to_string())]),
                ))
                .child(
                    plex(13., FontWeight::MEDIUM)
                        .flex_none()
                        .text_color(muted3())
                        .child(fmt_igt(last.time_ms)),
                ),
        );
    }
    cells
}

/// `hover_ring` for a group named at run time (one per tile, so only the hovered tile lights).
fn group_ring(group: SharedString) -> Div {
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

/// The lookup's age line, for Home.
pub fn ranked_age(l: &Lookup<RankedOverview>) -> Option<Div> {
    l.value.as_ref().and_then(|o| age_line(&o.profile, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(fmt_igt(872_481), "14:32.481");
        assert_eq!(fmt_igt(3_723_004), "1:02:03.004");
        assert_eq!(fmt_split(151_999), "2:31");
        assert_eq!(fmt_delta(8_400), "+0:08");
        assert_eq!(fmt_delta(-9_000), "−0:09");
        assert_eq!(fmt_delta(0), "+0:00");
    }

    #[test]
    fn ages_and_dates() {
        crate::copy::set_lang(crate::backend::Language::En);
        let age = |s| fmt_age(Duration::from_secs(s), false);
        assert_eq!(age(20), "Updated just now");
        assert_eq!(age(3 * 60), "Updated 3 min ago");
        assert_eq!(age(3 * 3600 + 5), "Updated 3 h ago");
        assert_eq!(age(86_400), "Updated 1 day ago");
        assert_eq!(fmt_age(Duration::from_secs(2 * 86_400), true), "2 days ago");
        let oct24 = Local
            .with_ymd_and_hms(2026, 10, 24, 20, 0, 0)
            .earliest()
            .unwrap()
            .timestamp_millis();
        assert_eq!(fmt_date(oct24), "Oct 24");
        let (today, month) = today_and_month_ms();
        assert!(month <= today && today <= srl_stats::cache::now_ms());
    }
}
