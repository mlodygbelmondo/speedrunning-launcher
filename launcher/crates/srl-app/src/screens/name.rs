//! The first-run prompt for the offline player name. It comes before anything else while the
//! settings have no name, and when Play is pressed without one. Its copy says what an offline
//! account can do and that Ranked and Draftout need a Microsoft account online.
use gpui::{
    AnyElement, Context, FontWeight, ParentElement, SharedString, Styled, Window, div, prelude::*,
    px,
};
use srl_core::OfflineAccount;

use crate::{app::Launcher, copy::t, v3::*};

use super::{
    field::Field,
    game::{brand_mark, hero_backdrop},
};

impl Launcher {
    pub fn render_name(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Type straight away: focus the field the first time the prompt is drawn.
        if !self.prompt_focused {
            self.prompt_focused = true;
            window.focus(&self.name_prompt.focus, cx);
        }
        let valid = OfflineAccount::is_valid_name(&self.name_prompt.text);
        // With Microsoft login in the build, the name is the offline one and the login is on
        // Settings.
        let body = if self.account.available {
            "name.body_msa"
        } else {
            "name.body"
        };
        let field = self.name_input(Field::Prompt, 360., 56., 20., window, cx);
        let next = play_button(
            "name-continue",
            &t("name.continue"),
            SharedString::default(),
            288.,
            64.,
            19.,
            if valid { Press::Live } else { Press::Off },
        )
        .when(valid, |b| {
            b.on_click(cx.listener(|this, _, _, cx| this.submit_name(cx)))
        });

        div()
            .size_full()
            .relative()
            .children(hero_backdrop(self.hero_images(), 0.45))
            .child(brand_mark())
            .child(
                div()
                    .absolute()
                    .left(px(64.))
                    .top(px(128.))
                    .w(px(600.))
                    .flex()
                    .flex_col()
                    .child(
                        tight(
                            &t("name.title"),
                            72.,
                            FontWeight::EXTRA_BOLD,
                            -0.03,
                            0.9,
                            text3(),
                        )
                        .mt(px(16.)),
                    )
                    .child(
                        inter(16., FontWeight::NORMAL, 1.5)
                            .mt(px(20.))
                            .whitespace_nowrap()
                            .text_color(muted3())
                            .child(wrap(&t(body), INTER, FontWeight::NORMAL, 16., 520.)),
                    )
                    .child(label(&t("name.label")).mt(px(34.)))
                    .child(div().mt(px(10.)).child(field))
                    .child(self.name_rule(Field::Prompt, None, 360.).mt(px(10.)))
                    .child(div().mt(px(30.)).child(next)),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom(px(24.))
                    .flex()
                    .justify_center()
                    .child(
                        inter(11., FontWeight::NORMAL, 1.4)
                            .text_color(faint3())
                            .child(t("settings.disclaimer")),
                    ),
            )
            .into_any_element()
    }
}
