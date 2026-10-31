//! Settings in the v3 style: two columns of sections with hairline rows over the overworld art band.
//! Left: language, the player name and the settings import, the Microsoft account (`account.rs`; in a build without
//! Microsoft login only its one line), the Ranked and Draftout names for Stats, the optional mods,
//! the data folder. Right: every mode's Java and memory in one table, then About (the version,
//! Mojang's disclaimer and the credits). The page scrolls once it is taller than the window. Every
//! change is saved at once through the backend; the names when their edit ends.
//!
//! The credits are a view of their own (`Launcher::settings_credits`): `assets/credits.md` (or
//! `credits.pl.md`) for the player, and every adapted project's licence from
//! `THIRD_PARTY_NOTICES.md` (Pandora Launcher, EasyInjectBundled), quoted as written.
use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, prelude::*, px,
};
use srl_core::Mode;

use crate::{
    app::Launcher,
    assets::{CREDITS_MD, CREDITS_PL_MD, NOTICES_MD, card_path},
    backend::{Language, MEMORY_CHOICES_MB, Settings, mode_needs},
    catalog,
    copy::{lang, t, tf},
    v3::*,
};

use super::{
    field::Field,
    practice::{art_band, page_title},
};

/// The column widths: the mode table needs the wider one. 500 + 24 + 644 = 1168.
const LEFT_W: f32 = 500.;
const RIGHT_W: f32 = 644.;
/// The mode table's name column: wide enough for "Ranked · RSG · FSG · SSG".
const NAME_W: f32 = 196.;
const GAP: f32 = 24.;
/// Space between the sections of a column.
const SECTION_GAP: f32 = 30.;

/// A choice in GB, the way Settings and the game screen write it ("3 GB").
fn gb(mb: u32) -> SharedString {
    tf("settings.memory_value", &[("n", &(mb / 1024).to_string())])
}

/// The version line: the CI build names its commit, a local build says it is one.
fn version_line() -> SharedString {
    let v = env!("CARGO_PKG_VERSION");
    match option_env!("SRL_COMMIT") {
        Some(commit) => tf("settings.version_build", &[("v", v), ("commit", commit)]),
        None => tf("settings.version_local", &[("v", v)]),
    }
}

impl Launcher {
    /// A square segmented control: the selected cell is the light Play fill with ink text. A cell
    /// that isn't `enabled` (a memory choice below what the mode needs) is drawn faint and takes
    /// no clicks.
    pub fn segments(
        id: impl Into<SharedString>,
        labels: Vec<SharedString>,
        mono: bool,
        selected: usize,
        enabled: impl Fn(usize) -> bool,
        cx: &mut Context<Self>,
        on_pick: impl Fn(&mut Self, usize, &mut Window, &mut Context<Self>) + Clone + 'static,
    ) -> Div {
        let id: SharedString = id.into();
        div().flex_none().flex().flex_row().gap(px(3.)).children(
            labels.into_iter().enumerate().map(|(i, label)| {
                let on = i == selected;
                let live = !on && enabled(i);
                let pick = on_pick.clone();
                let color = if on {
                    ink()
                } else if live {
                    muted3()
                } else {
                    text3a(0.22)
                };
                div()
                    .id((id.clone(), i))
                    .h(px(30.))
                    .px(px(13.))
                    .flex()
                    .items_center()
                    .bg(if on {
                        text3()
                    } else if live {
                        text3a(0.05)
                    } else {
                        text3a(0.02)
                    })
                    .when(live, |d| {
                        d.cursor_pointer().hover(|s| s.bg(text3a(0.1))).on_click(
                            cx.listener(move |this, _, window, cx| pick(this, i, window, cx)),
                        )
                    })
                    .child(if mono {
                        plex(12., FontWeight::SEMIBOLD)
                            .text_color(color)
                            .child(label)
                    } else {
                        inter(12., FontWeight::SEMIBOLD, 1.)
                            .whitespace_nowrap()
                            .text_color(color)
                            .child(label)
                    })
            }),
        )
    }

    /// A square on/off switch: `accent-2` with an ink knob on the right when on, a dim track with
    /// the knob on the left when off.
    fn switch(
        id: &'static str,
        on: bool,
        cx: &mut Context<Self>,
        on_toggle: impl Fn(&mut Self, bool, &mut Context<Self>) + 'static,
    ) -> gpui::Stateful<Div> {
        div()
            .id(id)
            .flex_none()
            .w(px(44.))
            .h(px(24.))
            .p(px(4.))
            .flex()
            .flex_row()
            .items_center()
            .when(on, |d| d.justify_end())
            .bg(if on { accent2() } else { text3a(0.06) })
            // Off has the text field's edge, so the track reads as a control, not a gap.
            .when(!on, |d| {
                d.p(px(3.))
                    .border_1()
                    .border_color(text3a(0.16))
                    .hover(|s| s.bg(text3a(0.12)))
            })
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| on_toggle(this, !on, cx)))
            .child(
                div()
                    .flex_none()
                    .size(px(16.))
                    .bg(if on { ink() } else { muted3() }),
            )
    }

    /// One settings row: the label (and what's under it) on the left, the control on the right.
    fn setting_row(label: SharedString, under: Option<Div>, control: impl IntoElement) -> Div {
        div()
            .min_h(px(64.))
            .py(px(13.))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(24.))
            .border_t_1()
            .border_color(line3())
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(
                        inter(15., FontWeight::SEMIBOLD, 1.2)
                            .text_color(text3())
                            .child(label),
                    )
                    .children(under),
            )
            .child(control)
    }

    fn section(title: SharedString, rows: Vec<Div>) -> Div {
        div()
            .flex()
            .flex_col()
            .child(section_head(&title).mb(px(14.)))
            .children(rows)
    }

    /// A hint under a row's label, Inter 12 `muted`, wrapped at `w`.
    fn hint(s: &str, w: f32) -> Div {
        inter(12., FontWeight::NORMAL, 1.4)
            .whitespace_nowrap()
            .text_color(muted3())
            .child(wrap(s, INTER, FontWeight::NORMAL, 12., w))
    }

    /// A note under a section, Inter 12 `faint`.
    fn note(s: &str, w: f32) -> Div {
        inter(12., FontWeight::NORMAL, 1.4)
            .mt(px(12.))
            .whitespace_nowrap()
            .text_color(faint3())
            .child(wrap(s, INTER, FontWeight::NORMAL, 12., w))
    }

    fn general_section(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let lang_i = Language::ALL.iter().position(|l| *l == lang()).unwrap_or(0);
        let language = Self::segments(
            "settings-lang",
            Language::ALL
                .iter()
                .map(|l| t(&format!("lang.{}", l.code())))
                .collect(),
            false,
            lang_i,
            |_| true,
            cx,
            |this, i, window, cx| this.set_language(Language::ALL[i], window, cx),
        );
        const FIELD_W: f32 = 220.;
        let name = self.name_input(Field::Settings, FIELD_W, 36., 14., window, cx);
        // The hint shares the row with the field and the 24 px gap. It reads like every other row
        // hint on the page (Inter, muted); while the typed name breaks the rule, the rule takes
        // its place in `behind`, as under the name prompt.
        let hint_w = LEFT_W - FIELD_W - GAP;
        // The offline name plays whenever no Microsoft account does (`account::play_as`).
        let available = self.account.available;
        let name_hint = if self.name_breaks_rule(Field::Settings) {
            self.name_rule(Field::Settings, None, hint_w)
        } else if available {
            Self::hint(&t("settings.player_hint_msa"), hint_w)
        } else {
            Self::hint(&t("settings.player_hint_offline"), hint_w)
        };
        // "Import my current settings": the same page the first start offers.
        let import = secondary("settings-import", &t("settings.import_open"))
            .on_click(cx.listener(|this, _, _, cx| this.open_import(false, cx)));
        Self::section(
            t("settings.section_general"),
            vec![
                Self::setting_row(
                    t("settings.language"),
                    Some(Self::hint(&t("settings.language_hint"), 240.)),
                    language,
                ),
                Self::setting_row(t("settings.player_name"), Some(name_hint), name),
                Self::setting_row(
                    t("settings.import"),
                    Some(Self::hint(&t("settings.import_hint"), LEFT_W - 150.)),
                    import,
                ),
            ],
        )
        // A build without Microsoft login: this one line, and nothing else about accounts.
        .when(!available, |d| {
            d.child(Self::account_unavailable_line(LEFT_W))
        })
    }

    /// The names Stats looks the player up by (`StatsSettings`). Not the offline name: people
    /// play Ranked and Draftout under their Microsoft account's name.
    fn stats_section(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        const FIELD_W: f32 = 220.;
        let hint_w = LEFT_W - FIELD_W - GAP;
        let mut row = |which: Field, label: &str, hint: &str| {
            let input = self.name_input(which, FIELD_W, 36., 14., window, cx);
            let under = if self.name_breaks_rule(which) {
                self.name_rule(which, None, hint_w)
            } else {
                Self::hint(&t(hint), hint_w)
            };
            Self::setting_row(t(label), Some(under), input)
        };
        let ranked = row(
            Field::Ranked,
            "settings.ranked_name",
            "settings.ranked_name_hint",
        );
        let draftout = row(
            Field::Draftout,
            "settings.draftout_name",
            "settings.draftout_name_hint",
        );
        Self::section(t("settings.section_stats"), vec![ranked, draftout])
    }

    fn mods_section(&self, cx: &mut Context<Self>) -> Div {
        let text_w = LEFT_W - 44. - GAP;
        let accessibility = Self::switch(
            "settings-accessibility",
            self.settings.accessibility_mods,
            cx,
            |this, on, cx| {
                let settings = Settings {
                    accessibility_mods: on,
                    ..this.settings.clone()
                };
                this.save_settings(settings, cx);
            },
        );
        let extras = Self::switch(
            "settings-draftout-extras",
            self.settings.draftout_extras,
            cx,
            |this, on, cx| {
                let settings = Settings {
                    draftout_extras: on,
                    ..this.settings.clone()
                };
                this.save_settings(settings, cx);
            },
        );
        Self::section(
            t("settings.section_mods"),
            vec![
                Self::setting_row(
                    t("settings.accessibility"),
                    Some(Self::hint(&t("settings.accessibility_hint"), text_w)),
                    accessibility,
                ),
                Self::setting_row(
                    t("settings.draftout_extras"),
                    Some(Self::hint(&t("settings.draftout_extras_hint"), text_w)),
                    extras,
                ),
            ],
        )
        .child(Self::note(&t("settings.mods_note"), LEFT_W))
    }

    /// Temporary switches for development (Piotr, 2026-10-29).
    fn dev_section(&self, cx: &mut Context<Self>) -> Div {
        let hide = Self::switch(
            "settings-dev-taglines",
            self.settings.dev_hide_taglines,
            cx,
            |this, on, cx| {
                let settings = Settings {
                    dev_hide_taglines: on,
                    ..this.settings.clone()
                };
                this.save_settings(settings, cx);
            },
        );
        Self::section(
            t("settings.section_dev"),
            vec![Self::setting_row(
                t("settings.dev_taglines"),
                Some(Self::hint(
                    &t("settings.dev_taglines_hint"),
                    RIGHT_W - 44. - GAP,
                )),
                hide,
            )],
        )
    }

    fn data_section(&self, cx: &mut Context<Self>) -> Div {
        let open = secondary("settings-open-folder", &t("settings.open_folder"))
            .on_click(cx.listener(|this, _, _, _| this.backend().open_data_folder()));
        let text_w = LEFT_W - 140.;
        let path = self.data_dir.display().to_string();
        let under = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(Self::hint(&t("settings.data_folder_hint"), text_w))
            .child(
                plex(12., FontWeight::MEDIUM)
                    .line_height(px(17.))
                    .text_color(text3a(0.8))
                    .child(wrap_paths(&path, PLEX, FontWeight::MEDIUM, 12., text_w)),
            );
        Self::section(
            t("settings.section_data"),
            vec![Self::setting_row(
                t("settings.data_folder"),
                Some(under),
                open,
            )],
        )
    }

    /// One row of the mode table: the modes (the four 1.16.1 modes share one instance, so one
    /// row sets them together), their Java and their memory.
    fn mode_settings_row(&self, modes: &[Mode], cx: &mut Context<Self>) -> Div {
        let first = modes[0];
        let d = catalog::def(if modes.len() > 1 { Mode::Rsg } else { first });
        let needs = mode_needs(first);
        // Settings from before the shared row may give the four different values: then no
        // choice is lit until one is picked for all of them.
        let current = self.settings.memory_mb(first);
        let same = modes.iter().all(|m| self.settings.memory_mb(*m) == current);
        let selected = MEMORY_CHOICES_MB
            .iter()
            .position(|m| *m == current && same)
            .unwrap_or(usize::MAX);
        let ids: Vec<&str> = modes.iter().map(|m| m.id()).collect();
        let row_modes = modes.to_vec();
        let memory = Self::segments(
            format!("settings-mem-{}", ids.join("-")),
            MEMORY_CHOICES_MB.iter().map(|mb| gb(*mb)).collect(),
            true,
            selected,
            |i| MEMORY_CHOICES_MB[i] >= needs.min_memory_mb,
            cx,
            move |this, i, _, cx| {
                let mut settings = this.settings.clone();
                for m in &row_modes {
                    settings.set_memory_mb(*m, MEMORY_CHOICES_MB[i]);
                }
                this.save_settings(settings, cx);
            },
        );
        // The installed runtime's version when there is one ("Java 21.0.7"), else the major
        // version the modes get.
        let java = modes
            .iter()
            .find_map(|m| self.modes[crate::backend::index(*m)].info.java.clone())
            .unwrap_or_else(|| needs.java_major.to_string());
        let names: Vec<&str> = modes.iter().map(|m| catalog::def(*m).short).collect();
        let two_lines =
            |top: Div, bottom: Div| div().flex().flex_col().gap(px(6.)).child(top).child(bottom);
        div()
            .h(px(60.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(14.))
            .border_t_1()
            .border_color(line3())
            .child(
                div()
                    .relative()
                    .flex_none()
                    .w(px(56.))
                    .h(px(32.))
                    .child(overlay(tile_bg()))
                    .child(art_fill(card_path(d.art))),
            )
            .child(
                two_lines(
                    inter(15., FontWeight::SEMIBOLD, 1.)
                        .text_color(text3())
                        .whitespace_nowrap()
                        .child(SharedString::from(names.join(" · "))),
                    plex(11., FontWeight::MEDIUM)
                        .text_color(muted3())
                        .child(tf("settings.mode_mc", &[("mc", first.minecraft_version())])),
                )
                .flex_none()
                .w(px(NAME_W)),
            )
            .child(
                plex(13., FontWeight::MEDIUM)
                    .flex_1()
                    .min_w(px(0.))
                    .text_color(text3())
                    .child(tf("settings.java_value", &[("n", &java)])),
            )
            .child(div().flex_none().child(memory))
    }

    fn modes_section(&self, cx: &mut Context<Self>) -> Div {
        // The column labels sit over their columns: after the 56 px art and 14 px gap, the
        // name column, then Java; Memory is right-aligned over the choices.
        let head = div()
            .h(px(24.))
            .flex()
            .flex_row()
            .items_start()
            .child(
                label(&t("settings.col_mode"))
                    .ml(px(70.))
                    .w(px(NAME_W + 14.)),
            )
            .child(label(&t("settings.col_java")))
            .child(div().flex_1())
            .child(label(&t("settings.col_memory")));
        let shared: Vec<Mode> = Mode::ALL.into_iter().filter(|m| m.is_shared()).collect();
        let draftout_min = mode_needs(Mode::Draftout).min_memory_mb / 1024;
        div()
            .flex()
            .flex_col()
            .child(section_head(&t("settings.section_modes")).mb(px(16.)))
            .child(head)
            .child(self.mode_settings_row(&shared, cx))
            .child(self.mode_settings_row(&[Mode::Draftout], cx))
            .child(Self::note(
                &tf("settings.modes_note", &[("n", &draftout_min.to_string())]),
                RIGHT_W,
            ))
    }

    fn about_section(&self, cx: &mut Context<Self>) -> Div {
        let version = div()
            .h(px(44.))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(line3())
            .child(
                inter(15., FontWeight::SEMIBOLD, 1.2)
                    .text_color(text3())
                    .child(t("settings.version")),
            )
            .child(
                plex(13., FontWeight::MEDIUM)
                    .text_color(muted3())
                    .child(version_line()),
            );
        Self::section(t("settings.section_about"), vec![version])
            .child(
                // Mojang's exact words, in English in every language (REDESIGN.md "Disclaimer").
                inter(12., FontWeight::NORMAL, 1.45)
                    .pt(px(12.))
                    .border_t_1()
                    .border_color(line3())
                    .whitespace_nowrap()
                    .text_color(faint3())
                    .child(wrap(
                        &t("settings.disclaimer"),
                        INTER,
                        FontWeight::NORMAL,
                        12.,
                        RIGHT_W,
                    )),
            )
            .child(div().mt(px(14.)).flex().flex_row().child(
                tertiary("settings-credits", &t("settings.credits_open")).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.settings_credits = true;
                        cx.notify();
                    },
                )),
            ))
    }

    pub fn render_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.settings_credits {
            return self.render_credits(cx);
        }
        let general = self.general_section(window, cx);
        let account = self
            .account
            .available
            .then(|| self.account_section(LEFT_W, cx));
        let stats = self.stats_section(window, cx);
        let left = div()
            .flex_none()
            .w(px(LEFT_W))
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            .child(general)
            .children(account)
            .child(stats)
            .child(self.mods_section(cx))
            .child(self.data_section(cx));
        let right = div()
            .flex_none()
            .w(px(RIGHT_W))
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            .child(self.modes_section(cx))
            .child(self.about_section(cx))
            .child(self.dev_section(cx));

        div()
            .size_full()
            .relative()
            // A click anywhere but the name field ends its edit, as in any text field: saved if
            // valid, else the saved name comes back.
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if this.name_settings.focus.is_focused(window) {
                        window.blur();
                        this.commit_settings_name(cx);
                    }
                    for which in [Field::Ranked, Field::Draftout] {
                        let focus = match which {
                            Field::Ranked => &this.ranked_field.focus,
                            _ => &this.draftout_field.focus,
                        };
                        if focus.is_focused(window) {
                            window.blur();
                            this.commit_stats_name(which, cx);
                        }
                    }
                }),
            )
            .child(art_band("overworld"))
            .child(
                div()
                    .id("settings-scroll")
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(104.))
                    .bottom_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .pl(px(64.))
                            .pr(px(48.))
                            .pb(px(40.))
                            .flex()
                            .flex_col()
                            .child(page_title(&t("settings.title"), &t("settings.subtitle")))
                            .child(
                                div()
                                    .mt(px(32.))
                                    .flex()
                                    .flex_row()
                                    .gap(px(GAP))
                                    .child(left)
                                    .child(right),
                            ),
                    ),
            )
            // The page scrolls under the window's bottom edge: text there fades out instead of
            // being cut (at the end only the page's bottom padding is under it).
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(px(40.))
                    .child(shade_up(1.0, 0.0, 1.0)),
            )
            .into_any_element()
    }

    /// The credits: CREDITS.md on the left, the code licences on the right, each scrolling on
    /// its own.
    fn render_credits(&mut self, cx: &mut Context<Self>) -> AnyElement {
        const COL: f32 = 572.;
        let column = |id: &'static str, title: SharedString, body: Vec<Div>| {
            div()
                .flex_none()
                .w(px(COL))
                .h_full()
                .flex()
                .flex_col()
                .child(section_head(&title).mb(px(4.)))
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_h(px(0.))
                        .child(
                            div()
                                .id(id)
                                .size_full()
                                .overflow_y_scroll()
                                .child(div().flex().flex_col().pb(px(40.)).children(body)),
                        )
                        // Text running under the bottom edge fades out, so a cut line reads as
                        // "scroll for more".
                        .child(
                            div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .h(px(40.))
                                .child(shade_up(1.0, 0.0, 1.0)),
                        ),
                )
        };
        // One block per adapted project: what uses it, its source line and its licence text.
        let code: Vec<Div> = notices()
            .into_iter()
            .enumerate()
            .flat_map(|(i, n)| {
                let intro = notice_intro_key(&n.project)
                    .map(t)
                    .unwrap_or_else(|| n.project.clone().into());
                [
                    section_head(&n.project).mt(px(if i == 0 { 10. } else { 30. })),
                    md_text(&intro, COL).mt(px(8.)),
                    plex(11., FontWeight::MEDIUM)
                        .mt(px(6.))
                        .text_color(faint3())
                        .child(SharedString::from(n.source)),
                    plex(11., FontWeight::NORMAL)
                        .mt(px(14.))
                        .line_height(px(16.))
                        .text_color(muted3())
                        .child(SharedString::from(n.licence)),
                ]
            })
            .collect();
        let credits = match lang() {
            Language::En => CREDITS_MD,
            Language::Pl => CREDITS_PL_MD,
        };
        div()
            .size_full()
            .relative()
            .child(art_band("overworld"))
            .child(
                div()
                    .absolute()
                    .left(px(64.))
                    .right(px(48.))
                    .top(px(104.))
                    .bottom(px(32.))
                    .flex()
                    .flex_col()
                    .child(page_title(
                        &t("settings.credits_title"),
                        &t("settings.credits_subtitle"),
                    ))
                    .child(
                        div().mt(px(18.)).flex().flex_row().child(
                            tertiary("credits-back", &format!("← {}", t("settings.credits_back")))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.settings_credits = false;
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(
                        div()
                            .mt(px(26.))
                            .flex_1()
                            .min_h(px(0.))
                            .flex()
                            .flex_row()
                            .gap(px(GAP))
                            .child(column(
                                "credits-art",
                                t("settings.credits_art"),
                                markdown(credits, COL),
                            ))
                            .child(column("credits-code", t("settings.credits_code"), code)),
                    ),
            )
            .into_any_element()
    }
}

/// Inter 13/1.5 `muted`, wrapped at `w`: the credits' running text.
fn md_text(s: &str, w: f32) -> Div {
    inter(13., FontWeight::NORMAL, 1.5)
        .whitespace_nowrap()
        .text_color(muted3())
        .child(wrap(s, INTER, FontWeight::NORMAL, 13., w))
}

/// One block of a small Markdown file.
#[derive(Debug, PartialEq, Eq)]
enum Block {
    /// `##` and deeper. The `#` title is left out: the column has its own.
    Heading(String),
    Para(String),
    /// A list item, or a table row (its cells joined with " · "; the header row is left out).
    Item(String),
}

/// Parses the Markdown our credit files use: headings, paragraphs, `-` lists (continuation lines
/// indented) and pipe tables. Bold and code marks are dropped; links stay as written.
fn blocks(md: &str) -> Vec<Block> {
    let clean = |s: &str| s.replace("**", "").replace('`', "");
    let mut out: Vec<Block> = Vec::new();
    let mut table_head = false;
    let mut open = false;
    let mut comment = false;
    for raw in md.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        // `<!-- -->` notes for whoever edits the file are not shown.
        if comment || trimmed.starts_with("<!--") {
            comment = !trimmed.ends_with("-->");
            continue;
        }
        if trimmed.is_empty() {
            open = false;
            table_head = false;
            continue;
        }
        if let Some(h) = trimmed.strip_prefix('#') {
            open = false;
            if h.starts_with('#') {
                out.push(Block::Heading(clean(h.trim_start_matches('#').trim())));
            }
            continue;
        }
        if trimmed.starts_with('|') {
            open = false;
            let cells: Vec<String> = trimmed
                .trim_matches('|')
                .split('|')
                .map(|c| clean(c.trim()))
                .collect();
            if cells
                .iter()
                .all(|c| c.chars().all(|ch| ch == '-' || ch == ':'))
            {
                continue;
            }
            if !table_head {
                // The first row of a table is its header.
                table_head = true;
                continue;
            }
            out.push(Block::Item(cells.join(" · ")));
            continue;
        }
        if let Some(item) = trimmed.strip_prefix("- ") {
            out.push(Block::Item(clean(item)));
            open = true;
            continue;
        }
        let text = clean(trimmed);
        match out.last_mut() {
            Some(Block::Item(s) | Block::Para(s)) if open || raw.starts_with(' ') => {
                s.push(' ');
                s.push_str(&text);
            }
            _ => {
                out.push(Block::Para(text));
                open = true;
            }
        }
    }
    out
}

/// The credit file as v3 elements: section heads, running text and square-bulleted items.
fn markdown(md: &str, w: f32) -> Vec<Div> {
    blocks(md)
        .into_iter()
        .map(|b| match b {
            Block::Heading(h) => section_head(&h).mt(px(22.)),
            Block::Para(p) => md_text(&p, w).mt(px(10.)),
            Block::Item(i) => div()
                .mt(px(8.))
                .flex()
                .flex_row()
                .items_start()
                .child(
                    div()
                        .flex_none()
                        .size(px(5.))
                        .mt(px(8.))
                        .mr(px(10.))
                        .bg(accent2()),
                )
                .child(md_text(&i, w - 15.)),
        })
        .collect()
}

/// One project whose code the launcher adapts, from THIRD_PARTY_NOTICES.md.
#[derive(Debug)]
struct Notice {
    /// The `##` heading: "Pandora Launcher".
    project: String,
    /// Its source line ("https://github.com/…, commit …"), up to the first sentence end.
    source: String,
    /// The licence text from its code block.
    licence: String,
}

/// Every `##` section of THIRD_PARTY_NOTICES.md that quotes a licence (projects only read for
/// behaviour have none).
fn notices() -> Vec<Notice> {
    NOTICES_MD
        .split("\n## ")
        .skip(1)
        .filter_map(|section| {
            let (project, body) = section.split_once('\n')?;
            let licence = body
                .split("```text")
                .nth(1)?
                .split("```")
                .next()?
                .trim()
                .to_string();
            // The paragraph before the licence block: "<url>, commit abc: what it is." The URL
            // and commit are the source line.
            let first = body
                .split("\n\n")
                .map(str::trim)
                .find(|p| !p.is_empty())
                .unwrap_or_default()
                .replace('\n', " ");
            let source = first
                .split_once(": ")
                .map_or(first.as_str(), |(s, _)| s)
                .trim_end_matches('.')
                .replace(['<', '>'], "");
            Some(Notice {
                project: project.trim().to_string(),
                source,
                licence,
            })
        })
        .collect()
}

/// The copy key that says what in the launcher uses a project's code.
fn notice_intro_key(project: &str) -> Option<&'static str> {
    match project {
        "Pandora Launcher" => Some("settings.credits_pandora"),
        "EasyInjectBundled" => Some("settings.credits_easyinject"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credits_markdown() {
        for (md, fonts, inter) in [
            (CREDITS_MD, "Fonts", "Inter · Rasmus Andersson"),
            (CREDITS_PL_MD, "Fonty", "Inter · Rasmus Andersson"),
        ] {
            let b = blocks(md);
            assert!(b.contains(&Block::Heading(fonts.into())), "{b:#?}");
            assert!(
                b.iter()
                    .any(|x| matches!(x, Block::Item(s) if s.starts_with(inter) && s.ends_with("SIL Open Font License 1.1"))),
                "{b:#?}"
            );
            // The table's header row and the editor's comment are left out, no block keeps
            // Markdown marks, and nothing for developers (repo paths, the demo) shows.
            assert!(!b.iter().any(|x| match x {
                Block::Heading(s) | Block::Para(s) | Block::Item(s) =>
                    s.starts_with("Font ·")
                        || s.contains("**")
                        || s.contains('`')
                        || s.contains("<!--")
                        || s.contains(".md")
                        || s.contains("bakeoff")
                        || s.contains("SRL_DEMO"),
            }));
            assert!(
                b.iter()
                    .any(|x| matches!(x, Block::Para(s) if s.starts_with("NOT AN OFFICIAL")))
            );
        }
        // A list item's indented second line joins it.
        let b = blocks("- one\n  two\n");
        assert_eq!(b, [Block::Item("one two".into())]);
    }

    #[test]
    fn every_adapted_project_is_credited() {
        let n = notices();
        let names: Vec<&str> = n.iter().map(|n| n.project.as_str()).collect();
        assert_eq!(names, ["Pandora Launcher", "EasyInjectBundled"]);
        for notice in &n {
            assert!(
                notice.source.starts_with("https://github.com/"),
                "{notice:?}"
            );
            assert!(notice.source.contains(", commit "), "{notice:?}");
            assert!(notice.licence.starts_with("MIT License"), "{notice:?}");
            assert!(notice.licence.ends_with("SOFTWARE."), "{notice:?}");
            let key = notice_intro_key(&notice.project).expect("an intro line");
            for json in [crate::assets::COPY_JSON, crate::assets::COPY_PL_JSON] {
                let table: serde_json::Value = serde_json::from_str(json).unwrap();
                assert!(table[key].is_string(), "{key}");
            }
        }
        assert!(n[0].licence.contains("Copyright (c) 2025 Moulberry"));
        assert!(n[1].licence.contains("Copyright (c) 2026 jojoe77777"));
        assert_eq!(
            n[1].source,
            "https://github.com/jojoe77777/EasyInjectBundled, commit f59501e"
        );
    }
}
