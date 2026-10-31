//! 1. Onboarding (full window, no rail): step list on the left, a 600 px content column on the
//! right, footer pinned 40 px from the bottom. Step state survives Back/Next.
use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::*, px, svg,
};

use crate::{
    app::{Page, SIGNIN_ONBOARDING, Scene, Signin, framed_image},
    assets::icon_path,
    copy::{t, tf},
    data::{os, screens},
    theme::*,
    ui::*,
};

const STEPS: [&str; 7] = ["welcome", "signin", "mode", "java", "eye", "obs", "done"];

impl Scene {
    fn ob_tools_summary(&self) -> String {
        let tools = &screens().tools;
        let name = |id: &'static str| -> &'static str {
            tools
                .iter()
                .find(|tool| tool.id == id)
                .map(|tool| tool.name.as_str())
                .unwrap_or(id)
        };
        let eye = if os() == "windows" {
            "toolscreen"
        } else {
            "waywall"
        };
        format!("{}, {}", name(eye), name("ninjabrain"))
    }

    fn ob_left_panel(&self) -> Div {
        let step = self.ob_step;
        let rows = STEPS.iter().enumerate().map(|(i, id)| {
            let done = i < step;
            let current = i == step;
            div()
                .h(px(40.))
                .px(px(12.))
                .rounded(px(10.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .when(current, |d| d.bg(raised()))
                .child(
                    div()
                        .flex_none()
                        .relative()
                        .size(px(10.))
                        .bg(if done || current { accent() } else { border() })
                        .when(current, |d| {
                            // 2 px ring outside the square, so labels stay aligned.
                            d.child(
                                div()
                                    .absolute()
                                    .top(px(-2.))
                                    .left(px(-2.))
                                    .size(px(14.))
                                    .border_2()
                                    .border_color(alpha(accent(), 0.3)),
                            )
                        }),
                )
                .child(
                    txt(14.)
                        .flex_1()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if current { text() } else { muted() })
                        .child(t(&format!("onboarding.steps.{id}"))),
                )
                .when(done, |d| d.child(icon("check", 14., ahead())))
        });
        div()
            .flex_none()
            .w(px(340.))
            .h_full()
            .bg(surface())
            .border_r_1()
            .border_color(border())
            .p(px(32.))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        svg()
                            .path(icon_path("logo"))
                            .flex_none()
                            .size(px(28.))
                            .text_color(accent()),
                    )
                    // The product name is not copy (SCREENS.md).
                    .child(
                        txt(18.)
                            .font_weight(FontWeight::BOLD)
                            .child("Speedrunluncher"),
                    ),
            )
            .child(div().mt(px(40.)).flex().flex_col().children(rows))
            .child(div().mt_auto().child(muted_txt(
                12.,
                tf(
                    "onboarding.step_of",
                    &[
                        ("n", &(step + 1).to_string()),
                        ("total", &STEPS.len().to_string()),
                    ],
                ),
            )))
    }

    fn ob_next(&mut self, cx: &mut Context<Self>) {
        if self.ob_step + 1 < STEPS.len() {
            self.ob_step += 1;
            cx.notify();
        }
    }

    fn ob_back(&mut self, cx: &mut Context<Self>) {
        if self.ob_step > 0 {
            self.ob_step -= 1;
            cx.notify();
        }
    }

    fn ob_mode_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let modes = &screens().modes;
        let n = modes.len();
        div()
            .bg(surface())
            .border_1()
            .border_color(border())
            .rounded(px(14.))
            .flex()
            .flex_col()
            .children(modes.iter().enumerate().map(|(i, m)| {
                let sel = i == self.ob_mode;
                div()
                    .id(("ob-mode", i))
                    .min_h(px(64.))
                    .px(px(18.))
                    .py(px(12.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .cursor_pointer()
                    .when(i > 0, |d| d.border_t_1().border_color(border()))
                    .when(i == 0, |d| d.rounded_t(px(13.)))
                    .when(i + 1 == n, |d| d.rounded_b(px(13.)))
                    .map(|d| {
                        if sel {
                            d.bg(alpha(accent(), 0.10))
                        } else {
                            d.hover(|s| s.bg(alpha(raised(), 0.6)))
                        }
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.ob_mode = i;
                        cx.notify();
                    }))
                    .child(icon_tile(&m.icon, 36., 18., 10.))
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
                                            .child(m.name.clone()),
                                    )
                                    .when(m.recommended, |d| {
                                        d.child(badge(t("common.recommended"), Badge::Accent))
                                    })
                                    .child(muted_txt(12., m.full.clone())),
                            )
                            .child(muted_txt(13., m.desc.clone())),
                    )
                    .child(badge_mono(m.mc.clone(), Badge::Muted))
                    .child(radio(sel))
            }))
            .into_any_element()
    }

    fn ob_content(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let s = screens();
        match self.ob_step {
            1 => self.signin_block(SIGNIN_ONBOARDING, 4.5, cx),
            2 => self.ob_mode_list(cx),
            3 => card()
                .py_0()
                .child(setting_row(
                    t("onboarding.java.java_label"),
                    None,
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .child(muted_txt(13., t("onboarding.java.java_value")))
                        .child(status_icon(Status::Ok, 16.)),
                    true,
                ))
                .child(setting_row(
                    t("onboarding.java.memory_label"),
                    Some(t("onboarding.java.memory_hint")),
                    self.segmented(
                        "ob-memory",
                        s.settings
                            .memory
                            .iter()
                            .map(|m| SharedString::from(m.clone()))
                            .collect(),
                        self.memory,
                        cx,
                        |this, i, _, _| this.memory = i,
                    ),
                    false,
                ))
                .into_any_element(),
            4 => {
                let rows: [(&str, Option<&str>, bool); 3] = [
                    (
                        "onboarding.eye.windows",
                        Some("tools.windows_only"),
                        os() != "windows",
                    ),
                    (
                        "onboarding.eye.linux",
                        Some("tools.linux_only"),
                        os() == "windows",
                    ),
                    ("onboarding.eye.ninjabrain", None, false),
                ];
                card()
                    .py_0()
                    .children(
                        rows.into_iter()
                            .enumerate()
                            .map(|(i, (key, plat, other_os))| {
                                div()
                                    .id(("ob-eye", i))
                                    .h(px(56.))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(12.))
                                    .cursor_pointer()
                                    .when(i > 0, |d| d.border_t_1().border_color(border()))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.ob_eye[i] = !this.ob_eye[i];
                                        cx.notify();
                                    }))
                                    .child(checkbox(self.ob_eye[i]))
                                    .child(
                                        txt(14.)
                                            .flex_1()
                                            .min_w(px(0.))
                                            .text_color(if other_os { muted() } else { text() })
                                            .child(t(key)),
                                    )
                                    .when_some(plat, |d, p| d.child(badge(t(p), Badge::Muted)))
                            }),
                    )
                    .into_any_element()
            }
            5 => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    card()
                        .py_0()
                        .child(
                            div()
                                .h(px(56.))
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(10.))
                                .child(status_icon(Status::Ok, 20.))
                                .child(txt(14.).font_weight(FontWeight::MEDIUM).child(tf(
                                    "onboarding.obs.found",
                                    &[("version", &s.onboarding.obs_version)],
                                ))),
                        )
                        .child(setting_row(
                            t("onboarding.obs.add"),
                            None,
                            self.toggle("onboarding.obs", cx),
                            false,
                        )),
                )
                .child(div().flex().flex_row().ml(px(-18.)).child(
                    button("ob-no-obs", t("onboarding.obs.no_obs"), Kind::Ghost).on_click(
                        cx.listener(|this, _, _, cx| this.set_toggle("onboarding.obs", false, cx)),
                    ),
                ))
                .into_any_element(),
            6 => {
                let mode = &s.modes[self.ob_mode];
                let java = format!(
                    "{} · {}",
                    s.layer("java").detail.clone().unwrap_or_default(),
                    s.settings.memory[self.memory]
                );
                let rows: [(SharedString, String); 3] = [
                    (t("nav.modes"), mode.name.clone()),
                    (t("onboarding.java.java_label"), java),
                    (t("nav.tools"), self.ob_tools_summary()),
                ];
                card()
                    .py_0()
                    .children(rows.into_iter().enumerate().map(|(i, (label, value))| {
                        div()
                            .h(px(40.))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .when(i > 0, |d| d.border_t_1().border_color(border()))
                            .child(muted_txt(13., label))
                            .child(txt(14.).child(value))
                    }))
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }

    fn ob_footer(&mut self, cx: &mut Context<Self>) -> Div {
        let step = self.ob_step;
        let s = screens();
        let back = (step != 0 && step != 6).then(|| {
            div().ml(px(-18.)).child(
                button("ob-back", t("common.back"), Kind::Ghost)
                    .icon("chevron-left")
                    .on_click(cx.listener(|this, _, _, cx| this.ob_back(cx))),
            )
        });
        let next_enabled =
            step != 1 || self.ob_signin_unlocked || self.signin[SIGNIN_ONBOARDING] == Signin::Done;
        let next = |label: SharedString, cx: &mut Context<Self>| {
            button("ob-next", label, Kind::Primary)
                .disabled(!next_enabled)
                .on_click(cx.listener(|this, _, _, cx| this.ob_next(cx)))
        };
        let right = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .map(|d| match step {
                0 => d.child(next(t("onboarding.welcome.cta"), cx)),
                4 => d
                    .child(
                        button("ob-later", t("onboarding.eye.later"), Kind::Ghost)
                            .on_click(cx.listener(|this, _, _, cx| this.ob_next(cx))),
                    )
                    .child(next(t("common.next"), cx).trailing("chevron-right")),
                6 => {
                    let mode = s.modes[self.ob_mode].name.clone();
                    d.child(
                        button("ob-learn", t("onboarding.done.learn"), Kind::Secondary)
                            .on_click(cx.listener(|this, _, _, cx| this.go(Page::Learn, cx))),
                    )
                    .child(
                        button(
                            "ob-launch",
                            tf("onboarding.done.launch", &[("mode", &mode)]),
                            Kind::Primary,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.go(Page::Home, cx))),
                    )
                }
                _ => d.child(next(t("common.next"), cx).trailing("chevron-right")),
            });
        div()
            .w(px(600.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .h(px(40.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .children(back)
                    .child(div().flex_1())
                    .child(right),
            )
            .when(step == 0, |d| {
                d.child(div().flex().flex_row().ml(px(-18.)).child(button(
                    "ob-have-setup",
                    t("onboarding.welcome.have_setup"),
                    Kind::Ghost,
                )))
            })
    }

    pub fn render_onboarding(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let id = STEPS[self.ob_step];
        let s = screens();
        let body = if id == "done" {
            tf(
                "onboarding.done.body",
                &[("mode", &s.modes[self.ob_mode].name)],
            )
        } else {
            t(&format!("onboarding.{id}.body"))
        };
        let column = div()
            .w(px(600.))
            .flex()
            .flex_col()
            .when(id == "welcome", |d| {
                d.child(
                    div()
                        .relative()
                        .w(px(600.))
                        .h(px(180.))
                        .mb(px(24.))
                        .children(framed_image("hero.jpg", 14.)),
                )
            })
            .when(id == "done", |d| {
                d.child(
                    div()
                        .size(px(64.))
                        .mb(px(24.))
                        .rounded_full()
                        .bg(alpha(accent(), 0.18))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon("check", 32., accent())),
                )
            })
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(28.))
                    .line_height(px(36.))
                    .child(t(&format!("onboarding.{id}.title"))),
            )
            .child(
                div()
                    .mt(px(10.))
                    .max_w(px(560.))
                    .text_size(px(15.))
                    .line_height(px(22.))
                    .text_color(muted())
                    .child(body),
            )
            .child(div().mt(px(28.)).child(self.ob_content(cx)));

        div()
            .size_full()
            .flex()
            .flex_row()
            .child(self.ob_left_panel())
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .relative()
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .pt(px(72.))
                            .child(column),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom(px(40.))
                            .flex()
                            .justify_center()
                            .child(self.ob_footer(cx)),
                    )
                    // Mojang usage guidelines (REDESIGN.md "Disclaimer and credits"): centred
                    // under the content column, at the bottom of the window.
                    .when(id == "welcome", |d| {
                        d.child(
                            crate::v3::inter(11., FontWeight::NORMAL, 16. / 11.)
                                .absolute()
                                .left_0()
                                .right_0()
                                .bottom(px(12.))
                                .text_center()
                                .text_color(crate::v3::faint3())
                                .child(t("settings.disclaimer")),
                        )
                    }),
            )
            .into_any_element()
    }
}
