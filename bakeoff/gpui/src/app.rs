//! The root view: state, navigation with SPEC's page transition, the v3 shell (top nav, see
//! `shell.rs`), the v2 page header for the pages that keep their v2 layout, toast and the per-frame
//! bookkeeping.
//!
//! Every animation is time-driven: `render` reads `Instant::now()`, derives page opacity/offset,
//! glow opacity/scale, the list scroll offset and the short prototype animations (toggle knob,
//! spinner, toast, modal fade, fake-install progress), and calls `window.request_animation_frame()`
//! only while something moves. When nothing animates, no frames are requested at all. Holds
//! without motion (toast on screen, "Add account" highlight, "Fix and launch" wait) use timers.
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, IntoElement, ObjectFit, ParentElement, Render,
    SharedString, Styled, UniformListScrollHandle, Window, div, img, point, prelude::*, px,
    relative,
};
use serde_json::json;

use crate::{
    assets::embedded,
    copy::{Lang, t, tf},
    data::screens,
    report::{epoch_ms, report},
    screens::PracticeSel,
    theme::*,
    ui::*,
};

pub const OUT_MS: f32 = 150.0;
pub const IN_MS: f32 = 220.0;
const GLOW_PERIOD_S: f32 = 6.0;
const SCROLL_PX_PER_MS: f32 = 1.2;

pub const INSTALL_MS: f32 = 1200.0;
pub const SIGNIN_WAIT_MS: f32 = 1500.0;
pub const CHECK_NOW_MS: f32 = 1000.0;
const TOAST_IN_MS: f32 = 150.0;
const TOAST_HOLD_MS: f32 = 1800.0;
const TOAST_OUT_MS: f32 = 150.0;
pub const MODAL_IN_MS: f32 = 150.0;
pub const MODAL_OUT_MS: f32 = 120.0;
const HIGHLIGHT_MS: u64 = 600;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Home,
    Library,
    Onboarding,
    Accounts,
    Modes,
    Practice,
    Learn,
    Tools,
    Stats,
    Submit,
    Settings,
    Game,
}

impl Page {
    /// Onboarding and Game cover the whole window (no top nav).
    pub fn full_window(self) -> bool {
        matches!(self, Page::Onboarding | Page::Game)
    }

    /// Scene v3 pages draw their own full-window background and absolute layout (REDESIGN.md).
    /// The others keep their v2 layout under the new shell.
    pub fn v3(self) -> bool {
        matches!(self, Page::Home | Page::Modes | Page::Practice)
    }
}

#[derive(Clone, Copy)]
pub enum Anim {
    Idle,
    Out(Instant),
    In(Instant),
}

pub struct Recording {
    pub start: Instant,
    pub ts: Vec<f64>,
}

/// Something that can be fake-installed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Item {
    Map(usize),
    PracticeTool(usize),
    Tool(usize),
    Mode(usize),
}

pub enum Install {
    Done,
    Running(f32),
    Not,
}

/// Sign-in widget state (shared by the onboarding step and the Accounts card).
#[derive(Clone, Copy, PartialEq)]
pub enum Signin {
    Cards,
    Waiting(Instant),
    Code,
    Done,
}

pub const SIGNIN_ONBOARDING: usize = 0;
pub const SIGNIN_ACCOUNTS: usize = 1;

pub struct Modal {
    pub open: bool,
    pub start: Instant,
}

pub struct Scene {
    // --- measured scene (SPEC.md); the scenario drives these ---
    /// Page currently drawn.
    pub shown: Page,
    /// Page we are heading to (navigation requested while a transition runs is queued here).
    pub target: Page,
    pub anim: Anim,
    /// The running transition covers the whole window (to/from a full-window screen).
    trans_whole: bool,
    pub glow_running: bool,
    glow_start: Instant,
    pub list: UniformListScrollHandle,
    /// Scroll phase start (offset = f(now - start)).
    pub scroll_start: Option<Instant>,
    /// Frame timestamps for the transitions/scroll phases.
    pub recording: Option<Recording>,
    /// Render counter for the minimized phase.
    pub minimized_frames: Option<u64>,
    first_render_done: bool,
    phase_ms: Option<u64>,

    // --- prototype state (SCREENS.md) ---
    pub epoch: Instant,
    /// Set by components that animate while drawn (spinner, toggle slide, progress).
    pub want_frame: Cell<bool>,
    /// Copy key + start; the text is looked up when drawn, so it follows a language switch.
    pub toast: Option<(&'static str, Instant)>,
    pub toggles: HashMap<&'static str, bool>,
    pub toggle_anim: HashMap<&'static str, Instant>,
    pub installs: HashMap<Item, Instant>,
    pub installed: HashSet<Item>,
    pub signin: [Signin; 2],
    // onboarding
    pub ob_step: usize,
    pub ob_mode: usize,
    pub ob_eye: [bool; 3],
    /// Next is enabled on the sign-in step (deep-link states, see SCREENS.md).
    pub ob_signin_unlocked: bool,
    // accounts
    pub active_account: usize,
    pub add_highlight: Option<Instant>,
    // the selected mode (Home and Modes share it), an index into `modes[]`
    pub mode_sel: usize,
    // the mode selected before it, whose hero stays drawn under the new one (`hero_images`)
    pub prev_mode: Option<usize>,
    // pre-launch check: the modal and the mode it checks
    pub modal: Option<Modal>,
    pub check_mode: usize,
    pub check_fixed: Vec<bool>,
    // practice / learn
    pub practice_sel: PracticeSel,
    pub learn_filter: Option<&'static str>,
    // tools
    pub tool_keys: Vec<(&'static str, &'static str)>,
    pub tools_check: Option<Instant>,
    pub tools_checked: bool,
    // submit
    pub submit_link: bool,
    pub submit_focus: bool,
    pub submit_rules: [bool; 2],
    // settings (memory is shared with onboarding's Java step)
    pub memory: usize,
    pub keybinds: usize,
    pub channel: usize,
}

impl Scene {
    pub fn new(phase_ms: Option<u64>) -> Self {
        let s = screens();
        let mut toggles = HashMap::new();
        toggles.insert("settings.close_on_launch", s.settings.close_on_launch);
        toggles.insert("settings.start_with_system", s.settings.start_with_system);
        toggles.insert("settings.standard_settings", s.settings.standard_settings);
        toggles.insert("settings.auto_update", s.settings.auto_update);
        toggles.insert("onboarding.obs", true);
        // Per-tool toggle keys, leaked once (the tool list is static).
        let tool_keys: Vec<(&'static str, &'static str)> = s
            .tools
            .iter()
            .map(|tool| {
                let a: &'static str = Box::leak(format!("tool.{}.start", tool.id).into_boxed_str());
                let b: &'static str = Box::leak(format!("tool.{}.auto", tool.id).into_boxed_str());
                toggles.insert(a, tool.start_with_game);
                toggles.insert(b, tool.auto_update);
                (a, b)
            })
            .collect();
        let default_mode = s.mode_index(&s.v3.default_mode).unwrap_or(0);
        let ob_mode = s.modes.iter().position(|m| m.recommended).unwrap_or(0);
        let practice_default = s
            .practice_categories
            .iter()
            .position(|c| c.id == s.v3.practice_default)
            .unwrap_or(0);
        let on_windows = crate::data::os() == "windows";
        Self {
            shown: Page::Home,
            target: Page::Home,
            anim: Anim::Idle,
            trans_whole: false,
            glow_running: false,
            glow_start: Instant::now(),
            list: UniformListScrollHandle::new(),
            scroll_start: None,
            recording: None,
            minimized_frames: None,
            first_render_done: false,
            phase_ms,

            epoch: Instant::now(),
            want_frame: Cell::new(false),
            toast: None,
            toggles,
            toggle_anim: HashMap::new(),
            installs: HashMap::new(),
            installed: HashSet::new(),
            signin: [Signin::Cards; 2],
            ob_step: 0,
            ob_mode,
            ob_eye: [on_windows, !on_windows, true],
            ob_signin_unlocked: false,
            active_account: s.accounts.list.iter().position(|a| a.active).unwrap_or(0),
            add_highlight: None,
            mode_sel: default_mode,
            prev_mode: None,
            modal: None,
            check_mode: default_mode,
            check_fixed: vec![false; s.checks.len()],
            practice_sel: PracticeSel::Category(practice_default),
            learn_filter: None,
            tool_keys,
            tools_check: None,
            tools_checked: false,
            submit_link: false,
            submit_focus: false,
            submit_rules: [false; 2],
            memory: s.settings.memory_default,
            keybinds: s.settings.keybinds_default,
            channel: 0,
        }
    }

    /// `BAKEOFF_SCREEN=<id>`: open straight on a screen state (SCREENS.md "Screen ids").
    pub fn apply_deep_link(&mut self, id: &str) -> bool {
        let s = screens();
        let page = match id {
            "home" => {
                self.mode_sel = s.mode_index(&s.v3.default_mode).unwrap_or(0);
                Page::Home
            }
            "modes" => {
                self.mode_sel = s.mode_index("ranked").unwrap_or(0);
                Page::Modes
            }
            "library" => Page::Library,
            "accounts" => Page::Accounts,
            "practice" => Page::Practice,
            // Scene v3 additions (REDESIGN.md "Screen ids").
            "home-rsg" | "home-fsg" | "home-ssg" | "home-draftout" => {
                let Some(mi) = s.mode_index(&id["home-".len()..]) else {
                    return false;
                };
                self.mode_sel = mi;
                Page::Home
            }
            "modes-ssg" => {
                let Some(mi) = s.mode_index("ssg") else {
                    return false;
                };
                self.mode_sel = mi;
                Page::Modes
            }
            "practice-end" => {
                let Some(ci) = s.practice_categories.iter().position(|c| c.id == "end") else {
                    return false;
                };
                self.practice_sel = PracticeSel::Category(ci);
                Page::Practice
            }
            "learn" => Page::Learn,
            "tools" => Page::Tools,
            "stats" => Page::Stats,
            "submit" => Page::Submit,
            "settings" => Page::Settings,
            "game" => Page::Game,
            "check" => {
                self.modal = Some(Modal {
                    open: true,
                    start: Instant::now() - Duration::from_secs(1),
                });
                Page::Home
            }
            other => {
                let Some(step) = other.strip_prefix("onboarding-") else {
                    return false;
                };
                self.ob_step = match step {
                    "welcome" => 0,
                    "signin" | "signin-code" => 1,
                    "mode" => 2,
                    "java" => 3,
                    "eye" => 4,
                    "obs" => 5,
                    "done" => 6,
                    _ => return false,
                };
                if step == "signin-code" {
                    self.signin[SIGNIN_ONBOARDING] = Signin::Code;
                }
                // Screenshot states past or on the sign-in step don't block Next.
                self.ob_signin_unlocked = self.ob_step >= 1;
                Page::Onboarding
            }
        };
        self.shown = page;
        self.target = page;
        true
    }

    /// Selects a mode from a click (Home tiles, Modes rows).
    pub fn select_mode(&mut self, mi: usize, cx: &mut Context<Self>) {
        if self.mode_sel != mi {
            self.prev_mode = Some(self.mode_sel);
            self.mode_sel = mi;
            cx.notify();
        }
    }

    /// The selected mode's full-window hero over the previously selected one. `img()` decodes a
    /// hero on first use in the background and draws nothing until it's done, so on the first
    /// switch to a mode the old hero fills those frames instead of bare background. Both are
    /// opaque and already in the atlas, so the extra layer costs nothing visible or in memory.
    pub fn hero_images(&self) -> Vec<gpui::Img> {
        let s = screens();
        let hero = |mi: usize| {
            img(embedded(&crate::assets::hero_path(&s.v3_mode(mi).art)))
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .object_fit(ObjectFit::Cover)
        };
        let mut v: Vec<gpui::Img> = self
            .prev_mode
            .filter(|&p| p != self.mode_sel)
            .map(hero)
            .into_iter()
            .collect();
        v.push(hero(self.mode_sel));
        v
    }

    /// Same semantics as the web scene: outgoing 150 ms ease-in, then incoming 220 ms ease-out.
    /// A request made during a transition is remembered and started once the current one ends.
    pub fn navigate(&mut self, to: Page, cx: &mut Context<Self>) {
        self.target = to;
        if !matches!(self.anim, Anim::Idle) || to == self.shown {
            return;
        }
        self.trans_whole = self.shown.full_window() || to.full_window();
        self.anim = Anim::Out(Instant::now());
        cx.notify();
    }

    /// UI navigation: closes the pre-launch modal first.
    pub fn go(&mut self, to: Page, cx: &mut Context<Self>) {
        self.close_modal(cx);
        self.navigate(to, cx);
    }

    pub fn go_onboarding(&mut self, step: usize, cx: &mut Context<Self>) {
        self.ob_step = step;
        self.ob_signin_unlocked = false;
        self.signin[SIGNIN_ONBOARDING] = Signin::Cards;
        self.go(Page::Onboarding, cx);
    }

    pub fn set_glow(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.glow_running != on {
            self.glow_running = on;
            self.glow_start = Instant::now();
            cx.notify();
        }
    }

    /// Advances the page transition state machine; returns (opacity, translate-y px).
    fn page_transform(&mut self, now: Instant) -> (f32, f32) {
        match self.anim {
            Anim::Idle => (1.0, 0.0),
            Anim::Out(start) => {
                let t = ms_since(start, now) / OUT_MS;
                if t >= 1.0 {
                    self.trans_whole |= self.target.full_window();
                    self.shown = self.target;
                    if self.shown == Page::Library {
                        // A freshly mounted list starts at the top (as in the DOM).
                        self.list
                            .0
                            .borrow()
                            .base_handle
                            .set_offset(point(px(0.), px(0.)));
                    }
                    self.anim = Anim::In(now);
                    (0.0, 8.0)
                } else {
                    let e = ease_in(t);
                    (1.0 - e, 8.0 * e)
                }
            }
            Anim::In(start) => {
                let t = ms_since(start, now) / IN_MS;
                if t >= 1.0 {
                    self.anim = if self.target != self.shown {
                        self.trans_whole = self.shown.full_window() || self.target.full_window();
                        Anim::Out(now)
                    } else {
                        Anim::Idle
                    };
                    (1.0, 0.0)
                } else {
                    let e = ease_out(t);
                    (e, 8.0 * (1.0 - e))
                }
            }
        }
    }

    /// Glow pulse: 6 s loop, keyframes 0% / 50% / 100%, ease-in-out per segment.
    fn glow_transform(&self, now: Instant) -> (f32, f32) {
        if !self.glow_running {
            return (0.8, 1.0);
        }
        let t = now.saturating_duration_since(self.glow_start).as_secs_f32() % GLOW_PERIOD_S;
        let half = GLOW_PERIOD_S / 2.0;
        let k = if t < half {
            ease_in_out(t / half)
        } else {
            1.0 - ease_in_out((t - half) / half)
        };
        (0.65 + 0.35 * k, 1.0 + 0.08 * k)
    }

    fn apply_scroll(&self, now: Instant) {
        let Some(start) = self.scroll_start else {
            return;
        };
        if self.shown != Page::Library {
            return;
        }
        let state = self.list.0.borrow();
        let max = state.base_handle.max_offset().y.as_f32();
        if max <= 0.0 {
            return; // not laid out yet
        }
        let d = (ms_since(start, now) * SCROLL_PX_PER_MS) % (2.0 * max);
        let y = if d <= max { d } else { 2.0 * max - d };
        state.base_handle.set_offset(point(px(0.), px(-y)));
    }

    /// Advances the short prototype animations and timers.
    fn tick(&mut self, now: Instant) {
        let done: Vec<Item> = self
            .installs
            .iter()
            .filter(|(_, start)| ms_since(**start, now) >= INSTALL_MS)
            .map(|(k, _)| *k)
            .collect();
        for k in done {
            self.installs.remove(&k);
            self.installed.insert(k);
        }
        if !self.installs.is_empty() {
            self.want_frame.set(true);
        }
        for s in &mut self.signin {
            if let Signin::Waiting(start) = *s
                && ms_since(start, now) >= SIGNIN_WAIT_MS
            {
                *s = Signin::Done;
            }
        }
        if let Some(start) = self.tools_check
            && ms_since(start, now) >= CHECK_NOW_MS
        {
            self.tools_check = None;
            self.tools_checked = true;
        }
        if let Some((_, start)) = &self.toast {
            let e = ms_since(*start, now);
            if e >= TOAST_IN_MS + TOAST_HOLD_MS + TOAST_OUT_MS {
                self.toast = None;
            } else if e < TOAST_IN_MS || e >= TOAST_IN_MS + TOAST_HOLD_MS {
                self.want_frame.set(true);
            }
        }
        if let Some(m) = &self.modal {
            let e = ms_since(m.start, now);
            if m.open {
                if e < MODAL_IN_MS {
                    self.want_frame.set(true);
                }
            } else if e >= MODAL_OUT_MS {
                self.modal = None;
            } else {
                self.want_frame.set(true);
            }
        }
        self.toggle_anim
            .retain(|_, start| ms_since(*start, now) < crate::ui::TOGGLE_MS);
        if let Some(start) = self.add_highlight
            && ms_since(start, now) >= HIGHLIGHT_MS as f32
        {
            self.add_highlight = None;
        }
    }

    /// Notifies after `ms`, for holds that don't need frames.
    pub fn wake_after(&self, ms: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(ms))
                .await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    pub fn show_toast(&mut self, key: &'static str, cx: &mut Context<Self>) {
        self.toast = Some((key, Instant::now()));
        self.wake_after((TOAST_IN_MS + TOAST_HOLD_MS) as u64, cx);
        cx.notify();
    }

    /// Settings → Language: switches every string and the sample content at once (no transition)
    /// and retitles the window.
    pub fn set_language(&mut self, lang: Lang, window: &mut Window, cx: &mut Context<Self>) {
        if lang != crate::copy::lang() {
            crate::copy::set_lang(lang);
            window.set_window_title(&crate::copy::window_title());
            cx.notify();
        }
    }

    pub fn highlight_add_account(&mut self, cx: &mut Context<Self>) {
        self.add_highlight = Some(Instant::now());
        self.wake_after(HIGHLIGHT_MS, cx);
        cx.notify();
    }

    pub fn start_install(&mut self, item: Item, cx: &mut Context<Self>) {
        if !self.installed.contains(&item) && !self.installs.contains_key(&item) {
            self.installs.insert(item, Instant::now());
            cx.notify();
        }
    }

    pub fn install_state(&self, item: Item, installed_in_data: bool) -> Install {
        if installed_in_data || self.installed.contains(&item) {
            Install::Done
        } else if let Some(start) = self.installs.get(&item) {
            Install::Running(ms_since(*start, Instant::now()) / INSTALL_MS)
        } else {
            Install::Not
        }
    }

    // -----------------------------------------------------------------------------------------
    // Page header v2
    // -----------------------------------------------------------------------------------------

    fn header(&mut self, cx: &mut Context<Self>) -> Div {
        let s = screens();
        let (title, sub): (SharedString, SharedString) = match self.shown {
            Page::Library => (t("library.title"), t("library.subtitle")),
            Page::Accounts => (t("accounts.title"), t("accounts.subtitle")),
            Page::Learn => (t("learn.title"), t("learn.subtitle")),
            Page::Tools => {
                let ago = if self.tools_checked {
                    t("tools.checked_now")
                } else {
                    s.tools_checked_ago.clone().into()
                };
                (t("tools.title"), tf("tools.subtitle", &[("ago", &ago)]))
            }
            Page::Stats => (
                t("stats.title"),
                tf("stats.subtitle", &[("n", &s.stats.total_runs)]),
            ),
            Page::Submit => (
                t("submit.title"),
                tf(
                    "submit.subtitle",
                    &[
                        ("time", &s.submit.time),
                        ("category", &s.submit.category),
                        ("date", &s.submit.date),
                    ],
                ),
            ),
            Page::Settings => (t("settings.title"), t("settings.subtitle")),
            Page::Home | Page::Modes | Page::Practice | Page::Onboarding | Page::Game => {
                (SharedString::default(), SharedString::default())
            }
        };
        // Right-side actions, vertically centred on the 32 px title row.
        let actions: Option<(AnyElement, f32)> = match self.shown {
            Page::Stats | Page::Library => Some((self.stats_tabs(cx).into_any_element(), 36.)),
            Page::Tools => Some((self.tools_check_button(cx), 40.)),
            _ => None,
        };
        let title_block = div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(26.))
                    .line_height(px(32.))
                    .child(title),
            )
            .child(
                div()
                    .mt(px(4.))
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .text_color(muted())
                    .child(sub),
            );
        let row = div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(16.))
            .child(title_block)
            .when_some(actions, |d, (el, h)| {
                d.child(div().flex_none().mt(px((32. - h) / 2.)).child(el))
            });
        div()
            .flex_none()
            .flex()
            .flex_col()
            .when(self.shown == Page::Submit, |d| {
                d.child(
                    div().flex().flex_row().mt(px(-6.)).mb(px(8.)).ml(px(-12.)).child(
                        button("submit-back", t("nav.stats"), Kind::Ghost)
                            .small()
                            .icon("chevron-left")
                            .on_click(cx.listener(|this, _, _, cx| this.go(Page::Stats, cx))),
                    ),
                )
            })
            .child(row)
    }

    /// Stats header tabs: Overview | Leaderboard (Library).
    fn stats_tabs(&self, cx: &mut Context<Self>) -> Div {
        let selected = if self.shown == Page::Library { 1 } else { 0 };
        self.segmented(
            "stats-tabs",
            vec![t("stats.tab_overview"), t("stats.tab_leaderboard")],
            selected,
            cx,
            |this, i, _, cx| this.go(if i == 0 { Page::Stats } else { Page::Library }, cx),
        )
    }

    /// Body of a page that keeps its v2 layout (under the v2 header).
    fn page_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        match self.shown {
            Page::Library => self.render_library(),
            Page::Accounts => self.render_accounts(cx),
            Page::Learn => self.render_learn(cx),
            Page::Tools => self.render_tools(cx),
            Page::Stats => self.render_stats(cx),
            Page::Submit => self.render_submit(cx),
            Page::Settings => self.render_settings(cx),
            Page::Home | Page::Modes | Page::Practice | Page::Onboarding | Page::Game => {
                div().into_any_element()
            }
        }
    }

    fn toast_el(&self, now: Instant) -> Option<Div> {
        let (key, start) = self.toast.as_ref()?;
        let e = ms_since(*start, now);
        let a = if e < TOAST_IN_MS {
            e / TOAST_IN_MS
        } else if e < TOAST_IN_MS + TOAST_HOLD_MS {
            1.0
        } else {
            1.0 - (e - TOAST_IN_MS - TOAST_HOLD_MS) / TOAST_OUT_MS
        }
        .clamp(0.0, 1.0);
        Some(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(24.))
                .flex()
                .justify_center()
                .opacity(a)
                .child(
                    div()
                        .px(px(16.))
                        .py(px(10.))
                        .rounded(px(12.))
                        .bg(raised())
                        .border_1()
                        .border_color(border())
                        .font_weight(FontWeight::MEDIUM)
                        .text_size(px(13.))
                        .line_height(px(18.))
                        .child(t(key)),
                ),
        )
    }

    /// Called on the frame after the first frame was presented.
    fn on_first_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        report(json!({"ev": "first_frame", "t": epoch_ms()}));
        let gpu = window.gpu_specs();
        let vp = window.viewport_size();
        report(json!({
            "ev": "info",
            "source": "host",
            "gpu": gpu.as_ref().map(|g| g.device_name.clone()),
            "driver": gpu.as_ref().map(|g| format!("{} {}", g.driver_name, g.driver_info)),
            "software_gpu": gpu.as_ref().map(|g| g.is_software_emulated),
            "scale_factor": window.scale_factor(),
            "viewport": [vp.width.as_f32(), vp.height.as_f32()],
            "compositor": cx.compositor_name(),
            "inactive_frame_interval": null,
            // REDESIGN.md: v3 results are not comparable with v1/v2.
            "scene": "v3",
        }));
        if let Some(ms) = self.phase_ms {
            let handle = window.window_handle();
            cx.spawn(async move |this, cx| {
                if let Err(e) =
                    crate::scenario::run_scenario(this, handle, Duration::from_millis(ms), cx).await
                {
                    report(json!({"ev": "error", "message": format!("{e:#}")}));
                }
            })
            .detach();
        }
    }
}

/// Page fade as a solid-colour veil over the page instead of `opacity()` on it. GPUI multiplies
/// opacity into every element separately (no group opacity), so a faded page let the hero art
/// shine through its half-transparent shade overlays and the background flashed brighter
/// mid-transition. Veiling the opaque page with the colour beneath it is exactly what the web's
/// group opacity shows. `None` when the page is fully visible.
fn fade_veil(color: Hsla, page_opacity: f32) -> Option<Div> {
    (page_opacity < 1.0).then(|| {
        div().absolute().top_0().left_0().size_full().bg(Hsla {
            a: 1.0 - page_opacity,
            ..color
        })
    })
}

/// `glow.png`, 900 px × scale, centred on the top-right corner of its (relative) parent.
fn glow(opacity: f32, scale: f32) -> gpui::Img {
    let glow_size = 900.0 * scale;
    img(embedded("glow.png"))
        .absolute()
        .top(px(-glow_size / 2.0))
        .right(px(-glow_size / 2.0))
        .w(px(glow_size))
        .h(px(glow_size))
        .opacity(opacity)
}

/// Hero-style image with its own corner radius and a border overlay (GPUI can't clip children to
/// a rounded rect, and CSS paints borders above backgrounds).
pub fn framed_image(path: &str, radius: f32) -> [AnyElement; 2] {
    [
        img(embedded(path))
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded(px(radius))
            .object_fit(ObjectFit::Cover)
            .into_any_element(),
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded(px(radius))
            .border_1()
            .border_color(border())
            .into_any_element(),
    ]
}

impl Render for Scene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        if let Some(rec) = &mut self.recording {
            let ms = now.saturating_duration_since(rec.start).as_secs_f64() * 1000.0;
            rec.ts.push((ms * 100.0).round() / 100.0);
        }
        if let Some(n) = &mut self.minimized_frames {
            *n += 1;
        }
        if !self.first_render_done {
            self.first_render_done = true;
            // Frame N's callbacks run right before frame N is presented, so wait for the second
            // one (same idea as the web scene's double requestAnimationFrame).
            cx.on_next_frame(window, |_, window, cx| {
                cx.on_next_frame(window, |this, window, cx| this.on_first_frame(window, cx));
            });
        }
        self.want_frame.set(false);

        let (page_opacity, page_dy) = self.page_transform(now);
        let (glow_opacity, glow_scale) = self.glow_transform(now);
        self.apply_scroll(now);
        self.tick(now);
        let whole = self.trans_whole && !matches!(self.anim, Anim::Idle);
        let (op, dy) = if whole {
            (1.0, 0.0)
        } else {
            (page_opacity, page_dy)
        };

        let content: AnyElement = if self.shown.full_window() {
            let screen = match self.shown {
                Page::Onboarding => self.render_onboarding(cx),
                _ => self.render_game(cx),
            };
            div()
                .size_full()
                .relative()
                .overflow_hidden()
                .bg(bg())
                .child(glow(0.8, 1.0))
                .child(screen)
                .into_any_element()
        } else {
            // The page (background included) fades and moves under the static top nav.
            let page: AnyElement = if self.shown.v3() {
                match self.shown {
                    Page::Home => self.render_home(cx),
                    Page::Modes => self.render_modes(cx),
                    _ => self.render_practice(cx),
                }
            } else {
                // v2 pages: header + body, 104 from the top, 64 left / 48 right (REDESIGN.md).
                div()
                    .size_full()
                    .relative()
                    .child(glow(glow_opacity, glow_scale))
                    .child(
                        div()
                            .absolute()
                            .left(px(64.))
                            .right(px(48.))
                            .top(px(104.))
                            .bottom(px(28.))
                            .flex()
                            .flex_col()
                            .gap(px(20.))
                            .child(self.header(cx))
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(0.))
                                    .flex()
                                    .flex_col()
                                    .child(self.page_body(cx)),
                            ),
                    )
                    .into_any_element()
            };
            div()
                .size_full()
                .relative()
                .overflow_hidden()
                .bg(crate::v3::bg3())
                .child(
                    div()
                        .absolute()
                        .top(px(dy))
                        .left_0()
                        .size_full()
                        .child(page),
                )
                .children(fade_veil(crate::v3::bg3(), op))
                .child(self.top_nav(cx))
                .into_any_element()
        };

        let modal = self.render_check_modal(now, cx);
        let toast = self.toast_el(now);

        let animating = !matches!(self.anim, Anim::Idle)
            || self.glow_running
            || self.recording.is_some()
            || self.scroll_start.is_some()
            || self.minimized_frames.is_some();
        if animating || self.want_frame.get() {
            window.request_animation_frame();
        }

        div()
            .size_full()
            .relative()
            .bg(bg())
            .font_family(INTER)
            .text_color(text())
            .text_size(px(14.))
            .line_height(relative(1.21))
            .child(
                div()
                    .size_full()
                    .relative()
                    .top(px(if whole { page_dy } else { 0.0 }))
                    .child(content),
            )
            .children(fade_veil(bg(), if whole { page_opacity } else { 1.0 }))
            .children(modal)
            .children(toast)
    }
}
