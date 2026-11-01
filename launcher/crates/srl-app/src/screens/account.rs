//! The Microsoft account on screen (srl-auth's "For the UI"): the Account section of Settings
//! and the login dialog over any page.
//!
//! - Without a client ID in the build, Settings says so in the player name's hint
//!   (`settings.player_hint_no_login`), with no button that can't work.
//! - With one, `account_section`: each account with its face, the active one, "Log in again" for
//!   a sign-in that can't be renewed, Use and Log out, then Log in with Microsoft (or Add another
//!   account), who plays (`auth.policy`), the file-keystore note and the last error.
//! - `login_dialog`: the browser path and the phone path side by side (QR, address, code, minutes
//!   left), then the chain's steps, Cancel; then the account that logged in, or the error with its
//!   help page. Errors are worded through `AuthError::copy_key` (`AccountProblem::key`).
use std::time::Duration;

use chrono::{DateTime, Utc};
use gpui::{
    ClipboardItem, Context, Div, FontWeight, ParentElement, SharedString, Styled, div, prelude::*,
    px,
};
use srl_auth::LoginStep;

use crate::{
    app::Launcher,
    backend::{
        AccountProblem,
        account::{AccountView, LoginOutcome, LoginView},
    },
    copy::{fill, t, tf},
    v3::*,
};

/// A problem in the player's language: its copy text with `{code}` filled.
pub fn problem_text(p: &AccountProblem) -> SharedString {
    let code = p.code.map(|c| c.to_string()).unwrap_or_default();
    fill(&t(p.key), &[("code", &code)])
}

/// `at` in the player's time zone, "18:40".
pub fn local_time(at: DateTime<Utc>) -> String {
    at.with_timezone(&chrono::Local).format("%H:%M").to_string()
}

/// An account's face, or its initial on a tile when there is none yet (no skin fetched, or the
/// default skin).
pub fn account_face(name: &str, face: Option<&crate::backend::account::Face>, size: f32) -> Div {
    match face {
        Some(f) => pixel_face(f, size),
        None => {
            let initial = name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            div()
                .flex_none()
                .size(px(size))
                .flex()
                .items_center()
                .justify_center()
                .bg(text3a(0.08))
                .child(
                    arch(size * 0.47, FontWeight::BOLD)
                        .text_color(accent2())
                        .child(initial),
                )
        }
    }
}

impl Launcher {
    /// Settings' Microsoft account section (`w` wide), in a build with Microsoft login.
    pub fn account_section(&self, w: f32, cx: &mut Context<Self>) -> Div {
        let a = &self.account;
        let mut rows: Vec<Div> = Vec::new();
        let row = || {
            div()
                .min_h(px(64.))
                .py(px(12.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(14.))
                .border_t_1()
                .border_color(line3())
        };
        for (i, v) in a.accounts.iter().enumerate() {
            let active = a.active == Some(v.uuid);
            let uuid = v.uuid;
            // The action next to it says what to do; the badge says what happened.
            let status = if v.needs_login {
                Some(status_dot(&t("auth.expired"), behind3()))
            } else if active {
                Some(status_dot(&t("auth.active"), ok3()))
            } else {
                None
            };
            let mut actions: Vec<gpui::Stateful<Div>> = Vec::new();
            if v.needs_login {
                actions.push(
                    secondary(("account-relogin", i), &t("auth.needs_login"))
                        .on_click(cx.listener(|this, _, _, _| this.backend().account().login())),
                );
            } else if !active {
                actions.push(tertiary(("account-use", i), &t("auth.use")).on_click(
                    cx.listener(move |this, _, _, _| this.backend().account().set_active(uuid)),
                ));
            }
            actions.push(
                tertiary(("account-logout", i), &t("auth.logout")).on_click(
                    cx.listener(move |this, _, _, _| this.backend().account().logout(uuid)),
                ),
            );
            rows.push(
                row()
                    .child(account_face(&v.name, v.face.as_deref(), 36.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(7.))
                            .child(
                                inter(15., FontWeight::SEMIBOLD, 1.2)
                                    .text_color(text3())
                                    .child(v.name.clone()),
                            )
                            .children(status),
                    )
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .flex_row()
                            .gap(px(22.))
                            .children(actions),
                    ),
            );
        }
        let login_label = if a.accounts.is_empty() {
            t("auth.login")
        } else {
            t("auth.add_account")
        };
        // Signing in is the section's one main action; another account is a lesser one.
        let login = if a.accounts.is_empty() {
            primary("account-login", &login_label)
        } else {
            secondary("account-login", &login_label)
        }
        .on_click(cx.listener(|this, _, _, _| this.backend().account().login()));
        rows.push(
            row()
                .flex_col()
                .items_start()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(5.))
                        .when(a.accounts.is_empty(), |d| {
                            d.child(
                                inter(15., FontWeight::SEMIBOLD, 1.2)
                                    .text_color(text3())
                                    .child(t("auth.not_logged_in")),
                            )
                        })
                        .child(
                            inter(12., FontWeight::NORMAL, 1.4)
                                .whitespace_nowrap()
                                .text_color(muted3())
                                .child(wrap(&t("auth.why"), INTER, FontWeight::NORMAL, 12., w)),
                        ),
                )
                .child(login),
        );
        let note = |s: &str, color: gpui::Hsla| {
            inter(12., FontWeight::NORMAL, 1.4)
                .mt(px(10.))
                .whitespace_nowrap()
                .text_color(color)
                .child(wrap(s, INTER, FontWeight::NORMAL, 12., w))
        };
        div()
            .flex()
            .flex_col()
            .child(section_head(&t("auth.title")).mb(px(14.)))
            .children(rows)
            .child(note(&t("auth.policy"), faint3()))
            .when(a.keystore_file, |d| {
                d.child(note(&t("auth.keystore_file_note"), faint3()))
            })
            .children(a.problem.as_ref().map(|p| {
                div()
                    .flex()
                    .flex_col()
                    .child(note(&problem_text(p), behind3()))
                    .children(self.help_link(p, "account-help", cx).map(|b| b.mt(px(10.))))
            }))
    }

    /// "How to fix it", when the problem has a page that helps.
    fn help_link(
        &self,
        p: &AccountProblem,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<Div>> {
        let url = p.help_url?;
        Some(
            tertiary(id, &t("auth.help"))
                .on_click(cx.listener(move |_, _, _, cx| cx.open_url(url))),
        )
    }

    /// The login dialog over the whole window, while a login is open.
    pub fn login_dialog(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let view = self.account.login.clone()?;
        const W: f32 = 880.;
        // Two columns, the gaps and the divider fill the panel's inside: 2 * 359 + 2 * 32 + 1.
        const COL: f32 = 359.;
        let body: Div = match &view.outcome {
            LoginOutcome::Waiting => self.login_paths(&view, COL, cx),
            LoginOutcome::Done(v) => Self::login_done(v),
            LoginOutcome::Failed(p) => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    inter(22., FontWeight::SEMIBOLD, 1.2)
                        .text_color(text3())
                        .child(t("auth.failed")),
                )
                .child(
                    inter(14., FontWeight::NORMAL, 1.5)
                        .whitespace_nowrap()
                        .text_color(muted3())
                        .child(wrap(
                            &problem_text(p),
                            INTER,
                            FontWeight::NORMAL,
                            14.,
                            W - 96.,
                        )),
                ),
        };
        let mut actions: Vec<gpui::Stateful<Div>> = Vec::new();
        match &view.outcome {
            LoginOutcome::Waiting => actions
                .push(tertiary("login-cancel", &t("auth.cancel")).on_click(
                    cx.listener(|this, _, _, _| this.backend().account().cancel_login()),
                )),
            LoginOutcome::Done(_) => actions.push(
                primary("login-done", &t("auth.done"))
                    .on_click(cx.listener(|this, _, _, _| this.backend().account().close_login())),
            ),
            // A problem with a page that helps (a child account, the Xbox terms) is fixed outside
            // the launcher, so that page is the main action and trying again comes second.
            LoginOutcome::Failed(p) => {
                let retry = |b: gpui::Stateful<Div>| {
                    b.on_click(cx.listener(|this, _, _, _| {
                        let account = this.backend().account();
                        account.close_login();
                        account.login();
                    }))
                };
                match p.help_url {
                    Some(url) => {
                        actions.push(
                            primary("login-help", &t("auth.help"))
                                .on_click(cx.listener(move |_, _, _, cx| cx.open_url(url))),
                        );
                        actions.push(retry(secondary("login-retry", &t("auth.try_again"))));
                    }
                    None => actions.push(retry(primary("login-retry", &t("auth.try_again")))),
                }
                actions.push(
                    tertiary("login-close", &t("auth.close")).on_click(
                        cx.listener(|this, _, _, _| this.backend().account().close_login()),
                    ),
                );
            }
        }
        let steps = (view.outcome == LoginOutcome::Waiting || view.step.is_some())
            .then(|| Self::login_steps(&view));
        let panel = div()
            .w(px(W))
            .flex()
            .flex_col()
            .bg(c(0x14101D))
            .border_1()
            .border_color(text3a(0.12))
            .px(px(48.))
            .pt(px(40.))
            .pb(px(36.))
            .child(section_head(&t("auth.login")).mb(px(26.)))
            .child(body)
            .children(steps)
            .child(
                div()
                    .mt(px(30.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(28.))
                    .children(actions),
            );
        Some(
            div()
                .id("login-veil")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .bg(bg3a(0.82))
                .flex()
                .items_center()
                .justify_center()
                .child(panel)
                .into_any_element(),
        )
    }

    /// Both ways to sign in, side by side.
    fn login_paths(&mut self, view: &LoginView, col: f32, cx: &mut Context<Self>) -> Div {
        let title = |s: SharedString| {
            inter(18., FontWeight::SEMIBOLD, 1.2)
                .text_color(text3())
                .child(s)
        };
        let text = |s: &str, color: gpui::Hsla| {
            inter(13., FontWeight::NORMAL, 1.45)
                .whitespace_nowrap()
                .text_color(color)
                .child(wrap(s, INTER, FontWeight::NORMAL, 13., col))
        };
        // Left: the browser.
        let mut left = div()
            .flex_none()
            .w(px(col))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(title(t("auth.browser_title")));
        match &view.browser {
            Some(b) if b.opened => left = left.child(text(&t("auth.browser_hint"), muted3())),
            Some(b) => {
                let url = b.url.clone();
                let copied = self.link_copied;
                left = left
                    .child(text(&t("auth.browser_not_opened"), muted3()))
                    .child(
                        plex(11., FontWeight::NORMAL)
                            .line_height(px(15.))
                            .text_color(text3a(0.8))
                            .child(link_lines(&b.url, col, 11., 3)),
                    )
                    .child(
                        div().flex().flex_row().child(
                            secondary(
                                "login-copy",
                                &if copied {
                                    t("auth.copied")
                                } else {
                                    t("auth.copy_link")
                                },
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
                                    this.link_copied = true;
                                    cx.notify();
                                },
                            )),
                        ),
                    );
            }
            None => left = left.child(text(&t("auth.waiting"), faint3())),
        }
        if let Some(p) = &view.browser_failed {
            left = left.child(text(&problem_text(p), behind3()));
        }

        // Right: the phone.
        let mut right = div()
            .flex_none()
            .w(px(col))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(title(t("auth.device_title")));
        match (&view.device, &view.device_failed) {
            (_, Some(_)) => right = right.child(text(&t("auth.device_unavailable"), faint3())),
            (Some(d), None) => {
                let left_min = (d.expires_at - Utc::now()).num_seconds();
                let expiry = if left_min > 0 {
                    tf(
                        "auth.device_expires",
                        &[("minutes", &((left_min + 59) / 60).to_string())],
                    )
                } else {
                    t("auth.device_expired")
                };
                self.tick_each_minute(left_min, cx);
                let qr = d.qr.as_ref().map(|m| qr_code(m, 164.).0);
                let host = d
                    .verification_uri
                    .trim_start_matches("https://")
                    .trim_start_matches("http://");
                right = right.child(
                    div().flex().flex_row().gap(px(20.)).children(qr).child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .child(
                                inter(13., FontWeight::NORMAL, 1.45)
                                    .whitespace_nowrap()
                                    .text_color(muted3())
                                    .child(wrap(
                                        &tf("auth.device_hint", &[("url", host)]),
                                        INTER,
                                        FontWeight::NORMAL,
                                        13.,
                                        col - 184.,
                                    )),
                            )
                            .child(
                                plex(26., FontWeight::SEMIBOLD)
                                    .text_color(text3())
                                    .child(spaced_code(&d.user_code)),
                            )
                            .child(
                                inter(12., FontWeight::NORMAL, 1.)
                                    .text_color(faint3())
                                    .child(expiry),
                            ),
                    ),
                );
            }
            (None, None) => right = right.child(text(&t("auth.waiting"), faint3())),
        }
        div()
            .flex()
            .flex_row()
            .gap(px(32.))
            .child(left)
            .child(div().flex_none().w(px(1.)).bg(line3()))
            .child(right)
    }

    /// Re-renders when the code's minutes-left label changes (once a minute, only while the
    /// dialog shows a code), so an open dialog stays idle in between.
    fn tick_each_minute(&mut self, seconds_left: i64, cx: &mut Context<Self>) {
        if self.login_tick || seconds_left <= 0 {
            return;
        }
        self.login_tick = true;
        let wait = Duration::from_secs((seconds_left % 60) as u64 + 1);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |app, cx| {
                app.login_tick = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn login_done(v: &AccountView) -> Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(20.))
            .child(account_face(&v.name, v.face.as_deref(), 56.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        inter(22., FontWeight::SEMIBOLD, 1.2)
                            .text_color(text3())
                            .child(tf("auth.logged_in", &[("name", &v.name)])),
                    )
                    .child(
                        inter(14., FontWeight::NORMAL, 1.4)
                            .text_color(muted3())
                            .child(t("auth.logged_in_hint")),
                    ),
            )
    }

    /// The chain after the Microsoft sign-in: done steps bright, the current one in the accent,
    /// the rest faint.
    fn login_steps(view: &LoginView) -> Div {
        let at = match view.step {
            None => 0,
            Some(LoginStep::SignedIn(_)) => 1,
            Some(LoginStep::Xbox) => 2,
            Some(LoginStep::Minecraft) => 3,
            Some(LoginStep::Profile) => 4,
        };
        let done_all = matches!(view.outcome, LoginOutcome::Done(_));
        let failed = matches!(view.outcome, LoginOutcome::Failed(_));
        let labels = [
            "auth.step_signed_in",
            "auth.step_xbox",
            "auth.step_minecraft",
            "auth.step_profile",
        ];
        div()
            .mt(px(30.))
            .pt(px(18.))
            .border_t_1()
            .border_color(line3())
            .flex()
            .flex_row()
            .flex_wrap()
            .gap_x(px(24.))
            .gap_y(px(8.))
            .children(labels.iter().enumerate().map(|(i, key)| {
                let n = i + 1;
                let done = n < at || (n == at && (done_all || n == 1));
                let (color, mark) = if done {
                    (text3(), ok3())
                } else if n == at && failed {
                    (behind3(), behind3())
                } else if n == at {
                    (accent2(), accent2())
                } else {
                    (faint3(), text3a(0.16))
                };
                // "Checking Xbox Live…" only while it runs; the other steps lose the dots, and the
                // step that failed says so ("Xbox Live refused") instead of still being in progress.
                let label = if n == at && failed && !done {
                    t(&format!("{key}_failed"))
                } else if n == at && !failed && !done {
                    t(key)
                } else {
                    t(key).trim_end_matches('…').to_string().into()
                };
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(div().flex_none().size(px(7.)).mr(px(8.)).bg(mark))
                    .child(
                        inter(12., FontWeight::MEDIUM, 1.)
                            .text_color(color)
                            .child(label),
                    )
            }))
    }
}

/// The phone code in groups of four, the way Microsoft's page shows it ("ABCD 1234").
fn spaced_code(code: &str) -> String {
    let chars: Vec<char> = code.chars().collect();
    chars
        .chunks(4)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `s` cut to `max` characters with "…", for a link shown in full only in the clipboard.
fn ellipsis(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

/// A link in the mono font broken anywhere into at most `lines` lines of `width` px: the login
/// link's query has no place a word wrap could break, so it would run out of the dialog. The
/// copy button copies it whole. (IBM Plex Mono's advance is 0.6 em.)
fn link_lines(url: &str, width: f32, size_px: f32, lines: usize) -> String {
    let per_line = ((width / (size_px * 0.6)).floor() as usize).max(4);
    let chars: Vec<char> = ellipsis(url, per_line * lines - 1).chars().collect();
    chars
        .chunks(per_line)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_read_in_groups() {
        assert_eq!(spaced_code("ABCD1234"), "ABCD 1234");
        assert_eq!(spaced_code("RW7KQ2PXZ"), "RW7K Q2PX Z");
        assert_eq!(ellipsis("abcdef", 3), "abc…");
        // 34 px of 11 px mono (6.6 px a character): five characters a line, the last line ends in "…".
        assert_eq!(link_lines("abcdefghijklmnopq", 34., 11., 2), "abcde\nfghi…");
        assert_eq!(link_lines("abc", 34., 11., 2), "abc");
    }

    /// Every srl-auth text is in the app's copy in both languages, word for word, so
    /// `AuthError::copy_key` always finds its text.
    #[test]
    fn srl_auth_copy_is_merged() {
        let pairs = [
            (
                include_str!("../../../srl-auth/copy/en.json"),
                include_str!("../../../../assets/copy.json"),
            ),
            (
                include_str!("../../../srl-auth/copy/pl.json"),
                include_str!("../../../../assets/copy.pl.json"),
            ),
        ];
        for (auth, app) in pairs {
            let auth: std::collections::HashMap<String, String> =
                serde_json::from_str(auth).unwrap();
            let app: std::collections::HashMap<String, String> = serde_json::from_str(app).unwrap();
            for (k, v) in auth.iter().filter(|(k, _)| !k.starts_with('_')) {
                assert_eq!(app.get(k), Some(v), "{k}");
            }
        }
    }
}
