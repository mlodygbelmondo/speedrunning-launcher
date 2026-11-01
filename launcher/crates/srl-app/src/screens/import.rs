//! "Import my current settings" (srl-import through `backend::importer`): one full-window page,
//! offered once on the first start (after the name) and opened from Settings. Left: the
//! instances found on this computer, best match preselected, and "Choose folder…". Right: what
//! to bring (four ticks with the screen's defaults) and one Import button. After the import the
//! page says where everything went and where the replaced files are.
use std::path::PathBuf;

use chrono::{Local, TimeZone};
use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, ParentElement, PathPromptOptions, SharedString,
    Styled, Window, div, prelude::*, px,
};
use srl_core::{Instance, Mode};
use srl_import::{Choice, Launcher as From, Source};

use crate::{
    app::{Launcher, Page},
    backend::{
        ImportEvent, ImportResult, ModeState, Settings,
        importer::{effective, instance_of, versions},
    },
    catalog,
    copy::{t, tf, tn},
    v3::*,
};

use super::{
    game::{brand_mark, hero_backdrop},
    stats::{fmt_date, today_and_month_ms},
};

/// The column widths: 520 + 80 + 552 = 1152 of the 1280 px window.
const LEFT_W: f32 = 520.;
const RIGHT_W: f32 = 552.;
const RIGHT_X: f32 = 664.;
/// Where the list and the ticks start, under the title and its text.
const BODY_TOP: f32 = 300.;
const ROW_H: f32 = 62.;
/// Rows the list shows before it scrolls.
const ROWS: usize = 4;

/// A note under "Choose folder…".
#[derive(Clone, Debug, PartialEq)]
pub enum PickNote {
    /// Nothing to import in that folder.
    Nothing(PathBuf),
    /// It is this launcher's own data.
    Own,
    /// The system's folder picker didn't open.
    Picker(String),
}

/// The Import page's state.
#[derive(Default)]
pub struct ImportView {
    /// A scan has answered.
    pub scanned: bool,
    /// What the scan found, best match first.
    pub found: Vec<Source>,
    /// What picked folders added, newest first; listed above the scan's.
    pub picked: Vec<Source>,
    /// The selected instance, by its game directory (the list may change under it).
    pub selected: Option<PathBuf>,
    pub choice: Choice,
    /// Files done and the total while an import runs.
    pub progress: Option<(u64, u64)>,
    pub result: Option<ImportResult>,
    pub note: Option<PickNote>,
    /// Opened as the first start's offer (Skip), not from Settings (Back).
    pub offer: bool,
    /// This start offers the import (once the name is in), and goes on by itself when there is
    /// nothing to offer. Never for a deep link.
    pub offer_pending: bool,
    /// The list's scroll.
    pub scroll: gpui::ScrollHandle,
}

impl ImportView {
    /// Every instance on the page: the picked ones, then the scan's, each once.
    pub fn sources(&self) -> Vec<&Source> {
        let mut seen = std::collections::HashSet::new();
        self.picked
            .iter()
            .chain(&self.found)
            .filter(|s| seen.insert(s.game_dir.clone()))
            .collect()
    }

    pub fn selected_source(&self) -> Option<&Source> {
        let all = self.sources();
        self.selected
            .as_ref()
            .and_then(|sel| all.iter().find(|s| &s.game_dir == sel).copied())
            .or_else(|| all.first().copied())
    }
}

impl Launcher {
    /// Opens the Import page: from Settings, or as the first start's offer.
    pub fn open_import(&mut self, offer: bool, cx: &mut Context<Self>) {
        let v = &mut self.import;
        v.offer = offer;
        v.result = None;
        v.note = None;
        if !self.target.full_window() {
            self.return_to = self.target;
        }
        self.backend().import().scan();
        self.navigate(Page::Import, cx);
    }

    /// Skip, Back or Continue: the offer is done for good, and the page goes back to where the
    /// player came from.
    pub fn leave_import(&mut self, cx: &mut Context<Self>) {
        self.import.offer_pending = false;
        if !self.settings.import_offered {
            let settings = Settings {
                import_offered: true,
                ..self.settings.clone()
            };
            self.save_settings(settings, cx);
        }
        self.navigate(self.return_to, cx);
    }

    /// After the name prompt on the first start: the import's offer, unless the scan already
    /// found nothing to offer.
    pub fn offer_import_or_go_on(&mut self, cx: &mut Context<Self>) {
        if !self.import.offer_pending || self.settings.import_offered {
            self.navigate(self.return_to, cx);
        } else if self.import.scanned && self.import.sources().is_empty() {
            self.leave_import(cx);
        } else {
            self.open_import(true, cx);
        }
    }

    pub fn apply_import(&mut self, ev: ImportEvent, cx: &mut Context<Self>) {
        let v = &mut self.import;
        match ev {
            ImportEvent::Sources(found) => {
                v.scanned = true;
                v.found = found;
                // The first start's offer with nothing to offer goes on by itself.
                if v.offer
                    && v.offer_pending
                    && v.sources().is_empty()
                    && v.progress.is_none()
                    && v.result.is_none()
                    && self.target == Page::Import
                {
                    self.leave_import(cx);
                }
            }
            ImportEvent::Chosen { dir, found, own } => {
                if own {
                    v.note = Some(PickNote::Own);
                } else if let Some(first) = found.first() {
                    v.note = None;
                    v.selected = Some(first.game_dir.clone());
                    let mut picked = found;
                    picked.append(&mut v.picked);
                    v.picked = picked;
                    v.scroll.scroll_to_item(0);
                } else {
                    v.note = Some(PickNote::Nothing(dir));
                }
            }
            ImportEvent::Progress { done, total } => v.progress = Some((done, total)),
            ImportEvent::Done(result) => {
                v.progress = None;
                v.result = Some(*result);
                if !self.settings.import_offered {
                    let settings = Settings {
                        import_offered: true,
                        ..self.settings.clone()
                    };
                    self.save_settings(settings, cx);
                }
            }
        }
    }

    /// "Choose folder…": the system's folder picker (`SRL_IMPORT_PICK` stands in for it in
    /// tests), then the backend reads the folder.
    fn choose_folder(&mut self, cx: &mut Context<Self>) {
        self.import.note = None;
        if let Some(dir) = std::env::var_os("SRL_IMPORT_PICK").filter(|v| !v.is_empty()) {
            self.backend().import().inspect(PathBuf::from(dir));
            return;
        }
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t("import.choose_ok")),
        });
        cx.spawn(async move |this, cx| {
            let answer = picked.await;
            let _ = this.update(cx, |app, cx| {
                match answer {
                    Ok(Ok(Some(mut dirs))) if !dirs.is_empty() => {
                        app.backend().import().inspect(dirs.swap_remove(0))
                    }
                    Ok(Err(e)) => app.import.note = Some(PickNote::Picker(e.to_string())),
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// A game runs or installs somewhere: the import waits for it, since the game writes its
    /// options.txt when it quits.
    fn import_blocked(&self) -> bool {
        let busy = |s: &ModeState| matches!(s, ModeState::Running | ModeState::Installing(_));
        self.modes.iter().any(|m| busy(&m.state)) || busy(&self.practice.game.state)
    }

    fn start_import(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self.import.selected_source().cloned() else {
            return;
        };
        let choice = effective(self.import.choice, &source, &versions());
        if !choice.any() || self.import.progress.is_some() || self.import_blocked() {
            return;
        }
        self.import.progress = Some((0, 0));
        self.backend().import().import(source, choice);
        cx.notify();
    }

    pub fn render_import(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let page = div()
            .size_full()
            .relative()
            .children(hero_backdrop(self.hero_images(), 0.72))
            .child(brand_mark());
        let body = match self.import.result.clone() {
            Some(result) => self.import_result(&result, cx),
            None => self.import_choose(cx),
        };
        page.child(body)
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom(px(24.))
                    .flex()
                    .justify_center()
                    .child(
                        inter(11., FontWeight::NORMAL, 1.4)
                            .text_color(faint3())
                            .child(t("settings.disclaimer")),
                    ),
            )
            .into_any_element()
    }

    /// The heading: title and text.
    fn import_head(title: &str, body: &str) -> Div {
        div()
            .absolute()
            .left(px(64.))
            .top(px(130.))
            .w(px(1100.))
            .flex()
            .flex_col()
            .child(tight(
                title,
                56.,
                FontWeight::EXTRA_BOLD,
                -0.03,
                0.95,
                text3(),
            ))
            .child(
                inter(15., FontWeight::NORMAL, 1.5)
                    .mt(px(18.))
                    .whitespace_nowrap()
                    .text_color(muted3())
                    .child(wrap(body, INTER, FontWeight::NORMAL, 15., 620.)),
            )
    }

    fn import_choose(&mut self, cx: &mut Context<Self>) -> Div {
        let head = Self::import_head(&t("import.title"), &t("import.body"));
        div()
            .size_full()
            .child(head)
            .child(self.import_list(cx))
            .child(self.import_ticks(cx))
    }

    /// The instances found, and "Choose folder…".
    fn import_list(&mut self, cx: &mut Context<Self>) -> Div {
        let v = &self.import;
        let sources: Vec<Source> = v.sources().into_iter().cloned().collect();
        let selected = v.selected_source().map(|s| s.game_dir.clone());
        let busy = v.progress.is_some();
        let mut col = div()
            .absolute()
            .left(px(64.))
            .top(px(BODY_TOP))
            .w(px(LEFT_W))
            .flex()
            .flex_col()
            .child(section_head(&t("import.found")).mb(px(14.)));
        if sources.is_empty() {
            let text = if v.scanned {
                t("import.none")
            } else {
                t("import.scanning")
            };
            col = col.child(
                div().border_t_1().border_color(line3()).pt(px(16.)).child(
                    inter(14., FontWeight::NORMAL, 1.5)
                        .whitespace_nowrap()
                        .text_color(muted3())
                        .child(wrap(&text, INTER, FontWeight::NORMAL, 14., LEFT_W)),
                ),
            );
        } else {
            let rows: Vec<Div> = sources
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let on = selected.as_ref() == Some(&s.game_dir);
                    let dir = s.game_dir.clone();
                    let row = div()
                        .id(("import-source", i))
                        .h(px(ROW_H))
                        .px(px(14.))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(14.))
                        .border_t_1()
                        .border_color(line3())
                        .when(on, |d| d.bg(text3a(0.06)))
                        .when(!on && !busy, |d| {
                            d.cursor_pointer()
                                .hover(|s| s.bg(text3a(0.03)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.import.selected = Some(dir.clone());
                                    cx.notify();
                                }))
                        })
                        .child(radio(on))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .child(
                                    inter(15., FontWeight::SEMIBOLD, 1.1)
                                        .text_color(text3())
                                        .child(ellipsize(
                                            &s.name,
                                            INTER,
                                            FontWeight::SEMIBOLD,
                                            15.,
                                            LEFT_W - 70.,
                                            Cut::End,
                                        )),
                                )
                                .child(
                                    plex(11., FontWeight::MEDIUM)
                                        .text_color(muted3())
                                        .child(source_line(s)),
                                ),
                        );
                    div().child(row)
                })
                .collect();
            col = col.child(
                div()
                    .id("import-list")
                    .max_h(px(ROW_H * ROWS as f32 + 2.))
                    .overflow_y_scroll()
                    .track_scroll(&self.import.scroll)
                    .children(rows),
            );
            if let Some(dir) = &selected {
                col = col.child(
                    plex(11., FontWeight::NORMAL)
                        .mt(px(10.))
                        .text_color(faint3())
                        .child(ellipsize(
                            &dir.display().to_string(),
                            PLEX,
                            FontWeight::NORMAL,
                            11.,
                            LEFT_W,
                            Cut::Middle,
                        )),
                );
            }
        }
        let choose = secondary("import-choose", &t("import.choose")).when(!busy, |b| {
            b.on_click(cx.listener(|this, _, _, cx| this.choose_folder(cx)))
        });
        col = col.child(div().mt(px(18.)).flex().flex_row().child(choose));
        if let Some(note) = &self.import.note {
            let text = match note {
                PickNote::Nothing(dir) => tf(
                    "import.picked_nothing",
                    &[("path", &dir.display().to_string())],
                ),
                PickNote::Own => t("import.picked_own"),
                PickNote::Picker(e) => tf("import.picker_failed", &[("error", e)]),
            };
            col = col.child(
                inter(12., FontWeight::NORMAL, 1.4)
                    .mt(px(10.))
                    .whitespace_nowrap()
                    .text_color(behind3())
                    .child(wrap_paths(&text, INTER, FontWeight::NORMAL, 12., LEFT_W)),
            );
        }
        col
    }

    /// What to bring, the notes on versions, Import and Skip.
    fn import_ticks(&mut self, cx: &mut Context<Self>) -> Div {
        let source = self.import.selected_source().cloned();
        let choice = self.import.choice;
        let busy = self.import.progress.is_some();
        let all = versions();
        let same = source
            .as_ref()
            .is_some_and(|s| all.iter().any(|v| s.runs(v)));
        let c = source
            .as_ref()
            .map(|s| s.contents.clone())
            .unwrap_or_default();
        // Why a part can't be ticked: the instance lacks it, or its version can't take it.
        let other_version = || match source.as_ref().and_then(|s| s.version.clone()) {
            Some(v) => tf("import.other_part", &[("v", &v)]),
            None => t("import.other_part_unknown"),
        };
        let none = t("import.none_part");
        // Before an instance is chosen (or found) the rows say what each part is, not what an
        // empty instance lacks.
        let chosen = source.is_some();
        let mods_hint = if !chosen {
            t("import.mods_hint")
        } else if c.mods.is_empty() {
            tn("import.mods_files", c.mod_files as u64)
        } else {
            mod_names(&c.mods)
        };
        let parts: [(&'static str, SharedString, Option<SharedString>, bool); 4] = [
            (
                "import.options",
                if same {
                    t("import.options_hint")
                } else {
                    t("import.options_hint_keys")
                },
                (chosen && !c.options).then(|| none.clone()),
                choice.options,
            ),
            (
                "import.mods",
                mods_hint,
                if !chosen {
                    None
                } else if c.mod_files == 0 {
                    Some(none.clone())
                } else if !same {
                    Some(other_version())
                } else {
                    None
                },
                choice.mod_settings,
            ),
            (
                "import.packs",
                if chosen {
                    tf(
                        "import.packs_hint",
                        &[("count", &tn("import.packs_n", c.resource_packs as u64))],
                    )
                } else {
                    t("import.packs_hint_any")
                },
                if !chosen {
                    None
                } else if c.resource_packs == 0 {
                    Some(none.clone())
                } else if !same {
                    Some(other_version())
                } else {
                    None
                },
                choice.resource_packs,
            ),
            (
                "import.worlds",
                if chosen {
                    tf(
                        "import.worlds_hint",
                        &[("count", &tn("import.worlds_n", c.worlds as u64))],
                    )
                } else {
                    t("import.worlds_hint_any")
                },
                if !chosen {
                    None
                } else if c.worlds == 0 {
                    Some(none.clone())
                } else if !same {
                    Some(other_version())
                } else {
                    None
                },
                choice.worlds,
            ),
        ];
        let rows: Vec<gpui::Stateful<Div>> = parts
            .into_iter()
            .enumerate()
            .map(|(i, (label, hint, off, ticked))| {
                let live = off.is_none() && chosen && !busy;
                let on = ticked && off.is_none() && chosen;
                let dim = off.is_some() || !chosen;
                div()
                    .id(("import-part", i))
                    .min_h(px(58.))
                    .py(px(11.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(16.))
                    .border_t_1()
                    .border_color(line3())
                    .when(live, |d| {
                        d.cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let f = part(&mut this.import.choice, i);
                                *f = !*f;
                                cx.notify();
                            }))
                    })
                    .child(tick(on, live))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                inter(15., FontWeight::SEMIBOLD, 1.2)
                                    .text_color(if dim { faint3() } else { text3() })
                                    .child(t(label)),
                            )
                            .child(
                                inter(12., FontWeight::NORMAL, 1.4)
                                    .whitespace_nowrap()
                                    .text_color(if dim { faint3() } else { muted3() })
                                    .child(ellipsize(
                                        &off.unwrap_or(hint),
                                        INTER,
                                        FontWeight::NORMAL,
                                        12.,
                                        RIGHT_W - 40.,
                                        Cut::End,
                                    )),
                            ),
                    )
            })
            .collect();

        // One line on versions: what Draftout (or every mode) gets. Another version's rows say
        // what doesn't come over, so it needs no line.
        let version_note = source.as_ref().and_then(|s| {
            if s.runs(srl_import::VERSION_1_16_1) {
                Some((t("import.note_draftout"), faint3()))
            } else if s.runs(Mode::Draftout.minecraft_version()) {
                Some((t("import.note_from_draftout"), gold()))
            } else {
                None
            }
        });
        let effective_choice = source
            .as_ref()
            .map(|s| effective(choice, s, &all))
            .unwrap_or(Choice {
                options: false,
                mod_settings: false,
                resource_packs: false,
                worlds: false,
            });
        let blocked = self.import_blocked();
        let (press, hint) = match self.import.progress {
            Some((done, total)) if total > 0 => (
                Press::Busy,
                tf(
                    "import.button_progress",
                    &[("done", &done.to_string()), ("total", &total.to_string())],
                ),
            ),
            Some(_) => (Press::Busy, t("import.button_starting")),
            None if source.is_none() => (Press::Off, t("import.button_none")),
            None if blocked => (Press::Off, t("import.button_wait")),
            None if !effective_choice.any() => (Press::Off, t("import.button_nothing")),
            None => (Press::Live, SharedString::default()),
        };
        let label = if busy {
            t("import.button_busy")
        } else {
            t("import.button")
        };
        let button = play_button("import-go", &label, hint, 288., 64., 19., press)
            .when(press == Press::Live, |b| {
                b.on_click(cx.listener(|this, _, _, cx| this.start_import(cx)))
            });
        let leave = tertiary(
            "import-leave",
            &t(if self.import.offer {
                "import.skip"
            } else {
                "import.back"
            }),
        )
        .when(!busy, |b| {
            b.on_click(cx.listener(|this, _, _, cx| this.leave_import(cx)))
        });

        let note = |s: &str, color: Hsla| {
            inter(12., FontWeight::NORMAL, 1.45)
                .whitespace_nowrap()
                .text_color(color)
                .child(wrap(s, INTER, FontWeight::NORMAL, 12., RIGHT_W))
        };
        div()
            .absolute()
            .left(px(RIGHT_X))
            .top(px(BODY_TOP))
            .w(px(RIGHT_W))
            .flex()
            .flex_col()
            .child(section_head(&t("import.what")).mb(px(14.)))
            .children(rows)
            .children(version_note.map(|(s, color)| note(&s, color).mt(px(14.))))
            .child(
                div()
                    .mt(px(22.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(32.))
                    .child(button)
                    .child(leave),
            )
    }

    /// After the import: where everything went.
    fn import_result(&mut self, r: &ImportResult, cx: &mut Context<Self>) -> Div {
        let source = self.import.selected_source().cloned();
        // The shared 1.16.1 instance is its four modes, so a list of targets reads "Ranked, RSG,
        // FSG, SSG and Practice", not "Ranked, RSG, FSG and SSG and Practice".
        let names = |id: &str| -> Vec<String> {
            if id == srl_core::shared::DIR {
                return Mode::ALL
                    .into_iter()
                    .filter(|m| m.is_shared())
                    .map(|m| catalog::def(m).short.to_string())
                    .collect();
            }
            vec![match instance_of(id) {
                Some(Instance::Mode(m)) => catalog::def(m).short.to_string(),
                Some(Instance::Practice) => t("import.practice").to_string(),
                Some(Instance::Custom(_)) | None => id.to_string(),
            }]
        };
        let name = |id: &str| list_and(&names(id));
        let ok = |full: bool| {
            r.report
                .targets
                .iter()
                .filter(|t| t.full == full && t.error.is_none())
                .flat_map(|t| names(&t.id))
                .collect::<Vec<_>>()
        };
        let mut lines: Vec<(SharedString, Hsla)> = Vec::new();
        let (full, keys) = (ok(true), ok(false));
        if !full.is_empty() {
            lines.push((
                tf("import.done_full", &[("modes", &list_and(&full))]),
                text3(),
            ));
        }
        if !keys.is_empty() {
            lines.push((
                tf("import.done_keys", &[("modes", &list_and(&keys))]),
                text3(),
            ));
        }
        if !r.busy.is_empty() {
            let busy: Vec<String> = r.busy.iter().flat_map(|i| names(&i.dir_id())).collect();
            lines.push((
                tf("import.done_busy", &[("modes", &list_and(&busy))]),
                gold(),
            ));
        }
        for t in r.report.targets.iter().filter(|t| t.error.is_some()) {
            lines.push((
                tf(
                    "import.done_error",
                    &[
                        ("mode", &name(&t.id)),
                        ("error", t.error.as_deref().unwrap_or("")),
                    ],
                ),
                behind3(),
            ));
        }
        let kept: u32 = r
            .report
            .targets
            .iter()
            .map(|t| t.worlds_kept)
            .max()
            .unwrap_or(0);
        if kept > 0 {
            lines.push((tn("import.done_worlds_kept", kept as u64), muted3()));
        }
        let skipped = r.report.skipped.len() as u64 + r.report.skipped_more as u64;
        if skipped > 0 {
            lines.push((tn("import.done_skipped", skipped), muted3()));
        }
        let backups: Vec<&srl_import::TargetReport> = r
            .report
            .targets
            .iter()
            .filter(|t| t.backup.is_some())
            .collect();
        // Where the replaced files are: named in the line, opened by the button under it.
        let backup = backups.first().and_then(|t| t.backup.clone());
        if let Some(first) = backups.first() {
            let folder = first
                .backup
                .as_ref()
                .and_then(|b| b.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let modes: Vec<String> = backups.iter().flat_map(|t| names(&t.id)).collect();
            lines.push((
                tf(
                    "import.done_backup",
                    &[("folder", &folder), ("modes", &list_and(&modes))],
                ),
                muted3(),
            ));
        } else if r.report.files() == 0 && r.report.targets.iter().all(|t| t.error.is_none()) {
            lines.push((t("import.done_unchanged"), muted3()));
        }
        let from = source
            .as_ref()
            .map(|s| {
                let launcher = launcher_name(s.launcher);
                if launcher.is_empty() {
                    tf("import.done_from_folder", &[("name", &s.name)])
                } else {
                    tf(
                        "import.done_from",
                        &[("name", &s.name), ("launcher", &launcher)],
                    )
                }
            })
            .unwrap_or_default();
        let head = Self::import_head(&t("import.done_title"), &from);
        let go = play_button(
            "import-continue",
            &t("import.done_continue"),
            SharedString::default(),
            288.,
            64.,
            19.,
            Press::Live,
        )
        .on_click(cx.listener(|this, _, _, cx| this.leave_import(cx)));
        div().size_full().child(head).child(
            div()
                .absolute()
                .left(px(64.))
                .top(px(BODY_TOP - 30.))
                .w(px(760.))
                .flex()
                .flex_col()
                .children(lines.into_iter().map(|(s, color)| {
                    div()
                        .py(px(12.))
                        .border_t_1()
                        .border_color(line3())
                        .flex()
                        .flex_row()
                        .gap(px(14.))
                        .child(div().flex_none().size(px(6.)).mt(px(7.)).bg(color))
                        .child(
                            inter(14., FontWeight::NORMAL, 1.5)
                                .whitespace_nowrap()
                                .text_color(if color == text3() { text3() } else { muted3() })
                                .child(wrap_paths(&s, INTER, FontWeight::NORMAL, 14., 740.)),
                        )
                }))
                .child(
                    div()
                        .mt(px(26.))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(32.))
                        .child(go)
                        .children(backup.map(|b| {
                            tertiary("import-backup", &t("import.open_backup")).on_click(
                                cx.listener(move |this, _, _, _| {
                                    this.backend().open_containing_folder(&b)
                                }),
                            )
                        })),
                ),
        )
    }
}

/// The tick box `i` of the page, in its order: settings, mods, packs, worlds.
fn part(c: &mut Choice, i: usize) -> &mut bool {
    match i {
        0 => &mut c.options,
        1 => &mut c.mod_settings,
        2 => &mut c.resource_packs,
        _ => &mut c.worlds,
    }
}

/// The round radio of a list row (one instance at a time), so it doesn't read as one of the
/// square tick boxes next to it: an `accent-2` dot inside when selected.
fn radio(on: bool) -> Div {
    div()
        .flex_none()
        .size(px(16.))
        .rounded_full()
        .border_1()
        .border_color(if on { accent2() } else { text3a(0.3) })
        .flex()
        .items_center()
        .justify_center()
        .when(on, |d| {
            d.child(div().size(px(8.)).rounded_full().bg(accent2()))
        })
}

/// A square tick box: `accent-2` with an ink square when ticked; faint when it can't change.
pub(crate) fn tick(on: bool, live: bool) -> Div {
    div()
        .flex_none()
        .size(px(18.))
        .flex()
        .items_center()
        .justify_center()
        .when(on, |d| {
            d.bg(accent2())
                .child(spaced("✓", INTER, FontWeight::BOLD, 12., 0., 1., ink()))
        })
        .when(!on, |d| {
            d.border_1()
                .border_color(if live { text3a(0.35) } else { text3a(0.12) })
        })
}

/// "MultiMC · Minecraft 1.16.1 · played 3 days ago".
fn source_line(s: &Source) -> SharedString {
    let mut parts = Vec::new();
    let launcher = launcher_name(s.launcher);
    if !launcher.is_empty() {
        parts.push(launcher.to_string());
    }
    parts.push(match &s.version {
        Some(v) => tf("import.version", &[("v", v)]).to_string(),
        None => t("import.version_unknown").to_string(),
    });
    if let Some(when) = s.last_played {
        parts.push(played(when).to_string());
    }
    parts.join(" · ").into()
}

pub fn launcher_name(l: From) -> String {
    match l {
        From::Folder => String::new(),
        From::Official => t("import.official").to_string(),
        l => l.name().to_string(),
    }
}

/// When an instance was last played: today, yesterday, a few days ago, else the date.
fn played(unix: u64) -> SharedString {
    let ms = (unix as i64).saturating_mul(1000);
    let (today, _) = today_and_month_ms();
    const DAY: i64 = 86_400_000;
    if ms >= today {
        t("import.played_today")
    } else if ms >= today - DAY {
        t("import.played_yesterday")
    } else if ms >= today - 6 * DAY {
        tf(
            "import.played_days",
            &[("n", &((today - ms) / DAY + 1).to_string())],
        )
    } else if Local.timestamp_millis_opt(ms).earliest().is_some() {
        tf("import.played_date", &[("date", &fmt_date(ms))])
    } else {
        SharedString::default()
    }
}

/// The mods whose settings an instance has: four by name, then how many more.
fn mod_names(mods: &[&str]) -> SharedString {
    const SHOWN: usize = 4;
    let names: Vec<String> = mods.iter().map(|m| m.to_string()).collect();
    if names.len() <= SHOWN {
        return list_and(&names).into();
    }
    tf(
        "import.mods_more",
        &[
            ("list", &names[..SHOWN].join(", ")),
            ("n", &(names.len() - SHOWN).to_string()),
        ],
    )
}

/// "Ranked, RSG and FSG" in the current language.
fn list_and(items: &[String]) -> String {
    let and = t("list.and");
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {and} {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_read_naturally() {
        crate::copy::set_lang(crate::backend::Language::En);
        let s = |v: &[&str]| list_and(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(s(&[]), "");
        assert_eq!(s(&["RSG"]), "RSG");
        assert_eq!(
            s(&["Ranked", "RSG", "Practice"]),
            "Ranked, RSG and Practice"
        );
    }
}
