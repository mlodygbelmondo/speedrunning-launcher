//! 6. Learn: level chips and tutorial cards grouped by topic, in a scroll container.
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div,
    linear_color_stop, linear_gradient, prelude::*, px,
};

use crate::{
    app::Scene,
    copy::t,
    data::{Tutorial, screens},
    theme::*,
    ui::*,
};

const LEVELS: [&str; 3] = ["beginner", "intermediate", "advanced"];

fn tutorial_card(id: usize, group_icon: &str, tut: &'static Tutorial) -> gpui::Stateful<gpui::Div> {
    let level = match tut.level.as_str() {
        "beginner" => Badge::Ok,
        "intermediate" => Badge::Accent,
        _ => Badge::Pb,
    };
    let length: SharedString = if tut.length.is_empty() {
        t(&format!("learn.kind_{}", tut.kind))
    } else {
        tut.length.clone().into()
    };
    let thumb = div()
        .relative()
        .w_full()
        .h(px(155.))
        .rounded_t(px(13.))
        .bg(linear_gradient(
            180.,
            linear_color_stop(raised(), 0.),
            linear_color_stop(c(0x120F1A), 1.),
        ))
        .flex()
        .items_center()
        .justify_center()
        .child(icon(group_icon, 56., alpha(accent(), 0.35)))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px(44.))
                        .rounded_full()
                        .bg(alpha(black(), 0.55))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(
                            if tut.kind == "guide" {
                                "book-open"
                            } else {
                                "play"
                            },
                            20.,
                            text(),
                        )),
                ),
        )
        .child(
            div()
                .absolute()
                .right(px(8.))
                .bottom(px(8.))
                .h(px(22.))
                .px(px(8.))
                .rounded(px(6.))
                .bg(alpha(black(), 0.65))
                .flex()
                .items_center()
                .child(mono(11.).child(length)),
        );
    div()
        .id(("tutorial", id))
        .bg(surface())
        .border_1()
        .border_color(border())
        .rounded(px(14.))
        .flex()
        .flex_col()
        .cursor_pointer()
        .hover(|s| s.border_color(accent()))
        .child(thumb)
        .child(
            div()
                .pt(px(12.))
                .px(px(14.))
                .pb(px(14.))
                .flex()
                .flex_col()
                .child(
                    txt(14.)
                        .h(px(40.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_clamp(2)
                        .child(tut.title.clone()),
                )
                .child(muted_txt(12., tut.creator.clone()).mt(px(2.)))
                .child(
                    div()
                        .mt(px(10.))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.))
                        .child(badge(t(&format!("learn.level_{}", tut.level)), level))
                        .when(tut.current, |d| {
                            d.child(badge(t("learn.current"), Badge::Ok))
                        })
                        .child(div().flex_1())
                        .child(muted_txt(12., tut.date.clone())),
                ),
        )
}

impl Scene {
    pub fn render_learn(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let s = screens();
        let filter = self.learn_filter;
        let chips = div()
            .flex_none()
            .flex()
            .flex_row()
            .gap(px(8.))
            .child(
                chip("lv-all", t("learn.filter_all"), filter.is_none()).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.learn_filter = None;
                        cx.notify();
                    },
                )),
            )
            .children(LEVELS.iter().enumerate().map(|(i, lv)| {
                let lv: &'static str = lv;
                chip(
                    ("lv-chip", i),
                    t(&format!("learn.level_{lv}")),
                    filter == Some(lv),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.learn_filter = Some(lv);
                    cx.notify();
                }))
            }));

        let sections = s.tutorial_groups.iter().enumerate().filter_map(|(gi, g)| {
            let cards: Vec<AnyElement> = g
                .items
                .iter()
                .enumerate()
                .filter(|(_, tut)| filter.is_none_or(|f| tut.level == f))
                .map(|(ti, tut)| {
                    tutorial_card(gi * 100 + ti, &g.icon, tut)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_toast("common.opens_in_browser", cx)
                        }))
                        .into_any_element()
                })
                .collect();
            if cards.is_empty() {
                return None;
            }
            Some(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .mb(px(12.))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .child(icon(&g.icon, 16., accent()))
                            .child(
                                txt(16.)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(g.title.clone()),
                            ),
                    )
                    .child(grid(4, 16., cards)),
            )
        });

        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(chips)
            .child(
                div()
                    .id("learn-scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .child(div().flex().flex_col().gap(px(28.)).children(sections)),
            )
            .into_any_element()
    }
}
