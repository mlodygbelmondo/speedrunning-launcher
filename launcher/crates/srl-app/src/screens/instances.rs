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
    SharedString, Styled, Window, div, img, prelude::*, px,
};
use srl_core::{CustomId, Instance, Mode, Stage};

use crate::{
    app::{Launcher, Page},
    assets::card_path,
    backend::{
        COPY_CANCELLED, CopyProgress, Created, CustomInstance, JvmArgProblem, MEMORY_CHOICES_MB,
        ModeState, NameProblem, NewInstance,
        content::{self, Content},
        index, open_folder, open_path, validate_jvm_args,
    },
    catalog,
    copy::{size, t, tf, tn},
    v3::*,
};

use super::field::Field;
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
    /// One the player made (`Settings::custom_instances`), `instances/custom-<n>`.
    Custom(CustomId),
}

impl InstKey {
    /// The launcher's own instances, listed first.
    pub const BUILT_IN: [InstKey; 3] = [InstKey::Shared, InstKey::Draftout, InstKey::Practice];

    /// The folder under `instances/`.
    pub fn dir(self) -> String {
        match self {
            InstKey::Shared => srl_core::shared::DIR.to_string(),
            InstKey::Draftout => "draftout".to_string(),
            InstKey::Practice => "practice".to_string(),
            InstKey::Custom(id) => Instance::Custom(id).dir_id().into_owned(),
        }
    }

    /// The copy key part and the element ids: `shared`, `draftout`, `practice`, `c<n>`.
    fn id(self) -> String {
        match self {
            InstKey::Shared => "shared".into(),
            InstKey::Draftout => "draftout".into(),
            InstKey::Practice => "practice".into(),
            InstKey::Custom(id) => format!("c{}", id.0),
        }
    }

    /// `shared`, `draftout`, `practice` or `c<n>` (the deep links).
    pub fn from_id(id: &str) -> Option<InstKey> {
        if let Some(n) = id.strip_prefix('c').and_then(|n| n.parse().ok()) {
            return Some(InstKey::Custom(CustomId(n)));
        }
        InstKey::BUILT_IN.into_iter().find(|k| k.id() == id)
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
    /// Remove the selected custom instance (its folder goes to the trash).
    Instance,
}

/// The New instance form while it's open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewForm {
    pub base: Mode,
    /// Start with the options, keys and packs of the instance of the same Minecraft version.
    pub copy: bool,
    /// Or bring this instance of another launcher over (its game directory and name).
    pub source: Option<(PathBuf, String)>,
    /// With `source`: its worlds too.
    pub worlds: bool,
}

impl Default for NewForm {
    fn default() -> Self {
        Self {
            base: Mode::Rsg,
            copy: true,
            source: None,
            worlds: false,
        }
    }
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
    pub list_scroll: gpui::ScrollHandle,
    /// The backdrop art before the last switch, faded out under the new one.
    pub prev_art: Option<&'static str>,
    pub confirm: Option<Confirm>,
    /// What the last change did, or why it failed (`true`: a failure).
    pub note: Option<(SharedString, bool)>,
    /// The game log's last lines, read with the scan, for the Logs tab.
    pub tail: Vec<String>,
    /// The seed last copied with "Copy seed" (its button says Copied).
    pub copied: Option<i64>,
    /// The New instance form, in place of the pane.
    pub creating: Option<NewForm>,
    /// Why the name in the New instance or rename field can't be saved.
    pub name_problem: Option<SharedString>,
    /// Why the JVM arguments in the field can't be saved.
    pub jvm_problem: Option<SharedString>,
    /// "Make a copy" takes the worlds too.
    pub copy_worlds: bool,
    /// The New instance form's create is under way.
    pub submitting: bool,
    /// The New instance form is bringing another launcher's instance over (`copying` has the
    /// progress).
    pub importing: bool,
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
            list_scroll: gpui::ScrollHandle::new(),
            prev_art: None,
            confirm: None,
            note: None,
            tail: Vec::new(),
            copied: None,
            creating: None,
            name_problem: None,
            jvm_problem: None,
            copy_worlds: false,
            submitting: false,
            importing: false,
        }
    }
}

impl Launcher {
    /// The game directory of `key`.
    pub fn inst_dir(&self, key: InstKey) -> PathBuf {
        self.data_dir.join("instances").join(key.dir())
    }

    /// The mode Play and the Mods tab use for a built-in `key`: the selected mode in the shared
    /// instance (Ranked when Draftout is selected), Draftout in its own. `None` for practice and
    /// custom instances (`inst_base` gives a custom one's base).
    fn inst_mode(&self, key: InstKey) -> Option<Mode> {
        match key {
            InstKey::Shared => {
                let m = self.mode(self.mode_sel);
                Some(if m.is_shared() { m } else { Mode::Ranked })
            }
            InstKey::Draftout => Some(Mode::Draftout),
            InstKey::Practice | InstKey::Custom(_) => None,
        }
    }

    /// The instance the pipeline knows `key` as (the shared one as its selected mode).
    pub fn inst_instance(&self, key: InstKey) -> Instance {
        match key {
            InstKey::Custom(id) => Instance::Custom(id),
            InstKey::Practice => Instance::Practice,
            _ => Instance::Mode(self.inst_mode(key).unwrap_or(Mode::Ranked)),
        }
    }

    /// The mode whose rules `key` plays by: its own, a custom instance's base, RSG for practice.
    fn inst_base(&self, key: InstKey) -> Mode {
        match key {
            InstKey::Custom(id) => self.settings.custom(id).map_or(Mode::Rsg, |c| c.base),
            InstKey::Practice => Mode::Rsg,
            _ => self.inst_mode(key).unwrap_or(Mode::Ranked),
        }
    }

    fn inst_minecraft(&self, key: InstKey) -> &'static str {
        self.inst_base(key).minecraft_version()
    }

    /// The key art its row and backdrop use: a custom instance has its base mode's.
    fn inst_art(&self, key: InstKey) -> &'static str {
        let mode = match key {
            InstKey::Shared => Mode::Rsg,
            InstKey::Practice => Mode::Fsg,
            _ => self.inst_base(key),
        };
        catalog::MODES[index(mode)].art
    }

    /// What the page calls the instance.
    pub fn inst_name(&self, key: InstKey) -> SharedString {
        match key {
            InstKey::Custom(id) => self
                .settings
                .custom(id)
                .map(|c| SharedString::from(c.name.clone()))
                .unwrap_or_default(),
            _ => t(&format!("inst.{}.name", key.id())),
        }
    }

    /// A custom instance's state (Play), `None` for the others.
    fn custom_state(&self, key: InstKey) -> Option<&ModeState> {
        match key {
            InstKey::Custom(id) => Some(
                self.customs
                    .get(&id)
                    .map(|s| &s.state)
                    .unwrap_or(&ModeState::NotInstalled),
            ),
            _ => None,
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
        // The fields' blur saves them only at the next draw, after their text was replaced
        // with the new instance's: save what was typed for the old one first.
        if self.instances.sel != key {
            self.commit_rename(cx);
            self.commit_jvm(cx);
        }
        if self.instances.sel != key || self.instances.creating.is_some() {
            self.instances.prev_art = Some(self.inst_art(self.instances.sel));
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
        self.instances.creating = None;
        self.load_custom_fields(key);
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
            InstKey::Custom(_) => matches!(
                self.custom_state(key),
                Some(ModeState::Running | ModeState::Installing(_))
            ),
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

    pub fn render_instances(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let key = self.instances.sel;
        let art = self.inst_art(key);
        let fade = self.mode_fade().art;
        // The new instance's art fades in over the old one's, as Home's key art does.
        let prev = self
            .instances
            .prev_art
            .filter(|p| *p != art && fade < 1.0)
            .map(|p| art_fill(card_path(p)));
        div()
            .size_full()
            .relative()
            .children(prev)
            .child(art_fill(card_path(art)).when(fade < 1.0, |i| i.opacity(fade)))
            .child(overlay(bg3a(0.9)))
            .child(shade_up(1.0, 0.0, 0.6))
            .child(self.inst_list(cx))
            .child(self.inst_pane(key, window, cx))
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
            .bottom(px(28.))
            .child(section_head(&t("inst.list")).pb(px(4.)))
            .child(
                div()
                    .id("inst-keys")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.instances.list_scroll)
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .children({
                        let mut rows = Vec::new();
                        for k in InstKey::BUILT_IN {
                            rows.push(self.inst_row(k, cx));
                        }
                        for c in &self.settings.custom_instances {
                            rows.push(self.inst_row(InstKey::Custom(c.id), cx));
                        }
                        rows
                    }),
            )
            .child(div().flex_none().pt(px(8.)).child(
                secondary("inst-new", &t("inst.new.open")).on_click(cx.listener(
                    |this, _, window, cx| {
                        this.open_new_instance(cx);
                        window.focus(&this.inst_new_name.focus, cx);
                    },
                )),
            ))
    }

    /// One row of the list: the art, the name, the version and who plays in it.
    fn inst_row(&self, key: InstKey, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
        let on = key == self.instances.sel && self.instances.creating.is_none();
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
            .child(art_fill(card_path(self.inst_art(key))))
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
                        &self.inst_name(key),
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
                            .child(self.inst_minecraft(key)),
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
            InstKey::Custom(_) => tf(
                "inst.custom.who",
                &[("mode", catalog::MODES[index(self.inst_base(key))].short)],
            ),
            _ => t(&format!("inst.{}.who", key.id())),
        }
    }

    fn inst_pane(&mut self, key: InstKey, window: &Window, cx: &mut Context<Self>) -> Div {
        if self.instances.creating.is_some() {
            return self.new_instance_pane(window, cx);
        }
        let mut facts: Vec<(SharedString, SharedString)> = Vec::new();
        if let Some(c) = self.instances.content.as_ref().filter(|c| c.exists) {
            facts.push((t("inst.fact.worlds"), c.worlds.len().to_string().into()));
            facts.push((t("inst.fact.disk"), size(c.size).into()));
        }
        let info = match key {
            InstKey::Custom(id) => self.customs.get(&id).map(|s| &s.info),
            _ => self.inst_mode(key).map(|m| &self.modes[index(m)].info),
        };
        if let Some(info) = info {
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
                &tf("inst.eyebrow", &[("mc", self.inst_minecraft(key))]),
                accent2(),
            ))
            .child(
                tight(
                    &self.inst_name(key),
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
                        &match key {
                            InstKey::Custom(_) => tf(
                                "inst.custom.body",
                                &[("mode", catalog::MODES[index(self.inst_base(key))].short)],
                            ),
                            _ => t(&format!("inst.{}.body", key.id())),
                        },
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
            .children(self.instances.note.clone().map(|(s, bad)| {
                inter(12., FontWeight::MEDIUM, 1.3)
                    .flex_none()
                    .pt(px(12.))
                    .text_color(if bad { behind3() } else { ok3() })
                    .child(s)
            }))
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
                    .child(self.inst_tab_body(key, window, cx))
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
            InstKey::Custom(id) => {
                row = row.child(self.custom_play(id, cx));
                if matches!(
                    self.custom_state(key),
                    Some(ModeState::Installing(_) | ModeState::Running)
                ) {
                    let running = self.custom_state(key) == Some(&ModeState::Running);
                    row = row.child(
                        tertiary(
                            "inst-custom-cancel",
                            &t(if running { "game.stop" } else { "v3.cancel" }),
                        )
                        .on_click(
                            cx.listener(move |this, _, _, _| this.backend().cancel_custom(id)),
                        ),
                    );
                }
            }
        }
        let instance = self.inst_instance(key);
        row.child(div().flex_1()).child(
            tertiary("inst-folder", &t("inst.open_folder")).on_click(
                cx.listener(move |this, _, _, _| this.backend().open_game_folder(instance)),
            ),
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

    fn inst_tab_body(
        &mut self,
        key: InstKey,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.instances.tab == Tab::Mods && key != InstKey::Practice {
            return self.inst_mods(self.inst_instance(key), cx);
        }
        if self.instances.tab == Tab::Settings {
            return self.inst_settings(key, window, cx);
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
                            &match key {
                                InstKey::Custom(_) => t("inst.custom.empty"),
                                _ => t(&format!("inst.{}.empty", key.id())),
                            },
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

    fn inst_settings(
        &mut self,
        key: InstKey,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut col = div().pt(px(20.)).flex().flex_col().gap(px(26.));
        let custom = match key {
            InstKey::Custom(id) => self.settings.custom(id).cloned(),
            _ => None,
        };
        if custom.is_some() {
            let field = self.name_input(Field::Rename, 420., 36., 14., window, cx);
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(section_head(&t("inst.custom.name")))
                    .child(field)
                    .children(self.instances.name_problem.clone().map(|p| {
                        inter(12., FontWeight::MEDIUM, 1.3)
                            .text_color(behind3())
                            .child(p)
                    })),
            );
        }
        if let Some(mode) = self.inst_mode(key).or(custom.as_ref().map(|c| c.base)) {
            let now = self.settings.memory_mb_of(self.inst_instance(key));
            let own = custom.as_ref().and_then(|c| c.memory_mb);
            let custom_id = custom.as_ref().map(|c| c.id);
            let floor = crate::backend::mode_needs(mode).min_memory_mb;
            let short = catalog::MODES[index(mode)].short;
            let same = custom_id.map(|id| {
                chip(
                    "inst-mem-same",
                    &tf("inst.custom.memory_same", &[("mode", short)]),
                    own.is_none(),
                    false,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let mut s = this.settings.clone();
                    if let Some(c) = s.custom_mut(id) {
                        c.memory_mb = None;
                    }
                    this.save_settings(s, cx);
                }))
            });
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(section_head(&if custom_id.is_some() {
                        t("inst.custom.memory")
                    } else {
                        tf("inst.memory", &[("mode", short)])
                    }))
                    .child(div().flex().flex_row().gap(px(6.)).children(same).children(
                        MEMORY_CHOICES_MB.into_iter().map(|mb| {
                            let text = format!("{} GB", mb / 1024);
                            let on = mb == now && (custom_id.is_none() || own.is_some());
                            if mb < floor {
                                chip_off(&text).into_any_element()
                            } else {
                                chip(("inst-mem", mb as usize), &text, on, false)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let mut s = this.settings.clone();
                                        match custom_id.and_then(|id| s.custom_mut(id)) {
                                            Some(c) => c.memory_mb = Some(mb),
                                            None => s.set_memory_mb(mode, mb),
                                        }
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
        if custom.is_some() {
            let field = self.name_input(Field::Jvm, 640., 36., 13., window, cx);
            let below = match self.instances.jvm_problem.clone() {
                Some(p) => inter(12., FontWeight::MEDIUM, 1.3)
                    .text_color(behind3())
                    .child(p),
                None => inter(12., FontWeight::NORMAL, 1.4)
                    .text_color(faint3())
                    .child(t("inst.custom.jvm_hint")),
            };
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(section_head(&t("inst.custom.jvm")))
                    .child(field)
                    .child(below),
            );
        }
        if key != InstKey::Practice {
            col = col.child(self.copy_section(key, cx));
        }
        let instance = self.inst_instance(key);
        let shown = self.inst_dir(key).display().to_string();
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
                .child(tertiary("inst-folder-2", &t("inst.open_folder")).on_click(
                    cx.listener(move |this, _, _, _| this.backend().open_game_folder(instance)),
                )),
        );
        if let Some(c) = custom {
            let id = c.id;
            let body = if self.instances.confirm == Some(Confirm::Instance) {
                self.ask(
                    tf("inst.custom.delete_ask", &[("name", &c.name)]),
                    &t("inst.custom.delete_yes"),
                    move |this, cx| {
                        this.instances.confirm = None;
                        this.backend().delete_instance(id);
                        cx.notify();
                    },
                    cx,
                )
            } else if self.inst_busy(key) {
                div().child(tertiary_off(&t("inst.custom.delete")))
            } else {
                div().child(tertiary("inst-delete", &t("inst.custom.delete")).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.instances.confirm = Some(Confirm::Instance);
                        cx.notify();
                    }),
                ))
            };
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(section_head(&t("inst.custom.remove")))
                    .child(
                        inter(12., FontWeight::NORMAL, 1.4)
                            .text_color(faint3())
                            .child(t("inst.custom.remove_hint")),
                    )
                    .child(body),
            );
        }
        div()
            .id("inst-settings")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .track_scroll(&self.instances.scroll)
            .child(col.pb(px(24.)))
            .into_any_element()
    }

    /// "Make a copy": a new custom instance with this one's files (worlds only if asked).
    fn copy_section(&self, key: InstKey, cx: &mut Context<Self>) -> Div {
        let mut body = div().flex().flex_row().items_center().gap(px(16.));
        if let Some(p) = &self.copying {
            let frac = if p.bytes_total > 0 {
                p.bytes as f32 / p.bytes_total as f32
            } else {
                0.
            };
            body = body
                .child(
                    div()
                        .w(px(420.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(thin_progress(frac, 3.))
                        .child(plex(11., FontWeight::MEDIUM).text_color(muted3()).child(
                            if p.bytes_total == 0 {
                                tf("inst.copy.counting", &[("name", &p.name)])
                            } else {
                                tf(
                                    "inst.copy.progress",
                                    &[
                                        ("name", &p.name),
                                        ("done", &size(p.bytes)),
                                        ("total", &size(p.bytes_total)),
                                    ],
                                )
                            },
                        )),
                )
                .child(
                    tertiary("inst-copy-cancel", &t("v3.cancel"))
                        .on_click(cx.listener(|this, _, _, _| this.backend().cancel_copy())),
                );
        } else {
            let worlds = self.instances.copy_worlds;
            let saves = self.instances.content.as_ref().map_or(0, |c| c.saves_size);
            body = body
                .child(
                    secondary("inst-copy", &t("inst.copy.make")).on_click(cx.listener(
                        move |this, _, _, cx| {
                            let from = this.inst_instance(key);
                            let name = this.settings.unique_instance_name(&tf(
                                "inst.copy.name",
                                &[("name", &this.inst_name(key))],
                            ));
                            // Shown at once, so a second click can't start another copy
                            // while the files are being counted.
                            this.copying = Some(CopyProgress {
                                id: CustomId(0),
                                name: name.clone(),
                                files: 0,
                                files_total: 0,
                                bytes: 0,
                                bytes_total: 0,
                            });
                            this.backend().duplicate_instance(
                                from,
                                name,
                                this.instances.copy_worlds,
                            );
                            this.instances.note = None;
                            cx.notify();
                        },
                    )),
                )
                .child(
                    chip(
                        "inst-copy-worlds",
                        &if saves > 0 {
                            tf("inst.copy.worlds_size", &[("size", &size(saves))])
                        } else {
                            t("inst.copy.worlds")
                        },
                        worlds,
                        true,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.instances.copy_worlds = !this.instances.copy_worlds;
                        cx.notify();
                    })),
                );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(section_head(&t("inst.copy.head")))
            .child(
                inter(12., FontWeight::NORMAL, 1.4)
                    .text_color(faint3())
                    .child(t("inst.copy.hint")),
            )
            .child(body)
    }

    /// Fills the rename and JVM fields from the selected custom instance.
    pub fn load_custom_fields(&mut self, key: InstKey) {
        self.instances.name_problem = None;
        self.instances.jvm_problem = None;
        if let InstKey::Custom(id) = key
            && let Some(c) = self.settings.custom(id)
        {
            self.inst_rename.text = c.name.clone();
            self.inst_jvm.text = c.jvm_args.join(" ");
        }
    }

    /// The custom instances changed (created, removed, renamed): the list follows; a removed
    /// instance that was selected gives way to the shared one.
    pub fn customs_changed(&mut self, list: Vec<CustomInstance>, cx: &mut Context<Self>) {
        let ids: Vec<CustomId> = list.iter().map(|c| c.id).collect();
        self.settings.custom_instances = list;
        self.customs.retain(|id, _| ids.contains(id));
        if let InstKey::Custom(id) = self.instances.sel
            && !ids.contains(&id)
        {
            self.select_instance(InstKey::Shared, cx);
        }
        cx.notify();
    }

    /// The New instance form's create ended.
    pub fn instance_created(&mut self, answer: Result<Created, String>, cx: &mut Context<Self>) {
        self.instances.submitting = false;
        match answer {
            Ok(made) => {
                self.instances.creating = None;
                self.inst_new_name.text.clear();
                self.refocus_root = true;
                self.select_instance(InstKey::Custom(made.id), cx);
                self.instances.tab = Tab::Mods;
                self.instances.note = Some((tf("inst.new.made", &[("name", &made.name)]), false));
            }
            // Under the name field, where the form is.
            Err(why) => {
                self.instances.name_problem = Some(tf("inst.new.failed", &[("why", &why)]));
            }
        }
        cx.notify();
    }

    /// A "Make a copy" ended. The copy is selected, unless the New instance form is open (the
    /// player is typing there).
    pub fn instance_copied(&mut self, answer: Result<Created, String>, cx: &mut Context<Self>) {
        self.copying = None;
        if std::mem::take(&mut self.instances.importing) {
            match answer {
                Ok(made) => {
                    self.instances.creating = None;
                    self.inst_new_name.text.clear();
                    self.refocus_root = true;
                    self.select_instance(InstKey::Custom(made.id), cx);
                    self.instances.tab = Tab::Worlds;
                    self.instances.note = Some(if made.skipped.is_empty() {
                        (tf("inst.copy.done", &[("name", &made.name)]), false)
                    } else {
                        (tn("inst.copy.skipped", made.skipped.len() as u64), true)
                    });
                }
                Err(why) if why == COPY_CANCELLED => {
                    self.instances.name_problem = Some(t("inst.copy.cancelled"));
                }
                Err(why) => {
                    self.instances.name_problem = Some(tf("inst.copy.failed", &[("why", &why)]));
                }
            }
            cx.notify();
            return;
        }
        let note = match answer {
            Ok(made) => {
                if self.instances.creating.is_none() {
                    self.select_instance(InstKey::Custom(made.id), cx);
                }
                if made.skipped.is_empty() {
                    (tf("inst.copy.done", &[("name", &made.name)]), false)
                } else {
                    (tn("inst.copy.skipped", made.skipped.len() as u64), true)
                }
            }
            Err(why) if why == COPY_CANCELLED => (t("inst.copy.cancelled"), false),
            Err(why) => (tf("inst.copy.failed", &[("why", &why)]), true),
        };
        self.instances.note = Some(note);
        cx.notify();
    }

    pub fn instance_deleted(&mut self, answer: Result<CustomId, String>, cx: &mut Context<Self>) {
        self.instances.note = Some(match answer {
            Ok(_) => (t("inst.custom.deleted"), false),
            Err(why) => (tf("inst.custom.delete_failed", &[("why", &why)]), true),
        });
        cx.notify();
    }

    pub fn open_new_instance(&mut self, cx: &mut Context<Self>) {
        if !self.import.scanned {
            self.backend().import().scan();
        }
        self.instances.creating = Some(NewForm::default());
        self.instances.name_problem = None;
        self.instances.confirm = None;
        self.inst_new_name.text.clear();
        self.kick_fade();
        cx.notify();
    }

    /// The name a new instance gets when the field is left empty: "My instance", numbered if
    /// taken.
    pub fn default_instance_name(&self) -> String {
        let wanted = match self
            .instances
            .creating
            .as_ref()
            .and_then(|f| f.source.as_ref())
        {
            Some((_, name)) => name.clone(),
            None => t("inst.new.default_name").to_string(),
        };
        self.settings.unique_instance_name(&wanted)
    }

    pub fn submit_new_instance(&mut self, cx: &mut Context<Self>) {
        // One create at a time: a double click or Enter then a click would make two.
        if self.instances.submitting {
            return;
        }
        let Some(form) = self.instances.creating.clone() else {
            return;
        };
        let mut name = self.inst_new_name.text.clone();
        if name.trim().is_empty() {
            name = self.default_instance_name();
        }
        if let Err(p) = self.settings.validate_instance_name(&name, None) {
            self.instances.name_problem = Some(name_problem(p));
            cx.notify();
            return;
        }
        self.instances.name_problem = None;
        if let Some((dir, _)) = form.source {
            if self.copying.is_some() {
                self.instances.name_problem = Some(t("inst.new.wait_copy"));
                cx.notify();
                return;
            }
            self.instances.importing = true;
            self.copying = Some(CopyProgress {
                id: CustomId(0),
                name: name.clone(),
                files: 0,
                files_total: 0,
                bytes: 0,
                bytes_total: 0,
            });
            self.backend()
                .import_instance(dir, form.base, name, form.worlds);
            cx.notify();
            return;
        }
        self.instances.submitting = true;
        let from = Self::settings_source(form.base);
        self.backend().create_instance(NewInstance {
            name,
            base: form.base,
            copy_settings_from: form.copy.then(|| self.inst_instance(from)),
        });
    }

    /// Saves the rename field (on Enter or when it loses focus).
    pub fn commit_rename(&mut self, cx: &mut Context<Self>) {
        let InstKey::Custom(id) = self.instances.sel else {
            return;
        };
        let text = self.inst_rename.text.clone();
        if self
            .settings
            .custom(id)
            .is_some_and(|c| c.name == text.trim())
        {
            self.instances.name_problem = None;
            return;
        }
        let mut s = self.settings.clone();
        match s.rename_custom(id, &text) {
            Ok(()) => {
                self.instances.name_problem = None;
                self.save_settings(s, cx);
            }
            Err(p) => self.instances.name_problem = Some(name_problem(p)),
        }
        cx.notify();
    }

    /// Saves the JVM arguments field (split on spaces), if every argument is allowed.
    pub fn commit_jvm(&mut self, cx: &mut Context<Self>) {
        let InstKey::Custom(id) = self.instances.sel else {
            return;
        };
        let args: Vec<String> = self
            .inst_jvm
            .text
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        if let Err(p) = validate_jvm_args(&args) {
            self.instances.jvm_problem = Some(jvm_problem(&p));
            cx.notify();
            return;
        }
        self.instances.jvm_problem = None;
        let mut s = self.settings.clone();
        if let Some(c) = s.custom_mut(id) {
            c.jvm_args = args;
        }
        self.save_settings(s, cx);
        cx.notify();
    }

    /// Escape in a field of this page: the new instance form closes, the others go back to the
    /// saved value.
    pub fn cancel_inst_field(&mut self, which: Field, cx: &mut Context<Self>) {
        match which {
            Field::NewInstance => {
                self.instances.creating = None;
                self.instances.name_problem = None;
                self.refocus_root = true;
            }
            _ => self.load_custom_fields(self.instances.sel),
        }
        cx.notify();
    }

    /// Play for a custom instance: the same states as a mode's button, with the install progress
    /// under it.
    fn custom_play(&self, id: CustomId, cx: &mut Context<Self>) -> Div {
        let state = self
            .customs
            .get(&id)
            .map(|s| s.state.clone())
            .unwrap_or(ModeState::NotInstalled);
        let base = self.settings.custom(id).map_or(Mode::Rsg, |c| c.base);
        let (w, h) = (200., 44.);
        let mut progress = None;
        let (label, hint, live) = match &state {
            ModeState::NotInstalled => (
                t("v3.download"),
                tf(
                    "v3.download_about",
                    &[("size", &size(catalog::approx_download(base)))],
                ),
                true,
            ),
            ModeState::Installing(p) => {
                progress = Some(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(h + 7.))
                        .child(thin_progress(super::play::bar_fraction(p), 3.)),
                );
                let label = if matches!(p.stage, Stage::Launching | Stage::SigningIn) {
                    t("v3.starting")
                } else {
                    t("v3.installing")
                };
                (label, t(super::play::stage_key(p.stage)), false)
            }
            ModeState::Ready | ModeState::UpdateAvailable => (
                t("v3.play"),
                tf("v3.play_hint.plain", &[("mc", base.minecraft_version())]),
                true,
            ),
            ModeState::Running => (t("v3.running"), SharedString::default(), false),
            ModeState::Failed(_) => (t("v3.retry"), t("v3.retry_hint"), true),
        };
        let press = if live { Press::Live } else { Press::Busy };
        let btn = play_button("inst-custom-play", &label, hint, w, h, 15., press).when(live, |b| {
            b.on_click(cx.listener(move |this, _, _, cx| {
                if this.settings.player_name.is_none() {
                    this.return_to = Page::Instances;
                    this.navigate(Page::Name, cx);
                } else {
                    this.backend().play_custom(id);
                }
            }))
        });
        let failed = match &state {
            ModeState::Failed(f) => Some(
                inter(12., FontWeight::NORMAL, 1.3)
                    .absolute()
                    .left_0()
                    .top(px(h + 8.))
                    .w(px(560.))
                    .whitespace_nowrap()
                    .text_color(behind3())
                    .child(ellipsize(
                        &super::play::fail_detail(&f.detail),
                        INTER,
                        FontWeight::NORMAL,
                        12.,
                        560.,
                        Cut::End,
                    )),
            ),
            _ => None,
        };
        div()
            .relative()
            .flex_none()
            .child(btn)
            .children(progress)
            .children(failed)
    }

    /// The New instance form, in place of the instance's pane.
    fn new_instance_pane(&mut self, window: &Window, cx: &mut Context<Self>) -> Div {
        let form = self.instances.creating.clone().unwrap_or_default();
        let problem = self.instances.name_problem.clone();
        let base_chips = div()
            .flex()
            .flex_row()
            .gap(px(6.))
            .children(Mode::ALL.map(|m| {
                let i = index(m);
                chip(
                    ("inst-new-base", i),
                    catalog::MODES[i].short,
                    form.base == m,
                    false,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(f) = &mut this.instances.creating {
                        if f.base.minecraft_version() != m.minecraft_version() {
                            f.source = None;
                        }
                        f.base = m;
                    }
                    cx.notify();
                }))
            }));
        let from = Self::settings_source(form.base);
        let from_name = self.inst_name(from);
        let copy = chip(
            "inst-new-copy",
            &tf("inst.new.copy", &[("from", &from_name)]),
            form.copy && form.source.is_none(),
            true,
        )
        .on_click(cx.listener(|this, _, _, cx| {
            if let Some(f) = &mut this.instances.creating {
                f.copy = f.source.is_some() || !f.copy;
                f.source = None;
            }
            cx.notify();
        }));
        let start = self.import_choices(&form, cx);
        let section = |title: SharedString, body: Div| {
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(section_head(&title))
                .child(body)
        };
        let name_field = self.name_input(Field::NewInstance, 420., 40., 14., window, cx);
        div()
            .absolute()
            .left(px(PANE_X))
            .right(px(PANE_R))
            .top(px(100.))
            .flex()
            .flex_col()
            .gap(px(26.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(kicker(&t("inst.new.eyebrow"), accent2()))
                    .child(
                        tight(
                            &t("inst.new.title"),
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
                                &t("inst.new.body"),
                                INTER,
                                FontWeight::NORMAL,
                                14.,
                                600.,
                            )),
                    ),
            )
            .child(section(
                t("inst.new.name"),
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(name_field)
                    .children(problem.map(|p| {
                        inter(12., FontWeight::MEDIUM, 1.3)
                            .text_color(behind3())
                            .child(p)
                    })),
            ))
            .child(section(
                t("inst.new.base"),
                div().flex().flex_col().gap(px(8.)).child(base_chips).child(
                    inter(12., FontWeight::NORMAL, 1.4)
                        .text_color(faint3())
                        .child(tf(
                            "inst.new.base_hint",
                            &[("mc", form.base.minecraft_version())],
                        )),
                ),
            ))
            .child(section(
                t("inst.new.start"),
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(div().flex().flex_row().child(copy))
                    .children(start),
            ))
            .child(self.new_instance_actions(cx))
    }

    /// The other launchers' instances on the form's Minecraft version, to bring one over, and
    /// "With worlds" once one is picked. `None` when there are none.
    fn import_choices(&self, form: &NewForm, cx: &mut Context<Self>) -> Option<Div> {
        let mc = form.base.minecraft_version();
        let found: Vec<_> = self
            .import
            .found
            .iter()
            .filter(|s| s.runs(mc))
            .take(4)
            .collect();
        if found.is_empty() {
            return (!self.import.scanned).then(|| {
                inter(12., FontWeight::NORMAL, 1.4)
                    .text_color(faint3())
                    .child(t("inst.new.looking"))
            });
        }
        let mut chips = div().flex().flex_row().flex_wrap().gap(px(6.));
        for (i, s) in found.into_iter().enumerate() {
            let on = form.source.as_ref().is_some_and(|(d, _)| *d == s.game_dir);
            let launcher = super::import::launcher_name(s.launcher);
            let text = if launcher.is_empty() {
                s.name.clone()
            } else {
                format!("{} · {launcher}", s.name)
            };
            let pick = (s.game_dir.clone(), s.name.clone());
            chips = chips.child(
                chip(("inst-new-src", i), &text, on, true).on_click(cx.listener(
                    move |this, _, _, cx| {
                        if let Some(f) = &mut this.instances.creating {
                            f.source = if on { None } else { Some(pick.clone()) };
                        }
                        cx.notify();
                    },
                )),
            );
        }
        let worlds = form.source.is_some().then(|| {
            chip("inst-new-worlds", &t("inst.copy.worlds"), form.worlds, true).on_click(
                cx.listener(|this, _, _, cx| {
                    if let Some(f) = &mut this.instances.creating {
                        f.worlds = !f.worlds;
                    }
                    cx.notify();
                }),
            )
        });
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    inter(12., FontWeight::NORMAL, 1.4)
                        .text_color(faint3())
                        .child(t("inst.new.other")),
                )
                .child(chips)
                .children(worlds.map(|w| div().flex().flex_row().child(w))),
        )
    }

    /// Create and Cancel, or the progress of bringing an instance over.
    fn new_instance_actions(&self, cx: &mut Context<Self>) -> Div {
        let row = div().flex().flex_row().items_center().gap(px(24.));
        if self.instances.importing {
            let (frac, text) = match &self.copying {
                Some(p) if p.bytes_total > 0 => (
                    p.bytes as f32 / p.bytes_total as f32,
                    tf(
                        "inst.copy.progress",
                        &[
                            ("name", &p.name),
                            ("done", &size(p.bytes)),
                            ("total", &size(p.bytes_total)),
                        ],
                    ),
                ),
                Some(p) => (0., tf("inst.copy.counting", &[("name", &p.name)])),
                None => (0., SharedString::default()),
            };
            return row
                .child(
                    div()
                        .w(px(420.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(thin_progress(frac, 3.))
                        .child(
                            plex(11., FontWeight::MEDIUM)
                                .text_color(muted3())
                                .child(text),
                        ),
                )
                .child(
                    tertiary("inst-import-cancel", &t("v3.cancel"))
                        .on_click(cx.listener(|this, _, _, _| this.backend().cancel_copy())),
                );
        }
        row.child(
            primary("inst-new-create", &t("inst.new.create"))
                .on_click(cx.listener(|this, _, _, cx| this.submit_new_instance(cx))),
        )
        .child(
            tertiary("inst-new-cancel", &t("v3.cancel")).on_click(
                cx.listener(|this, _, _, cx| this.cancel_inst_field(Field::NewInstance, cx)),
            ),
        )
    }

    /// Where a new instance on `base` takes its options, keys and packs from: the instance of the
    /// same Minecraft version.
    fn settings_source(base: Mode) -> InstKey {
        if base.is_shared() {
            InstKey::Shared
        } else {
            InstKey::Draftout
        }
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

fn name_problem(p: NameProblem) -> SharedString {
    t(match p {
        NameProblem::Empty => "inst.name_problem.empty",
        NameProblem::TooLong => "inst.name_problem.too_long",
        NameProblem::Control => "inst.name_problem.control",
        NameProblem::Taken => "inst.name_problem.taken",
    })
}

fn jvm_problem(p: &JvmArgProblem) -> SharedString {
    match p {
        JvmArgProblem::Empty => t("inst.jvm_problem.empty"),
        JvmArgProblem::NotAnOption(a) => tf("inst.jvm_problem.not_option", &[("arg", a)]),
        JvmArgProblem::Memory(a) => tf("inst.jvm_problem.memory", &[("arg", a)]),
        JvmArgProblem::Command(a) => tf("inst.jvm_problem.command", &[("arg", a)]),
        JvmArgProblem::Agent(a) => tf("inst.jvm_problem.agent", &[("arg", a)]),
        JvmArgProblem::Loader(a) => tf("inst.jvm_problem.loader", &[("arg", a)]),
    }
}
