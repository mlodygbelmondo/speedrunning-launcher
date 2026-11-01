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
mod custom;
mod settings;
mod tabs;

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

/// "Today 18:40" for today, else the short date ("Oct 24").
fn fmt_when(ms: i64) -> SharedString {
    use chrono::TimeZone;
    let (today, _) = today_and_month_ms();
    if ms >= today
        && let Some(d) = chrono::Local.timestamp_millis_opt(ms).earliest()
    {
        return tf("inst.today", &[("time", &d.format("%H:%M").to_string())]);
    }
    fmt_date(ms)
}

const LIST_X: f32 = 48.;
const LIST_W: f32 = 300.;
const PANE_X: f32 = 388.;
const PANE_R: f32 = 48.;
const PANE_W: f32 = 1280. - PANE_X - PANE_R;
const ROW_H: f32 = 52.;
/// How long a new note takes to fade and slide in.
pub const NOTE_MS: f32 = 220.;
/// How long a new custom instance's row takes to slide into the list.
pub const ROW_MS: f32 = 320.;

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
    /// The note last drawn and when it first was, for its fade-in.
    pub note_drawn: Option<SharedString>,
    pub note_at: Option<std::time::Instant>,
    /// The question last drawn in place of the toolbar, and when it appeared (it fades in like
    /// the note).
    pub confirm_drawn: Option<Confirm>,
    pub confirm_at: Option<std::time::Instant>,
    /// The custom instances the list showed last (`None` before the first frame), and the one
    /// that just joined it, sliding in.
    pub rows_drawn: Option<Vec<CustomId>>,
    pub row_in: Option<(CustomId, std::time::Instant)>,
    /// The game log's last lines, read with the scan, for the Logs tab.
    pub tail: Vec<String>,
    /// The seed last copied with "Copy seed" (its button says Copied).
    pub copied: Option<i64>,
    /// The config file shown in the page (its path in the game directory) and its first lines,
    /// or why it can't be shown.
    pub viewing: Option<(String, Result<Vec<SharedString>, SharedString>)>,
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
            note_drawn: None,
            note_at: None,
            confirm_drawn: None,
            confirm_at: None,
            rows_drawn: None,
            row_in: None,
            tail: Vec::new(),
            copied: None,
            viewing: None,
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
            v.viewing = None;
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

    /// ↑ and ↓ on the page: the instance above or below in the list, wrapping round. Not while
    /// the New instance form is open (its fields take the keys).
    pub fn step_instance(&mut self, down: bool, cx: &mut Context<Self>) {
        if self.instances.creating.is_some() {
            return;
        }
        let keys: Vec<InstKey> = InstKey::BUILT_IN
            .into_iter()
            .chain(
                self.settings
                    .custom_instances
                    .iter()
                    .map(|c| InstKey::Custom(c.id)),
            )
            .collect();
        let at = keys
            .iter()
            .position(|k| *k == self.instances.sel)
            .unwrap_or(0);
        let n = keys.len();
        let next = if down { (at + 1) % n } else { (at + n - 1) % n };
        // The rows are the list's children in this order: keep the picked one in view.
        self.instances.list_scroll.scroll_to_item(next);
        self.select_instance(keys[next], cx);
    }

    /// ← and → on the page: the tab before or after the selected one, wrapping round (Practice
    /// has no Mods tab).
    pub fn step_tab(&mut self, right: bool, cx: &mut Context<Self>) {
        if self.instances.creating.is_some() {
            return;
        }
        let key = self.instances.sel;
        let tabs: Vec<Tab> = Tab::ALL
            .into_iter()
            .filter(|tab| !(key == InstKey::Practice && *tab == Tab::Mods))
            .collect();
        let at = tabs
            .iter()
            .position(|t| *t == self.instances.tab)
            .unwrap_or(0);
        let n = tabs.len();
        let next = if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        };
        self.select_tab(tabs[next], cx);
    }

    fn select_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        let v = &mut self.instances;
        if v.tab != tab {
            v.tab = tab;
            self.kick_fade();
            let v = &mut self.instances;
            v.confirm = None;
            v.note = None;
            v.viewing = None;
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
        let v = &mut self.instances;
        if v.confirm != v.confirm_drawn {
            v.confirm_at = v.confirm.is_some().then(std::time::Instant::now);
            v.confirm_drawn = v.confirm.clone();
        }
        let ids: Vec<CustomId> = self
            .settings
            .custom_instances
            .iter()
            .map(|c| c.id)
            .collect();
        let v = &mut self.instances;
        if v.rows_drawn.as_ref() != Some(&ids) {
            if let Some(old) = &v.rows_drawn
                && let Some(new) = ids.iter().find(|id| !old.contains(id))
            {
                v.row_in = Some((*new, std::time::Instant::now()));
            }
            v.rows_drawn = Some(ids);
        }
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
                // Rows hidden above or below fade out at the edge, as the tabs' lists do.
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    // Room for the selected row's ring, which sits 5 px outside the row: the
                    // scroll area clips at its padding edge.
                    .m(px(-6.))
                    .child(
                        div()
                            .id("inst-keys")
                            .size_full()
                            .p(px(6.))
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
                    .child(fade_above(self.instances.list_scroll.clone(), 24.))
                    .child(fade_below(self.instances.list_scroll.clone(), 24.)),
            )
            .child(
                div().flex_none().pt(px(8.)).child(
                    // A white edge, as on a selected row, while its form is the one shown.
                    secondary("inst-new", &t("inst.new.open"))
                        .when(self.instances.creating.is_some(), |d| {
                            d.border_2().border_color(text3())
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_new_instance(cx);
                            window.focus(&this.inst_new_name.focus, cx);
                        })),
                ),
            )
    }

    /// One row of the list: the art, the name, the version and who plays in it.
    fn inst_row(&self, key: InstKey, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
        let on = key == self.instances.sel && self.instances.creating.is_none();
        let busy = self.inst_busy(key);
        // A row that just joined the list slides in from the left.
        let f = match (key, self.instances.row_in) {
            (InstKey::Custom(id), Some((new, at))) if id == new => crate::app::ease_out(
                (crate::app::ms_since(at, std::time::Instant::now()) / ROW_MS).min(1.0),
            ),
            _ => 1.0,
        };
        div()
            .id(SharedString::from(format!("inst-row-{}", key.id())))
            .group("inst-row")
            .relative()
            .left(px(-24. * (1.0 - f)))
            .when(f < 1.0, |d| d.opacity(f))
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
        // What the tabs don't count: the size on disk (nothing when it isn't installed, "…" while
        // its folder is still being read) and the Java it runs on. Worlds and mods are counted
        // on their tabs.
        let mut facts: Vec<(SharedString, SharedString)> = Vec::new();
        match &self.instances.content {
            Some(c) if c.exists => facts.push((t("inst.fact.disk"), size(c.size).into())),
            Some(_) => {}
            None => facts.push((t("inst.fact.disk"), "…".into())),
        }
        if let Some(java) = self.inst_info(key).and_then(|info| info.java.as_ref()) {
            facts.push((t("inst.fact.java"), java.clone().into()));
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
                    // Two lines' room whatever the text, so Play and the tabs stay put when
                    // another instance is picked.
                    .min_h(px(2. * 14. * 1.5))
                    .whitespace_nowrap()
                    .text_color(muted3())
                    .child(wrap_even(
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
                        // The facts sit beside the title, not this: nearly the pane's width.
                        PANE_W - 80.,
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
            .children(self.note_line())
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

    /// The last change's note under the tabs; a new one fades and slides in.
    fn note_line(&mut self) -> Option<Div> {
        let now = std::time::Instant::now();
        let v = &mut self.instances;
        let text = v.note.as_ref().map(|(s, _)| s.clone());
        if text != v.note_drawn {
            v.note_at = text.is_some().then_some(now);
            v.note_drawn = text;
        }
        let (s, bad) = v.note.clone()?;
        let f = v.note_at.map_or(1.0, |at| {
            crate::app::ease_out((crate::app::ms_since(at, now) / NOTE_MS).min(1.0))
        });
        Some(
            inter(12., FontWeight::MEDIUM, 1.3)
                .flex_none()
                .pt(px(12.))
                .relative()
                .left(px(-8. * (1.0 - f)))
                .when(f < 1.0, |d| d.opacity(f))
                .text_color(if bad { behind3() } else { ok3() })
                .child(s),
        )
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
                    // The mode Play starts: its mods and Atum settings go in before the game.
                    row = row.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.))
                            .child(label(&t("inst.mode_pick")).mr(px(4.)))
                            .children(Mode::SHARED.into_iter().map(|m| {
                                let i = index(m);
                                chip(("inst-mode", i), catalog::MODES[i].short, i == mi, false)
                                    .on_click(
                                        cx.listener(move |this, _, _, cx| this.select_mode(i, cx)),
                                    )
                            })),
                    );
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

    /// The mode's (or custom instance's) install info: its mod catalogue and Java.
    fn inst_info(&self, key: InstKey) -> Option<&crate::backend::ModeInfo> {
        match key {
            InstKey::Custom(id) => self.customs.get(&id).map(|s| &s.info),
            _ => self.inst_mode(key).map(|m| &self.modes[index(m)].info),
        }
    }

    /// How many of the catalogue's mods are on.
    fn mods_on(info: &crate::backend::ModeInfo) -> Option<usize> {
        info.catalogue
            .as_ref()
            .map(|cat| cat.iter().filter(|a| a.on).count())
    }

    fn inst_tabs(&self, key: InstKey, cx: &mut Context<Self>) -> Div {
        let c = self.instances.content.as_ref();
        let mods = self.inst_info(key).and_then(Self::mods_on);
        let count = |tab: Tab| -> Option<usize> {
            if tab == Tab::Mods {
                return mods;
            }
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
        let grow = self.mode_fade().text;
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
                        // The selected tab's underline grows out from its middle while the tab's
                        // content fades in.
                        let inset = (1.0 - grow) / 2.0;
                        let bar = on.then(|| {
                            div()
                                .absolute()
                                .bottom_0()
                                .h(px(2.))
                                .left(gpui::relative(inset))
                                .right(gpui::relative(inset))
                                .bg(accent2())
                        });
                        div()
                            .id(SharedString::from(format!("inst-tab-{}", tab.key())))
                            .group("inst-tab")
                            .relative()
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .gap(px(6.))
                            .pb(px(12.))
                            .children(bar)
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
        for_good: bool,
        on_yes: impl Fn(&mut Launcher, &mut Context<Launcher>) + 'static,
        cx: &mut Context<Self>,
    ) -> Div {
        let yes = if for_good {
            danger("inst-yes", yes)
        } else {
            primary("inst-yes", yes)
        };
        let f = self.instances.confirm_at.map_or(1.0, |at| {
            crate::app::ease_out(
                (crate::app::ms_since(at, std::time::Instant::now()) / NOTE_MS).min(1.0),
            )
        });
        div()
            .flex_none()
            .relative()
            .top(px(4. * (1.0 - f)))
            .when(f < 1.0, |d| d.opacity(f))
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
            .child(yes.on_click(cx.listener(move |this, _, _, cx| on_yes(this, cx))))
            .child(
                tertiary("inst-no", &t("inst.keep")).on_click(cx.listener(|this, _, _, cx| {
                    this.instances.confirm = None;
                    cx.notify();
                })),
            )
    }

    /// A tick box and its label, clicked as one ("Include worlds (1 MB)").
    fn tick_line(&self, id: &'static str, text: &str, on: bool) -> gpui::Stateful<Div> {
        div()
            .id(id)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .cursor_pointer()
            .group(id)
            .child(crate::screens::import::tick(on, true))
            .child(
                inter(13., FontWeight::MEDIUM, 1.)
                    .whitespace_nowrap()
                    .text_color(if on { text3() } else { muted3() })
                    .group_hover(id, |s| s.text_color(text3()))
                    .child(SharedString::from(text.to_string())),
            )
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
