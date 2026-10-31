//! The one screen shown when the data folder can't be written (`Snapshot::data_error`): a
//! read-only or unplugged drive, a bad `SRL_HOME`. Nothing could be installed or saved, so
//! instead of a launcher that takes a name it can't keep and fails every Play with a path error,
//! the window says which folder and what to do.
use gpui::{AnyElement, FontWeight, ParentElement, Styled, div, prelude::*, px};

use crate::{app::Launcher, copy::t, v3::*};

use super::game::brand_mark;

impl Launcher {
    pub fn render_data_error(&self, problem: &str) -> AnyElement {
        div()
            .size_full()
            .relative()
            .child(brand_mark())
            .child(
                div()
                    .absolute()
                    .left(px(64.))
                    .top(px(128.))
                    .w(px(720.))
                    .flex()
                    .flex_col()
                    .child(eyebrow(&t("data.eyebrow"), behind3()))
                    .child(
                        tight(
                            &t("data.title"),
                            56.,
                            FontWeight::EXTRA_BOLD,
                            -0.03,
                            0.95,
                            text3(),
                        )
                        .mt(px(16.)),
                    )
                    .child(
                        inter(16., FontWeight::NORMAL, 1.5)
                            .mt(px(20.))
                            .whitespace_nowrap()
                            .text_color(muted3())
                            .child(wrap(&t("data.body"), INTER, FontWeight::NORMAL, 16., 640.)),
                    )
                    .child(
                        plex(13., FontWeight::MEDIUM)
                            .mt(px(18.))
                            .line_height(px(19.))
                            .text_color(behind3())
                            .child(wrap(problem, PLEX, FontWeight::MEDIUM, 13., 700.)),
                    )
                    .child(
                        inter(14., FontWeight::NORMAL, 1.5)
                            .mt(px(24.))
                            .whitespace_nowrap()
                            .text_color(muted3())
                            .child(wrap(&t("data.hint"), INTER, FontWeight::NORMAL, 14., 640.)),
                    ),
            )
            .into_any_element()
    }
}
