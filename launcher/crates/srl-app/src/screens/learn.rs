//! Learn: other runners' guides, credited and linked (`assets/learn.json`). The topics sit on the
//! left in two groups (every 1.16.1 run, then the modes with their own rules), like the mode list
//! on Modes; the selected topic's guides fill the right, in Practice's row style, over the topic's
//! card art. A row opens its guide in the browser; nothing is embedded or played here. Picking a
//! topic is instant (no transition), so idle stays idle.
use gpui::{
    AnyElement, Context, Div, FontWeight, ParentElement, SharedString, Styled, div, prelude::*, px,
    svg,
};
use srl_core::Mode;

use crate::{
    app::{Launcher, trace},
    assets::{card_path, icon_path},
    catalog::{self, Guide, LEARN_GROUPS, Topic},
    copy::{t, tf, tn},
    v3::*,
};

use super::practice::{art_band, page_title};

/// Height of the "more below" fade over a list's bottom edge (`v3::fade_below`).
const FADE_H: f32 = 40.;

/// Width of the topic column, and the gap to the guides.
const TOPICS_W: f32 = 316.;
const TOPICS_GAP: f32 = 36.;
const TOPIC_H: f32 = 48.;
const ROW_H: f32 = 52.;

/// "RSG · FSG · Ranked": the display names of mode ids.
fn mode_names(ids: &[String]) -> String {
    ids.iter()
        .filter_map(|id| Mode::from_id(id))
        .map(|m| catalog::def(m).short)
        .collect::<Vec<_>>()
        .join(" · ")
}

/// How long a guide is, when we know: minutes or the number of parts.
fn length(g: &Guide) -> Option<SharedString> {
    match (g.minutes, g.parts) {
        (Some(m), _) => Some(tn("learn.minutes", u64::from(m))),
        (None, Some(p)) => Some(tn("learn.parts", u64::from(p))),
        _ => None,
    }
}

impl Launcher {
    fn learn_topic_row(
        &self,
        ti: usize,
        topic: &Topic,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let on = ti == self.learn_topic;
        div()
            .id(("learn-topic", ti))
            .group("learn-topic")
            .relative()
            .flex_none()
            .h(px(TOPIC_H))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.learn_topic != ti {
                    this.learn_topic = ti;
                    this.learn_scroll.set_offset(gpui::Point::default());
                    cx.notify();
                }
            }))
            .child(overlay(tile_bg()))
            .child(art_fill(card_path(&topic.art)))
            // Modes' row shading, darker where the name sits.
            .map(|e| {
                if on {
                    e.child(shade_right(0., 0.6, 0.9, 0.55))
                        .child(shade_right(0.6, 1.0, 0.55, 0.3))
                } else {
                    e.child(shade_right(0., 0.6, 0.95, 0.8))
                        .child(shade_right(0.6, 1.0, 0.8, 0.62))
                }
            })
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(18.))
                    .right(px(16.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        arch(16., FontWeight::BOLD)
                            .text_color(if on { text3() } else { muted3() })
                            .child(t(&format!("learn.topic.{}", topic.id))),
                    )
                    .child(
                        plex(11., FontWeight::MEDIUM)
                            .text_color(if on { muted3() } else { faint3() })
                            .child(tn("learn.count", topic.guides.len() as u64)),
                    ),
            )
            .child(if on {
                ring()
            } else {
                hover_ring("learn-topic")
            })
    }

    fn learn_topics(&self, cx: &mut Context<Self>) -> Div {
        let topics = &catalog::learn().topics;
        let mut groups = Vec::new();
        for group in LEARN_GROUPS {
            let mut col = div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(section_head(&t(&format!("learn.group.{group}"))).mb(px(6.)));
            for (ti, tp) in topics
                .iter()
                .enumerate()
                .filter(|(_, tp)| tp.group == group)
            {
                col = col.child(self.learn_topic_row(ti, tp, cx));
            }
            groups.push(col);
        }
        div()
            .flex_none()
            .w(px(TOPICS_W))
            .flex()
            .flex_col()
            .gap(px(24.))
            .children(groups)
    }

    /// One guide: its title and what it covers, who made it, what it is and how long, and Open. The whole
    /// row opens the guide's page in the browser.
    fn guide_row(gi: usize, g: &Guide, cx: &mut Context<Self>) -> impl IntoElement {
        let url = g.url.clone();
        // Under the author: the system it is for, or the one mode of its topic it is for.
        let scope: Option<SharedString> = g
            .platform
            .as_ref()
            .map(|p| t(&format!("learn.platform.{p}")))
            .or_else(|| (!g.modes.is_empty()).then(|| mode_names(&g.modes).into()));
        // Under the kind: its length and date, as far as known ("25 min · 2026-04").
        let facts = [length(g), g.date.clone().map(SharedString::from)]
            .into_iter()
            .flatten()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let facts = (!facts.is_empty()).then(|| SharedString::from(facts.join(" · ")));
        div()
            .id(("learn-guide", gi))
            .group("learn-guide")
            .relative()
            .flex_none()
            .h(px(ROW_H))
            .pl(px(4.))
            .pr(px(4.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(20.))
            .border_t_1()
            .border_color(line3())
            .cursor_pointer()
            .hover(|s| s.bg(text3a(0.04)))
            .on_click(cx.listener(move |_, _, _, cx| {
                trace(format_args!("open_url {url}"));
                cx.open_url(&url);
            }))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        inter(14., FontWeight::SEMIBOLD, 1.25)
                            .text_color(text3())
                            .truncate()
                            .child(SharedString::from(g.title.clone())),
                    )
                    .child(
                        inter(12., FontWeight::NORMAL, 1.3)
                            .text_color(muted3())
                            .truncate()
                            .child(t(&format!("learn.g.{}", g.id))),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(176.))
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(
                        inter(13., FontWeight::MEDIUM, 1.2)
                            .text_color(text3())
                            .truncate()
                            .child(SharedString::from(g.author.clone())),
                    )
                    .children(scope.map(|s| {
                        inter(12., FontWeight::MEDIUM, 1.)
                            .text_color(gold())
                            .truncate()
                            .child(s)
                    })),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(124.))
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(6.))
                    .child(label(&t(&format!("learn.kind.{}", g.kind))))
                    .children(
                        facts.map(|f| plex(11., FontWeight::MEDIUM).text_color(muted3()).child(f)),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(88.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .gap(px(7.))
                    .child(
                        inter(13., FontWeight::SEMIBOLD, 1.)
                            .whitespace_nowrap()
                            .text_color(accent2())
                            .group_hover("learn-guide", |s| s.text_color(text3()))
                            .child(t("learn.open")),
                    )
                    .child(
                        svg()
                            .path(icon_path("external-link"))
                            .flex_none()
                            .size(px(13.))
                            .text_color(accent2())
                            .group_hover("learn-guide", |s| s.text_color(text3())),
                    ),
            )
    }

    pub fn render_learn(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let topics = &catalog::learn().topics;
        let ti = self.learn_topic.min(topics.len() - 1);
        let topic = &topics[ti];
        let name = t(&format!("learn.topic.{}", topic.id));

        let head = div()
            .flex_none()
            .flex()
            .flex_col()
            .child(kicker(
                &tf("learn.for", &[("modes", &mode_names(&topic.modes))]),
                accent2(),
            ))
            .child(tight(&name, 40., FontWeight::EXTRA_BOLD, -0.025, 0.95, text3()).mt(px(14.)))
            .child(
                inter(14., FontWeight::NORMAL, 1.45)
                    .mt(px(12.))
                    .text_color(muted3())
                    .child(t(&format!("learn.topic.{}.desc", topic.id))),
            );

        let rows: Vec<_> = topic
            .guides
            .iter()
            .enumerate()
            .map(|(gi, g)| Self::guide_row(gi, g, cx).into_any_element())
            .collect();
        let guides = div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .child(head)
            .child(
                div()
                    .flex_none()
                    .mt(px(24.))
                    .pb(px(12.))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .child(section_head(&format!(
                        "{name} · {}",
                        tn("learn.count", topic.guides.len() as u64)
                    ))),
            )
            // The rows scroll once they don't fit, the closing rule stays right under the last one. While
            // rows are hidden below, the bottom of the list fades out ("scroll for more"), as in
            // the credits.
            .child(
                div()
                    .relative()
                    .flex_initial()
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id(("learn-list", ti))
                            .flex_initial()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .track_scroll(&self.learn_scroll)
                            .child(div().flex().flex_col().children(rows)),
                    )
                    .child(fade_below(self.learn_scroll.clone(), FADE_H)),
            )
            .child(div().flex_none().border_t_1().border_color(line3()));

        div()
            .size_full()
            .relative()
            .child(art_band(&topic.art))
            .child(
                div()
                    .absolute()
                    .left(px(64.))
                    .right(px(48.))
                    .top(px(104.))
                    .bottom(px(28.))
                    .flex()
                    .flex_col()
                    .child(page_title(&t("learn.title"), &t("learn.subtitle")))
                    .child(
                        div()
                            .mt(px(30.))
                            .flex_1()
                            .min_h(px(0.))
                            .flex()
                            .flex_row()
                            .gap(px(TOPICS_GAP))
                            .child(self.learn_topics(cx))
                            .child(guides),
                    ),
            )
            .into_any_element()
    }
}
