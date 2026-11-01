//! The Instances page's Settings tab: name, memory, tools, Java arguments, Make a copy and
//! Remove.

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
            // The four 1.16.1 modes share one instance, so one memory for all of them, as the
            // Settings page sets it.
            let set_for: Vec<Mode> = if key == InstKey::Shared {
                Mode::SHARED.to_vec()
            } else {
                vec![mode]
            };
            let all_same = set_for
                .iter()
                .all(|m| self.settings.memory_mb(*m) == self.settings.memory_mb(set_for[0]));
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
                    .child(section_head(&t("inst.custom.memory")))
                    .child(div().flex().flex_row().gap(px(6.)).children(same).children(
                        MEMORY_CHOICES_MB.into_iter().map(|mb| {
                            let text = format!("{} GB", mb / 1024);
                            let on =
                                mb == now && all_same && (custom_id.is_none() || own.is_some());
                            let set_for = set_for.clone();
                            if mb < floor {
                                chip_off(&text).into_any_element()
                            } else {
                                chip(("inst-mem", mb as usize), &text, on, false)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let mut s = this.settings.clone();
                                        match custom_id.and_then(|id| s.custom_mut(id)) {
                                            Some(c) => c.memory_mb = Some(mb),
                                            None => {
                                                for m in &set_for {
                                                    s.set_memory_mb(*m, mb);
                                                }
                                            }
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
                            // SeedQueue advice only where SeedQueue is; a mode with a floor
                            // (Draftout) says why the smaller choices are off.
                            .child(if floor > MEMORY_CHOICES_MB[0] {
                                tf(
                                    "inst.memory_hint_floor",
                                    &[("mode", short), ("n", &floor.div_ceil(1024).to_string())],
                                )
                            } else {
                                t("inst.memory_hint")
                            }),
                    ),
            );
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    // A custom instance starts the tools of the mode it plays like.
                    .child(section_head(&match custom_id {
                        Some(_) => tf("inst.tools_like", &[("mode", short)]),
                        None => t("inst.tools"),
                    }))
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
            let below = self.instances.jvm_problem.clone().map(|p| {
                inter(12., FontWeight::MEDIUM, 1.3)
                    .text_color(behind3())
                    .child(p)
            });
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(section_head(&t("inst.custom.jvm")))
                    .child(field)
                    .children(below),
            );
        }
        if key != InstKey::Practice {
            col = col.child(self.copy_section(key, cx));
        }
        if let Some(c) = custom {
            let id = c.id;
            let body = if self.instances.confirm == Some(Confirm::Instance) {
                self.ask(
                    tf("inst.custom.delete_ask", &[("name", &c.name)]),
                    &t("inst.custom.delete_yes"),
                    false,
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
            col = col.child(div().flex().flex_col().gap(px(10.)).child(body));
        }
        // Fades at the edges while more is hidden, as the tabs' lists do.
        div()
            .flex_1()
            .min_h(px(0.))
            .relative()
            .child(
                div()
                    .id("inst-settings")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.instances.scroll)
                    .child(col.pb(px(24.))),
            )
            .child(fade_above(self.instances.scroll.clone(), 24.))
            .child(fade_below(self.instances.scroll.clone(), 36.))
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
                    self.tick_line(
                        "inst-copy-worlds",
                        &if saves > 0 {
                            tf("inst.copy.worlds_size", &[("size", &size(saves))])
                        } else {
                            t("inst.copy.worlds")
                        },
                        worlds,
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
            // The shared instance's copy plays as the mode picked on Play, so it says which; any
            // other copy is simply the same instance again.
            .children(
                self.inst_mode(key)
                    .filter(|_| key == InstKey::Shared)
                    .map(|m| {
                        inter(12., FontWeight::NORMAL, 1.4)
                            .text_color(faint3())
                            .child(tf(
                                "inst.copy.hint_mode",
                                &[("mode", catalog::def(m).short)],
                            ))
                    }),
            )
            .child(body)
    }
}
