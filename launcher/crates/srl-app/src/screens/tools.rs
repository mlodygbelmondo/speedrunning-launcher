//! Tools (Narzędzia): one card per tool over the stronghold art band, in the flat style of
//! Practice's Tools tile. Each card says what the tool does, where it runs, its version and state,
//! which modes start it with the game, the Install / Update action and its source and licence.
//! Everything comes from the tools extension of the facade (`Backend::tools`) and the settings
//! (`Settings::starts_with`).
use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, ParentElement, SharedString, Styled, div,
    prelude::*, px,
};
use srl_core::{Mode, Unit};

use srl_tools::Unavailable;

use crate::{
    app::Launcher,
    backend::{
        ToolId, ToolSnapshot, ToolState,
        tools::{self, ToolFailure},
    },
    catalog,
    copy::{size, t, tf},
    v3::*,
};

use super::{
    play::bar_fraction,
    practice::{art_band, page_title},
};

/// Three cards over 1168 px with 12 px gaps, and their padding.
const CARD_W: f32 = (1168. - 2. * 12.) / 3.;
const PAD: f32 = 24.;
const INNER_W: f32 = CARD_W - 2. * PAD - 2.;
/// The install's progress bar.
const BAR_W: f32 = 200.;

/// The status of a tool as the card's corner shows it: copy key and colour.
fn status(state: &ToolState) -> (&'static str, Hsla) {
    match state {
        ToolState::Unsupported => ("tools.status.unsupported", faint3()),
        ToolState::NotInstalled => ("tools.status.missing", accent2()),
        ToolState::Installing(_) => ("tools.status.installing", accent2()),
        ToolState::Installed => ("tools.status.installed", ok3()),
        ToolState::UpdateAvailable => ("tools.status.update", gold()),
        ToolState::Failed(_) => ("tools.status.failed", behind3()),
    }
}

/// The version line, when there is one to show. A tool of the system (waywall) says where it was
/// found instead: it has no version to show. Always one line of the card's width: a long path is
/// cut in the middle (the launcher log has it whole).
fn version_line(tool: &ToolSnapshot) -> Option<SharedString> {
    let installed = tool.installed.as_deref();
    let latest = tool.latest.as_deref();
    if let (None, Some(path)) = (installed, &tool.path) {
        let line = tf("tools.found_at", &[("path", &path.display().to_string())]);
        return Some(ellipsize(
            &line,
            PLEX,
            FontWeight::MEDIUM,
            12.,
            INNER_W,
            Cut::Middle,
        ));
    }
    match (&tool.state, installed, latest) {
        (ToolState::Unsupported, ..) => None,
        (ToolState::UpdateAvailable, Some(v), Some(new)) => {
            Some(tf("tools.v_update", &[("v", v), ("new", new)]))
        }
        (_, Some(v), _) => Some(tf("tools.v_installed", &[("v", v)])),
        (_, None, Some(v)) => Some(tf("tools.v_latest", &[("v", v)])),
        _ => None,
    }
}

impl Launcher {
    /// One "start with the game" toggle, in the Mods page's chip style: on is the accent fill with
    /// a tick, off is dim; a mode whose rules ban the tool (or that can't have it yet) is struck
    /// through and takes no clicks. The chips share out the card's width.
    fn tool_chip(&self, tool: ToolId, mode: Mode, cx: &mut Context<Self>) -> AnyElement {
        let label = catalog::def(mode).short;
        if tools::mode_rule(tool, mode).is_some() {
            return chip_off(label).flex_grow(1.).px(px(4.)).into_any_element();
        }
        let on = self.settings.starts_with(tool, mode);
        let id: SharedString = format!("tool-chip-{}", tool.id()).into();
        chip((id, crate::backend::index(mode)), label, on, true)
            .flex_grow(1.)
            .px(px(4.))
            .on_click(cx.listener(move |this, _, _, cx| {
                let mut settings = this.settings.clone();
                settings.set_starts_with(tool, mode, !on);
                this.save_settings(settings, cx);
            }))
            .into_any_element()
    }

    /// The failure of the last install: a translated line in `behind`, the detail under it,
    /// wrapped (at most three lines; the whole error is in the launcher log).
    fn tool_failure(f: &ToolFailure) -> Div {
        let head = if f.offline {
            t("tools.fail.offline")
        } else {
            t("tools.fail.install")
        };
        let w = INNER_W - 14.;
        let detail = wrap_paths(
            &super::play::fail_detail(&crate::backend::FailDetail::Core(f.detail.clone())),
            PLEX,
            FontWeight::NORMAL,
            11.,
            w,
        );
        let lines: Vec<&str> = detail.split('\n').collect();
        let mut detail = lines[..lines.len().min(3)].join("\n");
        if lines.len() > 3 {
            detail.push('…');
        }
        div()
            .mb(px(14.))
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
                        inter(13., FontWeight::MEDIUM, 1.3)
                            .text_color(behind3())
                            .child(head),
                    )
                    .child(
                        plex(11., FontWeight::NORMAL)
                            .line_height(px(15.))
                            .whitespace_nowrap()
                            .text_color(faint3())
                            .child(detail),
                    ),
            )
    }

    /// Install, Update or Retry (a secondary button: a page of cards has no one main action) with
    /// what it downloads next to it, the install in progress, or nothing (installed, or not made
    /// for this OS).
    fn tool_action(&self, i: usize, tool: &ToolSnapshot, cx: &mut Context<Self>) -> Option<Div> {
        let id = tool.id;
        let button =
            |label: SharedString, hint: Option<SharedString>, cx: &mut Context<Self>| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(14.))
                    .child(secondary(("tool-action", i), &label).on_click(
                        cx.listener(move |this, _, _, _| this.backend().tools().install(id)),
                    ))
                    .children(hint.map(|h| {
                        inter(12., FontWeight::NORMAL, 1.)
                            .whitespace_nowrap()
                            .text_color(faint3())
                            .child(h)
                    }))
            };
        let install_hint = || match tool.download {
            Some(b) => tf("tools.install_hint", &[("size", &size(b))]),
            None => tf("tools.install_hint_plain", &[("source", "GitHub")]),
        };
        Some(match &tool.state {
            ToolState::Unsupported | ToolState::Installed => return None,
            // The launcher doesn't install it: say how, instead of a button that can't.
            ToolState::NotInstalled if tool.system => div().child(
                inter(13., FontWeight::NORMAL, 1.5)
                    .whitespace_nowrap()
                    .text_color(muted3())
                    .child(wrap(
                        &t("tools.system_install"),
                        INTER,
                        FontWeight::NORMAL,
                        13.,
                        INNER_W,
                    )),
            ),
            ToolState::NotInstalled => button(t("tools.install"), Some(install_hint()), cx),
            // The version line already says which version is out.
            ToolState::UpdateAvailable => button(t("tools.update"), None, cx),
            ToolState::Failed(f) => div()
                .flex()
                .flex_col()
                .child(Self::tool_failure(f))
                .child(button(t("tools.retry"), None, cx)),
            ToolState::Installing(p) => {
                let pct = (p.fraction() * 100.).floor().to_string();
                let detail = (p.total > 0).then(|| match p.unit {
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
                });
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(20.))
                            .child(secondary_off(&t("tools.installing")))
                            .child(tertiary(("tool-cancel", i), &t("v3.cancel")).on_click(
                                cx.listener(move |this, _, _, _| this.backend().tools().cancel(id)),
                            )),
                    )
                    .child(div().mt(px(12.)).w(px(BAR_W)).child(thin_progress(
                        self.eased_bar(format!("tool-{i}"), bar_fraction(p)),
                        3.,
                    )))
                    .children(detail.map(|d| {
                        plex(11., FontWeight::MEDIUM)
                            .mt(px(9.))
                            .text_color(muted3())
                            .child(d)
                    }))
            }
        })
    }

    fn tool_card(&self, i: usize, tool: &ToolSnapshot, cx: &mut Context<Self>) -> Div {
        let id = tool.id;
        let info = id.info();
        let unsupported = tool.state == ToolState::Unsupported;
        let (status_key, status_color) = status(&tool.state);
        let mut notes: Vec<String> = Vec::new();
        let rule = |why: Unavailable| {
            Mode::ALL
                .iter()
                .any(|m| tools::mode_rule(id, *m) == Some(why))
        };
        if rule(Unavailable::Banned) {
            notes.push(t("tools.banned_draftout").to_string());
        }
        if rule(Unavailable::NotSupported) {
            notes.push(t("tools.not_ready_draftout").to_string());
        }
        let start_with = if unsupported {
            inter(13., FontWeight::NORMAL, 1.5)
                .mt(px(18.))
                .whitespace_nowrap()
                .text_color(muted3())
                .child(wrap(
                    &t(&format!("tools.other_os.{}", id.id())),
                    INTER,
                    FontWeight::NORMAL,
                    13.,
                    INNER_W,
                ))
        } else {
            div()
                .mt(px(18.))
                .flex()
                .flex_col()
                .child(section_head(&t("tools.start_with")))
                .child(
                    div()
                        .mt(px(12.))
                        .flex()
                        .flex_row()
                        .gap(px(3.))
                        .children(Mode::ALL.map(|m| self.tool_chip(id, m, cx))),
                )
                .when(!notes.is_empty(), |d| {
                    d.child(
                        inter(12., FontWeight::NORMAL, 1.4)
                            .mt(px(10.))
                            .whitespace_nowrap()
                            .text_color(faint3())
                            .child(wrap(
                                &notes.join(" "),
                                INTER,
                                FontWeight::NORMAL,
                                12.,
                                INNER_W,
                            )),
                    )
                })
        };
        div()
            .flex_none()
            .w(px(CARD_W))
            .p(px(PAD))
            .flex()
            .flex_col()
            .bg(gpui::rgb(0x17131F))
            .border_1()
            .border_color(line3())
            .child(status_dot(&t(status_key), status_color))
            .child(
                tight(
                    info.name,
                    34.,
                    FontWeight::EXTRA_BOLD,
                    -0.02,
                    1.,
                    if unsupported { muted3() } else { text3() },
                )
                .mt(px(20.)),
            )
            // The version, or an empty line of the same height, so the descriptions line up.
            .child(
                div()
                    .mt(px(10.))
                    .h(px(16.))
                    .flex()
                    .flex_col()
                    .children(version_line(tool).map(|v| {
                        plex(12., FontWeight::MEDIUM)
                            .line_height(px(16.))
                            .text_color(faint3())
                            .child(v)
                    })),
            )
            .child(
                inter(14., FontWeight::NORMAL, 1.5)
                    .mt(px(16.))
                    .h(px(84.))
                    .whitespace_nowrap()
                    .text_color(if unsupported { faint3() } else { muted3() })
                    .child(wrap(
                        &t(info.copy_key),
                        INTER,
                        FontWeight::NORMAL,
                        14.,
                        INNER_W,
                    )),
            )
            .child(div().mt(px(14.)).h(px(1.)).bg(line3()))
            .child(start_with)
            .child(div().flex_1())
            .children(self.tool_action(i, tool, cx).map(|a| a.mt(px(26.))))
            .child(
                div()
                    .mt(px(22.))
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(
                        inter(12., FontWeight::NORMAL, 1.)
                            .text_color(faint3())
                            .child(tf("tools.credit", &[("authors", info.credit)])),
                    )
                    .child(
                        plex(11., FontWeight::NORMAL)
                            .text_color(faint3())
                            .child(tools::source(id)),
                    ),
            )
    }

    pub fn render_tools(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let tools = self.tools.clone();
        div()
            .size_full()
            .relative()
            .child(art_band("stronghold"))
            .child(
                div()
                    .absolute()
                    .left(px(64.))
                    .right(px(48.))
                    .top(px(104.))
                    .bottom(px(36.))
                    .flex()
                    .flex_col()
                    .child(page_title(&t("tools.title"), ""))
                    // The cards are as tall as the tallest one, so their rules and actions line
                    // up.
                    .child(
                        div()
                            .mt(px(30.))
                            .flex_none()
                            .flex()
                            .flex_row()
                            .gap(px(12.))
                            .children(
                                tools
                                    .iter()
                                    .enumerate()
                                    .map(|(i, tool)| self.tool_card(i, tool, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }
}
