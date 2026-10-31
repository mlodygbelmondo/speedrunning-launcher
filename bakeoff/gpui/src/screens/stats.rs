//! 8. Stats: Overview tiles and the recent-runs table (Leaderboard tab = Library).
use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled,
    div, prelude::*, px,
};

use crate::{
    app::{Page, Scene},
    copy::{t, tf},
    data::screens,
    theme::*,
    ui::*,
};

fn stat_tile(
    label: SharedString,
    value: &str,
    value_color: Hsla,
    sub: &str,
    sub_color: Hsla,
) -> Div {
    card()
        .flex_1()
        .min_w(px(0.))
        .h(px(112.))
        .flex()
        .flex_col()
        .child(muted_txt(13., label))
        .child(
            mono(28.)
                .mt(px(4.))
                .font_weight(FontWeight::BOLD)
                .line_height(px(34.))
                .text_color(value_color)
                .child(value.to_string()),
        )
        .child(txt(12.).text_color(sub_color).child(sub.to_string()))
}

impl Scene {
    pub fn render_stats(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let st = &screens().stats;
        let tl = &st.tiles;
        let tiles = div()
            .flex_none()
            .flex()
            .flex_row()
            .gap(px(16.))
            .child(stat_tile(
                t("stats.tile_pb"),
                &tl.pb,
                pb(),
                &tl.pb_category,
                muted(),
            ))
            .child(stat_tile(
                t("stats.tile_runs"),
                &tl.runs_month,
                text(),
                &tl.runs_month_sub,
                muted(),
            ))
            .child(stat_tile(
                t("stats.tile_finish"),
                &tl.finished,
                text(),
                &tl.finished_sub,
                muted(),
            ))
            .child(stat_tile(
                t("stats.tile_avg_enter"),
                &tl.avg_enter,
                text(),
                &tl.avg_enter_sub,
                if tl.avg_enter_sub.starts_with('-') {
                    ahead()
                } else {
                    muted()
                },
            ));

        let split_cell = || div().flex_1().min_w(px(0.)).flex().flex_col().items_end();
        let header = div()
            .h(px(32.))
            .flex()
            .flex_row()
            .items_center()
            .font_weight(FontWeight::MEDIUM)
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(muted())
            .child(div().flex_none().w(px(80.)).child(t("stats.col_date")))
            .child(div().flex_none().w(px(90.)).child(t("stats.col_mode")))
            .child(div().flex_none().w(px(170.)).child(t("stats.col_result")))
            .children(
                st.splits
                    .iter()
                    .map(|name| split_cell().child(name.clone())),
            )
            .child(div().flex_none().w(px(90.)));

        let rows = st.runs.iter().enumerate().map(|(i, run)| {
            let result: AnyElement = if run.finished {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        mono(14.)
                            .text_color(if run.pb { pb() } else { text() })
                            .child(run.result.clone()),
                    )
                    .when(run.pb, |d| d.child(badge(t("stats.pb"), Badge::Pb)))
                    .into_any_element()
            } else {
                muted_txt(13., tf("stats.reset_at", &[("split", &run.result)])).into_any_element()
            };
            let submit = run.finished && run.mode == "RSG";
            // 45 px rows: the v3 top nav starts the page at 104 px, so all 8 runs fit.
            div()
                .h(px(45.))
                .flex()
                .flex_row()
                .items_center()
                .border_t_1()
                .border_color(border())
                .child(muted_txt(13., run.date.clone()).flex_none().w(px(80.)))
                .child(txt(13.).flex_none().w(px(90.)).child(run.mode.clone()))
                .child(div().flex_none().w(px(170.)).child(result))
                .children((0..st.splits.len()).map(|k| match run.splits.get(k) {
                    Some((time, delta, is_ahead)) => {
                        split_cell().child(mono(13.).child(time.clone())).when(
                            !delta.is_empty(),
                            |d| {
                                d.child(
                                    mono(11.)
                                        .text_color(if *is_ahead { ahead() } else { behind() })
                                        .child(delta.clone()),
                                )
                            },
                        )
                    }
                    None => split_cell().child(mono(13.).text_color(border()).child("—")),
                }))
                .child(
                    div()
                        .flex_none()
                        .w(px(90.))
                        .flex()
                        .flex_row()
                        .justify_end()
                        .when(submit, |d| {
                            d.child(
                                button(("stats-submit", i), t("stats.submit"), Kind::Secondary)
                                    .small()
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.go(Page::Submit, cx)),
                                    ),
                            )
                        }),
                )
        });

        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(tiles)
            .child(
                card()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .child(card_title(t("stats.recent"), true))
                    .child(header)
                    .children(rows),
            )
            .into_any_element()
    }
}
