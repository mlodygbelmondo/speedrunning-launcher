//! The player-name field of the first-run prompt and of Settings. GPUI has no text input, and a
//! Minecraft name is plain ASCII, so the field takes key events directly: printable characters,
//! Backspace, Enter and Ctrl+V. It accepts anything typed (up to 24 characters) and shows whether
//! it's a valid name (`OfflineAccount::is_valid_name`), so a space or an accent is explained
//! instead of silently dropped. The caret doesn't blink: a focused field is still idle.
//!
//! The Settings field saves when the edit ends (Enter, the field losing focus, leaving Settings),
//! not on every key: deleting a name to retype it would otherwise save each shorter prefix that is
//! still valid ("Rev" on the way down from "Review_Bot"). An edit that ends invalid or empty puts
//! the saved name back, so the field and the saved setting never disagree.
//!
//! The same field takes the MCSR Ranked and Draftout names on Settings (`Field::Ranked`,
//! `Field::Draftout`): a Minecraft name or UUID the APIs can look up (`srl_stats::is_lookup_name`),
//! and an edit that ends empty clears the name.
use gpui::{
    Context, Div, FontWeight, KeyDownEvent, ParentElement, SharedString, Styled, Window, div,
    prelude::*, px,
};
use srl_core::OfflineAccount;

use crate::{
    app::{Launcher, NameField},
    backend::{Settings, stats},
    copy::t,
    v3::*,
};

/// Longer input is never valid; stop there so the field doesn't overflow.
const MAX_TYPED: usize = 24;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Prompt,
    Settings,
    /// The MCSR Ranked name (Settings, Statistics).
    Ranked,
    /// The Draftout name (Settings, Statistics).
    Draftout,
}

impl Field {
    /// Whether `text` can be saved in this field.
    fn valid(self, text: &str) -> bool {
        match self {
            Field::Prompt | Field::Settings => OfflineAccount::is_valid_name(text),
            Field::Ranked | Field::Draftout => srl_stats::is_lookup_name(text),
        }
    }
}

impl Launcher {
    fn field(&mut self, which: Field) -> &mut NameField {
        match which {
            Field::Prompt => &mut self.name_prompt,
            Field::Settings => &mut self.name_settings,
            Field::Ranked => &mut self.ranked_field,
            Field::Draftout => &mut self.draftout_field,
        }
    }

    fn field_ref(&self, which: Field) -> &NameField {
        match which {
            Field::Prompt => &self.name_prompt,
            Field::Settings => &self.name_settings,
            Field::Ranked => &self.ranked_field,
            Field::Draftout => &self.draftout_field,
        }
    }

    fn name_key(&mut self, which: Field, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        let m = &ks.modifiers;
        let paste = (m.control || m.platform) && ks.key == "v";
        let field = self.field(which);
        let before = field.text.clone();
        match ks.key.as_str() {
            "backspace" => {
                field.text.pop();
            }
            "enter" if which == Field::Prompt => {
                cx.stop_propagation();
                self.submit_name(cx);
                return;
            }
            "enter" if which == Field::Settings => {
                cx.stop_propagation();
                self.commit_settings_name(cx);
                return;
            }
            "enter" => {
                cx.stop_propagation();
                self.commit_stats_name(which, cx);
                return;
            }
            _ if paste => {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    for ch in text.trim().chars() {
                        if field.text.chars().count() < MAX_TYPED && !ch.is_control() {
                            field.text.push(ch);
                        }
                    }
                }
            }
            _ if !m.control && !m.platform => {
                if let Some(typed) = &ks.key_char {
                    for ch in typed.chars() {
                        if field.text.chars().count() < MAX_TYPED && !ch.is_control() {
                            field.text.push(ch);
                        }
                    }
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        if field.text != before {
            cx.notify();
        }
    }

    /// Ends an edit of the Settings name: saves it if it's a valid name, else shows the saved
    /// name again.
    pub fn commit_settings_name(&mut self, cx: &mut Context<Self>) {
        let text = self.name_settings.text.clone();
        if OfflineAccount::is_valid_name(&text) {
            let settings = Settings {
                player_name: Some(text),
                ..self.settings.clone()
            };
            self.save_settings(settings, cx);
        } else {
            self.name_settings.text = self.settings.player_name.clone().unwrap_or_default();
            cx.notify();
        }
    }

    /// Ends an edit of a stats name: saves it if the APIs can look it up, clears it if the field
    /// is empty, else shows the saved name again. A new name is looked up at once (from the
    /// cache; the Stats page asks the network when it opens).
    pub fn commit_stats_name(&mut self, which: Field, cx: &mut Context<Self>) {
        let text = self.field(which).text.trim().to_string();
        let mut stats = self.settings.stats.clone();
        let slot = match which {
            Field::Ranked => &mut stats.ranked_nickname,
            Field::Draftout => &mut stats.draftout_username,
            Field::Prompt | Field::Settings => return,
        };
        if text.is_empty() || which.valid(&text) {
            *slot = (!text.is_empty()).then_some(text);
        }
        self.field(which).text = slot.clone().unwrap_or_default();
        if stats != self.settings.stats {
            let settings = Settings {
                stats,
                ..self.settings.clone()
            };
            self.save_settings(settings, cx);
            self.request_stats(srl_stats::Refresh::Never);
        }
        cx.notify();
    }

    /// The field: `w`×`h`, Plex Mono at `text_px`, a 1 px `line` border that turns `accent-2`
    /// while focused.
    pub fn name_input(
        &self,
        which: Field,
        w: f32,
        h: f32,
        text_px: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let field = self.field_ref(which);
        let focused = field.focus.is_focused(window);
        let empty = field.text.is_empty();
        let text: SharedString = if !empty {
            field.text.clone().into()
        } else if matches!(which, Field::Ranked | Field::Draftout) {
            // The name the lookup falls back to (the Microsoft account's), else "not set".
            stats::active_microsoft_name(&self.account)
                .map(SharedString::from)
                .unwrap_or_else(|| t("settings.stats_name_unset"))
        } else {
            t("name.placeholder")
        };
        let focus = field.focus.clone();
        div()
            .id(match which {
                Field::Prompt => "name-prompt",
                Field::Settings => "name-settings",
                Field::Ranked => "name-ranked",
                Field::Draftout => "name-draftout",
            })
            .track_focus(&field.focus)
            .on_key_down(
                cx.listener(move |this, ev: &KeyDownEvent, _, cx| this.name_key(which, ev, cx)),
            )
            .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                window.focus(&focus, cx);
                // The Settings page ends the edit on a click anywhere else; not on this one.
                cx.stop_propagation();
            })
            .cursor_text()
            .flex_none()
            .w(px(w))
            .h(px(h))
            .px(px(14.))
            .flex()
            .flex_row()
            .items_center()
            .bg(text3a(0.06))
            .border_1()
            .border_color(if focused { accent2() } else { text3a(0.16) })
            .when(!empty, |d| {
                d.child(
                    plex(text_px, FontWeight::MEDIUM)
                        .text_color(text3())
                        .child(text.clone()),
                )
            })
            .when(focused, |d| {
                d.child(
                    div()
                        .flex_none()
                        .w(px(2.))
                        .h(px(text_px * 1.2))
                        .ml(px(1.))
                        .bg(accent2()),
                )
            })
            .when(empty, |d| {
                d.child(
                    plex(text_px, FontWeight::NORMAL)
                        .ml(px(if focused { 6. } else { 0. }))
                        .text_color(faint3())
                        .child(text),
                )
            })
    }

    /// True while the name typed in `which` breaks the naming rule (an empty field doesn't).
    pub fn name_breaks_rule(&self, which: Field) -> bool {
        let text = &self.field_ref(which).text;
        !text.is_empty() && !which.valid(text)
    }

    /// The line under a field: the naming rule, in `behind` while the typed name breaks it.
    /// Wrapped at `max_w`, since the Polish hints are longer than the space next to the field.
    pub fn name_rule(&self, which: Field, hint: Option<SharedString>, max_w: f32) -> Div {
        let bad = self.name_breaks_rule(which);
        let rule = match which {
            Field::Prompt | Field::Settings => t("name.rule"),
            Field::Ranked | Field::Draftout => t("settings.stats_name_rule"),
        };
        let (msg, color) = match (bad, hint) {
            (true, _) => (rule, behind3()),
            (false, Some(h)) => (h, faint3()),
            (false, None) => (rule, faint3()),
        };
        inter(12., FontWeight::NORMAL, 1.4)
            .whitespace_nowrap()
            .text_color(color)
            .child(wrap(&msg, INTER, FontWeight::NORMAL, 12., max_w))
    }
}
