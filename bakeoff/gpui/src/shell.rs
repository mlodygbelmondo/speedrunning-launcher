//! Scene v3 app shell (REDESIGN.md "Shell: top nav"): a 68 px bar over every page except
//! Onboarding and Game, with no background of its own.
use gpui::{Context, Div, FontWeight, ParentElement, Styled, div, prelude::*, px, svg};

use crate::{
    app::{Page, Scene},
    assets::icon_path,
    copy::{t, tf},
    data::screens,
    v3::*,
};

/// Nav items, in order: copy key and page.
const NAV: [(&str, Page); 6] = [
    ("nav.play", Page::Home),
    ("nav.modes", Page::Modes),
    ("nav.practice", Page::Practice),
    ("nav.learn", Page::Learn),
    ("nav.tools", Page::Tools),
    ("nav.stats", Page::Stats),
];

impl Scene {
    /// The page whose nav item is underlined (Stats stays active on Library and Submit).
    fn nav_active(&self) -> Option<Page> {
        match self.shown {
            Page::Library | Page::Submit | Page::Stats => Some(Page::Stats),
            p @ (Page::Home | Page::Modes | Page::Practice | Page::Learn | Page::Tools) => Some(p),
            _ => None,
        }
    }

    pub fn top_nav(&self, cx: &mut Context<Self>) -> Div {
        let s = screens();
        let active = self.nav_active();
        let account = s
            .accounts
            .list
            .iter()
            .find(|a| a.active)
            .unwrap_or(&s.accounts.list[0]);
        let r = &s.v3.ranked;

        let brand = div()
            .id("nav-brand")
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.go(Page::Home, cx)))
            .child(
                svg()
                    .path(icon_path("logo"))
                    .flex_none()
                    .size(px(22.))
                    .text_color(accent2()),
            )
            .child(caps(
                "Speedrunluncher",
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
                let on = active == Some(page);
                div()
                    .id(key)
                    .group("nav-item")
                    .relative()
                    .h_full()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.go(page, cx)))
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

        let gear = div()
            .id("nav-settings")
            .group("nav-gear")
            .flex_none()
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.go(Page::Settings, cx)))
            .child(
                svg()
                    .path(icon_path("settings"))
                    .size(px(18.))
                    .text_color(if self.shown == Page::Settings {
                        text3()
                    } else {
                        muted3()
                    })
                    .group_hover("nav-gear", |s| s.text_color(text3())),
            );

        let me = div()
            .id("nav-account")
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(18.))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.go(Page::Accounts, cx)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .child(
                        inter(13., FontWeight::SEMIBOLD, 1.2)
                            .text_color(text3())
                            .child(account.name.clone()),
                    )
                    .child(
                        div()
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
                            ),
                    ),
            )
            .child(pixel_avatar(&account.avatar, 32.));

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
