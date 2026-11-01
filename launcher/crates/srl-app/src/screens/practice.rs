//! Practice (REDESIGN.md "Practice"): the category grid and the selected category's map list (or
//! the practice tools), over the category's art.
//!
//! The maps come from srl-practice's catalogue (`catalog::maps`), their state from the practice
//! extension of the facade (`Backend::practice`): Install with the size, Play (which installs and
//! starts the practice instance), or the author's page for a map the launcher can't download. A
//! click on a row opens its details: the worlds it put in Singleplayer, Reset, Update and Remove
//! with the author's links on one row, one credit line, and "About this map" (closed until
//! clicked: what the map needs, the licence, the tools note). Reset, Update and Remove wait while
//! the practice game runs, and ask first (`Question`, drawn over the page). The list scrolls with
//! a fade at the edge where rows are hidden.
//!
//! The practice tools are a list only: installing them comes later. With `SRL_DEMO` the rows show
//! the prototype's download counts.
use gpui::{
    AnyElement, Context, Div, FontWeight, ObjectFit, ParentElement, SharedString, Styled, div, img,
    prelude::*, px,
};
use srl_core::Unit;
use srl_practice::{
    LinkOnlyReason, MapEntry, MapStatus, RequirementKind, Step, UpdateChoice, WorldState, WorldView,
};

use crate::{
    app::{Launcher, PracticeSel},
    assets::card_path,
    backend::{
        FailKind, ModeState,
        practice::{JobKind, MapFailKind, MapJob, MapView, Question},
    },
    catalog::{self, practice},
    copy::{size, t, t_opt, tf, tn},
    demo,
    v3::*,
};

use super::play::{bar_fraction, fail_detail, stage_key};

const GRID_GAP: f32 = 12.;
const ROW_H: f32 = 128.;
/// The list's width (the page between its 64 and 48 px margins), and where a row's text starts:
/// the details and progress line up with it. Rows have no thumbnail: in a category every row's
/// would be the category's own picture, shown on its tile just above.
const LIST_W: f32 = 1280. - 64. - 48.;
const INDENT: f32 = 0.;
const DETAIL_W: f32 = LIST_W - INDENT - 170.;
const DIALOG_W: f32 = 560.;

/// A practice tool row (they don't install yet).
struct ToolRow<'a> {
    name: &'a str,
    authors: SharedString,
    desc: SharedString,
    meta: String,
    /// The demo's install state ("installed", "update", "available"), `None` without the demo.
    demo_status: Option<&'a str>,
}

/// The copy key of a practice step.
fn step_key(step: Step) -> &'static str {
    match step {
        Step::Verifying => "practice.step.verifying",
        Step::Downloading => "practice.step.downloading",
        Step::Extracting => "practice.step.extracting",
    }
}

fn job_key(kind: JobKind) -> &'static str {
    match kind {
        JobKind::Install => "practice.working.install",
        JobKind::Update => "practice.working.update",
        JobKind::Reset => "practice.working.reset",
        JobKind::Remove => "practice.working.remove",
    }
}

/// The one line a failed change gets, by kind (srl-practice's wording, "For the UI").
fn fail_text(kind: MapFailKind, detail: &str) -> SharedString {
    match kind {
        MapFailKind::Network => t("practice.fail.network"),
        MapFailKind::SourceGone => t("practice.fail.source_gone"),
        MapFailKind::HashMismatch => t("practice.fail.hash"),
        MapFailKind::NotAWorld => t("practice.fail.not_a_world"),
        MapFailKind::WorldInUse => t("practice.fail.world_in_use"),
        MapFailKind::Other => tf("practice.fail.other", &[("text", detail)]),
    }
}

fn world_state_key(s: WorldState) -> &'static str {
    match s {
        WorldState::Fresh => "practice.world.fresh",
        WorldState::Played => "practice.world.played",
        WorldState::Missing => "practice.world.missing",
    }
}

/// "“A”, “B”" („A”, „B” in Polish): world names as the Singleplayer list shows them.
fn names(worlds: &[WorldView]) -> String {
    worlds
        .iter()
        .map(|w| quoted(&w.name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A name in the language's quotes.
fn quoted(name: &str) -> String {
    tf("quoted", &[("text", name)]).to_string()
}

/// "40% · 37 MB of 92 MB", or "120 of 7,251 files".
fn amount(done: u64, total: u64, unit: Unit) -> Option<SharedString> {
    (total > 0).then(|| {
        let pct = ((done as f64 / total as f64).min(1.) * 100.)
            .floor()
            .to_string();
        match unit {
            Unit::Bytes => tf(
                "v3.progress_bytes",
                &[
                    ("pct", &pct),
                    ("done", &size(done)),
                    ("total", &size(total)),
                ],
            ),
            Unit::Files => tf(
                "v3.progress_files",
                &[
                    ("pct", &pct),
                    ("done", &done.to_string()),
                    ("total", &total.to_string()),
                ],
            ),
        }
    })
}

/// A progress strip under a row: the bar, what it is doing, and an action (Cancel, Stop).
fn strip(fraction: f32, what: SharedString, action: Option<gpui::Stateful<Div>>) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(20.))
        .child(
            div()
                .flex_none()
                .w(px(240.))
                .child(thin_progress(fraction, 3.)),
        )
        .child(
            plex(11., FontWeight::MEDIUM)
                .text_color(muted3())
                .child(what),
        )
        .children(action)
}

/// A line with a coloured square in front (a failure, the game's state).
fn dot_line(color: gpui::Hsla, text: SharedString, w: f32) -> Div {
    div()
        .flex()
        .flex_row()
        .items_start()
        .child(
            div()
                .flex_none()
                .size(px(6.))
                .mt(px(6.))
                .mr(px(8.))
                .bg(color),
        )
        .child(
            inter(13., FontWeight::MEDIUM, 1.4)
                .whitespace_nowrap()
                .text_color(color)
                .child(wrap(&text, INTER, FontWeight::MEDIUM, 13., w)),
        )
}

impl Launcher {
    fn practice_art(cat: usize) -> &'static str {
        &practice().categories[cat].art
    }

    fn cat_name(cat: usize) -> SharedString {
        t(&format!("practice.cat.{}", practice().categories[cat].id))
    }

    fn practice_count(n: usize) -> SharedString {
        tn("v3.practice_count", n as u64)
    }

    /// A tile's sub line, wrapped like the browser at the tile's text width and clamped to two
    /// lines. The grid is 1.5fr 1fr 1fr 1fr over 1168 px with 12 px gaps, text inset 16 px.
    fn tile_sub(sub: &str, big: bool) -> SharedString {
        let fr = (1168. - 3. * GRID_GAP) / 4.5;
        let w = if big { 1.5 * fr } else { fr } - 32.;
        let wrapped = wrap(sub, INTER, FontWeight::MEDIUM, 12., w);
        let mut lines = wrapped.split('\n');
        match (lines.next(), lines.next()) {
            (Some(a), Some(b)) => format!("{a}\n{b}").into(),
            _ => wrapped.clone(),
        }
    }

    fn cat_tile(
        &self,
        sel: PracticeSel,
        title: SharedString,
        sub: SharedString,
        art: Option<&'static str>,
        big: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let on = self.practice_sel == sel;
        let id = match sel {
            PracticeSel::Category(i) => i,
            PracticeSel::Tools => 99,
        };
        div()
            .id(("practice-cat", id))
            .group("practice-cat")
            .relative()
            .size_full()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.practice_sel != sel {
                    this.practice_sel = sel;
                    // A new list starts at its top.
                    this.practice_scroll.set_offset(gpui::Point::default());
                    cx.notify();
                }
            }))
            .map(|d| match art {
                Some(a) => d
                    .child(overlay(tile_bg()))
                    .child(art_fill(card_path(a)))
                    .when(!on, |d| d.child(overlay(bg3a(0.34)))),
                None => d.child(
                    overlay(gpui::rgb(0x17131F).into())
                        .border_1()
                        .border_color(line3()),
                ),
            })
            // The Tools tile is flat: no dim, no gradient.
            .when(art.is_some(), |d| d.child(shade_up(0.9, 0.0, 0.65)))
            .child(
                div()
                    .absolute()
                    .left(px(16.))
                    .right(px(16.))
                    .bottom(px(14.))
                    .flex()
                    .flex_col()
                    .child(
                        arch(if big { 30. } else { 20. }, FontWeight::BOLD)
                            .text_color(text3())
                            .child(title),
                    )
                    .child(
                        inter(12., FontWeight::MEDIUM, 1.3)
                            .mt(px(6.))
                            .whitespace_nowrap()
                            .text_color(muted3())
                            .child(Self::tile_sub(&sub, big)),
                    ),
            )
            .child(if on {
                ring()
            } else {
                hover_ring("practice-cat")
            })
    }

    /// The name and authors, the description and the meta column of a row.
    fn row_body(name: &str, authors: SharedString, desc: SharedString, meta: String) -> Div {
        div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(20.))
            .child(
                div()
                    .flex_none()
                    .w(px(250.))
                    .flex()
                    .flex_col()
                    .child(
                        inter(15., FontWeight::SEMIBOLD, 1.2)
                            .text_color(text3())
                            .truncate()
                            .child(SharedString::from(name.to_string())),
                    )
                    .child(
                        inter(12., FontWeight::NORMAL, 1.35)
                            .text_color(muted3())
                            .truncate()
                            .child(authors),
                    ),
            )
            .child(
                inter(13., FontWeight::NORMAL, 1.35)
                    .flex_1()
                    .min_w(px(0.))
                    .text_color(muted3())
                    .child(desc),
            )
            .child(
                plex(12., FontWeight::MEDIUM)
                    .flex_none()
                    .w(px(130.))
                    .flex()
                    .justify_end()
                    .text_color(faint3())
                    .child(SharedString::from(meta)),
            )
    }

    fn tool_row(info: ToolRow<'_>) -> Div {
        let (label, color) = match info.demo_status {
            Some("installed") => (t("common.installed"), ok3()),
            Some("update") => (t("common.update"), text3()),
            Some(_) => (t("common.install"), accent2()),
            None => (t("practice.later"), faint3()),
        };
        div()
            .relative()
            .flex_none()
            .h(px(58.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(20.))
            .border_t_1()
            .border_color(line3())
            .child(Self::row_body(
                info.name,
                info.authors,
                info.desc,
                info.meta,
            ))
            .child(
                div()
                    .flex_none()
                    .w(px(150.))
                    .flex()
                    .justify_end()
                    .child(status_text(&label, color)),
            )
    }

    /// Whether the practice game is starting or running for map `id`.
    fn game_on(&self, id: &str) -> bool {
        self.practice.game_busy() && self.practice.game.map.as_deref() == Some(id)
    }

    /// The action cell of a map's row: what the map can do now, and a hint under it.
    fn map_action(
        &self,
        i: usize,
        e: &'static MapEntry,
        v: &MapView,
        cx: &mut Context<Self>,
    ) -> Div {
        let cell = div()
            .flex_none()
            .w(px(170.))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(5.));
        let hint = |s: SharedString| {
            inter(11., FontWeight::NORMAL, 1.)
                .whitespace_nowrap()
                .text_color(faint3())
                .child(s)
        };
        let size_hint =
            |s: SharedString| plex(10., FontWeight::MEDIUM).text_color(faint3()).child(s);
        if !self.practice.loaded {
            return cell;
        }
        let id = e.id.clone();
        // The row's progress line has the numbers.
        if let Some(job) = v.job {
            return cell.child(status_text(&t(job_key(job.kind)), faint3()));
        }
        match &v.status {
            // Its licence lets us link, not download: say so where Install would be.
            MapStatus::LinkOnly => cell
                .child(
                    tertiary(("practice-page", i), &t("practice.open_page"))
                        .on_click(open_url(e.source_page.clone(), cx)),
                )
                .child(
                    inter(11., FontWeight::MEDIUM, 1.)
                        .whitespace_nowrap()
                        .text_color(faint3())
                        .child(match e.link_only.as_ref().map(|l| l.reason) {
                            Some(LinkOnlyReason::OtherMinecraftVersion) => {
                                tf("practice.link_only_needs", &[("mc", &e.minecraft)])
                            }
                            _ => t("practice.link_only_hint"),
                        }),
                ),
            MapStatus::NotInstalled => cell
                .child(
                    secondary(("practice-install", i), &t("practice.install")).on_click(
                        cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.backend().practice().install(&id);
                        }),
                    ),
                )
                .children(
                    e.download
                        .as_ref()
                        .map(|d| size_hint(SharedString::from(size(d.size)))),
                ),
            // The player deleted every world: Install puts them back.
            MapStatus::Installed(m) if m.all_missing() => cell
                .child(
                    secondary(("practice-install", i), &t("practice.install")).on_click(
                        cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.backend().practice().install(&id);
                        }),
                    ),
                )
                .child(hint(t("practice.worlds_deleted"))),
            MapStatus::Installed(m) => {
                let game = &self.practice.game;
                if self.game_on(&e.id) {
                    let (label, color) = match game.state {
                        ModeState::Running => (t("practice.in_game"), ok3()),
                        _ => (t("practice.starting"), faint3()),
                    };
                    cell.child(status_dot(&label, color))
                } else if self.practice.game_busy() {
                    // One practice game at a time; its Singleplayer list has every map.
                    cell.child(secondary_off(&t("practice.play")))
                        .child(hint(t("practice.play_busy")))
                } else {
                    cell.child(
                        secondary(("practice-play", i), &t("practice.play")).on_click(cx.listener(
                            move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.practice_play(&id, cx);
                            },
                        )),
                    )
                    // Where the map is in the game is in the details ("In the Singleplayer
                    // list"), so only an update needs a word here.
                    .children(
                        m.update
                            .as_ref()
                            .map(|new| hint(tf("practice.update_hint", &[("v", new)]))),
                    )
                }
            }
        }
    }

    /// Play on a map: the name prompt first if there is no name yet (as for a mode), then the
    /// practice game. The page stays: the row says where to find the map in the game.
    fn practice_play(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.settings.player_name.is_none() {
            self.navigate(crate::app::Page::Name, cx);
            return;
        }
        self.practice_open = Some(id.to_string());
        self.practice_reveal = true;
        self.practice_stopping = false;
        self.backend().practice().play(id);
        cx.notify();
    }

    /// What goes under a map's row: the progress of a change or of the practice game, the game's
    /// state, the last failure, and the details when the row is open.
    fn map_below(
        &self,
        i: usize,
        e: &'static MapEntry,
        v: &MapView,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let mut parts: Vec<Div> = Vec::new();
        if let Some(job) = v.job {
            parts.push(self.job_strip(i, e, job, cx));
        }
        if self.practice.game.map.as_deref() == Some(e.id.as_str()) {
            parts.extend(self.game_strip(v, cx));
        }
        if let Some(f) = &v.failure
            && v.job.is_none()
        {
            parts.push(dot_line(
                behind3(),
                fail_text(f.kind, &f.detail),
                LIST_W - INDENT - 14.,
            ));
        }
        if self.practice_open.as_deref() == Some(e.id.as_str()) {
            parts.push(self.map_details(i, e, v, cx));
        }
        (!parts.is_empty()).then(|| {
            div()
                .pl(px(INDENT))
                .pb(px(14.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .children(parts)
        })
    }

    fn job_strip(
        &self,
        i: usize,
        e: &'static MapEntry,
        job: MapJob,
        cx: &mut Context<Self>,
    ) -> Div {
        let (fraction, what) = match job.progress {
            Some(p) => {
                let unit = if p.step == Step::Extracting {
                    Unit::Files
                } else {
                    Unit::Bytes
                };
                let step = t(step_key(p.step));
                let what: SharedString = match amount(p.done, p.total, unit) {
                    Some(a) => format!("{step} · {a}").into(),
                    None => step,
                };
                let f = if p.total > 0 {
                    p.done as f32 / p.total as f32
                } else {
                    0.
                };
                (f, what)
            }
            None => (0., t("practice.step.waiting")),
        };
        let id = e.id.clone();
        let cancel = job.cancellable().then(|| {
            tertiary(("practice-cancel", i), &t("v3.cancel")).on_click(cx.listener(
                move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.backend().practice().cancel(&id);
                },
            ))
        });
        strip(fraction, what, cancel)
    }

    /// The practice game's state on the row of the map it was started for.
    fn game_strip(&self, v: &MapView, cx: &mut Context<Self>) -> Option<Div> {
        let game = &self.practice.game;
        let stop = |label: SharedString, cx: &mut Context<Self>| {
            tertiary("practice-stop", &label).on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.practice_stopping = true;
                this.backend().practice().stop();
                cx.notify();
            }))
        };
        match &game.state {
            ModeState::Installing(p) => {
                let stage = t(stage_key(p.stage));
                let what: SharedString = match amount(p.done, p.total, p.unit) {
                    Some(a) => format!("{stage} · {a}").into(),
                    None => stage,
                };
                Some(div().flex().flex_col().gap(px(8.)).child(strip(
                    bar_fraction(p),
                    what,
                    Some(stop(t("v3.cancel"), cx)),
                )))
            }
            ModeState::Running => {
                let worlds = match &v.status {
                    MapStatus::Installed(m) => m
                        .worlds
                        .iter()
                        .filter(|w| w.state != WorldState::Missing)
                        .cloned()
                        .collect::<Vec<_>>(),
                    _ => vec![],
                };
                let line = if worlds.is_empty() {
                    t("practice.in_game_any")
                } else {
                    tf("practice.in_game_line", &[("world", &names(&worlds))])
                };
                Some(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(24.))
                        .child(arch(15., FontWeight::BOLD).text_color(ok3()).child(line))
                        .child(if self.practice_stopping {
                            div().child(tertiary_off(&t("game.stopping")))
                        } else {
                            div().child(stop(t("game.stop"), cx))
                        }),
                )
            }
            ModeState::Failed(f) => {
                let key = match (f.kind, f.installed) {
                    (FailKind::Offline, false) => "practice.game_fail.offline",
                    (FailKind::Offline, true) => "practice.game_fail.offline_update",
                    (FailKind::Install, _) => "practice.game_fail.install",
                    (FailKind::Launch, _) => "practice.game_fail.launch",
                    (FailKind::SignIn, _) => "practice.game_fail.sign_in",
                };
                let w = LIST_W - INDENT - 14.;
                let log = f.log.clone();
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(dot_line(behind3(), t(key), w))
                        .child(
                            plex(11., FontWeight::NORMAL)
                                .pl(px(14.))
                                .line_height(px(15.))
                                .text_color(faint3())
                                .child(wrap_paths(
                                    &fail_detail(&f.detail),
                                    PLEX,
                                    FontWeight::NORMAL,
                                    11.,
                                    w,
                                )),
                        )
                        .children(log.map(|l| {
                            div().pl(px(14.)).pt(px(6.)).flex().child(
                                tertiary("practice-log", &t("v3.open_log")).on_click(cx.listener(
                                    move |this, _, _, _| this.backend().open_containing_folder(&l),
                                )),
                            )
                        })),
                )
            }
            _ => None,
        }
    }

    /// An open row: what the player acts on (the worlds, Reset, Update and Remove, the author's
    /// links), one credit line, and "About this map" (what the map needs, the licence, the tools
    /// note), closed until it is clicked.
    fn map_details(
        &self,
        i: usize,
        e: &'static MapEntry,
        v: &MapView,
        cx: &mut Context<Self>,
    ) -> Div {
        let text = |s: SharedString| {
            inter(13., FontWeight::NORMAL, 1.45)
                .w(px(DETAIL_W))
                .text_color(muted3())
                .child(s)
        };
        let small = |s: SharedString| {
            inter(12., FontWeight::NORMAL, 1.45)
                .w(px(DETAIL_W))
                .text_color(faint3())
                .child(s)
        };
        let link = |n: usize, label: &str, url: &str, cx: &mut Context<Self>| {
            tertiary(("practice-link", i * 8 + n), label).on_click(open_url(url.to_string(), cx))
        };
        // A map the launcher only links to has the author's page as the row's action already.
        let mut links = Vec::new();
        if !matches!(v.status, MapStatus::LinkOnly) {
            links.push(link(0, &t("practice.open_page"), &e.source_page, cx));
        }
        if let Some(video) = &e.video {
            links.push(link(1, &t("practice.video"), video, cx));
        }
        for (n, s) in e.support.iter().enumerate().take(4) {
            links.push(link(2 + n, &s.label, &s.url, cx));
        }

        let mut col = div().flex().flex_col().gap(px(10.));
        let mut changes = None;
        if let MapStatus::Installed(m) = &v.status {
            let worlds = m
                .worlds
                .iter()
                .map(|w| format!("{} ({})", quoted(&w.name), t(world_state_key(w.state))))
                .collect::<Vec<_>>()
                .join(", ");
            // While the game runs for this map, its line above already names the worlds.
            let in_game = self.practice.game.map.as_deref() == Some(e.id.as_str())
                && matches!(self.practice.game.state, ModeState::Running);
            if !in_game {
                col = col.child(text(tf("practice.worlds", &[("worlds", &worlds)])));
            }
            if !m.handed_over.is_empty() {
                col = col.child(small(tf(
                    "practice.handed_over",
                    &[(
                        "worlds",
                        &m.handed_over
                            .iter()
                            .map(|w| quoted(w))
                            .collect::<Vec<_>>()
                            .join(", "),
                    )],
                )));
            }
            changes = Some(self.map_changes(i, e, m, v.job.is_some(), cx));
        }
        if let Some(l) = &e.link_only {
            let why = match l.reason {
                LinkOnlyReason::OtherMinecraftVersion => tf(
                    "practice.link.other-minecraft-version",
                    &[("mc", &e.minecraft)],
                ),
                LinkOnlyReason::NoDirectDownload => t("practice.link.no-direct-download"),
                LinkOnlyReason::AuthorOptOut => t("practice.link.author-opt-out"),
            };
            col = col.child(text(why));
        }
        // The changes, then the author's links after a hairline, on one row.
        let (changes, game_busy) = changes.unzip();
        let has_changes = changes.is_some();
        if has_changes || !links.is_empty() {
            col = col.child(
                div()
                    .mt(px(2.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(24.))
                    .children(changes)
                    .when(has_changes && !links.is_empty(), |d| {
                        d.child(div().flex_none().w(px(1.)).h(px(16.)).bg(line3()))
                    })
                    .children(links),
            );
        }
        col = col.children(game_busy.flatten());
        // The row names the authors; the credit line is there only when it says more.
        if let Some(credit) = t_opt(&format!("practice.credit.{}", e.id)) {
            col = col.child(small(credit));
        }

        // The small print, behind "About this map".
        let about_open = self.practice_about.as_deref() == Some(e.id.as_str());
        let id = e.id.clone();
        col = col.child(div().mt(px(2.)).flex().flex_row().child(
            disclosure(("practice-about", i), &t("practice.about"), about_open).on_click(
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.practice_about = if this.practice_about.as_deref() == Some(id.as_str()) {
                        None
                    } else {
                        Some(id.clone())
                    };
                    cx.notify();
                }),
            ),
        ));
        if about_open {
            let mut about = div().pl(px(19.)).flex().flex_col().gap(px(6.));
            for r in &e.requirements {
                // Mods and datapacks are things to add; settings and notes are advice.
                let mut tags = Vec::new();
                if matches!(r.kind, RequirementKind::Mod | RequirementKind::Datapack) {
                    if r.optional {
                        tags.push(t("practice.req.optional").to_string());
                    }
                    if !r.provided {
                        tags.push(t("practice.req.not_installed").to_string());
                    }
                }
                let line = t(&format!("practice.req.{}", r.id));
                let line: SharedString = if tags.is_empty() {
                    line
                } else {
                    format!("{line} ({})", tags.join(", ")).into()
                };
                about = about.child(small(line));
            }
            col = col.child(about);
        }
        col
    }

    /// Reset, Update and Remove for an installed map: live, or greyed while the practice game
    /// runs (it may have the world open) or a change is under way. The second part is the line
    /// that says why, while the game runs.
    fn map_changes(
        &self,
        i: usize,
        e: &'static MapEntry,
        m: &srl_practice::InstalledMap,
        working: bool,
        cx: &mut Context<Self>,
    ) -> (Div, Option<Div>) {
        let reset_label = if m.version != m.stored_version {
            tf("practice.reset_to", &[("v", &m.stored_version)])
        } else {
            t("practice.reset")
        };
        let mut labels: Vec<(SharedString, JobKind)> = vec![(reset_label, JobKind::Reset)];
        if let Some(new) = &m.update {
            labels.push((tf("practice.update_to", &[("v", new)]), JobKind::Update));
        }
        labels.push((t("practice.remove"), JobKind::Remove));
        let busy = self.practice.game_busy();
        let row = div().flex().flex_row().items_center().gap(px(24.));
        if busy || working {
            let why = busy.then(|| {
                inter(12., FontWeight::NORMAL, 1.4)
                    .text_color(faint3())
                    .child(t("practice.game_busy"))
            });
            return (
                row.children(labels.iter().map(|(l, _)| tertiary_off(l))),
                why,
            );
        }
        let row = row.children(labels.into_iter().enumerate().map(|(n, (label, kind))| {
            let id = e.id.clone();
            tertiary(("practice-change", i * 4 + n), &label).on_click(cx.listener(
                move |this, _, _, cx| {
                    cx.stop_propagation();
                    let p = this.backend().practice();
                    match kind {
                        JobKind::Reset => p.ask_reset(&id),
                        JobKind::Update => p.ask_update(&id),
                        _ => p.ask_remove(&id),
                    }
                },
            ))
        }));
        (row, None)
    }

    fn map_row(&self, i: usize, e: &'static MapEntry, cx: &mut Context<Self>) -> Div {
        let Some(v) = self.practice.map(&e.id).cloned() else {
            return div();
        };
        let demo_downloads = demo::get()
            .and_then(|d| d.maps.get(&e.id))
            .map(|d| d.downloads.clone())
            .filter(|d| !d.is_empty());
        let version = match &v.status {
            MapStatus::Installed(m) => Some(m.version.clone()),
            _ => e.version().map(str::to_string),
        };
        let mut meta: Vec<String> = version.map(|v| format!("v{v}")).into_iter().collect();
        meta.extend(demo_downloads);
        let id = e.id.clone();
        let open = self.practice_open.as_deref() == Some(e.id.as_str());
        let row = div()
            .id(("practice-row", i))
            .relative()
            .flex_none()
            .h(px(58.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(20.))
            .cursor_pointer()
            .when(!open, |d| d.hover(|s| s.bg(text3a(0.03))))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.practice_open = if this.practice_open.as_deref() == Some(id.as_str()) {
                    None
                } else {
                    this.practice_reveal = true;
                    Some(id.clone())
                };
                cx.notify();
            }))
            .child(Self::row_body(
                &e.name,
                e.authors.clone().into(),
                t(&format!("map.{}", e.id)),
                meta.join(" · "),
            ))
            .child(self.map_action(i, e, &v, cx));
        div()
            .flex_none()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(line3())
            .when(open, |d| d.bg(text3a(0.03)))
            .child(row)
            .children(self.map_below(i, e, &v, cx))
    }

    /// The confirmation over the page (`Question`), when one is open.
    fn practice_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        let ask = self.practice_ask.as_ref()?;
        let map_name = |id: &str| {
            catalog::maps()
                .get(id)
                .map_or_else(|| id.to_string(), |m| m.name.clone())
        };
        let body = |s: SharedString| {
            inter(14., FontWeight::NORMAL, 1.5)
                .text_color(muted3())
                .child(wrap(&s, INTER, FontWeight::NORMAL, 14., DIALOG_W - 64.))
        };
        let close = cx.listener(|this, _, _, cx| {
            this.practice_ask = None;
            cx.notify();
        });
        let cancel = tertiary("practice-ask-cancel", &t("common.cancel")).on_click(close);
        let keep_what = |id: &str| t(&format!("practice.keep.{id}"));
        let (title, mut lines, buttons): (SharedString, Vec<Div>, Div) = match &ask.question {
            Question::Reset { id, plan } => {
                let title = tf(
                    "practice.ask.reset.title",
                    &[("map", &map_name(id)), ("v", &plan.version)],
                );
                let mut lines = Vec::new();
                let played: Vec<WorldView> = plan
                    .worlds
                    .iter()
                    .filter(|w| w.state == WorldState::Played)
                    .cloned()
                    .collect();
                lines.push(body(if played.is_empty() {
                    t("practice.ask.reset.fresh")
                } else {
                    tf("practice.ask.reset.played", &[("worlds", &names(&played))])
                }));
                if plan.carry_over.is_empty() {
                    lines.push(body(t("practice.ask.reset.records")));
                } else {
                    let keep = ask.keep;
                    lines.push(
                        div().child(
                            div()
                                .id("practice-ask-keep")
                                .cursor_pointer()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(10.))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(a) = &mut this.practice_ask {
                                        a.keep = !a.keep;
                                    }
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .flex_none()
                                        .size(px(14.))
                                        .border_1()
                                        .border_color(text3())
                                        .when(keep, |d| d.bg(text3())),
                                )
                                .child(
                                    inter(14., FontWeight::MEDIUM, 1.4)
                                        .text_color(text3())
                                        .child(tf(
                                            "practice.ask.reset.keep",
                                            &[("what", &keep_what(id))],
                                        )),
                                ),
                        ),
                    );
                }
                if plan.needs_download {
                    lines.push(body(t("practice.ask.reset.download")));
                }
                let (id, keep) = (id.clone(), ask.keep);
                let confirm = play_button(
                    "practice-ask-reset",
                    &t("practice.ask.reset.confirm"),
                    SharedString::default(),
                    220.,
                    50.,
                    14.,
                    Press::Live,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.practice_ask = None;
                    this.backend().practice().reset(&id, keep);
                    cx.notify();
                }));
                (
                    title,
                    lines,
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(28.))
                        .child(confirm)
                        .child(cancel),
                )
            }
            Question::Update {
                id, to, carry_over, ..
            } => {
                let lines = Vec::new();
                let choices = [
                    (UpdateChoice::ReplaceWorlds, "practice.ask.update.replace"),
                    (UpdateChoice::KeepBoth, "practice.ask.update.both"),
                    (UpdateChoice::StoreOnly, "practice.ask.update.later"),
                ];
                let keep = (!carry_over.is_empty()).then(|| keep_what(id));
                let options = choices.into_iter().enumerate().map(|(n, (choice, key))| {
                    let id = id.clone();
                    div()
                        .id(("practice-ask-choice", n))
                        .group("practice-choice")
                        .cursor_pointer()
                        .p(px(14.))
                        .border_1()
                        .border_color(line3())
                        .hover(|s| s.border_color(text3()).bg(text3a(0.04)))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.practice_ask = None;
                            this.backend().practice().update(&id, choice);
                            cx.notify();
                        }))
                        .child(
                            inter(14., FontWeight::SEMIBOLD, 1.3)
                                .text_color(text3())
                                .child(tf(key, &[("v", to)])),
                        )
                        .child(
                            inter(13., FontWeight::NORMAL, 1.45)
                                .text_color(muted3())
                                .child(wrap(
                                    // Replacing says what carries over, where something does.
                                    &match (&keep, choice) {
                                        (Some(what), UpdateChoice::ReplaceWorlds) => tf(
                                            "practice.ask.update.replace_keep",
                                            &[("what", what)],
                                        ),
                                        _ => tf(&format!("{key}_hint"), &[("v", to)]),
                                    },
                                    INTER,
                                    FontWeight::NORMAL,
                                    13.,
                                    DIALOG_W - 64. - 30.,
                                )),
                        )
                });
                (
                    tf(
                        "practice.ask.update.title",
                        &[("map", &map_name(id)), ("v", to)],
                    ),
                    lines,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .children(options)
                        .child(div().mt(px(8.)).child(cancel)),
                )
            }
            Question::Remove { id, worlds, kept } => {
                let title = tf("practice.ask.remove.title", &[("map", &map_name(id))]);
                let mut lines = vec![body(tf(
                    "practice.ask.remove.body",
                    &[("worlds", &names(worlds))],
                ))];
                if worlds.iter().any(|w| w.state == WorldState::Played) {
                    lines.push(body(t("practice.ask.remove.played")));
                }
                if !kept.is_empty() {
                    lines.push(body(tf(
                        "practice.ask.remove.kept",
                        &[("worlds", &kept.join(", "))],
                    )));
                }
                let id = id.clone();
                let confirm = play_button(
                    "practice-ask-remove",
                    &t("practice.ask.remove.confirm"),
                    SharedString::default(),
                    220.,
                    50.,
                    14.,
                    Press::Live,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.practice_ask = None;
                    this.backend().practice().remove(&id);
                    cx.notify();
                }));
                (
                    title,
                    lines,
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(28.))
                        .child(confirm)
                        .child(cancel),
                )
            }
        };
        lines.push(div().mt(px(10.)).child(buttons));
        Some(
            div().absolute().top_0().left_0().size_full().child(
                div()
                    .id("practice-ask")
                    .size_full()
                    .occlude()
                    .bg(bg3a(0.82))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .w(px(DIALOG_W))
                            .p(px(32.))
                            .bg(gpui::rgb(0x17131F))
                            .border_1()
                            .border_color(line3())
                            .flex()
                            .flex_col()
                            .gap(px(14.))
                            .child(
                                inter(22., FontWeight::SEMIBOLD, 1.25)
                                    .text_color(text3())
                                    .child(wrap(
                                        &title,
                                        INTER,
                                        FontWeight::SEMIBOLD,
                                        22.,
                                        DIALOG_W - 64.,
                                    )),
                            )
                            .children(lines),
                    ),
            ),
        )
    }

    pub fn render_practice(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = practice();
        let demo = demo::get();
        let cats = &p.categories;
        let count = |ci: usize| catalog::maps_in(&cats[ci].id).count();

        // Background: the selected category's art over the top 360 px at 22 % (bg @ 78 %),
        // fading into bg. Tools has no art of its own and keeps the first category's.
        let bg_art = match self.practice_sel {
            PracticeSel::Category(i) => Self::practice_art(i),
            PracticeSel::Tools => Self::practice_art(0),
        };

        let tool_names = p
            .tools
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(" · ");

        // Grid: 1.5fr 1fr 1fr 1fr, two 128 px rows; the first category spans both rows.
        let small = |ci: usize, cx: &mut Context<Self>| {
            self.cat_tile(
                PracticeSel::Category(ci),
                Self::cat_name(ci),
                Self::practice_count(count(ci)),
                Some(Self::practice_art(ci)),
                false,
                cx,
            )
        };
        let col = |a: gpui::Stateful<Div>, b: gpui::Stateful<Div>| {
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(GRID_GAP))
                .child(div().h(px(ROW_H)).child(a))
                .child(div().h(px(ROW_H)).child(b))
        };
        // The practice mods can't be installed yet, so outside the demo their tile isn't shown and
        // Blaze takes the whole column.
        let last_col = if demo::get().is_some() {
            let tools_tile = self.cat_tile(
                PracticeSel::Tools,
                t("v3.practice_tools_tile"),
                tool_names.into(),
                None,
                false,
                cx,
            );
            col(small(3, cx), tools_tile)
        } else {
            div()
                .flex_1()
                .min_w(px(0.))
                .h_full()
                .child(small(3, cx).h_full())
        };
        let grid = div()
            .mt(px(30.))
            .flex_none()
            .h(px(2. * ROW_H + GRID_GAP))
            .flex()
            .flex_row()
            .gap(px(GRID_GAP))
            .child(div().h_full().flex_basis(px(0.)).flex_grow(1.5).map(|d| {
                d.child(self.cat_tile(
                    PracticeSel::Category(0),
                    Self::cat_name(0),
                    // A count like every tile: the list right below names the maps.
                    Self::practice_count(count(0)),
                    Some(Self::practice_art(0)),
                    true,
                    cx,
                ))
            }))
            .child(col(small(1, cx), small(4, cx)).flex_basis(px(0.)))
            .child(col(small(2, cx), small(5, cx)).flex_basis(px(0.)))
            .child(last_col.flex_basis(px(0.)));

        // The list for the selection, most popular first.
        let (title, rows, notes): (SharedString, Vec<Div>, Vec<SharedString>) = match self
            .practice_sel
        {
            PracticeSel::Category(ci) => {
                let maps: Vec<&'static MapEntry> = catalog::maps_in(&cats[ci].id).collect();
                let n = maps.len();
                if self.practice_reveal {
                    self.practice_reveal = false;
                    if let Some(ix) = maps
                        .iter()
                        .position(|m| self.practice_open.as_deref() == Some(m.id.as_str()))
                    {
                        self.practice_scroll.scroll_to_item(ix);
                        // The page's first frame: the list has no size yet, so the scroll can
                        // only happen on the next one. Ask for exactly one more.
                        if self.practice_scroll.bounds().size.height <= px(0.) {
                            self.practice_reveal = true;
                            let this = cx.entity().downgrade();
                            cx.defer(move |cx| {
                                let _ = this.update(cx, |_, cx| cx.notify());
                            });
                        }
                    }
                }
                let rows = maps
                    .into_iter()
                    .enumerate()
                    .map(|(i, e)| self.map_row(i, e, cx))
                    .collect();
                (
                    tf(
                        "v3.practice_list",
                        &[
                            ("category", &Self::cat_name(ci)),
                            ("count", &Self::practice_count(n)),
                        ],
                    ),
                    rows,
                    vec![],
                )
            }
            PracticeSel::Tools => {
                let n = p.tools.len();
                let rows = p
                    .tools
                    .iter()
                    .map(|tl| {
                        Self::tool_row(ToolRow {
                            name: &tl.name,
                            authors: if tl.contributors {
                                tf("practice.and_contributors", &[("name", &tl.authors)])
                            } else {
                                tl.authors.clone().into()
                            },
                            desc: t(&format!("ptool.{}", tl.id)),
                            meta: tl.version.clone(),
                            demo_status: demo.and_then(|d| d.tools.get(&tl.id)).map(String::as_str),
                        })
                    })
                    .collect();
                (
                    tf(
                        "v3.practice_list",
                        &[
                            ("category", &t("v3.practice_tools_tile")),
                            ("count", &Self::practice_count(n)),
                        ],
                    ),
                    rows,
                    if demo.is_none() {
                        vec![t("practice.tools_later")]
                    } else {
                        vec![]
                    },
                )
            }
        };

        // The header, then the rows (they scroll once they don't fit), then the notes right under
        // the last row.
        let list = div()
            .mt(px(30.))
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .pb(px(12.))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .when(matches!(self.practice_sel, PracticeSel::Tools), |d| {
                        d.child(section_head(&title))
                    }),
            )
            // While rows are hidden above or below, they fade into the page at that edge instead
            // of being cut by it (and never run into the header).
            .child(
                div()
                    .relative()
                    .flex_initial()
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .border_b_1()
                    .border_color(line3())
                    .child(
                        div()
                            .id("practice-list")
                            .flex_initial()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .track_scroll(&self.practice_scroll)
                            // The rows are its direct children, so `scroll_to_item` finds one.
                            .flex()
                            .flex_col()
                            .children(rows),
                    )
                    .child(fade_above(self.practice_scroll.clone(), 28.))
                    .child(fade_below(self.practice_scroll.clone(), 40.)),
            )
            .children(notes.into_iter().enumerate().map(|(i, n)| {
                inter(12., FontWeight::NORMAL, 1.4)
                    .flex_none()
                    .mt(px(if i == 0 { 10. } else { 4. }))
                    .text_color(muted3())
                    .child(n)
            }));

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(360.))
                    .overflow_hidden()
                    // `background: center 40% / cover`: the 16:9 card at the window width is
                    // 720 px tall, and 40 % of the 360 px overflow is 144 px.
                    .child(
                        img(card_path(bg_art))
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(px(-144.))
                            .h(px(720.))
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(overlay(bg3a(0.78)))
                    .child(shade_up(1.0, 0.0, 1.0)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(64.))
                    .right(px(48.))
                    .top(px(104.))
                    .bottom(px(28.))
                    .flex()
                    .flex_col()
                    .child(page_title(&t("practice.title"), ""))
                    .child(grid)
                    .child(list),
            )
            .children(self.practice_dialog(cx))
            .into_any_element()
    }
}

/// A click that opens `url` in the browser (the author's page, a video, a support link).
fn open_url(
    url: String,
    cx: &mut Context<Launcher>,
) -> impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static {
    cx.listener(move |_, _, _, cx| {
        cx.stop_propagation();
        crate::app::trace(format_args!("open_url {url}"));
        cx.open_url(&url);
    })
}

/// A page header (Practice, Settings, the "coming later" pages): the title in Archivo SRL
/// ExtraBold 56 on the left, a muted subtitle right-aligned at the bottom.
pub fn page_title(title: &str, subtitle: &str) -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_end()
        .justify_between()
        .child(tight(
            title,
            56.,
            FontWeight::EXTRA_BOLD,
            -0.025,
            0.9,
            text3(),
        ))
        .child(
            inter(14., FontWeight::NORMAL, 1.45)
                .whitespace_nowrap()
                .text_right()
                .text_color(muted3())
                .child(wrap_even(subtitle, INTER, FontWeight::NORMAL, 14., 420.)),
        )
}

/// Practice's background band: a card image over the top 360 px at 22 % (bg @ 78 %), fading into
/// bg.
pub fn art_band(art: &str) -> Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(360.))
        .overflow_hidden()
        .child(
            img(card_path(art))
                .absolute()
                .left_0()
                .right_0()
                .top(px(-144.))
                .h(px(720.))
                .object_fit(ObjectFit::Cover),
        )
        .child(overlay(bg3a(0.78)))
        .child(shade_up(1.0, 0.0, 1.0))
}
