//! The Play control that Home and Modes share, in every state of a mode: the light button (Play,
//! Download, Installing, Running, Retry), the install progress under it, the ghost actions next to
//! it and the error line.
use gpui::{Context, Div, FontWeight, ParentElement, SharedString, Styled, div, prelude::*, px};
use srl_core::{Mode, Progress, Stage, Unit};

use crate::{
    app::Launcher,
    backend::{FailDetail, FailKind, ModeState},
    catalog,
    copy::{fill, size, t, tf},
    demo,
    v3::*,
};

/// The copy key of an install stage.
pub fn stage_key(stage: Stage) -> &'static str {
    match stage {
        Stage::Waiting => "stage.waiting",
        Stage::Metadata => "stage.metadata",
        Stage::Java => "stage.java",
        Stage::Game => "stage.game",
        Stage::Assets => "stage.assets",
        Stage::Mods => "stage.mods",
        Stage::Tools => "stage.tools",
        Stage::SigningIn => "stage.signing_in",
        Stage::Launching => "stage.launching",
    }
}

/// How full an install bar is. Signing in and starting the game come after a finished install,
/// so their bar is full; any other stage with nothing counted yet (waiting, checking versions)
/// is empty.
pub fn bar_fraction(p: &Progress) -> f32 {
    if matches!(p.stage, Stage::Launching | Stage::SigningIn) {
        1.0
    } else {
        p.fraction()
    }
}

/// The mode's description under its name. The demo has its own wording where the prototype
/// promises what M1 doesn't do yet (RSG: "the launcher collects your times and splits").
pub fn mode_desc(mode: Mode) -> SharedString {
    let key = format!("v3.desc.{}", mode.id());
    demo::get()
        .and_then(|_| crate::copy::t_opt(&format!("{key}.demo")))
        .unwrap_or_else(|| t(&key))
}

/// The longest text a failure quotes (an error chain, the game's words); the full text is in the
/// launcher log.
const DETAIL_MAX: usize = 240;

/// `s`, cut to `DETAIL_MAX` characters.
fn shorten(s: &str) -> String {
    let mut out: String = s.chars().take(DETAIL_MAX).collect();
    if out.len() < s.len() {
        out.push('…');
    }
    out
}

/// The detail under a failure's first line, in the player's language. What the game or the core
/// said is quoted as it was written (English, from the game or a mod).
pub fn fail_detail(detail: &FailDetail) -> SharedString {
    match detail {
        FailDetail::Core(s) => tf("v3.fail.details", &[("text", &shorten(s))]),
        FailDetail::Exited { code, said } => {
            let exit = match code {
                Some(c) => tf("v3.fail.exit_code", &[("code", &c.to_string())]),
                None => t("v3.fail.exit_signal"),
            };
            match said {
                Some(s) => {
                    format!("{exit} {}", tf("v3.fail.said", &[("text", &shorten(s))])).into()
                }
                None => exit,
            }
        }
        FailDetail::Stopped(reason) => tf("v3.fail.stopped", &[("text", &shorten(reason))]),
        // srl-auth's reason in the player's language ("unlock your keyring", "log in again").
        FailDetail::Account(p) => super::account::problem_text(p),
    }
}

/// What fixes a failed Play when the failure says so: the main button does it instead of Retry,
/// and the message says what's wrong in one sentence.
#[derive(Clone, Debug, PartialEq)]
pub enum Fix {
    /// The JVM couldn't reserve the heap: start again with `to_mb`. `from_gb` is what it asked
    /// for ("6").
    Memory { from_gb: String, to_mb: u32 },
    /// Draftout refused a mod: the Mods page turns it off. The mods, when Draftout named them.
    Mods(Option<String>),
    /// The saved Microsoft sign-in is gone: sign in again.
    Relogin,
    /// Nobody has signed in here yet, and this build plays offline only after that
    /// (`account::play_as`): sign in.
    SignInFirst,
}

/// GB with at most one decimal ("6", "1.5").
fn gb(mb: u32) -> String {
    format!("{}", (mb as f32 / 1024. * 10.).round() / 10.)
}

/// The heap the JVM couldn't reserve, in MiB, from its words ("Could not reserve enough space
/// for 6291456KB object heap"). `Some(0)` when it says so without a size.
fn heap_refused(said: &str) -> Option<u32> {
    let at = said.find("Could not reserve enough space")?;
    let rest = &said[at..];
    let kb = rest
        .split_once("for ")
        .and_then(|(_, r)| r.split_once("KB"))
        .and_then(|(n, _)| n.trim().parse::<u64>().ok());
    Some(kb.map_or(0, |kb| (kb / 1024) as u32))
}

/// The mods Draftout named when it refused to start ("Disallowed mods (1): planifolia Remove
/// the disallowed mods…"), `Some(None)` when it refused without naming them, `None` when the
/// reason isn't Draftout's.
fn draftout_refused(reason: &str) -> Option<Option<String>> {
    if !reason.contains("Draftout blocked launch") && !reason.contains("Disallowed mods") {
        return None;
    }
    let names = reason
        .split_once("Disallowed mods")
        .and_then(|(_, r)| r.split_once("): "))
        .map(|(_, r)| r.split(" Remove").next().unwrap_or(r).trim().to_string())
        .filter(|n| !n.is_empty());
    Some(names)
}

impl Launcher {
    /// The fix of mode `mi`'s failed Play, when the failure names one.
    pub fn fix(&self, mi: usize) -> Option<Fix> {
        let ModeState::Failed(f) = self.state(mi) else {
            return None;
        };
        let mode = self.mode(mi);
        match &f.detail {
            FailDetail::Exited {
                said: Some(said), ..
            } => {
                let asked = heap_refused(said)?;
                let now = self.settings.memory_mb(mode);
                let from = if asked > 0 { asked } else { now };
                // 3 GB is the recommended value; below it only when 3 GB is what failed.
                let to_mb = if from > 3072 { 3072 } else { 2048 };
                (to_mb >= crate::backend::mode_needs(mode).min_memory_mb).then(|| Fix::Memory {
                    from_gb: gb(from),
                    to_mb,
                })
            }
            FailDetail::Stopped(reason) => draftout_refused(reason).map(Fix::Mods),
            FailDetail::Account(p) if p.key == crate::backend::account::SIGN_IN_FIRST => {
                Some(Fix::SignInFirst)
            }
            FailDetail::Account(p) if p.relogin => Some(Fix::Relogin),
            _ => None,
        }
    }

    /// Does the fix: less memory for the mode (all four 1.16.1 modes share one instance and one
    /// memory row, so all of them), then Play again; the Mods page; the sign-in dialog.
    fn apply_fix(&mut self, mi: usize, fix: &Fix, cx: &mut Context<Self>) {
        match fix {
            Fix::Memory { to_mb, .. } => {
                let mode = self.mode(mi);
                let mut settings = self.settings.clone();
                for m in Mode::ALL {
                    if m == mode || (mode.is_shared() && m.is_shared()) {
                        settings.set_memory_mb(m, *to_mb);
                    }
                }
                self.save_settings(settings, cx);
                self.play(mi, cx);
            }
            Fix::Mods(_) => self.open_mods(cx),
            Fix::Relogin | Fix::SignInFirst => self.backend().account().login(),
        }
    }

    /// A shared 1.16.1 mode that isn't installed while another of them is: the game, Java and
    /// most mods are already there, so Play only adds its own mods.
    pub fn shares_install(&self, mi: usize) -> bool {
        let mode = self.mode(mi);
        mode.is_shared()
            && self.state(mi) == &ModeState::NotInstalled
            && (0..self.modes.len())
                .any(|o| o != mi && self.mode(o).is_shared() && self.installed(o))
    }

    /// The Play / Download / Installing / Running / Retry button of mode `mi`, with the install
    /// progress under it (absolute, so the layout doesn't move while it fills).
    pub fn mode_play(
        &self,
        mi: usize,
        id: &'static str,
        w: f32,
        h: f32,
        label_size: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        let mode = self.mode(mi);
        let info = &self.modes[mi].info;
        let mut progress: Option<Div> = None;
        let fix = self.fix(mi);
        let (label, hint, interactive): (SharedString, SharedString, bool) = match self.state(mi) {
            ModeState::NotInstalled if self.shares_install(mi) => {
                (t("v3.play"), t("v3.play_hint.adds_mods"), true)
            }
            ModeState::NotInstalled => (
                t("v3.download"),
                match info.download {
                    Some(b) => tf("v3.download_hint", &[("size", &size(b))]),
                    None => tf(
                        "v3.download_about",
                        &[("size", &size(catalog::approx_download(mode)))],
                    ),
                },
                true,
            ),
            ModeState::Installing(p) => {
                let p = *p;
                let label = if matches!(p.stage, Stage::Launching | Stage::SigningIn) {
                    t("v3.starting")
                } else {
                    t("v3.installing")
                };
                // A stage with nothing to count (starting the game) shows only its name.
                let detail = (p.total > 0).then(|| {
                    let pct = (p.fraction() * 100.).floor().to_string();
                    let text = match p.unit {
                        Unit::Bytes => tf(
                            "v3.progress_bytes",
                            &[
                                ("pct", &pct),
                                ("done", &size(p.done)),
                                ("total", &size(p.total)),
                            ],
                        ),
                        Unit::Files => tf(
                            "v3.progress_files",
                            &[
                                ("pct", &pct),
                                ("done", &p.done.to_string()),
                                ("total", &p.total.to_string()),
                            ],
                        ),
                    };
                    plex(11., FontWeight::MEDIUM)
                        .mt(px(9.))
                        .text_color(muted3())
                        .child(text)
                });
                progress = Some(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(h + 7.))
                        .flex()
                        .flex_col()
                        .child(thin_progress(bar_fraction(&p), 3.))
                        .children(detail),
                );
                (label, t(stage_key(p.stage)), false)
            }
            ModeState::Ready => (t("v3.play"), self.play_hint(mode), true),
            ModeState::UpdateAvailable => (
                t("v3.play"),
                tf(
                    "v3.play_hint.update",
                    &[(
                        "mod",
                        info.update.as_deref().unwrap_or(catalog::def(mode).short),
                    )],
                ),
                true,
            ),
            ModeState::Running => (t("v3.running"), SharedString::default(), true),
            ModeState::Failed(_) if fix.is_some() => (
                match &fix {
                    Some(Fix::Memory { to_mb, .. }) => tf("v3.fix.memory", &[("n", &gb(*to_mb))]),
                    Some(Fix::Mods(_)) => t("v3.fix.mods"),
                    Some(Fix::SignInFirst) => t("v3.fix.sign_in"),
                    _ => t("v3.fix.relogin"),
                },
                SharedString::default(),
                true,
            ),
            ModeState::Failed(f) => (
                t("v3.retry"),
                // Only an install picks up where it stopped; a launch just starts again.
                match f.kind {
                    FailKind::Launch => t("v3.retry_hint.launch"),
                    FailKind::SignIn => t("v3.retry_hint.sign_in"),
                    FailKind::Offline | FailKind::Install => t("v3.retry_hint"),
                },
                true,
            ),
        };
        let interactive = interactive && self.sibling_busy(mi).is_none();
        let press = if interactive {
            Press::Live
        } else {
            Press::Busy
        };
        let btn = play_button(id, &label, hint, w, h, label_size, press).when(interactive, |b| {
            b.on_click(cx.listener(move |this, _, _, cx| match this.fix(mi) {
                Some(fix) => this.apply_fix(mi, &fix, cx),
                None => this.play(mi, cx),
            }))
        });
        div().relative().flex_none().child(btn).children(progress)
    }

    /// The hint under Play for a ready mode.
    fn play_hint(&self, mode: Mode) -> SharedString {
        if mode == Mode::Ranked {
            t("v3.play_hint.ranked")
        } else if demo::get().is_some() {
            // "launches Minecraft and your tools": true in the prototype, where tools exist.
            t("v3.play_hint.run")
        } else {
            tf("v3.play_hint.plain", &[("mc", mode.minecraft_version())])
        }
    }

    /// The ghost actions next to Play: Cancel while installing; with the demo, the prototype's
    /// Verify files (and Open folder on Modes) for an installed mode. Without the demo there is
    /// nothing to verify or open yet, so nothing shows.
    pub fn play_ghosts(
        &self,
        mi: usize,
        prefix: &'static str,
        open_folder: bool,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::Stateful<Div>> {
        if matches!(self.state(mi), ModeState::Installing(_)) {
            let id: SharedString = format!("{prefix}-cancel").into();
            return vec![
                tertiary(id, &t("v3.cancel"))
                    .on_click(cx.listener(move |this, _, _, cx| this.cancel(mi, cx))),
            ];
        }
        // The Microsoft sign-in stopped Play: the page that helps (signing in again is the main
        // button, `Fix::Relogin`).
        if let ModeState::Failed(f) = self.state(mi)
            && let FailDetail::Account(p) = &f.detail
        {
            let mut v = Vec::new();
            if let Some(url) = p.help_url {
                v.push(
                    tertiary(
                        SharedString::from(format!("{prefix}-help")),
                        &t("auth.help"),
                    )
                    .on_click(cx.listener(move |_, _, _, cx| cx.open_url(url))),
                );
            }
            return v;
        }
        if demo::get().is_none() || !self.installed(mi) {
            return Vec::new();
        }
        let mut v = vec![tertiary(
            SharedString::from(format!("{prefix}-verify")),
            &t("v3.verify"),
        )];
        if open_folder {
            v.push(tertiary(
                SharedString::from(format!("{prefix}-folder")),
                &t("v3.open_folder"),
            ));
        }
        v
    }

    /// Under Play: the mods the player chose make the run not legal for the mode (the game still
    /// starts), or another mode of the shared instance runs, so this one waits.
    pub fn mods_line(&self, mi: usize, max_w: f32) -> Option<Div> {
        let short = catalog::MODES[mi].short;
        let (text, color) = if let Some(other) = self.sibling_busy(mi) {
            (
                tf("v3.sibling_busy", &[("other", catalog::MODES[other].short)]),
                muted3(),
            )
        } else {
            let line = super::mods::concern_line(&self.modes[mi].info.concerns, short)?;
            (line, gold())
        };
        Some(
            div()
                .mt(px(14.))
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
                        .child(wrap(&text, INTER, FontWeight::MEDIUM, 13., max_w - 14.)),
                ),
        )
    }

    /// Another mode of the shared 1.16.1 instance is running or installing: this one can't
    /// start until it is done (they share `mods/`).
    pub fn sibling_busy(&self, mi: usize) -> Option<usize> {
        let mode = self.mode(mi);
        if !mode.is_shared() {
            return None;
        }
        (0..self.modes.len()).find(|&o| {
            o != mi
                && self.mode(o).is_shared()
                && matches!(self.state(o), ModeState::Running | ModeState::Installing(_))
        })
    }

    /// The error of a failed `play`, under the actions: one translated line that says what
    /// happened (no internet, couldn't install, the game closed while starting, or what the fix
    /// on the main button is for) in the `behind` colour, then the detail (`fail_detail`,
    /// translated around what the game said) fainter, and Open log when the game ran. Everything
    /// wrapped to `max_w`.
    pub fn fail_line(&self, mi: usize, max_w: f32, cx: &mut Context<Self>) -> Option<Div> {
        let ModeState::Failed(f) = self.state(mi) else {
            return None;
        };
        let short = catalog::MODES[mi].short;
        let key = match (f.kind, f.installed) {
            (FailKind::Offline, false) => "v3.fail.offline",
            (FailKind::Offline, true) => "v3.fail.offline_update",
            (FailKind::Install, _) => "v3.fail.install",
            (FailKind::Launch, _) => "v3.fail.launch",
            (FailKind::SignIn, _) => "v3.fail.sign_in",
        };
        // A known fix says it all in one sentence; the game's English words would only repeat it.
        let (head, detail) = match self.fix(mi) {
            Some(Fix::Memory { from_gb, .. }) => (tf("v3.fail.memory", &[("gb", &from_gb)]), None),
            Some(Fix::Mods(Some(names))) => (
                tf("v3.fail.draftout_mod", &[("mode", short), ("mod", &names)]),
                None,
            ),
            Some(Fix::Mods(None)) => (fill(&t("v3.fail.draftout_mods"), &[("mode", short)]), None),
            Some(Fix::Relogin) => (fill(&t(key), &[("mode", short)]), None),
            Some(Fix::SignInFirst) => (t("v3.fail.sign_in_first"), None),
            None => (
                fill(&t(key), &[("mode", short)]),
                Some(fail_detail(&f.detail)),
            ),
        };
        let log = f.log.clone();
        let w = max_w - 14.;
        // A sign-in failure's detail is our own sentence, not the game's log line.
        let prose = f.kind == FailKind::SignIn;
        Some(
            div()
                .mt(px(18.))
                .flex()
                .flex_row()
                .items_start()
                .child(
                    div()
                        .flex_none()
                        .size(px(6.))
                        .mt(px(6.))
                        .mr(px(8.))
                        .bg(behind3()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(
                            inter(13., FontWeight::MEDIUM, 1.4)
                                .whitespace_nowrap()
                                .text_color(behind3())
                                .child(wrap(&head, INTER, FontWeight::MEDIUM, 13., w)),
                        )
                        .children(detail.map(|d| {
                            if prose {
                                inter(12., FontWeight::NORMAL, 1.4)
                                    .whitespace_nowrap()
                                    .text_color(muted3())
                                    .child(wrap(&d, INTER, FontWeight::NORMAL, 12., w))
                            } else {
                                plex(11., FontWeight::NORMAL)
                                    .line_height(px(15.))
                                    .text_color(faint3())
                                    .child(wrap_paths(&d, PLEX, FontWeight::NORMAL, 11., w))
                            }
                        }))
                        .children(log.map(|l| {
                            tertiary(("fail-log", mi), &t("v3.open_log"))
                                .mt(px(8.))
                                .on_click(cx.listener(move |this, _, _, _| {
                                    this.backend().open_containing_folder(&l)
                                }))
                        })),
                ),
        )
    }
}
