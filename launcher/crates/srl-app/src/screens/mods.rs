//! Mods: turn an instance's mods on and off, the way MultiMC does, and add jars of your own. The
//! Mods tab of the Instances page. Ranked, RSG, FSG and SSG play in one instance
//! (`srl_core::shared`), so a mode there is a set of mods, changed in `Settings::mod_choices`; a
//! custom instance changes its base mode's set in its own `CustomInstance::mods`. Changes apply on
//! the next Play. A set that makes the run not legal for the mode says so, and the game still
//! starts.
use std::path::PathBuf;

use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, PathPromptOptions,
    SharedString, Styled, div, prelude::*, px,
};
use srl_core::{Instance, Mode};
use srl_mods::{Available, Concern, CustomJar};

use crate::{
    app::Launcher,
    catalog,
    copy::{fill, t, t_opt, tf},
    v3::*,
};

const ROW_H: f32 = 60.;

/// The Mods page's own state.
#[derive(Default)]
pub struct ModsView {
    pub scroll: gpui::ScrollHandle,
    /// Why the last jar could not be added.
    pub note: Option<String>,
}

impl Launcher {
    /// The Instances page on the selected mode's Mods tab (Play's "fix the mods" link).
    pub fn open_mods(&mut self, cx: &mut Context<Self>) {
        self.mods.note = None;
        self.open_instances(Some(super::instances::Tab::Mods), cx);
    }

    fn toggle_mod(&mut self, target: Instance, row: &Available, cx: &mut Context<Self>) {
        let mut settings = self.settings.clone();
        match target {
            Instance::Mode(mode) => settings.set_mod(mode, &row.modid, !row.on, row.in_mode),
            Instance::Custom(id) => settings.set_custom_mod(id, &row.modid, !row.on, row.in_mode),
            Instance::Practice => return,
        }
        self.save_settings(settings, cx);
    }

    fn reset_mods(&mut self, target: Instance, cx: &mut Context<Self>) {
        let mut settings = self.settings.clone();
        match target {
            Instance::Mode(mode) => settings.reset_mods(mode),
            Instance::Custom(id) => {
                if let Some(c) = settings.custom_mut(id) {
                    c.mods = Default::default();
                }
            }
            Instance::Practice => return,
        }
        self.save_settings(settings, cx);
    }

    fn mods_changed(&self, target: Instance) -> bool {
        match target {
            Instance::Mode(mode) => self.settings.mods_changed(mode),
            Instance::Custom(id) => self.settings.custom(id).is_some_and(|c| !c.mods.is_empty()),
            Instance::Practice => false,
        }
    }

    fn add_jar_to(&self, target: Instance, file: PathBuf) {
        match target {
            Instance::Mode(mode) => self.backend().add_jar(mode, file),
            Instance::Custom(id) => self.backend().add_instance_jar(id, file),
            Instance::Practice => {}
        }
    }

    /// The system's file picker for a `.jar` (or `SRL_MODS_PICK`, so tests open no window).
    fn pick_jar(&mut self, target: Instance, cx: &mut Context<Self>) {
        self.mods.note = None;
        if let Some(file) = std::env::var_os("SRL_MODS_PICK").filter(|v| !v.is_empty()) {
            self.add_jar_to(target, PathBuf::from(file));
            return;
        }
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t("mods.pick_ok")),
        });
        cx.spawn(async move |this, cx| {
            let answer = picked.await;
            let _ = this.update(cx, |app, cx| {
                match answer {
                    Ok(Ok(Some(mut files))) if !files.is_empty() => {
                        app.add_jar_to(target, files.swap_remove(0))
                    }
                    Ok(Err(e)) => app.mods.note = Some(e.to_string()),
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// `Event::JarAdded`: the jar is in the store, so it goes into the library and on in the
    /// mode.
    pub fn jar_added(
        &mut self,
        target: Instance,
        answer: Result<CustomJar, String>,
        cx: &mut Context<Self>,
    ) {
        match answer {
            Ok(jar) => {
                self.mods.note = None;
                let mut settings = self.settings.clone();
                match target {
                    Instance::Mode(mode) => settings.add_custom_jar(mode, jar),
                    Instance::Custom(id) => settings.add_custom_jar_to(id, jar),
                    Instance::Practice => return,
                }
                self.save_settings(settings, cx);
            }
            Err(e) => self.mods.note = Some(e),
        }
        cx.notify();
    }

    /// The Instances page's Mods tab for `mode`: whether the set is legal, adding a jar and going
    /// back to the mode's set, then every mod the player can turn on or off.
    pub fn inst_mods(&mut self, target: Instance, cx: &mut Context<Self>) -> AnyElement {
        let mode = self.settings.rules_mode(target);
        let mi = crate::backend::index(mode);
        let short = catalog::MODES[mi].short;
        let empty = crate::backend::ModeInfo::default();
        let info = match target {
            Instance::Custom(id) => self.customs.get(&id).map(|s| &s.info).unwrap_or(&empty),
            _ => &self.modes[mi].info,
        };
        let rows: Vec<Available> = info.catalogue.clone().unwrap_or_default();
        let concerns = info.concerns.clone();
        let known = info.catalogue.is_some();
        let changed = self.mods_changed(target);
        let on = rows.iter().filter(|r| r.on).count();

        let mut actions = div().flex().flex_row().items_center().gap(px(20.));
        if changed {
            actions = actions.child(
                tertiary("mods-reset", &tf("mods.reset", &[("mode", short)]))
                    .on_click(cx.listener(move |this, _, _, cx| this.reset_mods(target, cx))),
            );
        }
        actions = actions.child(
            secondary("mods-add", &t("mods.add"))
                .on_click(cx.listener(move |this, _, _, cx| this.pick_jar(target, cx))),
        );
        let bar = div()
            .flex_none()
            .h(px(52.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(20.))
            .when(!rows.is_empty(), |d| {
                d.child(label(&tf(
                    "mods.count",
                    &[("on", &on.to_string()), ("all", &rows.len().to_string())],
                )))
            })
            .child(div().flex_1())
            .child(actions);

        let list_w = 1280. - 388. - 48.;
        let items: Vec<AnyElement> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                self.mod_row(i, target, mode, r, list_w, cx)
                    .into_any_element()
            })
            .collect();
        let note = self.mods.note.clone().map(|note| {
            inter(12., FontWeight::NORMAL, 1.4)
                .pb(px(8.))
                .text_color(behind3())
                .child(tf("mods.add_failed", &[("error", &note)]))
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(bar)
            .children(note)
            .child(concern_block(mode, short, &concerns, known, list_w).pb(px(12.)))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .relative()
                    .border_b_1()
                    .border_color(line3())
                    .child(
                        div()
                            .id("mods-list")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.mods.scroll)
                            .children(items),
                    )
                    // While rows are hidden below, they fade out at the bottom edge instead of
                    // being cut by it (as on Learn).
                    .child(fade_above(self.mods.scroll.clone(), 28.))
                    .child(fade_below(self.mods.scroll.clone(), 40.)),
            )
            .into_any_element()
    }

    fn mod_row(
        &self,
        i: usize,
        target: Instance,
        mode: Mode,
        r: &Available,
        w: f32,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let mut tags: Vec<(SharedString, gpui::Hsla)> = Vec::new();
        if r.custom {
            tags.push((t("mods.tag.custom"), gold()));
        } else if r.not_legal {
            tags.push((t("mods.tag.not_legal"), gold()));
        }
        if let Some(only) = &r.only {
            let other = only != mode_category(mode);
            tags.push((
                tf("mods.tag.only", &[("only", only)]),
                if other { gold() } else { faint3() },
            ));
        }
        match (r.in_mode, r.on) {
            (true, false) => tags.push((t("mods.tag.removed"), behind3())),
            (false, true) if !r.custom && r.needed_by.is_none() => {
                tags.push((t("mods.tag.added"), accent2()))
            }
            _ => {}
        }
        if let Some(by) = &r.needed_by {
            tags.push((tf("mods.tag.needed", &[("name", by)]), faint3()));
        }
        // Our own one-liner when copy has one (`mods.desc.<modid>`, in both languages); the
        // upstream description (English, of every length and tone) only for the rest.
        let description = t_opt(&format!("mods.desc.{}", r.modid))
            .map(|d| d.to_string())
            .unwrap_or_else(|| r.description.clone());
        let desc = if description.is_empty() {
            None
        } else {
            Some(ellipsize(
                &description,
                INTER,
                FontWeight::NORMAL,
                12.,
                w - 60.,
                Cut::End,
            ))
        };
        let row = r.clone();
        div()
            .id(("mods-row", i))
            .h(px(ROW_H))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(16.))
            .px(px(4.))
            .border_t_1()
            .border_color(line3())
            .cursor_pointer()
            .hover(|s| s.bg(text3a(0.04)))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_mod(target, &row, cx)))
            .child(super::import::tick(r.on, true))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .gap(px(12.))
                            .child(
                                inter(15., FontWeight::SEMIBOLD, 1.2)
                                    .flex_none()
                                    .text_color(if r.on { text3() } else { muted3() })
                                    .child(r.name.clone()),
                            )
                            .children(tags.into_iter().map(|(tag, color)| {
                                inter(12., FontWeight::MEDIUM, 1.)
                                    .flex_none()
                                    .text_color(color)
                                    .child(tag)
                            })),
                    )
                    .children(desc.map(|d| {
                        inter(12., FontWeight::NORMAL, 1.3)
                            .whitespace_nowrap()
                            .text_color(faint3())
                            .child(d)
                    })),
            )
    }
}

/// `RSG` or `SSG`: the category a mode's runs count in (Ranked and FSG play random seeds).
fn mode_category(mode: Mode) -> &'static str {
    if mode == Mode::Ssg { "SSG" } else { "RSG" }
}

/// Whether the set is legal for the mode, and if not, why, one line per mod.
fn concern_block(mode: Mode, short: &str, concerns: &[Concern], known: bool, w: f32) -> Div {
    let block = div().flex().flex_col().gap(px(6.));
    if !known {
        return block.child(
            inter(13., FontWeight::MEDIUM, 1.3)
                .text_color(faint3())
                .child(t("mods.loading")),
        );
    }
    if concerns.is_empty() {
        return block.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(div().size(px(6.)).bg(ok3()))
                .child(
                    inter(13., FontWeight::MEDIUM, 1.3)
                        .text_color(ok3())
                        .child(tf("mods.legal", &[("mode", short)])),
                ),
        );
    }
    let _ = mode;
    let mut block = block.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(div().size(px(6.)).bg(gold()))
            .child(
                inter(13., FontWeight::MEDIUM, 1.3)
                    .text_color(gold())
                    .child(tf("mods.not_legal", &[("mode", short)])),
            ),
    );
    for c in concerns.iter().take(6) {
        block = block.child(
            inter(12., FontWeight::NORMAL, 1.4)
                .pl(px(14.))
                .whitespace_nowrap()
                .text_color(muted3())
                .child(wrap(
                    &concern_text(c, short),
                    INTER,
                    FontWeight::NORMAL,
                    12.,
                    w - 14.,
                )),
        );
    }
    if concerns.len() > 6 {
        block = block.child(
            inter(12., FontWeight::NORMAL, 1.4)
                .pl(px(14.))
                .text_color(faint3())
                .child(tf("mods.more", &[("n", &(concerns.len() - 6).to_string())])),
        );
    }
    block.child(
        inter(12., FontWeight::NORMAL, 1.4)
            .mt(px(4.))
            .text_color(faint3())
            .child(t("mods.still_starts")),
    )
}

/// One concern, in the player's language.
pub fn concern_text(c: &Concern, mode: &str) -> String {
    let (key, name, only) = match c {
        Concern::OtherCategory { name, only } => ("mods.concern.other", name, only.as_str()),
        Concern::NotLegal { name } => ("mods.concern.not_legal", name, ""),
        Concern::Unknown { name } => ("mods.concern.unknown", name, ""),
        Concern::MissingOwn { name } => ("mods.concern.missing", name, ""),
        Concern::RefusedByMode { name } => ("mods.concern.refused", name, ""),
    };
    fill(&t(key), &[("name", name), ("only", only), ("mode", mode)]).to_string()
}

/// The short line under Play when the set isn't legal for the mode: "Not legal for RSG:
/// SetSpawn".
pub fn concern_line(concerns: &[Concern], mode: &str) -> Option<SharedString> {
    let names: Vec<&str> = concerns
        .iter()
        .map(|c| match c {
            Concern::OtherCategory { name, .. }
            | Concern::NotLegal { name }
            | Concern::Unknown { name }
            | Concern::MissingOwn { name }
            | Concern::RefusedByMode { name } => name.as_str(),
        })
        .collect();
    if names.is_empty() {
        return None;
    }
    let list = if names.len() > 3 {
        format!("{} +{}", names[..2].join(", "), names.len() - 2)
    } else {
        names.join(", ")
    };
    Some(tf("v3.concern", &[("mode", mode), ("names", &list)]))
}
