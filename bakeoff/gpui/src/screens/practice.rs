//! Practice, scene v3 (REDESIGN.md "Practice"): the category grid and the selected category's map
//! list (or the practice tools), over the category's art.
use gpui::{
    AnyElement, Context, Div, FontWeight, ObjectFit, ParentElement, SharedString, Styled, div, img,
    prelude::*, px,
};

use crate::{
    app::{Install, Item, Scene},
    assets::card_path,
    copy::{t, tf},
    data::screens,
    v3::*,
};

/// What the map list shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PracticeSel {
    Category(usize),
    Tools,
}

const GRID_GAP: f32 = 12.;
const ROW_H: f32 = 128.;

struct RowInfo<'a> {
    item: Item,
    name: &'a str,
    authors: &'a str,
    desc: &'a str,
    meta: String,
    status: &'a str,
    /// Card art for the 84×46 thumbnail.
    thumb: &'a str,
}

impl Scene {
    fn practice_art(&self, cat: usize) -> &'static str {
        let s = screens();
        let id = &s.practice_categories[cat].id;
        s.v3.practice_art
            .get(id)
            .map(String::as_str)
            .unwrap_or("overworld")
    }

    fn practice_count(n: usize) -> SharedString {
        if n == 1 {
            t("v3.practice_count_one")
        } else {
            tf("v3.practice_count", &[("n", &n.to_string())])
        }
    }

    /// A tile's sub line, wrapped like the browser at the tile's text width and clamped to two
    /// lines. The grid is 1.5fr 1fr 1fr 1fr over 1168 px with 12 px gaps, text inset 16 px.
    fn tile_sub(sub: &str, big: bool) -> SharedString {
        let fr = (1168. - 3. * GRID_GAP) / 4.5;
        let w = if big { 1.5 * fr } else { fr } - 32.;
        let wrapped = wrap(sub, PLEX, FontWeight::MEDIUM, 11., w);
        let mut lines = wrapped.split('\n');
        match (lines.next(), lines.next()) {
            (Some(a), Some(b)) => format!("{a}\n{b}").into(),
            _ => wrapped.clone(),
        }
    }

    fn cat_tile(
        &self,
        sel: PracticeSel,
        title: SharedString,
        sub: SharedString,
        art: Option<&'static str>,
        big: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let on = self.practice_sel == sel;
        let id = match sel {
            PracticeSel::Category(i) => i,
            PracticeSel::Tools => 99,
        };
        div()
            .id(("practice-cat", id))
            .group("practice-cat")
            .relative()
            .size_full()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.practice_sel != sel {
                    this.practice_sel = sel;
                    cx.notify();
                }
            }))
            .map(|d| match art {
                Some(a) => d
                    .child(overlay(tile_bg()))
                    .child(art_fill(card_path(a)))
                    .when(!on, |d| d.child(overlay(bg3a(0.34)))),
                None => d.child(
                    overlay(gpui::rgb(0x17131F).into())
                        .border_1()
                        .border_color(line3()),
                ),
            })
            // The Tools tile is flat: no dim, no gradient.
            .when(art.is_some(), |d| d.child(shade_up(0.9, 0.0, 0.65)))
            .child(
                div()
                    .absolute()
                    .left(px(16.))
                    .right(px(16.))
                    .bottom(px(14.))
                    .flex()
                    .flex_col()
                    .child(
                        arch(if big { 30. } else { 20. }, FontWeight::BOLD)
                            .text_color(text3())
                            .child(title),
                    )
                    .child(
                        plex(11., FontWeight::MEDIUM)
                            .mt(px(6.))
                            .line_height(px(11. * 1.3))
                            .text_color(muted3())
                            .child(Self::tile_sub(&sub, big)),
                    ),
            )
            .child(if on { ring() } else { hover_ring("practice-cat") })
    }

    fn practice_row(&self, info: RowInfo<'_>, cx: &mut Context<Self>) -> Div {
        let item = info.item;
        let state = self.install_state(item, info.status == "installed");
        let (label, color) = match state {
            Install::Done => (t("common.installed"), ok3()),
            Install::Running(_) => (t("common.installing"), faint3()),
            Install::Not if info.status == "update" => (t("common.update"), text3()),
            Install::Not => (t("common.install"), accent2()),
        };
        let idx = match item {
            Item::Map(i) => i,
            Item::PracticeTool(i) => 1000 + i,
            _ => 2000,
        };
        let action = caps(&label, 12., FontWeight::BOLD, 0.16, color)
            .id(("practice-act", idx))
            .when(matches!(state, Install::Not), |d| {
                d.cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.start_install(item, cx)))
            });
        let thumb = div()
            .relative()
            .flex_none()
            .w(px(84.))
            .h(px(46.))
            .child(overlay(tile_bg()))
            .child(art_fill(card_path(info.thumb)));
        div()
            .relative()
            .flex_none()
            .h(px(58.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(20.))
            .border_t_1()
            .border_color(line3())
            .child(thumb)
            .child(
                div()
                    .flex_none()
                    .w(px(250.))
                    .flex()
                    .flex_col()
                    .child(
                        inter(15., FontWeight::SEMIBOLD, 1.2)
                            .text_color(text3())
                            .truncate()
                            .child(SharedString::from(info.name.to_string())),
                    )
                    .child(
                        inter(12., FontWeight::NORMAL, 1.35)
                            .text_color(muted3())
                            .truncate()
                            .child(SharedString::from(info.authors.to_string())),
                    ),
            )
            .child(
                inter(13., FontWeight::NORMAL, 1.35)
                    .flex_1()
                    .min_w(px(0.))
                    .text_color(muted3())
                    .child(SharedString::from(info.desc.to_string())),
            )
            .child(
                plex(12., FontWeight::MEDIUM)
                    .flex_none()
                    .w(px(130.))
                    .flex()
                    .justify_end()
                    .text_color(faint3())
                    .child(SharedString::from(info.meta)),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(150.))
                    .flex()
                    .justify_end()
                    .child(action),
            )
            .when_some(
                match state {
                    Install::Running(f) => Some(f),
                    _ => None,
                },
                |d, f| {
                    d.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .child(thin_progress(f, 2.)),
                    )
                },
            )
    }

    pub fn render_practice(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let s = screens();
        let cats = &s.practice_categories;
        let count = |ci: usize| s.maps.iter().filter(|m| m.category == cats[ci].id).count();

        // Background: the selected category's art over the top 360 px at 22 % (bg @ 78 %),
        // fading into bg. Tools has no art of its own and keeps the first category's.
        let bg_art = match self.practice_sel {
            PracticeSel::Category(i) => self.practice_art(i),
            PracticeSel::Tools => self.practice_art(0),
        };

        let big_sub = s
            .maps
            .iter()
            .find(|m| m.category == cats[0].id)
            .map(|m| format!("{} · {}", m.name, m.desc))
            .unwrap_or_default();
        let tool_names = s
            .practice_tools
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(" · ");

        // Grid: 1.5fr 1fr 1fr 1fr, two 128 px rows; the first category spans both rows.
        let small = |ci: usize, cx: &mut Context<Self>| {
            self.cat_tile(
                PracticeSel::Category(ci),
                cats[ci].name.clone().into(),
                Self::practice_count(count(ci)),
                Some(self.practice_art(ci)),
                false,
                cx,
            )
        };
        let col = |a: gpui::Stateful<Div>, b: gpui::Stateful<Div>| {
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(GRID_GAP))
                .child(div().h(px(ROW_H)).child(a))
                .child(div().h(px(ROW_H)).child(b))
        };
        let tools_tile = self.cat_tile(
            PracticeSel::Tools,
            t("v3.practice_tools_tile"),
            tool_names.into(),
            None,
            false,
            cx,
        );
        let grid = div()
            .mt(px(30.))
            .flex_none()
            .h(px(2. * ROW_H + GRID_GAP))
            .flex()
            .flex_row()
            .gap(px(GRID_GAP))
            .child(div().h_full().flex_basis(px(0.)).flex_grow(1.5).map(|d| {
                d.child(self.cat_tile(
                    PracticeSel::Category(0),
                    cats[0].name.clone().into(),
                    big_sub.into(),
                    Some(self.practice_art(0)),
                    true,
                    cx,
                ))
            }))
            .child(col(small(1, cx), small(4, cx)).flex_basis(px(0.)))
            .child(col(small(2, cx), small(5, cx)).flex_basis(px(0.)))
            .child(col(small(3, cx), tools_tile).flex_basis(px(0.)));

        // The list for the selection, most popular first.
        let (title, rows): (SharedString, Vec<Div>) = match self.practice_sel {
            PracticeSel::Category(ci) => {
                let maps: Vec<(usize, &crate::data::MapItem)> = s
                    .maps
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.category == cats[ci].id)
                    .collect();
                let art = self.practice_art(ci);
                let n = maps.len();
                let rows = maps
                    .into_iter()
                    .map(|(i, m)| {
                        self.practice_row(
                            RowInfo {
                                item: Item::Map(i),
                                name: &m.name,
                                authors: &m.authors,
                                desc: &m.desc,
                                meta: if m.downloads.is_empty() {
                                    m.version.clone()
                                } else {
                                    format!("{} · {}", m.version, m.downloads)
                                },
                                status: &m.status,
                                thumb: art,
                            },
                            cx,
                        )
                    })
                    .collect();
                (
                    tf(
                        "v3.practice_list",
                        &[
                            ("category", &cats[ci].name),
                            ("count", &Self::practice_count(n)),
                        ],
                    ),
                    rows,
                )
            }
            PracticeSel::Tools => {
                let n = s.practice_tools.len();
                let rows = s
                    .practice_tools
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        self.practice_row(
                            RowInfo {
                                item: Item::PracticeTool(i),
                                name: &p.name,
                                authors: &p.authors,
                                desc: &p.desc,
                                meta: p.version.clone(),
                                status: &p.status,
                                // The web build gives tool rows the overworld card.
                                thumb: "overworld",
                            },
                            cx,
                        )
                    })
                    .collect();
                (
                    tf(
                        "v3.practice_list",
                        &[
                            ("category", &t("v3.practice_tools_tile")),
                            ("count", &Self::practice_count(n)),
                        ],
                    ),
                    rows,
                )
            }
        };

        // The header, then the rows (they scroll once they don't fit), then the tip right under
        // the last row.
        let list = div()
            .mt(px(30.))
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .pb(px(12.))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .child(cap_label(&title))
                    .child(cap_label(&t("v3.practice_sort"))),
            )
            .child(
                div()
                    .id("practice-list")
                    .flex_initial()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .child(div().flex().flex_col().children(rows)),
            )
            .child(
                inter(12., FontWeight::NORMAL, 1.4)
                    .flex_none()
                    .mt(px(10.))
                    .text_color(faint3())
                    .child(t("practice.ranked_tip")),
            );

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(360.))
                    .overflow_hidden()
                    // `background: center 40% / cover`: the 16:9 card at the window width is
                    // 720 px tall, and 40 % of the 360 px overflow is 144 px.
                    .child(
                        img(card_path(bg_art))
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(px(-144.))
                            .h(px(720.))
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(overlay(bg3a(0.78)))
                    .child(shade_up(1.0, 0.0, 1.0)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(64.))
                    .right(px(48.))
                    .top(px(104.))
                    .bottom(px(28.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .flex_row()
                            .items_end()
                            .justify_between()
                            .child(tight(
                                &t("practice.title"),
                                56.,
                                FontWeight::EXTRA_BOLD,
                                -0.025,
                                0.9,
                                text3(),
                            ))
                            .child(
                                inter(14., FontWeight::NORMAL, 1.45)
                                    .whitespace_nowrap()
                                    .text_right()
                                    .text_color(muted3())
                                    .child(wrap(
                                        &t("practice.subtitle"),
                                        INTER,
                                        FontWeight::NORMAL,
                                        14.,
                                        420.,
                                    )),
                            ),
                    )
                    .child(grid)
                    .child(list),
            )
            .into_any_element()
    }
}
