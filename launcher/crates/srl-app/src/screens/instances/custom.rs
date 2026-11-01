//! Custom instances on the Instances page: create, copy, import, rename, remove, their Play
//! button and the New instance form.

use super::*;

impl Launcher {
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
                // Under the form's name field, or as a note when the form was closed meanwhile.
                Err(why) => {
                    let text = if why == COPY_CANCELLED {
                        t("inst.copy.cancelled")
                    } else {
                        tf("inst.copy.failed", &[("why", &why)])
                    };
                    if self.instances.creating.is_some() {
                        self.instances.name_problem = Some(text);
                    } else {
                        self.instances.note = Some((text, why != COPY_CANCELLED));
                    }
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
        if self.instances.submitting || self.instances.importing {
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
        // The form's own mode, not the one picked on Home: the shared instance's per-mode files
        // (Atum's seed settings) must come from the mode the new instance plays like.
        self.backend().create_instance(NewInstance {
            name,
            base: form.base,
            copy_settings_from: form.copy.then_some(Instance::Mode(form.base)),
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
            // Esc while bringing an instance over stops that; the form stays for its answer.
            Field::NewInstance if self.instances.importing => self.backend().cancel_copy(),
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
    pub(super) fn custom_play(&self, id: CustomId, cx: &mut Context<Self>) -> Div {
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
                progress = Some(div().absolute().left_0().right_0().top(px(h + 7.)).child(
                    thin_progress(
                        self.eased_bar(
                            format!("custom-{}", id.0),
                            crate::screens::play::bar_fraction(p),
                        ),
                        3.,
                    ),
                ));
                let label = if matches!(p.stage, Stage::Launching | Stage::SigningIn) {
                    t("v3.starting")
                } else {
                    t("v3.installing")
                };
                (label, t(crate::screens::play::stage_key(p.stage)), false)
            }
            ModeState::Ready | ModeState::UpdateAvailable => {
                (t("v3.play"), SharedString::default(), true)
            }
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
                        &crate::screens::play::fail_detail(&f.detail),
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
    pub(super) fn new_instance_pane(&mut self, window: &Window, cx: &mut Context<Self>) -> Div {
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
                    if this.instances.importing {
                        return;
                    }
                    if let Some(f) = &mut this.instances.creating {
                        // The picked instance can't run the other version: back to the start
                        // the form opens with.
                        if f.source.is_some() && f.base.minecraft_version() != m.minecraft_version()
                        {
                            f.source = None;
                            f.copy = true;
                            f.worlds = false;
                        }
                        f.base = m;
                    }
                    cx.notify();
                }))
            }));
        // Where the new instance's keys and options come from, as one choice: nothing (the
        // game's defaults), the launcher's instance of the same Minecraft, or another launcher's.
        let from = Self::settings_source(form.base);
        let from_name = self.inst_name(from);
        let pick = |f: &mut NewForm, copy: bool| {
            f.copy = copy;
            f.source = None;
        };
        let blank = chip(
            "inst-new-blank",
            &t("inst.new.blank"),
            !form.copy && form.source.is_none(),
            false,
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            if !this.instances.importing
                && let Some(f) = &mut this.instances.creating
            {
                pick(f, false);
            }
            cx.notify();
        }));
        let copy = chip(
            "inst-new-copy",
            &from_name,
            form.copy && form.source.is_none(),
            false,
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            if !this.instances.importing
                && let Some(f) = &mut this.instances.creating
            {
                pick(f, true);
            }
            cx.notify();
        }));
        let (sources, looking) = self.import_choices(&form, cx);
        let hint = match &form.source {
            // "“RSG” (MultiMC)": the other launcher's instance name alone could read as a mode.
            Some((dir, name)) => {
                let quoted = tf("quoted", &[("text", name)]);
                let launcher = self
                    .import
                    .found
                    .iter()
                    .find(|s| s.game_dir == *dir)
                    .map(|s| crate::screens::import::launcher_name(s.launcher))
                    .unwrap_or_default();
                let from = if launcher.is_empty() {
                    quoted.to_string()
                } else {
                    format!("{quoted} ({launcher})")
                };
                tf("inst.new.start_import_hint", &[("from", &from)])
            }
            None if form.copy => tf("inst.new.start_copy_hint", &[("from", &from_name)]),
            None => SharedString::default(),
        };
        let worlds = form.source.is_some().then(|| {
            self.tick_line("inst-new-worlds", &t("inst.copy.worlds"), form.worlds)
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.instances.importing {
                        return;
                    }
                    if let Some(f) = &mut this.instances.creating {
                        f.worlds = !f.worlds;
                    }
                    cx.notify();
                }))
        });
        let start = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(6.))
                    .child(blank)
                    .child(copy)
                    .children(sources),
            )
            .child(
                inter(12., FontWeight::NORMAL, 1.4)
                    .text_color(faint3())
                    .child(hint),
            )
            .children(looking)
            .children(worlds.map(|w| div().pt(px(4.)).flex().flex_row().child(w)));
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
                div().flex().flex_col().child(
                    tight(
                        &t("inst.new.title"),
                        48.,
                        FontWeight::EXTRA_BOLD,
                        -0.03,
                        0.95,
                        text3(),
                    )
                    // Level with the other instances' titles, which have an eyebrow above.
                    .mt(px(10. + 13.)),
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
            .child(section(t("inst.new.start"), start))
            .child(self.new_instance_actions(cx))
    }

    /// The other launchers' instances on the form's Minecraft version, as choices for "Start
    /// with", and a line while they are still being looked for.
    pub(super) fn import_choices(
        &self,
        form: &NewForm,
        cx: &mut Context<Self>,
    ) -> (Vec<gpui::Stateful<Div>>, Option<Div>) {
        let mc = form.base.minecraft_version();
        let looking = (!self.import.scanned).then(|| {
            inter(12., FontWeight::NORMAL, 1.4)
                .text_color(faint3())
                .child(t("inst.new.looking"))
        });
        let chips = self
            .import
            .found
            .iter()
            .filter(|s| s.runs(mc))
            .take(4)
            .enumerate()
            .map(|(i, s)| {
                let on = form.source.as_ref().is_some_and(|(d, _)| *d == s.game_dir);
                let launcher = crate::screens::import::launcher_name(s.launcher);
                let text = if launcher.is_empty() {
                    s.name.clone()
                } else {
                    format!("{} · {launcher}", s.name)
                };
                let pick = (s.game_dir.clone(), s.name.clone());
                chip(("inst-new-src", i), &text, on, false).on_click(cx.listener(
                    move |this, _, _, cx| {
                        if this.instances.importing {
                            return;
                        }
                        if let Some(f) = &mut this.instances.creating {
                            f.source = Some(pick.clone());
                        }
                        cx.notify();
                    },
                ))
            })
            .collect();
        (chips, looking)
    }

    /// Create and Cancel, or the progress of bringing an instance over.
    pub(super) fn new_instance_actions(&self, cx: &mut Context<Self>) -> Div {
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
    pub(super) fn settings_source(base: Mode) -> InstKey {
        if base.is_shared() {
            InstKey::Shared
        } else {
            InstKey::Draftout
        }
    }
}

pub(super) fn name_problem(p: NameProblem) -> SharedString {
    t(match p {
        NameProblem::Empty => "inst.name_problem.empty",
        NameProblem::TooLong => "inst.name_problem.too_long",
        NameProblem::Control => "inst.name_problem.control",
        NameProblem::Taken => "inst.name_problem.taken",
    })
}

pub(super) fn jvm_problem(p: &JvmArgProblem) -> SharedString {
    match p {
        JvmArgProblem::Empty => t("inst.jvm_problem.empty"),
        JvmArgProblem::NotAnOption(a) => tf("inst.jvm_problem.not_option", &[("arg", a)]),
        JvmArgProblem::Memory(a) => tf("inst.jvm_problem.memory", &[("arg", a)]),
        JvmArgProblem::Command(a) => tf("inst.jvm_problem.command", &[("arg", a)]),
        JvmArgProblem::Agent(a) => tf("inst.jvm_problem.agent", &[("arg", a)]),
        JvmArgProblem::Loader(a) => tf("inst.jvm_problem.loader", &[("arg", a)]),
    }
}
