//! Library = Leaderboard (SPEC.md "Library page"): 500 rows in a `uniform_list`.
use gpui::{
    AnyElement, Div, FontWeight, IntoElement, ParentElement, Stateful, Styled, div, img,
    prelude::*, px, uniform_list,
};

use crate::{
    app::Scene,
    data::{Row, data},
    theme::*,
};

fn mono(size: f32) -> Div {
    div().font_family(MONO).text_size(px(size))
}

fn library_row(r: &'static Row, ix: usize) -> Stateful<Div> {
    div()
        .id(ix)
        .w_full() // uniform_list lays items out as roots; without this they shrink to content
        .h(px(72.))
        .px(px(18.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(14.))
        .when(ix > 0, |d| d.border_t_1().border_color(border()))
        .hover(|s| s.bg(raised()))
        .child(
            mono(13.)
                .flex_none()
                .w(px(44.))
                .text_color(muted())
                .child(r.rank.clone()),
        )
        .child(img(r.avatar()).flex_none().size(px(48.)).rounded(px(8.)))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(15.))
                        .line_height(px(20.))
                        .child(r.name.clone()),
                )
                .child(
                    div()
                        .mt(px(2.))
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(muted())
                        .child(r.category.clone()),
                ),
        )
        .child(
            mono(15.)
                .flex_none()
                .w(px(110.))
                .text_right()
                .child(r.time.clone()),
        )
        .child(
            mono(13.)
                .flex_none()
                .w(px(70.))
                .text_right()
                .text_color(if r.ahead { ahead() } else { behind() })
                .child(r.delta.clone()),
        )
        .child(
            mono(13.)
                .flex_none()
                .w(px(56.))
                .text_right()
                .text_color(pb())
                .child(r.elo.clone()),
        )
}

impl Scene {
    pub fn render_library(&mut self) -> AnyElement {
        let data = data();
        div()
            .flex_1()
            .min_h(px(0.))
            .bg(surface())
            .border_1()
            .border_color(border())
            .rounded(px(14.))
            .overflow_hidden()
            .child(
                uniform_list("library", data.rows.len(), move |range, _, _| {
                    range.map(|ix| library_row(&data.rows[ix], ix)).collect()
                })
                .track_scroll(&self.list)
                .size_full(),
            )
            .into_any_element()
    }
}
