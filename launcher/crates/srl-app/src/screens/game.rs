//! The game screen: while Minecraft runs the launcher steps aside to one full-window state over the
//! mode's key art, with Stop and Back. It goes back by itself when the game exits.
use gpui::{
    AnyElement, Context, Div, FontWeight, ObjectFit, ParentElement, SharedString, Styled, div, img,
    prelude::*, px, svg,
};

use crate::{
    app::Launcher,
    assets::{SHADE_HERO, embedded, icon_path},
    catalog,
    copy::t,
    demo,
    v3::*,
};

/// The logo and wordmark at the nav's place, for the full-window pages that have no nav.
pub fn brand_mark() -> Div {
    div()
        .absolute()
        .left(px(48.))
        .top_0()
        .h(px(68.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.))
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
        ))
}

/// A full-window background: the hero art, the Home shading and an extra `bg` veil so text reads
/// on any render.
pub fn hero_backdrop(hero: Vec<gpui::Img>, veil: f32) -> Vec<AnyElement> {
    let mut v: Vec<AnyElement> = hero.into_iter().map(|i| i.into_any_element()).collect();
    v.push(
        img(embedded(SHADE_HERO))
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .object_fit(ObjectFit::Fill)
            .into_any_element(),
    );
    v.push(overlay(bg3a(veil)).into_any_element());
    v
}

impl Launcher {
    pub fn render_game(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mi = self
            .game_mode
            .map(crate::backend::index)
            .unwrap_or(self.mode_sel);
        let mode = self.mode(mi);
        let d = catalog::def(mode);
        // What the game was started with: the settings may have changed since.
        let launched = self.modes[mi].info.launched.clone();
        let player: SharedString = demo::get()
            .map(|d| d.account.name.clone())
            .or_else(|| launched.as_ref().map(|l| l.player.clone()))
            .or_else(|| self.settings.player_name.clone())
            .unwrap_or_default()
            .into();
        let stopping = self.stopping;
        let stop = play_button(
            "game-stop",
            &if stopping {
                t("game.stopping")
            } else {
                t("game.stop")
            },
            SharedString::default(),
            288.,
            64.,
            19.,
            if stopping { Press::Busy } else { Press::Live },
        )
        .when(!stopping, |b| {
            b.on_click(cx.listener(move |this, _, _, cx| this.cancel(mi, cx)))
        });
        let mem_gb = launched
            .as_ref()
            .map_or(self.settings.memory_mb(self.mode(mi)), |l| l.memory_mb)
            as f32
            / 1024.;
        let memory = crate::copy::tf(
            "settings.memory_value",
            &[("n", &format!("{}", (mem_gb * 10.).round() / 10.))],
        );
        // The tools this game runs with, and what was left out and why: only what the launch
        // reported (nothing while it isn't known).
        let tools: Option<SharedString> = launched.as_ref().map(|l| {
            if l.tools.is_empty() {
                t("game.no_tools")
            } else {
                l.tools
                    .iter()
                    .map(|t| t.info().name)
                    .collect::<Vec<_>>()
                    .join(" · ")
                    .into()
            }
        });
        let microsoft = launched.as_ref().is_some_and(|l| l.microsoft);
        // A sign-in that couldn't be renewed: the game started anyway, and works until then.
        let renew_warning = launched.as_ref().and_then(|l| l.valid_until).map(|until| {
            crate::copy::tf(
                "auth.renew_warning",
                &[("time", &super::account::local_time(until))],
            )
        });
        let notes: Vec<SharedString> = renew_warning
            .into_iter()
            .chain(launched.as_ref().into_iter().flat_map(|l| {
                l.tool_notes
                    .iter()
                    .map(|n| {
                        let (key, args) = n.copy();
                        let args: Vec<(&str, &str)> =
                            args.iter().map(|(k, v)| (*k, v.as_str())).collect();
                        crate::copy::tf(key, &args)
                    })
                    .collect::<Vec<_>>()
            }))
            .collect();

        div()
            .size_full()
            .relative()
            .children(hero_backdrop(self.hero_images_for(mi), 0.35))
            .child(brand_mark())
            .child(
                div()
                    .absolute()
                    .left(px(64.))
                    .top(px(116.))
                    .w(px(560.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .child(div().flex_none().size(px(6.)).mr(px(10.)).bg(ok3()))
                            .child(eyebrow(&t("game.eyebrow"), ok3())),
                    )
                    .child(
                        tight(d.short, 112., FontWeight::EXTRA_BOLD, -0.03, 0.86, text3())
                            .mt(px(16.)),
                    )
                    .child(
                        inter(16., FontWeight::NORMAL, 1.5)
                            .mt(px(20.))
                            .whitespace_nowrap()
                            .text_color(muted3())
                            .child(wrap(&t("game.body"), INTER, FontWeight::NORMAL, 16., 440.)),
                    )
                    .child(
                        div()
                            .mt(px(30.))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(28.))
                            .child(stop)
                            .child(
                                tertiary("game-back", &t("game.back"))
                                    .on_click(cx.listener(|this, _, _, cx| this.leave_game(cx))),
                            ),
                    )
                    .when(!notes.is_empty(), |d| {
                        d.child(div().mt(px(26.)).flex().flex_col().gap(px(6.)).children(
                            notes.into_iter().map(|n| {
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_start()
                                    .child(
                                        div()
                                            .flex_none()
                                            .size(px(5.))
                                            .mt(px(6.))
                                            .mr(px(9.))
                                            .bg(gold()),
                                    )
                                    .child(
                                        inter(13., FontWeight::NORMAL, 1.4)
                                            .whitespace_nowrap()
                                            .text_color(muted3())
                                            .child(wrap(&n, INTER, FontWeight::NORMAL, 13., 520.)),
                                    )
                            }),
                        ))
                    }),
            )
            .child(
                div()
                    .absolute()
                    .left(px(64.))
                    .bottom(px(44.))
                    .flex()
                    .flex_row()
                    .gap(px(36.))
                    .child(fact(&t("game.player"), player, false))
                    .child(fact(
                        &t("game.account"),
                        if microsoft {
                            t("game.microsoft")
                        } else {
                            t("game.offline")
                        },
                        false,
                    ))
                    .child(fact(&t("v3.f_mc"), mode.minecraft_version().into(), true))
                    .child(fact(&t("game.memory"), memory, true))
                    .children(tools.map(|v| fact(&t("game.tools"), v, false))),
            )
            .into_any_element()
    }

    /// The hero of mode `mi` (over the selected mode's, for the same flicker reason as
    /// `hero_images`).
    pub fn hero_images_for(&self, mi: usize) -> Vec<gpui::Img> {
        let hero = |i: usize| {
            img(embedded(&crate::assets::hero_path(catalog::MODES[i].art)))
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .object_fit(ObjectFit::Cover)
        };
        let mut v = Vec::new();
        if mi != self.mode_sel {
            v.push(hero(self.mode_sel));
        }
        v.push(hero(mi));
        v
    }
}
