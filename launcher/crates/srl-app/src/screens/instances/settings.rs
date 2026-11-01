//! The Instances page's Settings tab: name, memory, tools, Java arguments, Make a copy, folder
//! and Remove.

use super::*;

impl Launcher {
    pub(super) fn inst_settings(
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
    pub(super) fn copy_section(&self, key: InstKey, cx: &mut Context<Self>) -> Div {
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
}
