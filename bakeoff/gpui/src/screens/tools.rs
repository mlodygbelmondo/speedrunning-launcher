//! 7. Tools: one card listing the tools, with toggles, install buttons and the legal-list note.
use std::time::Instant;

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*, px,
};

use crate::{
    app::{Install, Item, Scene},
    copy::{t, tf},
    data::{os, screens},
    theme::*,
    ui::*,
};

impl Scene {
    /// Header action: "Check now"; the icon spins for 1000 ms.
    pub fn tools_check_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let b = button("tools-check", t("tools.check_now"), Kind::Secondary);
        if self.tools_check.is_some() {
            b.leading(self.spinner(16., text())).into_any_element()
        } else {
            b.icon("refresh-cw")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tools_check = Some(Instant::now());
                    cx.notify();
                }))
                .into_any_element()
        }
    }

    fn labelled(label: gpui::SharedString, width: f32, control: impl IntoElement) -> gpui::Div {
        div()
            .flex_none()
            .w(px(width))
            .flex()
            .flex_col()
            .items_start()
            .gap(px(6.))
            .child(muted_txt(12., label))
            .child(control)
    }

    pub fn render_tools(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let s = screens();
        let rows = s.tools.iter().enumerate().map(|(i, tool)| {
            let other_os = tool.platform != "all" && tool.platform != os();
            let (start_key, auto_key) = self.tool_keys[i];
            let right: AnyElement = match self.install_state(Item::Tool(i), tool.installed) {
                Install::Done => div()
                    .flex()
                    .flex_row()
                    .gap(px(24.))
                    .child(Self::labelled(
                        t("tools.start_with_game"),
                        116.,
                        self.toggle(start_key, cx),
                    ))
                    .child(Self::labelled(
                        t("tools.auto_update"),
                        144.,
                        if tool.pinned {
                            badge_icon(t("tools.pinned"), Badge::Pb, "key-round").into_any_element()
                        } else {
                            self.toggle(auto_key, cx).into_any_element()
                        },
                    ))
                    .into_any_element(),
                _ if tool.external => button(
                    ("tool-get", i),
                    tf("tools.get", &[("name", &tool.name)]),
                    Kind::Secondary,
                )
                .small()
                .icon("external-link")
                .on_click(
                    cx.listener(|this, _, _, cx| this.show_toast("common.opens_in_browser", cx)),
                )
                .into_any_element(),
                Install::Running(_) => div()
                    .w(px(160.))
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(8.))
                    .child(
                        button(("tool-install", i), t("common.installing"), Kind::Primary)
                            .small()
                            .disabled(true),
                    )
                    .child(match self.install_state(Item::Tool(i), false) {
                        Install::Running(f) => progress(f),
                        _ => progress(1.0),
                    })
                    .into_any_element(),
                Install::Not => div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(badge(t("tools.not_installed"), Badge::Muted))
                    .child(
                        button(("tool-install", i), t("common.install"), Kind::Primary)
                            .small()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.start_install(Item::Tool(i), cx)
                            })),
                    )
                    .into_any_element(),
            };
            div()
                .min_h(px(84.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(14.))
                .when(i > 0, |d| d.border_t_1().border_color(border()))
                .child(icon_tile(&tool.icon, 44., 22., 10.))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    txt(15.)
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(if other_os { muted() } else { text() })
                                        .child(tool.name.clone()),
                                )
                                .when(!tool.version.is_empty(), |d| {
                                    d.child(
                                        mono(12.).text_color(muted()).child(tool.version.clone()),
                                    )
                                })
                                .when(tool.platform != "all", |d| {
                                    let key = if tool.platform == "windows" {
                                        "tools.windows_only"
                                    } else {
                                        "tools.linux_only"
                                    };
                                    d.child(badge(t(key), Badge::Muted))
                                }),
                        )
                        .child(muted_txt(13., tool.desc.clone()))
                        .when_some(tool.newer.clone(), |d, newer| {
                            d.child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(icon("triangle-alert", 12., warn()))
                                    .child(
                                        txt(12.)
                                            .text_color(warn())
                                            .child(tf("tools.legal_note", &[("version", &newer)])),
                                    ),
                            )
                        }),
                )
                .child(right)
        });

        card()
            .py_0()
            .flex()
            .flex_col()
            .children(rows)
            .into_any_element()
    }
}
