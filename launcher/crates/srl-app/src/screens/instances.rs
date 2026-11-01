//! Instances: every game directory the launcher plays in, and what's inside it.
//!
//! The left column lists the instances: the shared 1.16.1 one that Ranked, RSG, FSG and SSG play
//! in (a mode there is only the set of mods Play puts into `mods/`), Draftout's, the practice
//! one. The right side is the selected instance: what it is, Play, and tabs for its worlds, mods,
//! resource packs, config files, screenshots, logs and settings, the way MultiMC and Prism let a
//! player look inside an instance without opening its folder.
//!
//! The file-system work (listing `saves/` with thousands of reset worlds, adding up sizes) runs on
//! a background thread (`backend::content`); the page shows the last answer until the next one
//! arrives, so it never blocks and idle stays idle.
use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, ObjectFit, ParentElement, PathPromptOptions,
    SharedString, Styled, div, img, prelude::*, px,
};
use srl_core::Mode;

use crate::{
    app::{Launcher, Page},
    assets::card_path,
    backend::{
        MEMORY_CHOICES_MB, ModeState,
        content::{self, Content},
        index, open_folder, open_path,
    },
    catalog,
    copy::{size, t, tf, tn},
    v3::*,
};

use super::stats::{fmt_date, today_and_month_ms};

/// "18:40" for today, else the short date ("Oct 24").
fn fmt_when(ms: i64) -> SharedString {
    use chrono::TimeZone;
    let (today, _) = today_and_month_ms();
    if ms >= today
        && let Some(d) = chrono::Local.timestamp_millis_opt(ms).earliest()
    {
        return d.format("%H:%M").to_string().into();
    }
    fmt_date(ms)
}

const LIST_X: f32 = 48.;
const LIST_W: f32 = 300.;
const PANE_X: f32 = 388.;
const PANE_R: f32 = 48.;
const PANE_W: f32 = 1280. - PANE_X - PANE_R;
const ROW_H: f32 = 52.;

/// An instance the page can show: a game directory under `instances/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InstKey {
    /// `instances/mcsr`: Ranked, RSG, FSG and SSG.
    Shared,
    Draftout,
    Practice,
}

impl InstKey {
    pub const ALL: [InstKey; 3] = [InstKey::Shared, InstKey::Draftout, InstKey::Practice];

    /// The folder under `instances/`.
    pub fn dir(self) -> &'static str {
        match self {
            InstKey::Shared => srl_core::shared::DIR,
            InstKey::Draftout => "draftout",
            InstKey::Practice => "practice",
        }
    }

    fn id(self) -> &'static str {
        match self {
            InstKey::Shared => "shared",
            InstKey::Draftout => "draftout",
            InstKey::Practice => "practice",
        }
    }

    pub fn from_id(id: &str) -> Option<InstKey> {
        InstKey::ALL.into_iter().find(|k| k.id() == id)
    }

    fn minecraft(self) -> &'static str {
        match self {
            InstKey::Draftout => Mode::Draftout.minecraft_version(),
            _ => "1.16.1",
        }
    }

    /// The key art its row and backdrop use.
    fn art(self) -> &'static str {
        match self {
            InstKey::Shared => catalog::MODES[index(Mode::Rsg)].art,
            InstKey::Draftout => catalog::MODES[index(Mode::Draftout)].art,
            InstKey::Practice => catalog::MODES[index(Mode::Fsg)].art,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Worlds,
    Mods,
    Packs,
    Config,
    Screenshots,
    Logs,
    Settings,
}

impl Tab {
    pub const ALL: [Tab; 7] = [
        Tab::Worlds,
        Tab::Mods,
        Tab::Packs,
        Tab::Config,
        Tab::Screenshots,
        Tab::Logs,
        Tab::Settings,
    ];

    fn key(self) -> &'static str {
        match self {
            Tab::Worlds => "worlds",
            Tab::Mods => "mods",
            Tab::Packs => "packs",
            Tab::Config => "config",
            Tab::Screenshots => "screenshots",
            Tab::Logs => "logs",
            Tab::Settings => "settings",
        }
    }

    pub fn from_id(id: &str) -> Option<Tab> {
        Tab::ALL.into_iter().find(|t| t.key() == id)
    }
}

/// A change the page asks to confirm first, in place of the list's toolbar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Confirm {
    /// Delete every reset world (`World::reset`) for good.
    Cleanup,
    /// Move one world to the trash.
    World(String),
    /// Move one resource pack to the trash.
    Pack(String),
}

/// The page's own state.
pub struct InstancesView {
    pub sel: InstKey,
    pub tab: Tab,
    /// The last scan of `sel`'s directory (`None` before the first one answers).
    pub content: Option<Content>,
    /// A scan is running; `generation` drops answers for an instance no longer selected.
    pub scanning: bool,
    pub generation: u64,
    pub scroll: gpui::ScrollHandle,
    pub confirm: Option<Confirm>,
    /// What the last change did, or why it failed (`true`: a failure).
    pub note: Option<(SharedString, bool)>,
    /// The game log's last lines, read with the scan, for the Logs tab.
    pub tail: Vec<String>,
    /// The seed last copied with "Copy seed" (its button says Copied).
    pub copied: Option<i64>,
}

impl Default for InstancesView {
    fn default() -> Self {
        Self {
            sel: InstKey::Shared,
            tab: Tab::Worlds,
            content: None,
            scanning: false,
            generation: 0,
            scroll: gpui::ScrollHandle::new(),
            confirm: None,
            note: None,
            tail: Vec::new(),
            copied: None,
        }
    }
}

impl Launcher {
    /// The game directory of `key`.
    pub fn inst_dir(&self, key: InstKey) -> PathBuf {
        self.data_dir.join("instances").join(key.dir())
    }

    /// The mode Play and the Mods tab use for `key`: the selected mode in the shared instance
    /// (Ranked when Draftout is selected), Draftout in its own.
    fn inst_mode(&self, key: InstKey) -> Option<Mode> {
        match key {
            InstKey::Shared => {
                let m = self.mode(self.mode_sel);
                Some(if m.is_shared() { m } else { Mode::Ranked })
            }
            InstKey::Draftout => Some(Mode::Draftout),
            InstKey::Practice => None,
        }
    }

    /// Opens the page (the nav, Home's Mods): the instance of the selected mode, on `tab`.
    pub fn open_instances(&mut self, tab: Option<Tab>, cx: &mut Context<Self>) {
        let key = if self.mode(self.mode_sel).is_shared() {
            InstKey::Shared
        } else {
            InstKey::Draftout
        };
        if let Some(tab) = tab {
            self.instances.tab = tab;
        }
        self.select_instance(key, cx);
        self.navigate(Page::Instances, cx);
    }

    pub fn select_instance(&mut self, key: InstKey, cx: &mut Context<Self>) {
        if self.instances.sel != key {
            self.kick_fade();
        }
        let v = &mut self.instances;
        if v.sel != key {
            v.sel = key;
            v.content = None;
            v.tail.clear();
            v.scroll.set_offset(gpui::point(px(0.), px(0.)));
        }
        // Play on the page plays the selected mode, so it follows the instance.
        let shared = self.mode(self.mode_sel).is_shared();
        match key {
            InstKey::Draftout if shared => self.select_mode(index(Mode::Draftout), cx),
            InstKey::Shared if !shared => self.select_mode(index(Mode::Ranked), cx),
            _ => {}
        }
        self.instances.confirm = None;
        self.instances.note = None;
        if key == InstKey::Practice && self.instances.tab == Tab::Mods {
            self.instances.tab = Tab::Worlds;
        }
        self.rescan(cx);
        cx.notify();
    }

    fn select_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        let v = &mut self.instances;
        if v.tab != tab {
            v.tab = tab;
            self.kick_fade();
            let v = &mut self.instances;
            v.confirm = None;
            v.note = None;
            v.scroll.set_offset(gpui::point(px(0.), px(0.)));
            cx.notify();
        }
    }

    /// Reads the selected instance's directory again, in the background.
    pub fn rescan(&mut self, cx: &mut Context<Self>) {
        let key = self.instances.sel;
        let dir = self.inst_dir(key);
        let data = self.data_dir.clone();
        self.instances.generation += 1;
        let generation = self.instances.generation;
        self.instances.scanning = true;
        let task = cx.background_spawn(async move {
            let modes = (key == InstKey::Shared)
                .then(|| srl_core::shared::WorldModes::load(&srl_core::Paths::new(data.clone())));
            let c = content::scan(&dir, modes.as_ref());
            let tail = content::tail(&dir.join("logs/latest.log"), 14);
            (c, tail)
        });
        cx.spawn(async move |this, cx| {
            let (c, tail) = task.await;
            let _ = this.update(cx, |app, cx| {
                if app.instances.generation == generation {
                    app.instances.content = Some(c);
                    app.instances.tail = tail;
                    app.instances.scanning = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// The selected instance's game runs (or installs): its files must not change under it.
    fn inst_busy(&self, key: InstKey) -> bool {
        match key {
            InstKey::Shared => Mode::SHARED.iter().any(|m| {
                matches!(
                    self.state(index(*m)),
                    ModeState::Running | ModeState::Installing(_)
                )
            }),
            InstKey::Draftout => matches!(
                self.state(index(Mode::Draftout)),
                ModeState::Running | ModeState::Installing(_)
            ),
            InstKey::Practice => self.practice.game_busy(),
        }
    }

    /// Runs a file change in the background, then says what it did and scans again.
    fn change(
        &mut self,
        work: impl FnOnce() -> Result<SharedString, String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        self.instances.confirm = None;
        let task = cx.background_spawn(async move { work() });
        cx.spawn(async move |this, cx| {
            let answer = task.await;
            let _ = this.update(cx, |app, cx| {
                app.instances.note = Some(match answer {
                    Ok(s) => (s, false),
                    Err(e) => (e.into(), true),
                });
                app.rescan(cx);
            });
        })
        .detach();
        cx.notify();
    }

    /// A game started while a question was open: nothing may change under it.
    fn refuse_if_busy(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.inst_busy(self.instances.sel) {
            return false;
        }
        self.instances.confirm = None;
        self.instances.note = Some((t("inst.busy"), true));
        cx.notify();
        true
    }

    fn cleanup_worlds(&mut self, cx: &mut Context<Self>) {
        if self.refuse_if_busy(cx) {
            return;
        }
        let Some(c) = &self.instances.content else {
            return;
        };
        let dir = c.game_dir.clone();
        let dirs: Vec<String> = c.reset_worlds().map(|w| w.dir.clone()).collect();
        self.change(
            move || {
                let (n, bytes) = content::delete_reset_worlds(&dir, &dirs);
                Ok(tf(
                    "inst.worlds.cleaned",
                    &[
                        ("n", &tn("inst.worlds.resets", n as u64)),
                        ("size", &size(bytes)),
                    ],
                ))
            },
            cx,
        );
    }

    fn trash_item(&mut self, folder: &'static str, rel: String, cx: &mut Context<Self>) {
        if self.refuse_if_busy(cx) {
            return;
        }
        let data = self.data_dir.clone();
        let dir = self.inst_dir(self.instances.sel);
        self.change(
            move || {
                content::trash(&data, &dir, folder, &rel)
                    .map(|_| tf("inst.trashed", &[("name", &rel)]))
                    .map_err(|e| format!("{e:#}"))
            },
            cx,
        );
    }

    fn toggle_pack(&mut self, rel: String, on: bool, cx: &mut Context<Self>) {
        if self.refuse_if_busy(cx) {
            return;
        }
        let dir = self.inst_dir(self.instances.sel);
        self.change(
            move || {
                content::set_pack(&dir, &rel, on)
                    .map(|_| {
                        tf(
                            if on {
                                "inst.packs.turned_on"
                            } else {
                                "inst.packs.turned_off"
                            },
                            &[("name", &rel)],
                        )
                    })
                    .map_err(|e| format!("{e:#}"))
            },
            cx,
        );
    }

    /// The system's file picker for a resource pack (`SRL_PACK_PICK` in tests).
    fn pick_pack(&mut self, cx: &mut Context<Self>) {
        let dir = self.inst_dir(self.instances.sel);
        let add = move |app: &mut Launcher, file: PathBuf, cx: &mut Context<Launcher>| {
            let dir = dir.clone();
            app.change(
                move || {
                    content::add_pack(&dir, &file)
                        .map(|name| tf("inst.packs.added", &[("name", &name)]))
                        .map_err(|e| format!("{e:#}"))
                },
                cx,
            );
        };
        if let Some(file) = std::env::var_os("SRL_PACK_PICK").filter(|v| !v.is_empty()) {
            add(self, PathBuf::from(file), cx);
            return;
        }
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t("inst.packs.pick_ok")),
        });
        cx.spawn(async move |this, cx| {
            let answer = picked.await;
            let _ = this.update(cx, |app, cx| {
                if let Ok(Ok(Some(mut files))) = answer
                    && !files.is_empty()
                {
                    add(app, files.swap_remove(0), cx);
                }
            });
        })
        .detach();
    }

    pub fn render_instances(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let key = self.instances.sel;
        div()
            .size_full()
            .relative()
            .child(art_fill(card_path(key.art())))
            .child(overlay(bg3a(0.9)))
            .child(shade_up(1.0, 0.0, 0.6))
            .child(self.inst_list(cx))
            .child(self.inst_pane(key, cx))
            .into_any_element()
    }

    fn inst_list(&self, cx: &mut Context<Self>) -> Div {
        div()
            .absolute()
            .left(px(LIST_X))
            .top(px(100.))
            .w(px(LIST_W))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(section_head(&t("inst.list")).pb(px(4.)))
            .children({
                let mut rows = Vec::new();
                for k in InstKey::ALL {
                    rows.push(self.inst_row(k, cx));
                }
                rows
            })
    }

    /// One row of the list: the art, the name, the version and who plays in it.
    fn inst_row(&self, key: InstKey, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
        let on = key == self.instances.sel;
        let busy = self.inst_busy(key);
        div()
            .id(SharedString::from(format!("inst-row-{}", key.id())))
            .group("inst-row")
            .relative()
            .flex_none()
            .h(px(84.))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.select_instance(key, cx)))
            .child(overlay(tile_bg()))
            .child(art_fill(card_path(key.art())))
            .map(|e| {
                if on {
                    e.child(shade_right(0., 0.6, 0.92, 0.6))
                        .child(shade_right(0.6, 1.0, 0.6, 0.35))
                } else {
                    e.child(shade_right(0., 0.6, 0.95, 0.8))
                        .child(shade_right(0.6, 1.0, 0.8, 0.65))
                }
            })
            .child(
                div()
                    .absolute()
                    .left(px(16.))
                    .top(px(18.))
                    .right(px(16.))
                    .flex()
                    .flex_col()
                    .child(tight(
                        &t(&format!("inst.{}.name", key.id())),
                        20.,
                        FontWeight::EXTRA_BOLD,
                        -0.01,
                        1.0,
                        text3(),
                    ))
                    .child(if busy {
                        status_text(&t("inst.in_use"), accent2()).mt(px(9.))
                    } else {
                        inter(12., FontWeight::MEDIUM, 1.)
                            .mt(px(9.))
                            .whitespace_nowrap()
                            .text_color(muted3())
                            .child(self.inst_who(key))
                    }),
            )
            .child(
                div()
                    .absolute()
                    .right(px(14.))
                    .top(px(14.))
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(
                        plex(11., FontWeight::MEDIUM)
                            .text_color(faint3())
                            .child(key.minecraft()),
                    ),
            )
            .child(if on { ring() } else { hover_ring("inst-row") })
    }

    /// Who plays in the instance: the modes, or how many practice maps are installed.
    fn inst_who(&self, key: InstKey) -> SharedString {
        match key {
            InstKey::Practice => {
                let n = self
                    .practice
                    .maps
                    .iter()
                    .filter(|m| matches!(m.status, srl_practice::MapStatus::Installed(_)))
                    .count();
                if n == 0 {
                    t("inst.practice.who_none")
                } else {
                    tn("inst.practice.who", n as u64)
                }
            }
            _ => t(&format!("inst.{}.who", key.id())),
        }
    }

    fn inst_pane(&mut self, key: InstKey, cx: &mut Context<Self>) -> Div {
        let mut facts: Vec<(SharedString, SharedString)> = Vec::new();
        if let Some(c) = self.instances.content.as_ref().filter(|c| c.exists) {
            facts.push((t("inst.fact.worlds"), c.worlds.len().to_string().into()));
            facts.push((t("inst.fact.disk"), size(c.size).into()));
        }
        if let Some(mode) = self.inst_mode(key) {
            let info = &self.modes[index(mode)].info;
            if let Some(cat) = &info.catalogue {
                facts.push((
                    t("inst.fact.mods"),
                    cat.iter().filter(|a| a.on).count().to_string().into(),
                ));
            }
            if let Some(java) = &info.java {
                facts.push((t("inst.fact.java"), java.clone().into()));
            }
        }
        let head = div()
            .flex()
            .flex_col()
            .child(kicker(
                &tf("inst.eyebrow", &[("mc", key.minecraft())]),
                accent2(),
            ))
            .child(
                tight(
                    &t(&format!("inst.{}.name", key.id())),
                    48.,
                    FontWeight::EXTRA_BOLD,
                    -0.03,
                    0.95,
                    text3(),
                )
                .mt(px(10.)),
            )
            .child(
                inter(14., FontWeight::NORMAL, 1.5)
                    .mt(px(12.))
                    .whitespace_nowrap()
                    .text_color(muted3())
                    .child(wrap(
                        &t(&format!("inst.{}.body", key.id())),
                        INTER,
                        FontWeight::NORMAL,
                        14.,
                        PANE_W - 260.,
                    )),
            );
        let facts_col = div()
            .absolute()
            .right_0()
            .top(px(4.))
            .flex()
            .flex_row()
            .gap(px(28.))
            .children(facts.into_iter().map(|(l, v)| {
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(6.))
                    .child(plex(18., FontWeight::MEDIUM).text_color(text3()).child(v))
                    .child(label(&l))
            }));

        div()
            .absolute()
            .left(px(PANE_X))
            .right(px(PANE_R))
            .top(px(100.))
            .bottom(px(32.))
            .flex()
            .flex_col()
            .child(div().relative().child(head).child(facts_col))
            .child(self.inst_actions(key, cx))
            .child(self.inst_tabs(key, cx))
            .child({
                let fade = self.mode_fade();
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .relative()
                    .top(px(fade.rise))
                    .when(fade.text < 1.0, |d| d.opacity(fade.text))
                    .child(self.inst_tab_body(key, cx))
            })
    }

    /// Play (the selected mode in the shared instance), the mode chips, Open folder.
    fn inst_actions(&mut self, key: InstKey, cx: &mut Context<Self>) -> Div {
        let mut row = div()
            .mt(px(20.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(20.));
        match key {
            InstKey::Shared | InstKey::Draftout => {
                // The instance's own mode, never another instance's: Draftout may still be
                // selected from Home while the shared instance is shown.
                let mi = index(self.inst_mode(key).unwrap_or(Mode::Ranked));
                row = row
                    .child(self.mode_play(mi, "inst-play", 200., 44., 15., cx))
                    .children(self.play_ghosts(mi, "inst", false, cx));
                if key == InstKey::Shared {
                    row = row.child(div().flex().flex_row().gap(px(6.)).children(
                        Mode::SHARED.into_iter().map(|m| {
                            let i = index(m);
                            chip(("inst-mode", i), catalog::MODES[i].short, i == mi, false)
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.select_mode(i, cx)),
                                )
                        }),
                    ));
                }
            }
            InstKey::Practice => {
                row = row.child(
                    secondary("inst-practice", &t("inst.practice.open"))
                        .on_click(cx.listener(|this, _, _, cx| this.navigate(Page::Practice, cx))),
                );
            }
        }
        let dir = self.inst_dir(key);
        row.child(div().flex_1()).child(
            tertiary("inst-folder", &t("inst.open_folder"))
                .on_click(cx.listener(move |_, _, _, _| open_folder(&dir))),
        )
    }

    fn inst_tabs(&self, key: InstKey, cx: &mut Context<Self>) -> Div {
        let c = self.instances.content.as_ref();
        let count = |tab: Tab| -> Option<usize> {
            let c = c?;
            Some(match tab {
                Tab::Worlds => c.worlds.len(),
                Tab::Packs => c.packs.len(),
                Tab::Config => c.config.len(),
                Tab::Screenshots => c.screenshots.len(),
                Tab::Logs => c.logs.len(),
                Tab::Mods | Tab::Settings => return None,
            })
        };
        div()
            // Room for the install progress that hangs under Play.
            .mt(px(44.))
            .flex()
            .flex_row()
            .gap(px(26.))
            .border_b_1()
            .border_color(line3())
            .children(
                Tab::ALL
                    .into_iter()
                    .filter(|tab| !(key == InstKey::Practice && *tab == Tab::Mods))
                    .map(|tab| {
                        let on = tab == self.instances.tab;
                        div()
                            .id(SharedString::from(format!("inst-tab-{}", tab.key())))
                            .group("inst-tab")
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .gap(px(6.))
                            .pb(px(10.))
                            .border_b_2()
                            .border_color(if on { accent2() } else { text3a(0.0) })
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| this.select_tab(tab, cx)))
                            .child(
                                inter(14., FontWeight::SEMIBOLD, 1.)
                                    .whitespace_nowrap()
                                    .text_color(if on { text3() } else { muted3() })
                                    .group_hover("inst-tab", |s| s.text_color(text3()))
                                    .child(t(&format!("inst.tab.{}", tab.key()))),
                            )
                            .children(count(tab).filter(|n| *n > 0).map(|n| {
                                plex(11., FontWeight::MEDIUM)
                                    .text_color(faint3())
                                    .child(n.to_string())
                            }))
                    }),
            )
    }

    fn inst_tab_body(&mut self, key: InstKey, cx: &mut Context<Self>) -> AnyElement {
        if self.instances.tab == Tab::Mods
            && let Some(mode) = self.inst_mode(key)
        {
            return self.inst_mods(mode, cx);
        }
        if self.instances.tab == Tab::Settings {
            return self.inst_settings(key, cx);
        }
        let Some(c) = self.instances.content.clone() else {
            return div()
                .pt(px(18.))
                .child(label(&t("inst.reading")))
                .into_any_element();
        };
        if !c.exists {
            return div()
                .pt(px(18.))
                .w(px(560.))
                .child(
                    inter(14., FontWeight::NORMAL, 1.5)
                        .whitespace_nowrap()
                        .text_color(muted3())
                        .child(wrap(
                            &t(&format!("inst.{}.empty", key.id())),
                            INTER,
                            FontWeight::NORMAL,
                            14.,
                            560.,
                        )),
                )
                .into_any_element();
        }
        let busy = self.inst_busy(key);
        let (bar, rows): (Div, Vec<AnyElement>) = match self.instances.tab {
            Tab::Worlds => self.worlds_tab(&c, busy, cx),
            Tab::Packs => self.packs_tab(&c, busy, cx),
            Tab::Config => self.files_tab(&c, &c.config, "config", "inst.config", cx),
            Tab::Screenshots => self.shots_tab(&c, cx),
            Tab::Logs => self.logs_tab(&c, cx),
            Tab::Mods | Tab::Settings => unreachable!("handled above"),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(bar)
            .children(self.instances.note.clone().map(|(s, bad)| {
                inter(12., FontWeight::MEDIUM, 1.3)
                    .pb(px(8.))
                    .text_color(if bad { behind3() } else { ok3() })
                    .child(s)
            }))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .relative()
                    .child(
                        div()
                            .id("inst-list")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.instances.scroll)
                            .children(rows),
                    )
                    .child(fade_above(self.instances.scroll.clone(), 24.))
                    .child(fade_below(self.instances.scroll.clone(), 36.)),
            )
            .into_any_element()
    }

    /// The bar over a list: a count on the left, actions on the right (or a question to confirm).
    fn bar(&self, count: SharedString, actions: Vec<AnyElement>) -> Div {
        div()
            .flex_none()
            .h(px(52.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(20.))
            .child(label(&count))
            .child(div().flex_1())
            .children(actions)
    }

    /// A yes/no in place of the bar's actions.
    fn ask(
        &self,
        question: SharedString,
        yes: &str,
        on_yes: impl Fn(&mut Launcher, &mut Context<Launcher>) + 'static,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_none()
            .min_h(px(52.))
            .py(px(8.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(20.))
            .child(
                inter(13., FontWeight::MEDIUM, 1.4)
                    .flex_1()
                    .text_color(gold())
                    .child(question),
            )
            .child(
                primary("inst-yes", yes)
                    .on_click(cx.listener(move |this, _, _, cx| on_yes(this, cx))),
            )
            .child(
                tertiary("inst-no", &t("inst.keep")).on_click(cx.listener(|this, _, _, cx| {
                    this.instances.confirm = None;
                    cx.notify();
                })),
            )
    }

    fn worlds_tab(
        &self,
        c: &Content,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> (Div, Vec<AnyElement>) {
        let resets: Vec<_> = c.reset_worlds().collect();
        let reset_bytes: u64 = resets.iter().map(|w| w.size).sum();
        let bar = match &self.instances.confirm {
            Some(Confirm::Cleanup) => self.ask(
                tf(
                    "inst.worlds.cleanup_ask",
                    &[
                        ("n", &tn("inst.worlds.resets", resets.len() as u64)),
                        ("size", &size(reset_bytes)),
                    ],
                ),
                &t("inst.worlds.cleanup_yes"),
                |this, cx| this.cleanup_worlds(cx),
                cx,
            ),
            Some(Confirm::World(dir)) => {
                let dir = dir.clone();
                let name = c
                    .worlds
                    .iter()
                    .find(|w| w.dir == dir)
                    .map(|w| w.name.clone())
                    .unwrap_or_else(|| dir.clone());
                self.ask(
                    tf("inst.worlds.trash_ask", &[("name", &name)]),
                    &t("inst.trash_yes"),
                    move |this, cx| this.trash_item("saves", dir.clone(), cx),
                    cx,
                )
            }
            _ => {
                let mut actions: Vec<AnyElement> = Vec::new();
                if !resets.is_empty() {
                    let label_text = tf(
                        "inst.worlds.cleanup",
                        &[("n", &tn("inst.worlds.resets", resets.len() as u64))],
                    );
                    actions.push(if busy {
                        secondary_off(&label_text).into_any_element()
                    } else {
                        secondary("inst-cleanup", &label_text)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.instances.confirm = Some(Confirm::Cleanup);
                                this.instances.note = None;
                                cx.notify();
                            }))
                            .into_any_element()
                    });
                }
                let saves = c.game_dir.join("saves");
                actions.push(
                    tertiary("inst-saves", &t("inst.open_folder"))
                        .on_click(cx.listener(move |_, _, _, _| open_folder(&saves)))
                        .into_any_element(),
                );
                self.bar(
                    if c.worlds.is_empty() {
                        t("inst.worlds.none")
                    } else {
                        tf(
                            "inst.worlds.summary",
                            &[
                                ("n", &tn("inst.worlds.count", c.worlds.len() as u64)),
                                ("size", &size(c.saves_size)),
                            ],
                        )
                    },
                    actions,
                )
            }
        };
        let saves = c.game_dir.join("saves");
        let rows = c
            .worlds
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let mut tags: Vec<(SharedString, gpui::Hsla)> = Vec::new();
                if let Some(m) = w.mode {
                    tags.push((catalog::MODES[index(m)].short.into(), accent2()));
                }
                if w.reset {
                    tags.push((t("inst.worlds.reset"), faint3()));
                }
                let sub = match (w.seed, w.name != w.dir) {
                    (Some(seed), true) => Some(format!(
                        "{}  ·  {}",
                        tf("inst.worlds.seed", &[("seed", &seed.to_string())]),
                        w.dir
                    )),
                    (Some(seed), false) => {
                        Some(tf("inst.worlds.seed", &[("seed", &seed.to_string())]).to_string())
                    }
                    (None, true) => Some(w.dir.clone()),
                    (None, false) => None,
                };
                let seed = w.seed;
                let open = saves.join(&w.dir);
                let dir = w.dir.clone();
                let mut actions: Vec<AnyElement> = Vec::new();
                if let Some(seed) = seed {
                    let copied = self.instances.copied == Some(seed);
                    actions.push(
                        tertiary_in(
                            ("inst-world-seed", i),
                            &t(if copied {
                                "inst.worlds.copied"
                            } else {
                                "inst.worlds.copy_seed"
                            }),
                            if copied { ok3() } else { muted3() },
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                seed.to_string(),
                            ));
                            this.instances.copied = Some(seed);
                            cx.notify();
                        }))
                        .into_any_element(),
                    );
                }
                actions.extend([tertiary(("inst-world-open", i), &t("inst.open"))
                    .on_click(cx.listener(move |_, _, _, _| open_folder(&open)))
                    .into_any_element()]);
                if !busy {
                    actions.push(
                        tertiary(("inst-world-trash", i), &t("inst.trash"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.instances.confirm = Some(Confirm::World(dir.clone()));
                                this.instances.note = None;
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
                row(
                    ("inst-world", i),
                    &w.name,
                    sub,
                    tags,
                    w.played_ms,
                    w.size,
                    actions,
                )
                .into_any_element()
            })
            .collect();
        (bar, rows)
    }

    fn packs_tab(&self, c: &Content, busy: bool, cx: &mut Context<Self>) -> (Div, Vec<AnyElement>) {
        let bar = match &self.instances.confirm {
            Some(Confirm::Pack(rel)) => {
                let rel = rel.clone();
                self.ask(
                    tf("inst.packs.trash_ask", &[("name", &rel)]),
                    &t("inst.trash_yes"),
                    move |this, cx| this.trash_item("resourcepacks", rel.clone(), cx),
                    cx,
                )
            }
            _ => {
                let on = c.packs.iter().filter(|p| p.on).count();
                let dir = c.game_dir.join("resourcepacks");
                self.bar(
                    if c.packs.is_empty() {
                        t("inst.packs.none")
                    } else {
                        tf(
                            "inst.packs.summary",
                            &[
                                ("n", &tn("inst.packs.count", c.packs.len() as u64)),
                                ("on", &on.to_string()),
                            ],
                        )
                    },
                    vec![
                        if busy {
                            secondary_off(&t("inst.packs.add")).into_any_element()
                        } else {
                            secondary("inst-pack-add", &t("inst.packs.add"))
                                .on_click(cx.listener(|this, _, _, cx| this.pick_pack(cx)))
                                .into_any_element()
                        },
                        tertiary("inst-packs-folder", &t("inst.open_folder"))
                            .on_click(cx.listener(move |_, _, _, _| open_folder(&dir)))
                            .into_any_element(),
                    ],
                )
            }
        };
        let mut rows: Vec<AnyElement> =
            c.packs
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let rel = p.item.rel.clone();
                    let rel2 = rel.clone();
                    let on = p.on;
                    let tick = div()
                        .id(("inst-pack-tick", i))
                        .flex_none()
                        .when(!busy, |d| {
                            d.cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.toggle_pack(rel.clone(), !on, cx)
                                }))
                        })
                        .child(super::import::tick(on, !busy));
                    let mut actions: Vec<AnyElement> = Vec::new();
                    if !busy {
                        actions.push(
                            tertiary(("inst-pack-trash", i), &t("inst.trash"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.instances.confirm = Some(Confirm::Pack(rel2.clone()));
                                    this.instances.note = None;
                                    cx.notify();
                                }))
                                .into_any_element(),
                        );
                    }
                    let tags = if on {
                        vec![(t("inst.packs.on"), ok3())]
                    } else {
                        Vec::new()
                    };
                    row(
                        ("inst-pack", i),
                        p.item.rel.trim_end_matches(".zip"),
                        None,
                        tags,
                        p.item.modified_ms,
                        p.item.size,
                        actions,
                    )
                    .child(div().absolute().left(px(4.)).top(px(17.)).child(tick))
                    .pl(px(34.))
                    .into_any_element()
                })
                .collect();
        if busy && !c.packs.is_empty() {
            rows.insert(
                0,
                inter(12., FontWeight::NORMAL, 1.4)
                    .pb(px(8.))
                    .text_color(faint3())
                    .child(t("inst.wait_game"))
                    .into_any_element(),
            );
        }
        (bar, rows)
    }

    /// A list of files that open in the system's app for them (config, logs).
    fn files_tab(
        &self,
        c: &Content,
        items: &[content::Item],
        folder: &'static str,
        copy: &str,
        cx: &mut Context<Self>,
    ) -> (Div, Vec<AnyElement>) {
        let dir = c.game_dir.join(folder);
        let bar = self.bar(
            if items.is_empty() {
                t(&format!("{copy}.none"))
            } else {
                tn(&format!("{copy}.count"), items.len() as u64)
            },
            vec![
                tertiary(
                    SharedString::from(format!("inst-{folder}-folder")),
                    &t("inst.open_folder"),
                )
                .on_click(cx.listener(move |_, _, _, _| open_folder(&dir)))
                .into_any_element(),
            ],
        );
        let rows = items
            .iter()
            .enumerate()
            .map(|(i, it)| file_row(&c.game_dir, folder, i, it, cx).into_any_element())
            .collect();
        (bar, rows)
    }

    fn logs_tab(&self, c: &Content, cx: &mut Context<Self>) -> (Div, Vec<AnyElement>) {
        let (bar, mut rows) = self.files_tab(c, &c.logs, "logs", "inst.logs", cx);
        if !self.instances.tail.is_empty() {
            let w = PANE_W - 32.;
            let lines = self.instances.tail.iter().map(|l| {
                plex(11., FontWeight::NORMAL)
                    .whitespace_nowrap()
                    .text_color(muted3())
                    .child(ellipsize(l, PLEX, FontWeight::NORMAL, 11., w, Cut::End))
            });
            rows.insert(
                0,
                div()
                    .mb(px(12.))
                    .p(px(16.))
                    .bg(bg3a(0.7))
                    .border_1()
                    .border_color(line3())
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(label(&t("inst.logs.latest")).pb(px(6.)))
                    .children(lines)
                    .into_any_element(),
            );
        }
        (bar, rows)
    }

    fn shots_tab(&self, c: &Content, cx: &mut Context<Self>) -> (Div, Vec<AnyElement>) {
        let dir = c.game_dir.join("screenshots");
        let bar = self.bar(
            if c.screenshots.is_empty() {
                t("inst.screenshots.none")
            } else {
                tn("inst.screenshots.count", c.screenshots.len() as u64)
            },
            vec![
                tertiary("inst-shots-folder", &t("inst.open_folder"))
                    .on_click(cx.listener(move |_, _, _, _| open_folder(&dir)))
                    .into_any_element(),
            ],
        );
        // Four across; only the newest 24 are drawn (each one is decoded into the atlas).
        let (cols, gap) = (4usize, 12.);
        let w = (PANE_W - gap * (cols as f32 - 1.)) / cols as f32;
        let h = (w * 9. / 16.).round();
        let tiles: Vec<AnyElement> = c
            .screenshots
            .iter()
            .take(24)
            .enumerate()
            .map(|(i, s)| {
                let path = c.game_dir.join("screenshots").join(&s.rel);
                let open = path.clone();
                div()
                    .id(("inst-shot", i))
                    .group("inst-shot")
                    .relative()
                    .w(px(w))
                    .h(px(h))
                    .bg(tile_bg())
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _, _, _| open_path(&open)))
                    .child(
                        img(path)
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(shade_up(0.85, 0.0, 0.45))
                    .child(
                        plex(11., FontWeight::MEDIUM)
                            .absolute()
                            .left(px(10.))
                            .bottom(px(8.))
                            .text_color(text3())
                            .child(s.modified_ms.map(fmt_date).unwrap_or_default()),
                    )
                    .child(hover_ring("inst-shot"))
                    .into_any_element()
            })
            .collect();
        let grid = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(gap))
            .children(tiles)
            .into_any_element();
        (bar, vec![grid])
    }

    fn inst_settings(&mut self, key: InstKey, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div().pt(px(20.)).flex().flex_col().gap(px(26.));
        if let Some(mode) = self.inst_mode(key) {
            let now = self.settings.memory_mb(mode);
            let floor = crate::backend::mode_needs(mode).min_memory_mb;
            let short = catalog::MODES[index(mode)].short;
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(section_head(&tf("inst.memory", &[("mode", short)])))
                    .child(div().flex().flex_row().gap(px(6.)).children(
                        MEMORY_CHOICES_MB.into_iter().map(|mb| {
                            let text = format!("{} GB", mb / 1024);
                            if mb < floor {
                                chip_off(&text).into_any_element()
                            } else {
                                chip(("inst-mem", mb as usize), &text, mb == now, false)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let mut s = this.settings.clone();
                                        s.set_memory_mb(mode, mb);
                                        this.save_settings(s, cx);
                                    }))
                                    .into_any_element()
                            }
                        }),
                    ))
                    .child(
                        inter(12., FontWeight::NORMAL, 1.4)
                            .text_color(faint3())
                            .child(t("inst.memory_hint")),
                    ),
            );
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(section_head(&t("inst.tools")))
                    .child(
                        inter(13., FontWeight::NORMAL, 1.4)
                            .text_color(muted3())
                            .child(self.tools_line(mode)),
                    )
                    .child(
                        tertiary("inst-tools", &t("inst.tools_open"))
                            .on_click(cx.listener(|this, _, _, cx| this.navigate(Page::Tools, cx))),
                    ),
            );
        }
        let dir = self.inst_dir(key);
        let shown = dir.display().to_string();
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(section_head(&t("inst.location")))
                .child(
                    plex(12., FontWeight::NORMAL)
                        .whitespace_nowrap()
                        .text_color(muted3())
                        .child(ellipsize(
                            &shown,
                            PLEX,
                            FontWeight::NORMAL,
                            12.,
                            PANE_W,
                            Cut::Middle,
                        )),
                )
                .child(
                    tertiary("inst-folder-2", &t("inst.open_folder"))
                        .on_click(cx.listener(move |_, _, _, _| open_folder(&dir))),
                ),
        );
        col.into_any_element()
    }

    /// "Ninjabrain Bot and Toolscreen start with the game", from the tool settings.
    fn tools_line(&self, mode: Mode) -> SharedString {
        let names: Vec<String> = crate::backend::ToolId::ALL
            .into_iter()
            .filter(|tool| self.settings.starts_with(*tool, mode))
            .map(|tool| tool.info().name.to_string())
            .collect();
        match names.as_slice() {
            [] => t("inst.tools_empty"),
            [one] => tf("inst.tools_single", &[("a", one)]),
            [rest @ .., last] => tf(
                "inst.tools_list",
                &[("list", &rest.join(", ")), ("last", last)],
            ),
        }
    }
}

/// A row of a list: a name with tags, an optional second line, the date and size on the right,
/// then the row's actions.
fn row(
    id: impl Into<gpui::ElementId>,
    name: &str,
    sub: Option<String>,
    tags: Vec<(SharedString, gpui::Hsla)>,
    when: Option<i64>,
    bytes: u64,
    actions: Vec<AnyElement>,
) -> gpui::Stateful<Div> {
    let name_w = PANE_W - 510.;
    div()
        .id(id.into())
        .relative()
        .h(px(ROW_H))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.))
        .px(px(4.))
        .border_t_1()
        .border_color(line3())
        .hover(|s| s.bg(text3a(0.04)))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_baseline()
                        .gap(px(10.))
                        .child(
                            inter(14., FontWeight::SEMIBOLD, 1.2)
                                .flex_none()
                                .whitespace_nowrap()
                                .text_color(text3())
                                .child(ellipsize(
                                    name,
                                    INTER,
                                    FontWeight::SEMIBOLD,
                                    14.,
                                    name_w,
                                    Cut::End,
                                )),
                        )
                        .children(tags.into_iter().map(|(tag, color)| {
                            inter(11., FontWeight::SEMIBOLD, 1.)
                                .flex_none()
                                .text_color(color)
                                .child(tag)
                        })),
                )
                .children(sub.map(|s| {
                    plex(11., FontWeight::NORMAL)
                        .whitespace_nowrap()
                        .text_color(faint3())
                        .child(ellipsize(
                            &s,
                            PLEX,
                            FontWeight::NORMAL,
                            11.,
                            name_w,
                            Cut::End,
                        ))
                })),
        )
        .child(
            plex(11., FontWeight::MEDIUM)
                .flex_none()
                .w(px(70.))
                .text_color(faint3())
                .child(when.map(fmt_when).unwrap_or_default()),
        )
        .child(
            plex(11., FontWeight::MEDIUM)
                .flex_none()
                .w(px(70.))
                .flex()
                .justify_end()
                .text_color(muted3())
                .child(if bytes > 0 {
                    size(bytes)
                } else {
                    String::new()
                }),
        )
        .child(
            div()
                .flex_none()
                .w(px(220.))
                .flex()
                .flex_row()
                .justify_end()
                .gap(px(16.))
                .children(actions),
        )
}

fn file_row(
    game_dir: &Path,
    folder: &'static str,
    i: usize,
    it: &content::Item,
    cx: &mut Context<Launcher>,
) -> gpui::Stateful<Div> {
    let path = game_dir.join(&it.rel);
    let open = path.clone();
    row(
        SharedString::from(format!("inst-{folder}-{i}")),
        &it.rel,
        None,
        Vec::new(),
        it.modified_ms,
        it.size,
        vec![
            tertiary(
                SharedString::from(format!("inst-{folder}-open-{i}")),
                &t("inst.open"),
            )
            .on_click(cx.listener(move |_, _, _, _| open_path(&open)))
            .into_any_element(),
        ],
    )
    .cursor_pointer()
    .on_click(cx.listener(move |_, _, _, _| open_path(&path)))
}
