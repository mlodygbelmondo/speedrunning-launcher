//! 10. Settings: two columns of section cards with setting rows.
use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, px,
};

use crate::{
    app::Scene,
    copy::{Lang, lang, t},
    data::{per_os, screens},
    theme::*,
    ui::*,
};

fn section(title_key: &str, rows: Vec<Div>) -> Div {
    card()
        .pb(px(4.))
        // Row lists bring their own vertical space: the title keeps 4 px under it, like the web.
        .child(card_title(t(title_key), true).mb(px(4.)))
        // 50 px rows: the v3 top nav starts the page at 104 px and About was added.
        .children(rows.into_iter().map(|r| r.min_h(px(50.)).py(px(6.))))
}

/// About (REDESIGN.md "Disclaimer and credits"): the Mojang disclaimer and the art and font
/// credits in Inter 12, faint.
fn about() -> Div {
    let line = |key: &str| {
        crate::v3::inter(12., FontWeight::NORMAL, 1.45)
            .mt(px(8.))
            .text_color(crate::v3::faint3())
            .child(t(key))
    };
    card()
        .pb(px(16.))
        .child(card_title(t("settings.section_about"), true).mb(px(4.)))
        .child(line("settings.disclaimer"))
        .child(line("settings.credits"))
}

fn items(v: &[String]) -> Vec<SharedString> {
    v.iter().map(|s| SharedString::from(s.clone())).collect()
}

impl Scene {
    pub fn render_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let st = &screens().settings;

        let general = section(
            "settings.section_general",
            vec![
                setting_row(
                    t("settings.language"),
                    None,
                    self.segmented(
                        "set-lang",
                        items(&st.languages),
                        lang().index(),
                        cx,
                        |this, i, window, cx| this.set_language(Lang::from_index(i), window, cx),
                    ),
                    true,
                ),
                setting_row(
                    t("settings.close_on_launch"),
                    Some(t("settings.close_on_launch_hint")),
                    self.toggle("settings.close_on_launch", cx),
                    false,
                ),
                setting_row(
                    t("settings.start_with_system"),
                    None,
                    self.toggle("settings.start_with_system", cx),
                    false,
                ),
            ],
        );
        let game = section(
            "settings.section_game",
            vec![
                setting_row(
                    t("settings.java"),
                    None,
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .child(muted_txt(13., t("settings.java_value")))
                        .child(status_icon(Status::Ok, 16.)),
                    true,
                ),
                setting_row(
                    t("settings.memory"),
                    Some(t("settings.memory_hint")),
                    self.segmented(
                        "set-mem",
                        items(&st.memory),
                        self.memory,
                        cx,
                        |this, i, _, _| this.memory = i,
                    ),
                    false,
                ),
            ],
        );
        let controls = section(
            "settings.section_controls",
            vec![
                setting_row(
                    t("settings.keybinds"),
                    None,
                    self.segmented(
                        "set-keys",
                        items(&st.keybinds),
                        self.keybinds,
                        cx,
                        |this, i, _, _| this.keybinds = i,
                    ),
                    true,
                ),
                setting_row(
                    t("settings.sensitivity"),
                    None,
                    div()
                        .flex_none()
                        .w(px(64.))
                        .h(px(32.))
                        .rounded(px(8.))
                        .bg(bg())
                        .border_1()
                        .border_color(border())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(mono(13.).child(st.sensitivity.clone())),
                    false,
                ),
                setting_row(
                    t("settings.standard_settings"),
                    None,
                    self.toggle("settings.standard_settings", cx),
                    false,
                ),
            ],
        );
        let updates = section(
            "settings.section_updates",
            vec![
                setting_row(
                    t("settings.auto_update"),
                    None,
                    self.toggle("settings.auto_update", cx),
                    true,
                ),
                setting_row(
                    t("settings.channel"),
                    None,
                    self.segmented(
                        "set-chan",
                        items(&st.channels),
                        self.channel,
                        cx,
                        |this, i, _, _| this.channel = i,
                    ),
                    false,
                ),
            ],
        );
        let advanced = section(
            "settings.section_advanced",
            vec![
                setting_row(
                    t("settings.data_folder"),
                    None,
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.))
                        .child(
                            mono(12.)
                                .text_color(muted())
                                .child(per_os(&st.data_folder).to_string()),
                        )
                        .child(
                            button("set-open-folder", t("common.open_folder"), Kind::Ghost)
                                .small()
                                .icon("folder-open"),
                        ),
                    true,
                ),
                setting_row(
                    t("settings.run_setup"),
                    None,
                    button("set-run-setup", t("settings.run_setup"), Kind::Secondary)
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| this.go_onboarding(0, cx))),
                    false,
                ),
                setting_row(
                    t("settings.reset_all"),
                    None,
                    button("set-reset", t("settings.reset_all"), Kind::Danger)
                        .small()
                        .on_click(
                            cx.listener(|this, _, _, cx| this.show_toast("settings.reset_all", cx)),
                        ),
                    false,
                ),
            ],
        );

        let column = |cards: Vec<Div>| {
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(12.))
                .children(cards)
        };
        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_row()
            .items_start()
            .gap(px(20.))
            .child(column(vec![general, game, about()]))
            .child(column(vec![controls, updates, advanced]))
            .into_any_element()
    }
}
