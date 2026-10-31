//! 11. Game running (full window, no rail): session card plus a fake taskbar and tray menu.
use gpui::{
    AnyElement, BoxShadow, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled,
    div, point, prelude::*, px, svg,
};

use crate::{
    app::{Page, Scene},
    assets::icon_path,
    copy::{t, tf},
    data::screens,
    theme::*,
    ui::*,
};

const TASKBAR_H: f32 = 36.0;
const CLOCK_RIGHT: f32 = 16.0;
/// "21:37" in JetBrains Mono 12 (0.6 em advance) = 36 px; then a 14 px gap to our tray tile.
const TRAY_TILE_RIGHT: f32 = CLOCK_RIGHT + 36.0 + 14.0;

impl Scene {
    pub fn render_game(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let g = &screens().game;
        let mini = |label: SharedString, value: &str, color: gpui::Hsla| {
            div()
                .flex_1()
                .min_w(px(0.))
                .h(px(84.))
                .p(px(14.))
                .rounded(px(12.))
                .bg(raised())
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(muted_txt(12., label))
                .child(
                    mono(22.)
                        .font_weight(FontWeight::BOLD)
                        .text_color(color)
                        .child(value.to_string()),
                )
        };
        let centre = card()
            .w(px(620.))
            .p(px(28.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .child(div().flex_none().size(px(10.)).bg(ahead()))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(28.))
                            .line_height(px(36.))
                            .child(t("game.title")),
                    ),
            )
            .child(muted_txt(
                13.,
                tf("game.subtitle", &[("mode", &g.mode), ("ago", &g.started)]),
            ))
            .child(div().mt(px(24.)).child(card_title(t("game.session"), true)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(12.))
                    .child(mini(t("game.resets"), &g.resets, text()))
                    .child(mini(t("game.best"), &g.best, pb()))
                    .child(mini(t("game.nether_entries"), &g.nether_entries, text())),
            )
            .child(
                div()
                    .mt(px(24.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(16.))
                    .child(
                        button("game-stop", t("game.stop"), Kind::Danger)
                            .icon("circle-x")
                            .on_click(cx.listener(|this, _, _, cx| this.go(Page::Home, cx))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap(px(8.))
                            .child(icon("info", 16., muted()))
                            .child(muted_txt(12., t("game.window_note")).flex_1().min_w(px(0.))),
                    ),
            );

        let item = |id: &'static str,
                    icon_name: &'static str,
                    key: &'static str,
                    danger: bool,
                    cx: &mut Context<Self>| {
            div()
                .id(id)
                .h(px(36.))
                .px(px(10.))
                .rounded(px(8.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.))
                .cursor_pointer()
                .hover(|s| s.bg(surface()))
                .child(icon(icon_name, 16., muted()))
                .child(
                    txt(13.)
                        .text_color(if danger { behind() } else { text() })
                        .child(t(key)),
                )
                .on_click(cx.listener(move |this, _, _, cx| match id {
                    "tray-show" | "tray-stop" => this.go(Page::Home, cx),
                    _ => this.show_toast(key, cx),
                }))
        };
        let tray_menu = div()
            .w(px(264.))
            .p(px(6.))
            .rounded(px(12.))
            .bg(raised())
            .border_1()
            .border_color(border())
            .shadow(vec![BoxShadow {
                color: alpha(black(), 0.5),
                offset: point(px(0.), px(12.)),
                blur_radius: px(32.),
                spread_radius: px(0.),
                inset: false,
            }])
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(40.))
                    .px(px(10.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        svg()
                            .path(icon_path("logo"))
                            .flex_none()
                            .size(px(16.))
                            .text_color(accent()),
                    )
                    .child(
                        txt(13.)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tf("tray.status", &[("mode", &g.mode)])),
                    ),
            )
            .child(hline().my(px(4.)))
            .child(item("tray-show", "house", "tray.show", false, cx))
            .child(item(
                "tray-ninjabrain",
                "eye",
                "tray.ninjabrain",
                false,
                cx,
            ))
            .child(hline().my(px(4.)))
            .child(item("tray-stop", "circle-x", "tray.stop", true, cx))
            .child(item("tray-quit", "power", "tray.quit", false, cx));

        let taskbar = div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .h(px(TASKBAR_H))
            .bg(c(0x050408))
            .border_t_1()
            .border_color(border())
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .pr(px(CLOCK_RIGHT))
            .gap(px(14.))
            .child(icon("globe", 16., muted()))
            .child(icon("video", 16., muted()))
            .child(icon("monitor", 16., muted()))
            .child(
                div()
                    .flex_none()
                    .size(px(28.))
                    .rounded(px(6.))
                    .bg(raised())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(icon_path("logo"))
                            .size(px(16.))
                            .text_color(accent()),
                    ),
            )
            .child(
                mono(12.)
                    .w(px(36.))
                    .text_color(muted())
                    // A fake clock, allowed as non-copy by SCREENS.md.
                    .child("21:37"),
            );

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .pb(px(80.))
                    .child(centre),
            )
            .child(taskbar)
            .child(
                div()
                    .absolute()
                    .right(px(TRAY_TILE_RIGHT))
                    .bottom(px(TASKBAR_H + 8.))
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(6.))
                    .child(muted_txt(11., t("game.tray_title")))
                    .child(tray_menu),
            )
            .into_any_element()
    }
}
