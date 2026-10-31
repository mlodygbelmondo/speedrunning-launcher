//! 2. Accounts: account list on the left, "Add a Microsoft account" card on the right.
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, img, prelude::*, px,
};

use crate::{
    app::{SIGNIN_ACCOUNTS, Scene},
    assets::embedded,
    copy::{t, tf},
    data::{per_os, screens},
    theme::*,
    ui::*,
};

impl Scene {
    pub fn render_accounts(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let acc = &screens().accounts;
        let rows = acc.list.iter().enumerate().map(|(i, a)| {
            let active = i == self.active_account;
            let status = if a.expired {
                txt(12.)
                    .text_color(warn())
                    .child(t("accounts.status_expired"))
            } else {
                muted_txt(12., tf("accounts.status_ok", &[("ago", &a.ago)]))
            };
            let action = if active {
                badge(t("accounts.active"), Badge::Ok).into_any_element()
            } else if a.expired {
                button(
                    ("acc-again", i),
                    t("accounts.sign_in_again"),
                    Kind::Secondary,
                )
                .small()
                .on_click(cx.listener(|this, _, _, cx| this.highlight_add_account(cx)))
                .into_any_element()
            } else {
                button(("acc-use", i), t("accounts.set_active"), Kind::Secondary)
                    .small()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.active_account = i;
                        cx.notify();
                    }))
                    .into_any_element()
            };
            div()
                .h(px(72.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(14.))
                .when(i > 0, |d| d.border_t_1().border_color(border()))
                .child(
                    img(embedded(&a.avatar))
                        .flex_none()
                        .size(px(44.))
                        .rounded(px(10.)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            txt(15.)
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(a.name.clone()),
                        )
                        .child(status),
                )
                .child(action)
                .child(button(("acc-out", i), t("accounts.sign_out"), Kind::Ghost).small())
        });

        let left = div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .child(card().py_0().children(rows))
            .child(
                div().mt(px(16.)).flex().flex_row().child(
                    button("acc-add", t("accounts.add"), Kind::Secondary)
                        .icon("plus")
                        .on_click(cx.listener(|this, _, _, cx| this.highlight_add_account(cx))),
                ),
            )
            .child(
                div()
                    .mt_auto()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(8.))
                    .child(icon("key-round", 16., muted()))
                    .child(
                        muted_txt(
                            12.,
                            tf("accounts.storage_note", &[("store", per_os(&acc.store))]),
                        )
                        .max_w(px(520.)),
                    ),
            );

        let highlight = self.add_highlight.is_some();
        let right = card()
            .flex_none()
            .w(px(440.))
            .when(highlight, |d| d.border_color(accent()))
            .child(card_title(t("accounts.add_title"), false))
            .child(self.signin_block(SIGNIN_ACCOUNTS, 4.0, cx));

        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_row()
            .items_stretch()
            .gap(px(20.))
            .child(left)
            .child(div().flex_none().child(right))
            .into_any_element()
    }
}
