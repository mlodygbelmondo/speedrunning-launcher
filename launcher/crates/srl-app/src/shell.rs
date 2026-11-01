//! The app shell (REDESIGN.md "Shell: top nav"): a 68 px bar over every page except the game
//! screen and the name prompt, with no background of its own. When a new version of the launcher
//! is ready (or out, to download by hand) the gear carries an accent square, as VS Code's does:
//! Settings → About says what and offers "Restart to update". A text notice doesn't fit the bar
//! at 1280 px in Polish.
use gpui::{Context, Div, FontWeight, ParentElement, Styled, div, prelude::*, px, svg};

use crate::{
    app::{Launcher, Page},
    assets::icon_path,
    backend::UpdateState,
    copy::{t, tf},
    demo,
    v3::*,
};

/// Nav items, in order: copy key and page.
const NAV: [(&str, Page); 6] = [
    ("nav.play", Page::Home),
    ("nav.instances", Page::Instances),
    ("nav.practice", Page::Practice),
    ("nav.learn", Page::Learn),
    ("nav.tools", Page::Tools),
    ("nav.stats", Page::Stats),
];

impl Launcher {
    /// The account block: who plays. With the demo, the prototype's Ranked player; with an active
    /// Microsoft account (`account::play_as`), its name and face, marked "Microsoft account";
    /// otherwise the offline name, marked as offline, with its initial in place of a skin (an
    /// offline account has none).
    fn account_block(&self) -> Div {
        let (name, line, avatar): (String, Div, Div) = match demo::get() {
            Some(d) => {
                let r = &d.ranked;
                let line = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .flex_none()
                            .size(px(7.))
                            .mr(px(6.))
                            .bg(tier_color(&r.tier)),
                    )
                    .child(
                        plex(11., FontWeight::MEDIUM)
                            .line_height(px(13.2))
                            .text_color(muted3())
                            .child(tf(
                                "nav.account_line",
                                &[
                                    ("elo", &r.elo.to_string()),
                                    ("tier", &t(&format!("tier.{}", r.tier))),
                                ],
                            )),
                    );
                (
                    d.account.name.clone(),
                    line,
                    pixel_avatar(&d.account.avatar, 32.),
                )
            }
            None if self.account.playing().is_some() => {
                let a = self.account.playing().expect("checked");
                let line = inter(12., FontWeight::MEDIUM, 1.)
                    .line_height(px(13.2))
                    .text_color(muted3())
                    .child(t("nav.microsoft"));
                (
                    a.name.clone(),
                    line,
                    crate::screens::account_face(&a.name, a.face.as_deref(), 32.),
                )
            }
            None => {
                let name = self.settings.player_name.clone().unwrap_or_default();
                let initial = name
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                let line = inter(12., FontWeight::MEDIUM, 1.)
                    .line_height(px(13.2))
                    .text_color(muted3())
                    .child(t("nav.offline"));
                let avatar = div()
                    .flex_none()
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(text3a(0.08))
                    .child(
                        arch(15., FontWeight::BOLD)
                            .text_color(accent2())
                            .child(initial),
                    );
                (name, line, avatar)
            }
        };
        div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(18.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .child(
                        inter(13., FontWeight::SEMIBOLD, 1.2)
                            .text_color(text3())
                            .child(name),
                    )
                    .child(line),
            )
            .child(avatar)
    }

    pub fn top_nav(&self, cx: &mut Context<Self>) -> Div {
        let active = self.shown;

        let brand = div()
            .id("nav-brand")
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.navigate(Page::Home, cx)))
            .child(
                svg()
                    .path(icon_path("logo"))
                    .flex_none()
                    .size(px(22.))
                    .text_color(accent2()),
            )
            .child(caps(
                "Speedrunning Launcher",
                13.,
                FontWeight::BOLD,
                0.16,
                text3(),
            ));

        let nav = div()
            .flex_none()
            .h_full()
            .flex()
            .flex_row()
            .gap(px(28.))
            .children(NAV.iter().map(|&(key, page)| {
                let on = active == page;
                div()
                    .id(key)
                    .group("nav-item")
                    .relative()
                    .h_full()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.navigate(page, cx)))
                    .child(caps_hover(
                        &t(key),
                        12.,
                        FontWeight::SEMIBOLD,
                        0.16,
                        if on { text3() } else { muted3() },
                        text3(),
                        "nav-item",
                    ))
                    .when(on, |d| {
                        // The tracking after the last letter isn't part of the word.
                        d.child(
                            div()
                                .absolute()
                                .left_0()
                                .right(px(0.16 * 12.))
                                .bottom(px(18.))
                                .h(px(2.))
                                .bg(accent2()),
                        )
                    })
            }));

        let update = matches!(
            self.update,
            UpdateState::Ready { .. } | UpdateState::Manual { .. }
        );
        let gear = div()
            .id("nav-settings")
            .group("nav-gear")
            .relative()
            .flex_none()
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.navigate(Page::Settings, cx)))
            .child(
                svg()
                    .path(icon_path("settings"))
                    .size(px(18.))
                    .text_color(if active == Page::Settings {
                        text3()
                    } else {
                        muted3()
                    })
                    .group_hover("nav-gear", |s| s.text_color(text3())),
            )
            .when(update, |d| {
                d.child(
                    div()
                        .absolute()
                        .top(px(-4.))
                        .right(px(-5.))
                        .size(px(10.))
                        .border_2()
                        .border_color(bg3())
                        .bg(accent2()),
                )
            });

        // Accounts isn't built yet; the player name lives in Settings, so the block opens it.
        let me = self
            .account_block()
            .id("nav-account")
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.navigate(Page::Settings, cx)));

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(68.))
            .px(px(48.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(44.))
            .child(brand)
            .child(nav)
            .child(
                div()
                    .ml_auto()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(18.))
                    .child(gear)
                    .child(me),
            )
    }
}
