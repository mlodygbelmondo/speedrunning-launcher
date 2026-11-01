//! The root view: app state, the backend's events, navigation with the page transition, the
//! top nav (`shell.rs`) and the per-frame bookkeeping.
//!
//! **Idle renders nothing.** The only frame loop is the page transition: `render` reads
//! `Instant::now()`, derives the page opacity and offset, and calls
//! `window.request_animation_frame()` only while a transition runs. Install progress and game state
//! come from the backend as events; each batch is one `notify`, so a running install redraws at
//! the rate the backend reports and an idle app draws no frames at all.
use std::{
    rc::Rc,
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

use futures::{StreamExt, channel::mpsc::UnboundedReceiver};
use gpui::{
    AnyElement, Context, Div, FocusHandle, Hsla, IntoElement, ObjectFit, ParentElement, Render,
    Styled, Window, div, img, prelude::*, px, relative,
};
use srl_core::Mode;

use crate::{
    assets::{embedded, hero_path},
    backend::{
        AccountSnapshot, Backend, Event, Language, ModeSnapshot, ModeState, PracticeEvent,
        PracticeSnapshot, Settings, StatsView, ToolSnapshot, ToolState, UpdateState, index,
        practice::Question,
        stats::{self, Refresh},
    },
    catalog,
    copy::{self, t},
    v3::*,
};

/// SPEC.md's page transition: outgoing 150 ms ease-in, then incoming 220 ms ease-out.
const OUT_MS: f32 = 150.0;
const IN_MS: f32 = 220.0;
/// A mode switch: the new key art fades in over the old one, the hero text and numbers rise in.
const MODE_MS: f32 = 420.0;
const MODE_TEXT_MS: f32 = 280.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Home,
    Modes,
    Practice,
    Learn,
    Tools,
    Stats,
    Settings,
    /// The game is running (full window).
    Game,
    /// The first-run prompt for the offline player name (full window).
    Name,
    /// "Import my current settings" (full window): offered once on the first start, and from
    /// Settings.
    Import,
    /// The selected mode's mods, turned on and off (from Modes).
    Mods,
}

impl Page {
    /// Game and the name prompt cover the whole window (no top nav).
    pub fn full_window(self) -> bool {
        matches!(self, Page::Game | Page::Name | Page::Import)
    }
}

/// See `Launcher::mode_fade`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeFade {
    pub art: f32,
    pub text: f32,
    pub rise: f32,
}

impl ModeFade {
    const SETTLED: ModeFade = ModeFade {
        art: 1.0,
        text: 1.0,
        rise: 0.0,
    };
}

#[derive(Clone, Copy)]
enum Anim {
    Idle,
    Out(Instant),
    In(Instant),
}

/// What the Practice list shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PracticeSel {
    Category(usize),
    Tools,
}

/// A one-line text field for a player name. GPUI has no text input; names are plain ASCII, so key
/// events are enough (no IME). The caret doesn't blink, so a focused field is still idle.
pub struct NameField {
    pub text: String,
    pub focus: FocusHandle,
}

/// Where the app starts: the deep link (`SRL_SCREEN`) resolved into state.
pub struct Start {
    pub page: Page,
    pub mode: Mode,
    pub practice: PracticeSel,
    /// Settings opens on its credits view.
    pub credits: bool,
    /// The Learn topic, an index into `catalog::learn().topics`.
    pub learn: usize,
    /// The Practice map whose details are open.
    pub practice_open: Option<String>,
    /// That map's "About this map" is open too.
    pub practice_about: bool,
    /// No deep link: the first start may offer the settings import.
    pub offer_import: bool,
}

/// A confirmation the Practice page shows over itself (`Question`), with the player's choice so
/// far.
pub struct PracticeAsk {
    pub question: Question,
    /// Reset: keep the map's player data (`ResetPlan::carry_over`). On until the player says no.
    pub keep: bool,
}

pub struct Launcher {
    backend: Rc<dyn Backend>,
    /// The last state the backend reported for each mode, in `Mode::ALL` order.
    pub modes: [ModeSnapshot; 5],
    pub settings: Settings,

    /// Page currently drawn.
    pub shown: Page,
    /// Page we are heading to (navigation requested while a transition runs is queued here).
    pub target: Page,
    anim: Anim,
    /// The running transition covers the whole window (to or from a full-window page).
    trans_whole: bool,
    /// Where the game screen, the name prompt and the Import page go back to.
    pub return_to: Page,

    /// The selected mode (Home and Modes share it), an index into `Mode::ALL`.
    pub mode_sel: usize,
    /// The mode selected before it, whose hero stays drawn under the new one (`hero_images`).
    pub prev_mode: Option<usize>,
    /// When the selected mode last changed by a click: the key art cross-fades and the hero
    /// rises in for `MODE_MS` after it (`mode_fade`). `None` once it's over.
    mode_switched: Option<Instant>,
    pub practice_sel: PracticeSel,
    /// The selected Learn topic, an index into `catalog::learn().topics`.
    pub learn_topic: usize,
    /// The scroll of the selected topic's guide list (back to the top on a topic change); its
    /// overflow decides the "more below" fade.
    pub learn_scroll: gpui::ScrollHandle,
    /// The mode whose game screen is up, while its game runs.
    pub game_mode: Option<Mode>,
    /// Stop was pressed and the game hasn't exited yet.
    pub stopping: bool,
    /// The first-run prompt's field and the Settings field.
    pub name_prompt: NameField,
    pub name_settings: NameField,
    /// The prompt's field was focused once (`render_name`), so a click elsewhere isn't undone.
    pub prompt_focused: bool,
    /// The data folder can't be written (`Snapshot::data_error`): the window shows only that.
    pub data_error: Option<String>,
    /// The data folder, as Settings shows it.
    pub data_dir: std::path::PathBuf,
    /// Settings shows its credits and licences instead of the settings.
    pub settings_credits: bool,
    /// The last state the tools extension reported, in `ToolId::ALL` order.
    pub tools: Vec<ToolSnapshot>,
    /// What the stats extension has reported: the local runs, Ranked and Draftout.
    pub stats: StatsView,
    /// The mode whose runs the Stats page lists (a PB tile clicked); `None`: every mode.
    pub stats_mode: Option<Mode>,
    /// The Settings fields for the MCSR Ranked and Draftout names.
    pub ranked_field: NameField,
    pub draftout_field: NameField,
    /// The Microsoft accounts and the login dialog, as the backend last reported them.
    /// `account.active_name()` is the Microsoft account Play uses, if any.
    pub account: AccountSnapshot,
    /// A re-render is booked for when the login dialog's "valid for N min" changes.
    pub login_tick: bool,
    /// The login dialog's link was copied (the button says so until the dialog closes).
    pub link_copied: bool,
    /// The last state the practice extension reported: the maps and the practice game.
    pub practice: PracticeSnapshot,
    /// The map whose details are open on the Practice page (one at a time).
    pub practice_open: Option<String>,
    /// The open map whose "About this map" is open (closed by default, and for every other map).
    pub practice_about: Option<String>,
    /// The confirmation open on the Practice page.
    pub practice_ask: Option<PracticeAsk>,
    /// Stop was pressed for the practice game and it hasn't exited yet.
    pub practice_stopping: bool,
    /// The scroll of the Practice list, and whether the next frame scrolls the open map into
    /// view (it was opened by a click, a deep link, Play or a question).
    pub practice_scroll: gpui::ScrollHandle,
    pub practice_reveal: bool,
    /// The Import page: the instances found, the ticks, the import's progress and result.
    pub import: crate::screens::import::ImportView,
    /// The Mods page: its list's scroll and the last jar that could not be added.
    pub mods: crate::screens::mods::ModsView,
    /// The launcher's own update, as the backend last reported it.
    pub update: UpdateState,
}

impl Launcher {
    pub fn new(
        backend: Rc<dyn Backend>,
        events: UnboundedReceiver<Event>,
        start: Start,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let snap = backend.snapshot();
        // Backend events arrive on another thread; drain them on the UI executor, a batch at a
        // time, and redraw once per batch. Awaiting the channel costs nothing while it's empty.
        cx.spawn(async move |this, cx| {
            let mut events = events;
            while let Some(first) = events.next().await {
                let mut batch = vec![first];
                while let Ok(ev) = events.try_recv() {
                    batch.push(ev);
                }
                let alive = this.update(cx, |app, cx| {
                    for ev in batch {
                        app.apply(ev, cx);
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();

        let tools = backend.tools().snapshot();
        let account = backend.account().snapshot();
        // srl-auth's background refresh, once, right after the first frame is on screen: it may
        // touch the keyring and the network, and the window must not wait for either.
        cx.on_next_frame(window, |this, _, _| {
            this.backend.account().refresh_in_background()
        });
        let practice = backend.practice().snapshot();
        let update = backend.updates().snapshot();
        let name = snap.settings.player_name.clone().unwrap_or_default();
        let mut page = start.page;
        // The first start (no deep link) offers the settings import once, after the name. The
        // scan starts now, so the offer knows by then whether there is anything to offer.
        let offer = start.offer_import && !snap.settings.import_offered;
        if offer && snap.settings.player_name.is_some() {
            page = Page::Import;
        }
        // No name yet: the prompt comes first, whatever the deep link asked for (but the
        // screenshot states of the game screen keep their page).
        if snap.settings.player_name.is_none() && page != Page::Game {
            page = Page::Name;
        }
        if offer || start.page == Page::Import {
            backend.import().scan();
        }
        let import_snap = backend.import().snapshot();
        let running = Mode::ALL
            .into_iter()
            .find(|m| snap.modes[index(*m)].state == ModeState::Running);
        // The Settings name is saved when its edit ends; clicking elsewhere ends it.
        let settings_focus = cx.focus_handle();
        cx.on_blur(&settings_focus, window, |this: &mut Self, _, cx| {
            this.commit_settings_name(cx)
        })
        .detach();
        // The stats names save the same way.
        let ranked_focus = cx.focus_handle();
        cx.on_blur(&ranked_focus, window, |this: &mut Self, _, cx| {
            this.commit_stats_name(crate::screens::field::Field::Ranked, cx)
        })
        .detach();
        let draftout_focus = cx.focus_handle();
        cx.on_blur(&draftout_focus, window, |this: &mut Self, _, cx| {
            this.commit_stats_name(crate::screens::field::Field::Draftout, cx)
        })
        .detach();
        let stats_names = snap.settings.stats.clone();
        let mut this = Self {
            backend,
            modes: snap.modes,
            settings: snap.settings,
            shown: page,
            target: page,
            anim: Anim::Idle,
            trans_whole: false,
            return_to: if start.page.full_window() {
                Page::Home
            } else {
                start.page
            },
            mode_sel: index(start.mode),
            prev_mode: None,
            mode_switched: None,
            practice_sel: start.practice,
            learn_topic: start.learn,
            learn_scroll: gpui::ScrollHandle::new(),
            game_mode: running,
            stopping: false,
            name_prompt: NameField {
                text: if start.page == Page::Name {
                    String::new()
                } else {
                    name.clone()
                },
                focus: cx.focus_handle(),
            },
            name_settings: NameField {
                text: name,
                focus: settings_focus,
            },
            prompt_focused: false,
            data_error: snap.data_error,
            data_dir: snap.data_dir,
            settings_credits: start.credits,
            tools,
            stats: StatsView::default(),
            stats_mode: None,
            ranked_field: NameField {
                text: stats_names.ranked_nickname.unwrap_or_default(),
                focus: ranked_focus,
            },
            draftout_field: NameField {
                text: stats_names.draftout_username.unwrap_or_default(),
                focus: draftout_focus,
            },
            account,
            login_tick: false,
            link_copied: false,
            practice,
            practice_reveal: start.practice_open.is_some(),
            practice_about: start.practice_open.clone().filter(|_| start.practice_about),
            practice_open: start.practice_open,
            practice_ask: None,
            practice_stopping: false,
            practice_scroll: gpui::ScrollHandle::new(),
            import: crate::screens::import::ImportView {
                scanned: import_snap.scanned,
                found: import_snap.sources,
                offer: offer || start.page == Page::Import,
                offer_pending: offer,
                ..Default::default()
            },
            mods: Default::default(),
            update,
        };
        // Stats on start: the records (Home's PBs) and the APIs from their cache only, no
        // network. A deep link to Stats opens it like a click would.
        this.backend.stats().scan();
        this.request_stats(Refresh::Never);
        if this.shown == Page::Stats {
            this.open_stats();
        }
        this
    }

    /// The names the Ranked and Draftout lookups use (`stats::lookup_name`): the ones set in
    /// Settings, else the active Microsoft account's.
    pub fn stats_names(&self) -> (Option<String>, Option<String>) {
        let ms = stats::active_microsoft_name(&self.account);
        let s = &self.settings.stats;
        (
            stats::lookup_name(s.ranked_nickname.as_deref(), ms.as_deref()),
            stats::lookup_name(s.draftout_username.as_deref(), ms.as_deref()),
        )
    }

    /// Asks for the Ranked and Draftout data of the current names.
    pub fn request_stats(&mut self, refresh: Refresh) {
        let (ranked, draftout) = self.stats_names();
        if self.stats.want_ranked(ranked.as_deref(), refresh)
            && let Some(name) = &ranked
        {
            self.backend.stats().ranked(name, refresh);
        }
        if self.stats.want_draftout(draftout.as_deref(), refresh)
            && let Some(name) = &draftout
        {
            self.backend.stats().draftout(name, refresh);
        }
    }

    /// The Stats page opens: the last numbers stay up while the records are scanned again and
    /// the APIs are asked for whatever is past its cache time.
    fn open_stats(&mut self) {
        self.backend.stats().scan();
        self.request_stats(Refresh::IfStale);
    }

    pub fn mode(&self, mi: usize) -> Mode {
        Mode::ALL[mi]
    }

    pub fn state(&self, mi: usize) -> &ModeState {
        &self.modes[mi].state
    }

    pub fn status(&self, mi: usize) -> Status {
        Status::of(self.state(mi))
    }

    /// Whether a mode's files are in place (it can be verified, opened, shared from). A failed
    /// Play of an installed mode (the game closed while starting, an update failed) leaves them
    /// in place.
    pub fn installed(&self, mi: usize) -> bool {
        match self.state(mi) {
            ModeState::Failed(f) => f.installed,
            _ => matches!(
                self.status(mi),
                Status::Ready | Status::Update | Status::Running
            ),
        }
    }

    fn apply(&mut self, ev: Event, cx: &mut Context<Self>) {
        let progress = matches!(
            ev,
            Event::State(_, ModeState::Installing(_))
                | Event::Practice(PracticeEvent::Map(crate::backend::practice::MapView {
                    job: Some(_),
                    ..
                }))
        );
        if !progress {
            trace(format_args!("event {ev:?}"));
        }
        match ev {
            Event::State(mode, state) => {
                let i = index(mode);
                let was_running = self.modes[i].state == ModeState::Running;
                let running = state == ModeState::Running;
                self.modes[i].state = state;
                // The game has exited: its run is in the records now, and its world is still
                // there to tie the run to the mode.
                if was_running && !running {
                    self.backend.stats().scan();
                }
                if running && !was_running {
                    // The launcher steps aside while the game runs (README "What the launcher
                    // does", step 4).
                    self.game_mode = Some(mode);
                    self.stopping = false;
                    self.mode_sel = i;
                    self.open_game(cx);
                } else if was_running && !running && self.game_mode == Some(mode) {
                    self.stopping = false;
                    // Another game still runs (two modes started one after the other): the game
                    // screen shows that one instead of going back as if nothing ran.
                    let other = Mode::ALL
                        .into_iter()
                        .find(|m| self.modes[index(*m)].state == ModeState::Running);
                    self.game_mode = other;
                    if let Some(m) = other {
                        self.mode_sel = index(m);
                    } else if self.target == Page::Game {
                        self.navigate(self.return_to, cx);
                    }
                }
            }
            Event::Info(mode, info) => self.modes[index(mode)].info = info,
            Event::Tool(snap) => {
                if let Some(slot) = self.tools.iter_mut().find(|t| t.id == snap.id) {
                    *slot = snap;
                }
            }
            Event::Stats(ev) => {
                self.stats.apply(ev);
            }
            Event::Account(snap) => {
                if snap.login.is_none() {
                    self.link_copied = false;
                }
                let names = self.stats_names();
                self.account = *snap;
                // The Ranked and Draftout names may default to the Microsoft account's: after a
                // login, a switch or a log out, look up the new name (its cache, or the API when
                // Stats is open, as opening it would).
                if self.stats_names() != names {
                    self.request_stats(if self.shown == Page::Stats {
                        Refresh::IfStale
                    } else {
                        Refresh::Never
                    });
                }
            }
            Event::Practice(ev) => self.apply_practice(ev),
            Event::Import(ev) => self.apply_import(ev, cx),
            Event::JarAdded(mode, answer) => self.jar_added(mode, answer, cx),
            Event::Update(state) => self.update = state,
        }
    }

    pub fn backend(&self) -> &dyn Backend {
        &*self.backend
    }

    fn apply_practice(&mut self, ev: PracticeEvent) {
        match ev {
            PracticeEvent::Maps(maps) => {
                self.practice.maps = maps;
                self.practice.loaded = true;
            }
            PracticeEvent::Map(view) => {
                if let Some(slot) = self.practice.maps.iter_mut().find(|m| m.id == view.id) {
                    *slot = view;
                }
            }
            PracticeEvent::Game(game) => {
                if !matches!(game.state, ModeState::Running | ModeState::Installing(_)) {
                    self.practice_stopping = false;
                }
                // The practice game has exited: SpeedRunIGT may have written records into the
                // shared folder, so Stats' counts (and its note on runs it can't tie to a mode)
                // are read again, as after a mode's game.
                if self.practice.game.state == ModeState::Running
                    && game.state != ModeState::Running
                {
                    self.backend.stats().scan();
                }
                self.practice.game = game;
            }
            PracticeEvent::Ask(question) => {
                // The page shows the map the question is about.
                let id = match &question {
                    Question::Reset { id, .. }
                    | Question::Update { id, .. }
                    | Question::Remove { id, .. } => id.clone(),
                };
                self.practice_open = Some(id);
                self.practice_reveal = true;
                self.practice_ask = Some(PracticeAsk {
                    question,
                    keep: true,
                });
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Actions
    // -----------------------------------------------------------------------------------------

    /// The Play button of mode `mi`: install if needed and launch, or show the game screen if it
    /// already runs. Without a player name the prompt comes first.
    pub fn play(&mut self, mi: usize, cx: &mut Context<Self>) {
        if self.settings.player_name.is_none() {
            self.return_to = self.target;
            self.navigate(Page::Name, cx);
            return;
        }
        match self.state(mi) {
            ModeState::Running => {
                self.game_mode = Some(self.mode(mi));
                self.open_game(cx);
            }
            ModeState::Installing(_) => {}
            _ => self.backend.play(self.mode(mi)),
        }
    }

    pub fn cancel(&mut self, mi: usize, cx: &mut Context<Self>) {
        trace(format_args!("cancel {:?}", self.mode(mi)));
        self.backend.cancel(self.mode(mi));
        if self.state(mi) == &ModeState::Running {
            self.stopping = true;
            cx.notify();
        }
    }

    /// Why "Restart to update" has to wait, as a copy key: a game this launcher started is
    /// running (restarting would lose track of it), or something is installing (a mode, the
    /// practice game, a map, a tool, an import). `None`: it can restart now.
    pub fn restart_blocker(&self) -> Option<&'static str> {
        let game = self.modes.iter().any(|m| m.state == ModeState::Running)
            || self.practice.game.state == ModeState::Running;
        if game {
            return Some("settings.update_wait_game");
        }
        let installing = self
            .modes
            .iter()
            .any(|m| matches!(m.state, ModeState::Installing(_)))
            || matches!(self.practice.game.state, ModeState::Installing(_))
            || self.practice.maps.iter().any(|m| m.job.is_some())
            || self
                .tools
                .iter()
                .any(|t| matches!(t.state, ToolState::Installing(_)))
            || self.import.progress.is_some();
        installing.then_some("settings.update_wait_install")
    }

    /// "Restart to update": starts the new version and quits, unless something has to finish
    /// first (`restart_blocker`).
    pub fn restart_to_update(&mut self, cx: &mut Context<Self>) {
        if self.restart_blocker().is_none() && self.backend.updates().restart() {
            cx.quit();
        }
    }

    fn open_game(&mut self, cx: &mut Context<Self>) {
        if self.target != Page::Game {
            if !self.target.full_window() {
                self.return_to = self.target;
            }
            self.navigate(Page::Game, cx);
        }
    }

    /// Back from the game screen while the game keeps running.
    pub fn leave_game(&mut self, cx: &mut Context<Self>) {
        self.navigate(self.return_to, cx);
    }

    /// Selects a mode from a click (Home tiles, Modes rows). Instant: no transition. The choice
    /// is saved, so the launcher opens on it next time.
    pub fn select_mode(&mut self, mi: usize, cx: &mut Context<Self>) {
        if self.mode_sel != mi {
            self.prev_mode = Some(self.mode_sel);
            self.mode_sel = mi;
            self.mode_switched = Some(Instant::now());
            let settings = Settings {
                last_mode: Some(self.mode(mi)),
                ..self.settings.clone()
            };
            self.save_settings(settings, cx);
            cx.notify();
        }
    }

    /// Stores settings in the backend and here.
    pub fn save_settings(&mut self, settings: Settings, cx: &mut Context<Self>) {
        if settings != self.settings {
            copy::set_hide_taglines(settings.dev_hide_taglines);
            self.settings = settings.clone();
            self.backend.set_settings(settings);
            cx.notify();
        }
    }

    /// The language switch: every string changes at once (no transition), and the window title.
    pub fn set_language(&mut self, lang: Language, window: &mut Window, cx: &mut Context<Self>) {
        copy::set_lang(lang);
        window.set_window_title(&t("app.window_title"));
        let settings = Settings {
            language: lang,
            ..self.settings.clone()
        };
        self.save_settings(settings, cx);
        cx.notify();
    }

    /// The name prompt's Continue: store a valid name and go on to where the user was heading.
    pub fn submit_name(&mut self, cx: &mut Context<Self>) {
        let name = self.name_prompt.text.clone();
        if !srl_core::OfflineAccount::is_valid_name(&name) {
            return;
        }
        self.name_settings.text = name.clone();
        let settings = Settings {
            player_name: Some(name),
            ..self.settings.clone()
        };
        self.save_settings(settings, cx);
        self.offer_import_or_go_on(cx);
    }

    /// Same semantics as the prototype: outgoing 150 ms ease-in, then incoming 220 ms ease-out.
    /// A request made during a transition is remembered and started once the current one ends.
    pub fn navigate(&mut self, to: Page, cx: &mut Context<Self>) {
        if self.target == Page::Settings && to != Page::Settings {
            self.commit_settings_name(cx);
            self.commit_stats_name(crate::screens::field::Field::Ranked, cx);
            self.commit_stats_name(crate::screens::field::Field::Draftout, cx);
        }
        if to == Page::Stats && self.target != Page::Stats {
            self.open_stats();
        }
        // The gear (or coming back later) opens the settings themselves, not the credits.
        if to == Page::Settings && self.target != Page::Settings {
            self.settings_credits = false;
        }
        // What is installed may have changed since the launcher started (waywall from the
        // system's package manager): read it again, in the background.
        if to == Page::Tools && self.target != Page::Tools {
            self.backend().tools().reload();
        }
        // The player may have played or deleted a practice world since.
        if to == Page::Practice && self.target != Page::Practice {
            self.backend().practice().reload();
        }
        self.target = to;
        if !matches!(self.anim, Anim::Idle) || to == self.shown {
            return;
        }
        self.trans_whole = self.shown.full_window() || to.full_window();
        self.anim = Anim::Out(Instant::now());
        cx.notify();
    }

    /// Advances the page transition; returns (opacity, translate-y px).
    fn page_transform(&mut self, now: Instant) -> (f32, f32) {
        match self.anim {
            Anim::Idle => (1.0, 0.0),
            Anim::Out(start) => {
                let t = ms_since(start, now) / OUT_MS;
                if t >= 1.0 {
                    self.trans_whole |= self.target.full_window();
                    self.shown = self.target;
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

    /// The selected mode's full-window hero over the previously selected one. `img()` decodes a
    /// hero on first use in the background and draws nothing until it's done, so on the first
    /// switch to a mode the old hero fills those frames instead of bare background (commit
    /// 747c3a5). Both are opaque and already in the atlas, so the extra layer costs nothing.
    /// While a mode switch runs, the new hero fades in over the old one (one opaque image's
    /// opacity, so none of the group-opacity trouble of `fade_veil`).
    pub fn hero_images(&self) -> Vec<gpui::Img> {
        let hero = |mi: usize| {
            img(embedded(&hero_path(catalog::MODES[mi].art)))
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
        let fade = self.mode_fade().art;
        v.push(hero(self.mode_sel).when(fade < 1.0, |i| i.opacity(fade)));
        v
    }

    /// Where the mode switch is: the new key art's opacity, then the hero text's opacity and how
    /// far below its place it still is (px). All settled (1, 1, 0) when no switch runs.
    pub fn mode_fade(&self) -> ModeFade {
        let Some(start) = self.mode_switched else {
            return ModeFade::SETTLED;
        };
        let ms = ms_since(start, Instant::now());
        let text = ease_out((ms / MODE_TEXT_MS).min(1.0));
        ModeFade {
            art: ease_out((ms / MODE_MS).min(1.0)),
            text,
            rise: 10.0 * (1.0 - text),
        }
    }

    fn page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match self.shown {
            Page::Home => self.render_home(cx),
            Page::Modes => self.render_modes(cx),
            Page::Practice => self.render_practice(cx),
            Page::Learn => self.render_learn(cx),
            Page::Stats => self.render_stats(cx),
            Page::Tools => self.render_tools(cx),
            Page::Settings => self.render_settings(window, cx),
            Page::Game => self.render_game(cx),
            Page::Name => self.render_name(window, cx),
            Page::Import => self.render_import(window, cx),
            Page::Mods => self.render_mods(cx),
        }
    }
}

/// Page fade as a solid-colour veil over the page instead of `opacity()` on it. GPUI multiplies
/// opacity into every element separately (no group opacity), so a faded page let the hero art
/// shine through its half-transparent shade overlays and the background flashed brighter
/// mid-transition (commit 747c3a5). Veiling the opaque page with the colour beneath it is exactly
/// what a group opacity shows. `None` when the page is fully visible.
fn fade_veil(color: Hsla, page_opacity: f32) -> Option<Div> {
    (page_opacity < 1.0).then(|| {
        div().absolute().top_0().left_0().size_full().bg(Hsla {
            a: 1.0 - page_opacity,
            ..color
        })
    })
}

/// `SRL_TRACE_RENDERS=1`: print one line per rendered frame to stderr, to check that an idle app
/// renders nothing (README "Idle").
pub static TRACE_RENDERS: AtomicBool = AtomicBool::new(false);

/// One `SRL_TRACE_RENDERS` line, stamped with the time since the first one.
pub fn trace(what: std::fmt::Arguments) {
    if TRACE_RENDERS.load(Ordering::Relaxed) {
        static START: OnceLock<Instant> = OnceLock::new();
        let now = Instant::now();
        let t = now.saturating_duration_since(*START.get_or_init(|| now));
        eprintln!("{what} t={}ms", t.as_millis());
    }
}

impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        if TRACE_RENDERS.load(Ordering::Relaxed) {
            static N: AtomicU64 = AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed) + 1;
            trace(format_args!("render {n} page={:?}", self.shown));
        }
        let login = self.login_dialog(cx);
        if let Some(problem) = &self.data_error {
            return div()
                .size_full()
                .relative()
                .bg(bg3())
                .font_family(INTER)
                .text_color(text3())
                .child(self.render_data_error(problem))
                .into_any_element();
        }
        let (page_opacity, page_dy) = self.page_transform(now);
        let whole = self.trans_whole && !matches!(self.anim, Anim::Idle);
        let (op, dy) = if whole {
            (1.0, 0.0)
        } else {
            (page_opacity, page_dy)
        };

        let page = self.page(window, cx);
        let content: AnyElement = if self.shown.full_window() {
            div()
                .size_full()
                .relative()
                .overflow_hidden()
                .bg(bg3())
                .child(page)
                .into_any_element()
        } else {
            // The page (background included) fades and moves under the static top nav.
            div()
                .size_full()
                .relative()
                .overflow_hidden()
                .bg(bg3())
                .child(
                    div()
                        .absolute()
                        .top(px(dy))
                        .left_0()
                        .size_full()
                        .child(page),
                )
                .children(fade_veil(bg3(), op))
                .child(self.top_nav(cx))
                .into_any_element()
        };

        if !matches!(self.anim, Anim::Idle) {
            window.request_animation_frame();
        }
        if let Some(start) = self.mode_switched {
            if ms_since(start, now) < MODE_MS {
                window.request_animation_frame();
            } else {
                self.mode_switched = None;
            }
        }

        div()
            .size_full()
            .relative()
            .bg(bg3())
            .font_family(INTER)
            .text_color(text3())
            .text_size(px(14.))
            .line_height(relative(1.21))
            .child(
                div()
                    .size_full()
                    .relative()
                    .top(px(if whole { page_dy } else { 0.0 }))
                    .child(content),
            )
            .children(fade_veil(bg3(), if whole { page_opacity } else { 1.0 }))
            // The login dialog covers every page (Play opens it when a sign-in has expired).
            .children(login)
            .into_any_element()
    }
}

// ---------------------------------------------------------------------------------------------
// Easing (CSS cubic-bezier, so the timing matches the web prototype)
// ---------------------------------------------------------------------------------------------

fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let bez = |a: f32, b: f32, s: f32| {
        let u = 1.0 - s;
        3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
    };
    // Solve x(s) = t by bisection (monotonic for CSS curves), then evaluate y(s).
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    let mut s = t;
    for _ in 0..24 {
        s = 0.5 * (lo + hi);
        if bez(x1, x2, s) < t {
            lo = s;
        } else {
            hi = s;
        }
    }
    bez(y1, y2, s)
}

fn ease_in(t: f32) -> f32 {
    cubic_bezier(0.42, 0.0, 1.0, 1.0, t)
}

fn ease_out(t: f32) -> f32 {
    cubic_bezier(0.0, 0.0, 0.58, 1.0, t)
}

fn ms_since(start: Instant, now: Instant) -> f32 {
    now.saturating_duration_since(start).as_secs_f32() * 1000.0
}
