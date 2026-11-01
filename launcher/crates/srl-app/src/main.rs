//! The Speedrunning Launcher desktop app (GPUI): scene v3 from the prototype, wired to the core through
//! the `Backend` facade. See `crates/srl-app/README.md`.
//!
//! - `app`: the root view, navigation and the backend's events. `shell`: the top nav.
//! - `screens/*`: one file per screen. `v3`: tokens and the shared primitives.
//! - `backend`: the facade, `RealBackend` and `FakeBackend`. `catalog`: modes and practice
//!   maps.
//! - `copy`: UI strings and the language. `demo`: the prototype's sample data (`SRL_DEMO=1`).
//! - `cli`: the headless command line (`speedrunning-launcher --install <mode>`), for CI and support.
//!
//! Dev switches (environment): `SRL_FAKE=1` uses `FakeBackend` (simulated installs, no game),
//! `SRL_SCREEN=<id>` opens on a screen state, `SRL_LANG=en|pl` picks the language for this run,
//! `SRL_DEMO=1` fills in the prototype's sample data for design comparison, `SRL_FAKE_FAIL=1`
//! makes the fake's first install fail, `SRL_TRACE_RENDERS=1` logs every rendered frame. The demo
//! and the screen ids with a state (`installing`, `error`, `running`, `account*`, `login*`,
//! `msa-*`, `practice-<state>`, `update-*`) imply `SRL_FAKE`. `SRL_MSA_CLIENT_ID` turns Microsoft
//! login on (srl-auth), `SRL_MSA_BASE_URL` points it at srl-auth's mock (`backend/account.rs`),
//! `SRL_UPDATE_URL` points the launcher's own update check at another release JSON
//! (`backend/update.rs`) and `SRL_DEV_GAME_HOME` gives test games (modes and practice) their own
//! `user.home` (`backend/pipeline.rs`).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod assets;
mod backend;
mod catalog;
mod cli;
mod copy;
mod demo;
mod screens;
mod shell;
mod v3;

use std::{borrow::Cow, path::Path, rc::Rc};

use gpui::{
    App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions, px,
    size,
};
use srl_core::{Mode, Progress, Stage, Unit};

use app::{Launcher, Page, PracticeSel, Start};
use assets::{Assets, FONTS};
use backend::{
    AccountsPreset, Backend, FakeBackend, FakeOptions, ImportPreset, Language, ModeState,
    PracticePreset, RealBackend, Settings, StatsPreset, ToolsPreset, UpdatePreset,
};
use screens::instances::{Confirm, InstKey, Tab};

fn flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| matches!(v.trim(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

/// What a deep link asks the fake backend to start with. Any preset implies the fake: these
/// states exist only there.
#[derive(Debug, PartialEq)]
struct Preset {
    mode: Option<(Mode, ModeState)>,
    tools: ToolsPreset,
    /// `None`: the fake's default for the page (`stats_default`).
    stats: Option<StatsPreset>,
    /// Every mode starts installed, so Home shows its numbers (`home-*-stats`).
    installed: bool,
    /// `None`: as the build is (login available only with `SRL_MSA_CLIENT_ID`).
    accounts: Option<AccountsPreset>,
    /// The running game's Microsoft sign-in couldn't be renewed (`msa-warning`).
    renew_warning: bool,
    practice: PracticePreset,
    import: ImportPreset,
    update: UpdatePreset,
}

impl Preset {
    fn new(mode: Option<(Mode, ModeState)>) -> Self {
        Self {
            mode,
            tools: ToolsPreset::Default,
            stats: None,
            installed: false,
            accounts: None,
            renew_warning: false,
            practice: PracticePreset::Default,
            import: ImportPreset::Found,
            update: UpdatePreset::Default,
        }
    }

    fn stats(stats: StatsPreset) -> Self {
        Preset {
            stats: Some(stats),
            ..Preset::new(None)
        }
    }
}

/// The fake's stats when no deep link chose them: the sample on the Stats page and in the demo
/// (so `SRL_FAKE=1 SRL_SCREEN=stats` shows the design with data), else nothing.
fn stats_default(page: Page, demo: bool) -> StatsPreset {
    if demo || page == Page::Stats {
        StatsPreset::Sample
    } else {
        StatsPreset::Nothing
    }
}

/// The Practice page's state screen ids (`practice-<state>`): the fake's preset, the category the
/// page shows and the map whose details are open. A category id never takes one of these names.
const PRACTICE_STATES: [(&str, PracticePreset, &str, Option<&str>); 10] = [
    ("installing", PracticePreset::Installing, "all-in-one", None),
    (
        "installed",
        PracticePreset::Installed,
        "overworld",
        Some("bt-practice-mescht"),
    ),
    // The same with its "About this map" open.
    (
        "about",
        PracticePreset::Installed,
        "overworld",
        Some("bt-practice-mescht"),
    ),
    ("update", PracticePreset::Update, "end", None),
    ("reset", PracticePreset::Reset, "end", None),
    ("remove", PracticePreset::Remove, "overworld", None),
    (
        "starting",
        PracticePreset::Starting,
        "overworld",
        Some("bt-practice-mescht"),
    ),
    (
        "running",
        PracticePreset::Running,
        "overworld",
        Some("bt-practice-mescht"),
    ),
    ("error", PracticePreset::Error, "overworld", None),
    (
        "link",
        PracticePreset::Installed,
        "stronghold",
        Some("llama-s-practice-map-llama"),
    ),
];

/// `SRL_SCREEN=<id>`: the page, the selected mode and Practice category, and for the state ids a
/// fake backend state to show (see README "Screen ids"). `None` for an unknown id. `data_dir` is
/// where the failure states say the game log is.
fn deep_link(id: &str, data_dir: &Path) -> Option<(Start, Option<Preset>)> {
    let mut start = Start {
        page: Page::Home,
        mode: Mode::Ranked,
        practice: PracticeSel::Category(0),
        credits: false,
        learn: 0,
        practice_open: None,
        practice_about: false,
        offer_import: false,
        inst: (InstKey::Shared, Tab::Worlds, None),
    };
    // `home-stats`, `home-<mode>-stats`: Home with the stats sample (PB, sum of best, elo).
    if let Some(home) = id.strip_suffix("-stats").filter(|h| h.starts_with("home")) {
        let (start, preset) = deep_link(home, data_dir)?;
        return preset.is_none().then(|| {
            let preset = Preset {
                installed: true,
                ..Preset::stats(StatsPreset::Sample)
            };
            (start, Some(preset))
        });
    }
    let (base, mode) = match id.split_once('-') {
        Some((b, m)) => (b, Some(m)),
        None => (id, None),
    };
    let pick_mode = |start: &mut Start| -> Option<()> {
        if let Some(m) = mode {
            start.mode = Mode::from_id(m)?;
        }
        Some(())
    };
    let mut preset = None;
    match base {
        "home" => pick_mode(&mut start)?,
        // The Modes page became Instances; its old ids open the instance of the mode.
        "modes" | "mods" => {
            pick_mode(&mut start)?;
            start.page = Page::Instances;
            let key = if start.mode.is_shared() {
                InstKey::Shared
            } else {
                InstKey::Draftout
            };
            let tab = if base == "mods" {
                Tab::Mods
            } else {
                Tab::Worlds
            };
            start.inst = (key, tab, None);
        }
        // `instances[-<instance>][-<tab>][-cleanup]`, e.g. `instances-draftout-logs`.
        "instances" => {
            start.page = Page::Instances;
            for part in mode.unwrap_or("").split('-').filter(|p| !p.is_empty()) {
                if let Some(k) = InstKey::from_id(part) {
                    start.inst.0 = k;
                    if k == InstKey::Draftout {
                        start.mode = Mode::Draftout;
                    }
                } else if let Some(t) = Tab::from_id(part) {
                    start.inst.1 = t;
                } else if part == "cleanup" {
                    start.inst.2 = Some(Confirm::Cleanup);
                } else {
                    start.mode = Mode::from_id(part)?;
                }
            }
        }
        "practice" => {
            start.page = Page::Practice;
            let category = |id: &str| {
                catalog::practice()
                    .categories
                    .iter()
                    .position(|c| c.id == id)
            };
            if let Some(&(state, preset, cat, open)) =
                PRACTICE_STATES.iter().find(|s| Some(s.0) == mode)
            {
                start.practice = PracticeSel::Category(category(cat)?);
                start.practice_open = open.map(str::to_string);
                start.practice_about = state == "about";
                return Some((
                    start,
                    Some(Preset {
                        practice: preset,
                        ..Preset::new(None)
                    }),
                ));
            }
            start.practice = match mode {
                None => PracticeSel::Category(0),
                Some("tools") => PracticeSel::Tools,
                Some(cat) => PracticeSel::Category(
                    catalog::practice()
                        .categories
                        .iter()
                        .position(|c| c.id == cat)?,
                ),
            };
        }
        "tools" if mode.is_some() => {
            start.page = Page::Tools;
            let tools = match mode {
                Some("installing") => ToolsPreset::Installing,
                Some("update") => ToolsPreset::Update,
                Some("failed") => ToolsPreset::Failed,
                _ => return None,
            };
            return Some((
                start,
                Some(Preset {
                    tools,
                    ..Preset::new(None)
                }),
            ));
        }
        // Settings with Microsoft login on: no account yet, or the list.
        "account" => {
            start.page = Page::Settings;
            let accounts = match mode {
                None => AccountsPreset::Empty,
                Some("list") => AccountsPreset::List,
                _ => return None,
            };
            return Some((
                start,
                Some(Preset {
                    accounts: Some(accounts),
                    ..Preset::new(None)
                }),
            ));
        }
        // The login dialog over Settings.
        "login" => {
            start.page = Page::Settings;
            let accounts = match mode {
                None => AccountsPreset::Login,
                Some("nobrowser") => AccountsPreset::LoginNoBrowser,
                Some("steps") => AccountsPreset::LoginSteps,
                Some("done") => AccountsPreset::LoginDone,
                Some("error") => AccountsPreset::LoginError,
                _ => return None,
            };
            return Some((
                start,
                Some(Preset {
                    accounts: Some(accounts),
                    ..Preset::new(None)
                }),
            ));
        }
        // Ranked with a Microsoft account active: Home, signing in, the game screen (with the
        // renew warning), and Play stopped by the account.
        "msa" => {
            let installed = |s| Some((start.mode, s));
            let (page, state, renew_warning) = match mode? {
                "home" => (Page::Home, None, false),
                "signing" => (
                    Page::Home,
                    installed(ModeState::Installing(Progress {
                        stage: Stage::SigningIn,
                        done: 0,
                        total: 0,
                        unit: Unit::Files,
                    })),
                    false,
                ),
                "running" => (Page::Game, installed(ModeState::Running), false),
                "warning" => (Page::Game, installed(ModeState::Running), true),
                "relogin" => (
                    Page::Home,
                    installed(ModeState::Failed(backend::fake_sign_in_failure(false))),
                    false,
                ),
                "keyring" => (
                    Page::Home,
                    installed(ModeState::Failed(backend::fake_sign_in_failure(true))),
                    false,
                ),
                "sign-in-first" => (
                    Page::Home,
                    installed(ModeState::Failed(backend::fake_sign_in_first())),
                    false,
                ),
                _ => return None,
            };
            // Before the first sign-in nobody is in the list yet.
            let accounts = if mode == Some("sign-in-first") {
                AccountsPreset::Empty
            } else {
                AccountsPreset::SignedIn
            };
            start.page = page;
            return Some((
                start,
                Some(Preset {
                    accounts: Some(accounts),
                    renew_warning,
                    ..Preset::new(state)
                }),
            ));
        }
        // The Import page (the first start's offer) in each of its states.
        "import" => {
            start.page = Page::Import;
            let import = match mode {
                None => ImportPreset::Found,
                Some("scanning") => ImportPreset::Scanning,
                Some("none") => ImportPreset::Nothing,
                Some("other") => ImportPreset::Other,
                Some("importing") => ImportPreset::Importing,
                Some("done") => ImportPreset::Done,
                _ => return None,
            };
            return Some((
                start,
                Some(Preset {
                    import,
                    ..Preset::new(None)
                }),
            ));
        }
        // The launcher's own update: the notice in the top nav over Home (`update-ready`), the
        // same while a mode installs (`update-blocked`, on Settings so About says why), and
        // About in every other state.
        "update" => {
            start.page = Page::Settings;
            let (update, state) = match mode? {
                "ready" => {
                    start.page = Page::Home;
                    (UpdatePreset::Ready, None)
                }
                "about" => (UpdatePreset::Ready, None),
                "blocked" => (
                    UpdatePreset::Ready,
                    Some((
                        Mode::Rsg,
                        ModeState::Installing(Progress {
                            stage: Stage::Mods,
                            done: 3,
                            total: 7,
                            unit: Unit::Files,
                        }),
                    )),
                ),
                "checking" => (UpdatePreset::Checking, None),
                "current" => (UpdatePreset::Default, None),
                "downloading" => (UpdatePreset::Downloading, None),
                "manual" => (UpdatePreset::Manual, None),
                "error" => (UpdatePreset::Failed, None),
                "local" => (UpdatePreset::Local, None),
                _ => return None,
            };
            return Some((
                start,
                Some(Preset {
                    update,
                    ..Preset::new(state)
                }),
            ));
        }
        "settings" if mode == Some("credits") => {
            start.page = Page::Settings;
            start.credits = true;
        }
        // Settings with the Ranked and Draftout names filled in.
        "settings" if mode == Some("stats") => {
            start.page = Page::Settings;
            return Some((start, Some(Preset::stats(StatsPreset::Sample))));
        }
        "stats" if mode.is_some() => {
            start.page = Page::Stats;
            let stats = match mode {
                Some("scanning") => StatsPreset::Scanning,
                Some("empty") => StatsPreset::Empty,
                Some("unknown") => StatsPreset::Unknown,
                Some("offline") => StatsPreset::Offline,
                Some("unreachable") => StatsPreset::Unreachable,
                Some("new") => StatsPreset::New,
                _ => return None,
            };
            return Some((start, Some(Preset::stats(stats))));
        }
        "crash" | "stopped" => {
            pick_mode(&mut start)?;
            preset = Some((
                start.mode,
                ModeState::Failed(backend::fake_launch_failure(
                    start.mode,
                    base == "stopped",
                    data_dir,
                )),
            ));
        }
        "learn" => {
            start.page = Page::Learn;
            if let Some(topic) = mode {
                start.learn = catalog::learn().topics.iter().position(|t| t.id == topic)?;
            }
        }
        "tools" | "stats" | "settings" | "name" if mode.is_none() => {
            start.page = match base {
                "tools" => Page::Tools,
                "stats" => Page::Stats,
                "settings" => Page::Settings,
                _ => Page::Name,
            };
        }
        "installing" => {
            pick_mode(&mut start)?;
            const MIB: u64 = 1024 * 1024;
            preset = Some((
                start.mode,
                ModeState::Installing(Progress {
                    stage: Stage::Assets,
                    done: 148 * MIB,
                    total: 352 * MIB,
                    unit: Unit::Bytes,
                }),
            ));
        }
        "error" => {
            pick_mode(&mut start)?;
            preset = Some((
                start.mode,
                ModeState::Failed(backend::fake_failure(Stage::Assets)),
            ));
        }
        "running" => {
            pick_mode(&mut start)?;
            start.page = Page::Game;
            preset = Some((start.mode, ModeState::Running));
        }
        _ => return None,
    }
    Some((start, preset.map(|p| Preset::new(Some(p)))))
}

fn main() {
    if let Some(code) = cli::run(std::env::args().skip(1).collect()) {
        std::process::exit(code);
    }
    // Parse the embedded JSON up front so a broken file fails at startup.
    copy::init();
    catalog::practice();
    catalog::learn();
    if flag("SRL_DEMO") {
        demo::enable();
    }
    if flag("SRL_TRACE_RENDERS") {
        app::TRACE_RENDERS.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let paths = srl_core::Paths::from_env()
        .map_err(|e| eprintln!("settings won't be saved: {e:#}"))
        .ok();
    let data_dir = paths
        .as_ref()
        .map(|p| p.root().to_path_buf())
        .unwrap_or_default();
    let screen = std::env::var("SRL_SCREEN").ok().filter(|s| !s.is_empty());
    let (mut start, preset) = match &screen {
        Some(id) => deep_link(id, &data_dir).unwrap_or_else(|| {
            eprintln!("SRL_SCREEN: unknown screen id {id:?}, starting on home");
            deep_link("home", &data_dir).unwrap()
        }),
        None => deep_link("home", &data_dir).unwrap(),
    };
    // Only a normal start offers the settings import; a deep link shows what it asks for.
    start.offer_import = screen.is_none();
    // The demo's numbers and the preset states exist only in the fake.
    let fake = flag("SRL_FAKE") || demo::get().is_some() || preset.is_some();
    let lang_override = std::env::var("SRL_LANG")
        .ok()
        .filter(|s| !s.is_empty())
        .and_then(|code| {
            let lang = Language::from_code(&code);
            if lang.is_none() {
                eprintln!("SRL_LANG: unknown language {code:?} (en|pl), ignoring it");
            }
            lang
        });

    // The fake backend keeps its settings where the real one does: `<data root>/settings.json`.
    // The demo uses the prototype's player and saves nothing.
    let settings_file = paths.as_ref().map(Settings::file);
    let demo_settings = demo::get().map(|d| Settings {
        player_name: Some(d.account.name.clone()),
        language: lang_override.unwrap_or(Language::En),
        ..Settings::default()
    });
    let options = FakeOptions {
        modes: demo::get().map(|d| d.modes()).or_else(|| {
            preset.as_ref().filter(|p| p.installed).map(|_| {
                std::array::from_fn(|_| backend::ModeSnapshot {
                    state: ModeState::Ready,
                    info: backend::ModeInfo::default(),
                })
            })
        }),
        settings: demo_settings,
        settings_file,
        fail_first: flag("SRL_FAKE_FAIL"),
        preset: preset.as_ref().and_then(|p| p.mode.clone()),
        tools: preset.as_ref().map_or(ToolsPreset::Default, |p| p.tools),
        stats: preset
            .as_ref()
            .and_then(|p| p.stats)
            .unwrap_or_else(|| stats_default(start.page, demo::get().is_some())),
        // The fake's login is on where a real build's would be (a client ID is set).
        accounts: preset.as_ref().and_then(|p| p.accounts).unwrap_or(
            if srl_auth::AuthConfig::detect().is_some() {
                AccountsPreset::Empty
            } else {
                AccountsPreset::Unavailable
            },
        ),
        renew_warning: preset.as_ref().is_some_and(|p| p.renew_warning),
        practice: preset
            .as_ref()
            .map_or(PracticePreset::Default, |p| p.practice),
        practice_demo: demo::get().map(|d| {
            d.maps
                .iter()
                .map(|(id, m)| (id.clone(), m.status.clone()))
                .collect()
        }),
        data_dir,
        import: preset.as_ref().map_or(ImportPreset::Found, |p| p.import),
        update: preset.as_ref().map_or(UpdatePreset::Default, |p| p.update),
    };

    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            cx.set_quit_mode(gpui::QuitMode::Explicit);
            cx.text_system()
                .add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect())
                .expect("bundled fonts");
            v3::init_text(cx);

            let (tx, rx) = futures::channel::mpsc::unbounded();
            let backend: Rc<dyn Backend> = if fake {
                // The Instances page reads real folders: give the shared instance something to
                // show (worlds, packs, configs, screenshots, logs).
                // Only in a throwaway home (the screenshot tools set SRL_HOME), never in the
                // player's real data folder.
                if std::env::var_os("SRL_HOME").is_some()
                    && let Err(e) = backend::content::write_sample(
                        &options
                            .data_dir
                            .join("instances")
                            .join(srl_core::shared::DIR),
                    )
                {
                    eprintln!("sample instance content: {e:#}");
                }
                Rc::new(FakeBackend::new(tx, options))
            } else {
                match RealBackend::new(tx) {
                    Ok(b) => Rc::new(b),
                    Err(e) => {
                        let msg = format!("The launcher can't start: {e:#}");
                        eprintln!("{msg}");
                        error_box(&msg);
                        std::process::exit(1);
                    }
                }
            };
            let saved = backend.snapshot().settings;
            copy::set_lang(lang_override.unwrap_or(saved.language));
            // Without a deep link, the mode selected last time.
            if screen.is_none()
                && let Some(mode) = saved.last_mode
            {
                start.mode = mode;
            }

            let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            let window_options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(copy::t("app.window_title")),
                    ..Default::default()
                }),
                window_decorations: Some(WindowDecorations::Server),
                app_id: Some("speedrunning-launcher".into()),
                // GPUI throttles an unfocused window's frames to ~30 fps by default. The launcher
                // is usually unfocused exactly when it animates on its own (the game exits and it
                // goes back to Home), and it only draws during transitions and installs, so there
                // is nothing to save. Same setting as the prototype.
                inactive_frame_interval: None,
                ..Default::default()
            };
            cx.open_window(window_options, move |window, cx| {
                cx.new(|cx| Launcher::new(backend, rx, start, window, cx))
            })
            .expect("open window");
            cx.activate(true);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
        });
}

/// The release build on Windows has no console (`windows_subsystem`), so an error that ends the
/// launcher before its window opens would vanish without a trace: show it in a message box.
#[cfg(windows)]
fn error_box(msg: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (text, title) = (wide(msg), wide("Speedrunning Launcher"));
    // SAFETY: both strings are NUL-terminated UTF-16 that outlive the call; no owner window.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// Elsewhere stderr reaches the terminal or the desktop's log.
#[cfg(not(windows))]
fn error_box(_msg: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_links() {
        let link = |id| deep_link(id, Path::new("/srl"));
        let (s, p) = link("home-ssg").unwrap();
        assert_eq!((s.page, s.mode, p), (Page::Home, Mode::Ssg, None));
        let (s, _) = link("practice-end").unwrap();
        assert_eq!(s.practice, PracticeSel::Category(5));
        let (s, p) = link("running-rsg").unwrap();
        assert_eq!(s.page, Page::Game);
        assert_eq!(p.unwrap().mode, Some((Mode::Rsg, ModeState::Running)));
        let (s, p) = link("tools-update").unwrap();
        assert_eq!(s.page, Page::Tools);
        assert_eq!(p.unwrap().tools, ToolsPreset::Update);
        let (s, p) = link("settings-credits").unwrap();
        assert!(s.credits && p.is_none());
        let (_, p) = link("crash-draftout").unwrap();
        let Some((Mode::Draftout, ModeState::Failed(f))) = p.unwrap().mode else {
            panic!("no failure")
        };
        assert_eq!(
            f.log.as_deref(),
            Some(Path::new("/srl/logs/draftout-latest.log"))
        );
        assert!(link("home-aa").is_none());
        assert!(link("settings-x").is_none());
        assert!(link("tools-x").is_none());
        assert!(link("library").is_none());
        let (s, _) = link("learn-eyes").unwrap();
        assert_eq!(s.page, Page::Learn);
        assert_eq!(catalog::learn().topics[s.learn].id, "eyes");
        assert_eq!(link("learn").unwrap().0.learn, 0);
        assert!(link("learn-nope").is_none());
        let (s, p) = link("stats-offline").unwrap();
        assert_eq!(
            (s.page, p.unwrap().stats),
            (Page::Stats, Some(StatsPreset::Offline))
        );
        // Plain `stats` keeps the real backend unless SRL_FAKE asks for the fake.
        assert_eq!(link("stats").unwrap().1, None);
        assert_eq!(stats_default(Page::Stats, false), StatsPreset::Sample);
        assert_eq!(stats_default(Page::Home, false), StatsPreset::Nothing);
        assert!(link("stats-nope").is_none());
        let (s, p) = link("home-rsg-stats").unwrap();
        assert_eq!((s.page, s.mode), (Page::Home, Mode::Rsg));
        let p = p.unwrap();
        assert_eq!((p.stats, p.installed), (Some(StatsPreset::Sample), true));
        assert_eq!(link("home-stats").unwrap().0.mode, Mode::Ranked);
        assert!(link("home-xx-stats").is_none());
        assert!(link("installing-rsg-stats").is_none());
        let (s, p) = link("settings-stats").unwrap();
        assert_eq!(
            (s.page, p.unwrap().stats),
            (Page::Settings, Some(StatsPreset::Sample))
        );
        let (s, p) = link("login-error").unwrap();
        assert_eq!(s.page, Page::Settings);
        assert_eq!(p.unwrap().accounts, Some(AccountsPreset::LoginError));
        let (s, p) = link("msa-warning").unwrap();
        let p = p.unwrap();
        assert_eq!(s.page, Page::Game);
        assert!(p.renew_warning && p.accounts == Some(AccountsPreset::SignedIn));
        assert_eq!(p.mode, Some((Mode::Ranked, ModeState::Running)));
        assert!(link("msa").is_none());
        let (s, p) = link("import-done").unwrap();
        assert_eq!(
            (s.page, p.unwrap().import),
            (Page::Import, ImportPreset::Done)
        );
        assert_eq!(
            link("import").unwrap().1.unwrap().import,
            ImportPreset::Found
        );
        assert!(link("import-x").is_none());
        let (s, p) = link("update-ready").unwrap();
        assert_eq!(
            (s.page, p.unwrap().update),
            (Page::Home, UpdatePreset::Ready)
        );
        let (s, p) = link("update-blocked").unwrap();
        let p = p.unwrap();
        assert_eq!((s.page, p.update), (Page::Settings, UpdatePreset::Ready));
        assert!(matches!(
            p.mode,
            Some((Mode::Rsg, ModeState::Installing(_)))
        ));
        assert_eq!(
            link("update-error").unwrap().1.unwrap().update,
            UpdatePreset::Failed
        );
        assert!(link("update").is_none());
        assert!(link("update-x").is_none());
        // A deep link never offers the import on its own.
        assert!(!link("home").unwrap().0.offer_import);
        assert!(link("account-x").is_none());
        let (s, p) = link("practice-running").unwrap();
        assert_eq!(s.page, Page::Practice);
        assert_eq!(s.practice_open.as_deref(), Some("bt-practice-mescht"));
        assert_eq!(p.unwrap().practice, PracticePreset::Running);
        assert!(!s.practice_about);
        let (s, _) = link("practice-about").unwrap();
        assert_eq!(s.practice_open.as_deref(), Some("bt-practice-mescht"));
        assert!(s.practice_about);
        let (s, p) = link("practice-all-in-one").unwrap();
        assert_eq!((s.practice, p), (PracticeSel::Category(0), None));
        // No category can be mistaken for a state, and every state's category and map exist.
        for (id, _, cat, open) in PRACTICE_STATES {
            assert!(!catalog::practice().categories.iter().any(|c| c.id == id));
            let screen = format!("practice-{id}");
            assert!(deep_link(&screen, Path::new("/srl")).is_some(), "{id}");
            assert!(catalog::practice().categories.iter().any(|c| c.id == cat));
            assert!(
                open.is_none_or(|m| catalog::maps().get(m).is_some()),
                "{id}"
            );
        }
    }
}
