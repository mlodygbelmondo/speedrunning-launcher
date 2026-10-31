//! 9. Submit run: run files and timing on the left; video link, checklist and actions on the right.
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*, px,
};

use crate::{app::Scene, copy::t, data::screens, theme::*, ui::*};

impl Scene {
    pub fn render_submit(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let sb = &screens().submit;

        let files = card()
            .child(card_title(t("submit.files"), true))
            .children(sb.files.iter().map(|(label, file)| {
                div()
                    .h(px(40.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(icon("file-check", 16., ahead()))
                    .child(txt(14.).flex_1().child(label.clone()))
                    .child(mono(12.).text_color(muted()).child(file.clone()))
            }))
            .child(hline().mt(px(4.)))
            .child(
                div()
                    .pt(px(14.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(icon("folder-open", 16., muted()))
                    .child(mono(13.).child(sb.zip.clone()))
                    .child(muted_txt(12., sb.zip_size.clone()))
                    .child(badge(t("submit.files_ready"), Badge::Ok))
                    .child(div().flex_1())
                    .child(button("submit-folder", t("submit.show_folder"), Kind::Ghost).small()),
            );

        let timing = card().child(card_title(t("submit.timing"), true)).child(
            div()
                .flex()
                .flex_row()
                .items_end()
                .gap(px(48.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(muted_txt(12., t("submit.igt")))
                        .child(
                            mono(30.)
                                .font_weight(FontWeight::BOLD)
                                .text_color(pb())
                                .child(sb.igt.clone()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .pb(px(4.))
                        .child(muted_txt(12., t("submit.rta")))
                        .child(
                            mono(22.)
                                .font_weight(FontWeight::BOLD)
                                .child(sb.rta.clone()),
                        ),
                ),
        );

        let filled = self.submit_link;
        let field = div()
            .id("submit-link")
            .flex_1()
            .min_w(px(0.))
            .h(px(40.))
            .px(px(12.))
            .rounded(px(10.))
            .bg(bg())
            .border_1()
            .border_color(if filled || self.submit_focus {
                accent()
            } else {
                border()
            })
            .flex()
            .items_center()
            .cursor_text()
            .on_click(cx.listener(|this, _, _, cx| {
                this.submit_focus = true;
                cx.notify();
            }))
            .child(if filled {
                mono(13.).truncate().child(sb.sample_link.clone())
            } else {
                muted_txt(14., t("submit.video_placeholder")).truncate()
            });
        let video = card().child(card_title(t("submit.video"), true)).child(
            div().flex().flex_row().gap(px(8.)).child(field).child(
                button("submit-paste", t("submit.paste"), Kind::Secondary).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.submit_link = true;
                        cx.notify();
                    },
                )),
            ),
        );

        let rule = |i: usize, key: &str, cx: &mut Context<Self>| {
            div()
                .id(("submit-rule", i))
                .py(px(8.))
                .flex()
                .flex_row()
                .items_start()
                .gap(px(12.))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.submit_rules[i] = !this.submit_rules[i];
                    cx.notify();
                }))
                .child(checkbox(self.submit_rules[i]))
                .child(txt(13.).flex_1().min_w(px(0.)).child(t(key)))
        };
        let checklist = card()
            .child(card_title(t("submit.checklist"), true))
            .child(rule(0, "submit.rule_video", cx))
            .child(rule(1, "submit.rule_files", cx))
            .child(
                div()
                    .mt(px(8.))
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(10.))
                    .bg(alpha(warn(), 0.08))
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(10.))
                    .child(icon("triangle-alert", 16., warn()))
                    .child(
                        txt(13.)
                            .flex_1()
                            .min_w(px(0.))
                            .text_color(warn())
                            .child(t("submit.rule_launcher")),
                    ),
            );

        let ready = filled && self.submit_rules.iter().all(|r| *r);
        let actions = div()
            .flex()
            .flex_row()
            .justify_end()
            .gap(px(8.))
            .child(
                button("submit-copy", t("submit.copy"), Kind::Secondary)
                    .icon("copy")
                    .on_click(
                        cx.listener(|this, _, _, cx| this.show_toast("submit.copied", cx)),
                    ),
            )
            .child(
                button("submit-open", t("submit.open_src"), Kind::Primary)
                    .icon("external-link")
                    .disabled(!ready)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_toast("common.opens_in_browser", cx)
                    })),
            );

        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_row()
            .items_start()
            .gap(px(20.))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(files)
                    .child(timing),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(480.))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(video)
                    .child(checklist)
                    .child(actions),
            )
            .into_any_element()
    }
}
