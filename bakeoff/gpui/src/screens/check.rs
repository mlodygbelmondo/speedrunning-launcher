//! 4. Pre-launch check: modal over Home, opened by Play (scene v3).
//!
//! GPUI has no group opacity; `.opacity()` multiplies the alpha of every primitive in the subtree,
//! which is exactly "interpolate bg/border/text alpha". Overlapping children blend slightly
//! differently mid-fade than a composited layer would.
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, BoxShadow, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled,
    div, point, prelude::*, px,
};

use crate::{
    app::{MODAL_IN_MS, MODAL_OUT_MS, Modal, Page, Scene},
    copy::{fill, t, t_opt},
    data::screens,
    theme::*,
    ui::*,
};

impl Scene {
    pub fn close_modal(&mut self, cx: &mut Context<Self>) {
        if self.modal.as_ref().is_some_and(|m| m.open) {
            self.modal = Some(Modal {
                open: false,
                start: Instant::now(),
            });
            cx.notify();
        }
    }

    fn check_warn_left(&self) -> bool {
        screens()
            .checks
            .iter()
            .zip(&self.check_fixed)
            .any(|(c, fixed)| c.status == "warn" && !fixed)
    }

    fn launch_game(&mut self, cx: &mut Context<Self>) {
        self.go(Page::Game, cx);
    }

    pub fn render_check_modal(
        &mut self,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let m = self.modal.as_ref()?;
        let e = ms_since(m.start, now);
        let k = if m.open {
            ease_out((e / MODAL_IN_MS).min(1.0))
        } else {
            1.0 - ease_in((e / MODAL_OUT_MS).min(1.0))
        };
        let s = screens();
        let mode = &s.modes[self.check_mode].name;

        let rows = s.checks.iter().enumerate().map(|(i, c)| {
            let args: Vec<(&str, &str)> = c
                .args
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let fixed = self.check_fixed[i];
            let warn_now = c.status == "warn" && !fixed;
            let detail_key = if fixed {
                format!("check.{}_fixed", c.id)
            } else {
                format!("check.{}_detail", c.id)
            };
            let action = warn_now
                .then(|| t_opt(&format!("check.{}_action", c.id)))
                .flatten()
                .map(|a| {
                    button(("check-fix", i), fill(&a, &args), Kind::Secondary)
                        .small()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.check_fixed[i] = true;
                            cx.notify();
                        }))
                });
            div()
                .min_h(px(52.))
                .py(px(8.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .when(i > 0, |d| d.border_t_1().border_color(border()))
                .child(status_icon(
                    if warn_now { Status::Warn } else { Status::Ok },
                    20.,
                ))
                .child(
                    txt(14.)
                        .flex_none()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(t(&format!("check.{}", c.id))),
                )
                .child(
                    muted_txt(13., fill(&t(&detail_key), &args))
                        .flex_1()
                        .min_w(px(0.)),
                )
                .children(action)
        });

        let warn_left = self.check_warn_left();
        let primary: SharedString = if warn_left {
            t("check.fix_and_launch")
        } else {
            t("check.launch")
        };

        let modal = div()
            .id("check-modal")
            .occlude()
            .relative()
            .top(px(8. * (1. - k)))
            .w(px(600.))
            .p(px(24.))
            .rounded(px(16.))
            .bg(surface())
            .border_1()
            .border_color(border())
            .shadow(vec![BoxShadow {
                color: alpha(black(), 0.5),
                offset: point(px(0.), px(16.)),
                blur_radius: px(48.),
                spread_radius: px(0.),
                inset: false,
            }])
            .opacity(k)
            .flex()
            .flex_col()
            .child(
                txt(20.)
                    .font_weight(FontWeight::BOLD)
                    .child(t("check.title")),
            )
            .child(muted_txt(13., fill(&t("check.subtitle"), &[("mode", mode)])).mt(px(4.)))
            .child(div().mt(px(12.)).flex().flex_col().children(rows))
            .child(
                div()
                    .mt(px(16.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .child(muted_txt(12., t("check.footer")).flex_1().min_w(px(0.)))
                    .child(
                        button("check-cancel", t("common.cancel"), Kind::Ghost)
                            .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
                    )
                    .child(
                        button("check-go", primary, Kind::Primary).on_click(cx.listener(
                            move |this, _, _, cx| {
                                if this.check_warn_left() {
                                    // Fix every warn row instantly, wait 400 ms, then launch.
                                    this.check_fixed.iter_mut().for_each(|f| *f = true);
                                    cx.notify();
                                    cx.spawn(async move |this, cx| {
                                        cx.background_executor()
                                            .timer(Duration::from_millis(400))
                                            .await;
                                        let _ = this.update(cx, |this, cx| this.launch_game(cx));
                                    })
                                    .detach();
                                } else {
                                    this.launch_game(cx);
                                }
                            },
                        )),
                    ),
            );

        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .child(
                    div()
                        .id("check-overlay")
                        .occlude()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .bg(alpha(c(0x050408), 0.7 * k))
                        .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(modal),
                )
                .into_any_element(),
        )
    }
}
