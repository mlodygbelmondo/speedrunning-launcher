//! Sign-in widget shared by the onboarding step and the Accounts "Add account" card: two option
//! cards, the browser "waiting" state, the device-code panel with the QR matrix, and the success row.
use std::time::Instant;

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, img, prelude::*, px,
};

use crate::{
    app::{Scene, Signin},
    assets::embedded,
    copy::{t, tf},
    data::screens,
    theme::*,
    ui::*,
};

impl Scene {
    /// `ctx` is `SIGNIN_ONBOARDING` or `SIGNIN_ACCOUNTS`; `module` is the QR module size in px.
    pub fn signin_block(&mut self, ctx: usize, module: f32, cx: &mut Context<Self>) -> AnyElement {
        let acc = &screens().accounts;
        match self.signin[ctx] {
            Signin::Cards | Signin::Waiting(_) => {
                let waiting = matches!(self.signin[ctx], Signin::Waiting(_));
                let option = |n: usize,
                              icon_name: &'static str,
                              title_key: &str,
                              hint_key: &str,
                              this: &Scene,
                              cx: &mut Context<Scene>| {
                    let busy = waiting && n == 0;
                    div()
                        .id(("signin-option", ctx * 2 + n))
                        .w_full()
                        .min_h(px(76.))
                        .px(px(18.))
                        .py(px(10.))
                        .bg(surface())
                        .border_1()
                        .border_color(border())
                        .rounded(px(14.))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(14.))
                        .when(!waiting, |d| {
                            d.cursor_pointer()
                                .hover(|s| s.border_color(accent()))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.signin[ctx] = if n == 0 {
                                        Signin::Waiting(Instant::now())
                                    } else {
                                        Signin::Code
                                    };
                                    cx.notify();
                                }))
                        })
                        .child(icon_tile(icon_name, 40., 20., 10.))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .h(px(20.))
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(
                                            txt(15.)
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(t(title_key)),
                                        )
                                        .when(n == 0, |d| {
                                            d.child(badge(t("common.recommended"), Badge::Accent))
                                        }),
                                )
                                .map(|d| {
                                    if busy {
                                        d.child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(8.))
                                                .child(this.spinner(14., muted()))
                                                .child(muted_txt(
                                                    13.,
                                                    t("onboarding.signin.waiting"),
                                                )),
                                        )
                                    } else {
                                        d.child(muted_txt(13., t(hint_key)))
                                    }
                                }),
                        )
                        .child(icon("chevron-right", 16., muted()))
                };
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(option(
                        0,
                        "globe",
                        "onboarding.signin.browser",
                        "onboarding.signin.browser_hint",
                        self,
                        cx,
                    ))
                    .child(option(
                        1,
                        "qr-code",
                        "onboarding.signin.code",
                        "onboarding.signin.code_hint",
                        self,
                        cx,
                    ))
                    .into_any_element()
            }
            Signin::Code => div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    card()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(16.))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .gap(px(8.))
                                .child(muted_txt(
                                    13.,
                                    tf("onboarding.signin.code_label", &[("url", &acc.device_url)]),
                                ))
                                .child(
                                    mono(32.)
                                        .font_weight(FontWeight::BOLD)
                                        .whitespace_nowrap()
                                        .child(acc.device_code.clone()),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_start()
                                        .gap(px(8.))
                                        .child(div().pt(px(2.)).child(self.spinner(14., muted())))
                                        .child(
                                            muted_txt(13., t("onboarding.signin.waiting"))
                                                .flex_1()
                                                .min_w(px(0.)),
                                        ),
                                ),
                        )
                        .child(qr_tile(&acc.device_qr, module)),
                )
                .child(
                    div().flex().flex_row().ml(px(-12.)).child(
                        button(("signin-back", ctx), t("common.back"), Kind::Ghost)
                            .small()
                            .icon("chevron-left")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.signin[ctx] = Signin::Cards;
                                cx.notify();
                            })),
                    ),
                )
                .into_any_element(),
            Signin::Done => {
                let first = &acc.list[0];
                card()
                    .h(px(64.))
                    .py_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(14.))
                    .child(
                        img(embedded(&first.avatar))
                            .flex_none()
                            .size(px(36.))
                            .rounded(px(8.)),
                    )
                    .child(
                        txt(15.)
                            .flex_1()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tf("onboarding.signin.done", &[("name", &first.name)])),
                    )
                    .child(status_icon(Status::Ok, 20.))
                    .into_any_element()
            }
        }
    }
}
