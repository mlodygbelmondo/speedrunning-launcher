//! The Instances page's content tabs: worlds, resource packs, config files, logs and
//! screenshots, and the rows they list.

use super::*;

impl Launcher {
    pub(super) fn worlds_tab(
        &self,
        c: &Content,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> (Div, Vec<AnyElement>) {
        let resets: Vec<_> = c.reset_worlds().collect();
        let reset_bytes: u64 = resets.iter().map(|w| w.size).sum();
        let bar = match &self.instances.confirm {
            Some(Confirm::Cleanup) => self.ask(
                tf(
                    "inst.worlds.cleanup_ask",
                    &[
                        ("n", &tn("inst.worlds.resets", resets.len() as u64)),
                        ("size", &size(reset_bytes)),
                    ],
                ),
                &tf(
                    "inst.worlds.cleanup_yes",
                    &[("n", &tn("inst.worlds.resets", resets.len() as u64))],
                ),
                true,
                |this, cx| this.cleanup_worlds(cx),
                cx,
            ),
            Some(Confirm::World(dir)) => {
                let dir = dir.clone();
                let name = c
                    .worlds
                    .iter()
                    .find(|w| w.dir == dir)
                    .map(|w| w.name.clone())
                    .unwrap_or_else(|| dir.clone());
                self.ask(
                    tf("inst.worlds.trash_ask", &[("name", &name)]),
                    &t("inst.trash_yes"),
                    false,
                    move |this, cx| this.trash_item("saves", dir.clone(), cx),
                    cx,
                )
            }
            _ => {
                let mut actions: Vec<AnyElement> = Vec::new();
                if !resets.is_empty() {
                    let label_text = tf(
                        "inst.worlds.cleanup",
                        &[("n", &tn("inst.worlds.resets", resets.len() as u64))],
                    );
                    actions.push(if busy {
                        secondary_off(&label_text).into_any_element()
                    } else {
                        secondary("inst-cleanup", &label_text)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.instances.confirm = Some(Confirm::Cleanup);
                                this.instances.note = None;
                                cx.notify();
                            }))
                            .into_any_element()
                    });
                }
                let saves = c.game_dir.join("saves");
                actions.push(
                    tertiary("inst-saves", &tf("inst.open_named", &[("folder", "saves")]))
                        .on_click(cx.listener(move |_, _, _, _| open_folder(&saves)))
                        .into_any_element(),
                );
                self.bar(
                    if c.worlds.is_empty() {
                        t("inst.worlds.none")
                    } else {
                        tf(
                            "inst.worlds.summary",
                            &[
                                ("n", &tn("inst.worlds.count", c.worlds.len() as u64)),
                                ("size", &size(c.saves_size)),
                            ],
                        )
                    },
                    actions,
                )
            }
        };
        let saves = c.game_dir.join("saves");
        let rows = c
            .worlds
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let mut tags: Vec<(SharedString, gpui::Hsla)> = Vec::new();
                if let Some(m) = w.mode {
                    tags.push((catalog::MODES[index(m)].short.into(), accent2()));
                }
                if w.reset {
                    tags.push((t("inst.worlds.reset"), faint3()));
                }
                let sub = match (w.seed, w.name != w.dir) {
                    (Some(seed), true) => Some(format!(
                        "{}  ·  {}",
                        tf("inst.worlds.seed", &[("seed", &seed.to_string())]),
                        w.dir
                    )),
                    (Some(seed), false) => {
                        Some(tf("inst.worlds.seed", &[("seed", &seed.to_string())]).to_string())
                    }
                    (None, true) => Some(w.dir.clone()),
                    (None, false) => None,
                };
                let seed = w.seed;
                let open = saves.join(&w.dir);
                let dir = w.dir.clone();
                let mut actions: Vec<AnyElement> = Vec::new();
                if let Some(seed) = seed {
                    let copied = self.instances.copied == Some(seed);
                    actions.push(
                        tertiary_in(
                            ("inst-world-seed", i),
                            &t(if copied {
                                "inst.worlds.copied"
                            } else {
                                "inst.worlds.copy_seed"
                            }),
                            if copied { ok3() } else { muted3() },
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                seed.to_string(),
                            ));
                            this.instances.copied = Some(seed);
                            cx.notify();
                        }))
                        .into_any_element(),
                    );
                }
                actions.extend([tertiary(("inst-world-open", i), &t("inst.show_folder"))
                    .on_click(cx.listener(move |_, _, _, _| open_folder(&open)))
                    .into_any_element()]);
                if !busy {
                    actions.push(
                        tertiary(("inst-world-trash", i), &t("inst.trash"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.instances.confirm = Some(Confirm::World(dir.clone()));
                                this.instances.note = None;
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
                row(
                    ("inst-world", i),
                    &w.name,
                    sub,
                    tags,
                    w.played_ms,
                    w.size,
                    actions,
                )
                .into_any_element()
            })
            .collect();
        (bar, rows)
    }

    pub(super) fn packs_tab(
        &self,
        c: &Content,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> (Div, Vec<AnyElement>) {
        let bar = match &self.instances.confirm {
            Some(Confirm::Pack(rel)) => {
                let rel = rel.clone();
                self.ask(
                    tf("inst.packs.trash_ask", &[("name", &rel)]),
                    &t("inst.trash_yes"),
                    false,
                    move |this, cx| this.trash_item("resourcepacks", rel.clone(), cx),
                    cx,
                )
            }
            _ => {
                let on = c.packs.iter().filter(|p| p.on).count();
                let dir = c.game_dir.join("resourcepacks");
                self.bar(
                    if c.packs.is_empty() {
                        t("inst.packs.none")
                    } else {
                        tf(
                            "inst.packs.summary",
                            &[
                                ("n", &tn("inst.packs.count", c.packs.len() as u64)),
                                ("on", &on.to_string()),
                            ],
                        )
                    },
                    vec![
                        if busy {
                            secondary_off(&t("inst.packs.add")).into_any_element()
                        } else {
                            secondary("inst-pack-add", &t("inst.packs.add"))
                                .on_click(cx.listener(|this, _, _, cx| this.pick_pack(cx)))
                                .into_any_element()
                        },
                        tertiary(
                            "inst-packs-folder",
                            &tf("inst.open_named", &[("folder", "resourcepacks")]),
                        )
                        .on_click(cx.listener(move |_, _, _, _| open_folder(&dir)))
                        .into_any_element(),
                    ],
                )
            }
        };
        let mut rows: Vec<AnyElement> =
            c.packs
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let rel = p.item.rel.clone();
                    let rel2 = rel.clone();
                    let on = p.on;
                    let tick = div()
                        .id(("inst-pack-tick", i))
                        .flex_none()
                        .when(!busy, |d| {
                            d.cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.toggle_pack(rel.clone(), !on, cx)
                                }))
                        })
                        .child(crate::screens::import::tick(on, !busy));
                    let mut actions: Vec<AnyElement> = Vec::new();
                    if !busy {
                        actions.push(
                            tertiary(("inst-pack-trash", i), &t("inst.trash"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.instances.confirm = Some(Confirm::Pack(rel2.clone()));
                                    this.instances.note = None;
                                    cx.notify();
                                }))
                                .into_any_element(),
                        );
                    }
                    // The tick box already says whether it's on.
                    let tags = Vec::new();
                    row(
                        ("inst-pack", i),
                        p.item.rel.trim_end_matches(".zip"),
                        None,
                        tags,
                        p.item.modified_ms,
                        p.item.size,
                        actions,
                    )
                    .child(div().absolute().left(px(4.)).top(px(17.)).child(tick))
                    .pl(px(34.))
                    .into_any_element()
                })
                .collect();
        if busy && !c.packs.is_empty() {
            rows.insert(
                0,
                inter(12., FontWeight::NORMAL, 1.4)
                    .pb(px(8.))
                    .text_color(faint3())
                    .child(t("inst.wait_game"))
                    .into_any_element(),
            );
        }
        (bar, rows)
    }

    /// A list of files that open in the system's app for them (config, logs).
    pub(super) fn files_tab(
        &self,
        c: &Content,
        items: &[content::Item],
        folder: &'static str,
        copy: &str,
        cx: &mut Context<Self>,
    ) -> (Div, Vec<AnyElement>) {
        let dir = c.game_dir.join(folder);
        let bar = self.bar(
            if items.is_empty() {
                t(&format!("{copy}.none"))
            } else {
                tn(&format!("{copy}.count"), items.len() as u64)
            },
            vec![
                tertiary(
                    SharedString::from(format!("inst-{folder}-folder")),
                    &tf("inst.open_named", &[("folder", folder)]),
                )
                .on_click(cx.listener(move |_, _, _, _| open_folder(&dir)))
                .into_any_element(),
            ],
        );
        let mut rows = Vec::new();
        for (i, it) in items.iter().enumerate() {
            let shown = self
                .instances
                .viewing
                .as_ref()
                .filter(|(rel, _)| *rel == it.rel);
            rows.push(file_row(&c.game_dir, folder, i, it, shown.is_some(), cx).into_any_element());
            if let Some((_, lines)) = shown {
                rows.push(file_view(lines).into_any_element());
            }
        }
        (bar, rows)
    }

    pub(super) fn logs_tab(&self, c: &Content, cx: &mut Context<Self>) -> (Div, Vec<AnyElement>) {
        let (bar, mut rows) = self.files_tab(c, &c.logs, "logs", "inst.logs", cx);
        if !self.instances.tail.is_empty() {
            let w = PANE_W - 32.;
            let lines = self.instances.tail.iter().map(|l| {
                plex(11., FontWeight::NORMAL)
                    .whitespace_nowrap()
                    .text_color(muted3())
                    .child(ellipsize(l, PLEX, FontWeight::NORMAL, 11., w, Cut::End))
            });
            rows.insert(
                0,
                div()
                    .mb(px(12.))
                    .p(px(16.))
                    .bg(bg3a(0.7))
                    .border_1()
                    .border_color(line3())
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(label(&t("inst.logs.latest")).pb(px(6.)))
                    .children(lines)
                    .into_any_element(),
            );
        }
        (bar, rows)
    }

    pub(super) fn shots_tab(&self, c: &Content, cx: &mut Context<Self>) -> (Div, Vec<AnyElement>) {
        let dir = c.game_dir.join("screenshots");
        let bar = self.bar(
            if c.screenshots.is_empty() {
                t("inst.screenshots.none")
            } else {
                tn("inst.screenshots.count", c.screenshots.len() as u64)
            },
            vec![
                tertiary(
                    "inst-shots-folder",
                    &tf("inst.open_named", &[("folder", "screenshots")]),
                )
                .on_click(cx.listener(move |_, _, _, _| open_folder(&dir)))
                .into_any_element(),
            ],
        );
        // Four across; only the newest 24 are drawn (each one is decoded into the atlas).
        let (cols, gap) = (4usize, 12.);
        let w = (PANE_W - gap * (cols as f32 - 1.)) / cols as f32;
        let h = (w * 9. / 16.).round();
        let tiles: Vec<AnyElement> = c
            .screenshots
            .iter()
            .take(24)
            .enumerate()
            .map(|(i, s)| {
                let path = c.game_dir.join("screenshots").join(&s.rel);
                let open = path.clone();
                div()
                    .id(("inst-shot", i))
                    .group("inst-shot")
                    .relative()
                    .w(px(w))
                    .h(px(h))
                    .bg(tile_bg())
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _, _, _| open_path(&open)))
                    .child(
                        img(path)
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(shade_up(0.85, 0.0, 0.45))
                    .child(
                        plex(11., FontWeight::MEDIUM)
                            .absolute()
                            .left(px(10.))
                            .bottom(px(8.))
                            .text_color(text3())
                            .child(s.modified_ms.map(fmt_date).unwrap_or_default()),
                    )
                    .child(hover_ring("inst-shot"))
                    .into_any_element()
            })
            .collect();
        let grid = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(gap))
            .children(tiles)
            .into_any_element();
        (bar, vec![grid])
    }
}

/// A row of a list: a name with tags, an optional second line, the date and size on the right,
/// then the row's actions.
pub(super) fn row(
    id: impl Into<gpui::ElementId>,
    name: &str,
    sub: Option<String>,
    tags: Vec<(SharedString, gpui::Hsla)>,
    when: Option<i64>,
    bytes: u64,
    actions: Vec<AnyElement>,
) -> gpui::Stateful<Div> {
    let name_w = PANE_W - 510.;
    div()
        .id(id.into())
        .relative()
        .h(px(ROW_H))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.))
        .px(px(4.))
        .border_t_1()
        .border_color(line3())
        .hover(|s| s.bg(text3a(0.04)))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_baseline()
                        .gap(px(10.))
                        .child(
                            inter(14., FontWeight::SEMIBOLD, 1.2)
                                .flex_none()
                                .whitespace_nowrap()
                                .text_color(text3())
                                .child(ellipsize(
                                    name,
                                    INTER,
                                    FontWeight::SEMIBOLD,
                                    14.,
                                    name_w,
                                    Cut::End,
                                )),
                        )
                        .children(tags.into_iter().map(|(tag, color)| {
                            inter(11., FontWeight::SEMIBOLD, 1.)
                                .flex_none()
                                .text_color(color)
                                .child(tag)
                        })),
                )
                .children(sub.map(|s| {
                    plex(11., FontWeight::NORMAL)
                        .whitespace_nowrap()
                        .text_color(faint3())
                        .child(ellipsize(
                            &s,
                            PLEX,
                            FontWeight::NORMAL,
                            11.,
                            name_w,
                            Cut::End,
                        ))
                })),
        )
        .child(
            plex(11., FontWeight::MEDIUM)
                .flex_none()
                .w(px(80.))
                .text_color(faint3())
                .child(when.map(fmt_when).unwrap_or_default()),
        )
        .child(
            plex(11., FontWeight::MEDIUM)
                .flex_none()
                .w(px(70.))
                .flex()
                .justify_end()
                .text_color(muted3())
                .child(if bytes > 0 {
                    size(bytes)
                } else {
                    String::new()
                }),
        )
        .child(
            div()
                .flex_none()
                .w(px(220.))
                .flex()
                .flex_row()
                .justify_end()
                .gap(px(16.))
                .children(actions),
        )
}

/// How many lines, and characters per line, the config viewer shows.
pub(super) const VIEW_LINES: usize = 400;
pub(super) const VIEW_WIDTH: usize = 110;

pub(super) fn file_row(
    game_dir: &Path,
    folder: &'static str,
    i: usize,
    it: &content::Item,
    shown: bool,
    cx: &mut Context<Launcher>,
) -> gpui::Stateful<Div> {
    let path = game_dir.join(&it.rel);
    let open = path.clone();
    let text = content::is_text_file(&it.rel);
    let rel = it.rel.clone();
    // A text file opens in the page (and closes again); anything else in the system's program.
    let toggle = move |this: &mut Launcher, cx: &mut Context<Launcher>| {
        let v = &mut this.instances;
        if v.viewing.as_ref().is_some_and(|(r, _)| *r == rel) {
            v.viewing = None;
        } else {
            let lines = content::preview(&path, VIEW_LINES, VIEW_WIDTH)
                .map(|l| l.into_iter().map(SharedString::from).collect())
                .map_err(|e| SharedString::from(format!("{e:#}")));
            v.viewing = Some((rel.clone(), lines));
        }
        cx.notify();
    };
    let mut actions = Vec::new();
    if text {
        let toggle = toggle.clone();
        actions.push(
            tertiary(
                SharedString::from(format!("inst-{folder}-view-{i}")),
                &t(if shown { "inst.view_hide" } else { "inst.view" }),
            )
            .on_click(cx.listener(move |this, _, _, cx| toggle(this, cx)))
            .into_any_element(),
        );
    }
    actions.push(
        tertiary(
            SharedString::from(format!("inst-{folder}-open-{i}")),
            // A config file opens in the system's editor; a log or anything else just opens.
            &t(if text && folder == "config" {
                "inst.edit"
            } else {
                "inst.open"
            }),
        )
        .on_click(cx.listener(move |_, _, _, _| open_path(&open)))
        .into_any_element(),
    );
    let target = game_dir.join(&it.rel);
    row(
        SharedString::from(format!("inst-{folder}-{i}")),
        &it.rel,
        None,
        Vec::new(),
        it.modified_ms,
        it.size,
        actions,
    )
    .cursor_pointer()
    .on_click(cx.listener(move |this, _, _, cx| {
        if text {
            toggle(this, cx);
        } else {
            open_path(&target);
        }
    }))
}

/// A config file's first lines under its row, in the page.
pub(super) fn file_view(lines: &Result<Vec<SharedString>, SharedString>) -> Div {
    let body = match lines {
        Ok(lines) if lines.is_empty() => vec![label(&t("inst.view_empty")).into_any_element()],
        Ok(lines) => lines
            .iter()
            .map(|l| {
                plex(11., FontWeight::NORMAL)
                    .h(px(16.))
                    .whitespace_nowrap()
                    .text_color(muted3())
                    .child(l.clone())
                    .into_any_element()
            })
            .collect(),
        Err(why) => vec![
            inter(12., FontWeight::MEDIUM, 1.3)
                .text_color(behind3())
                .child(tf("inst.view_failed", &[("why", why)]))
                .into_any_element(),
        ],
    };
    div()
        .mb(px(10.))
        .px(px(16.))
        .py(px(12.))
        .bg(text3a(0.03))
        .border_1()
        .border_color(line3())
        .flex()
        .flex_col()
        .children(body)
}
