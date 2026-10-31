//! Scenario driver (SPEC.md "Scenario mode"): wall-clock timers on the foreground executor.
//! Same events and timings as scene v1.
use std::time::{Duration, Instant};

use gpui::{AsyncApp, WeakEntity};
use serde_json::json;

use crate::{
    app::{Page, Recording, Scene},
    report::{epoch_ms, report},
};

fn phase(name: &str) -> Instant {
    report(json!({"ev": "phase", "name": name, "t": epoch_ms()}));
    Instant::now()
}

async fn sleep_until(cx: &mut AsyncApp, deadline: Instant) {
    let d = deadline.saturating_duration_since(Instant::now());
    if !d.is_zero() {
        cx.background_executor().timer(d).await;
    }
}

pub async fn run_scenario(
    this: WeakEntity<Scene>,
    window: gpui::AnyWindowHandle,
    ms: Duration,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    // 1. idle: Home static, glow off, no frames requested.
    this.update(cx, |s, cx| s.set_glow(false, cx))?;
    let t0 = phase("idle");
    sleep_until(cx, t0 + ms).await;

    // 2. transitions: glow on, toggle Home <-> Library every 1000 ms, frame loop all phase long.
    this.update(cx, |s, cx| s.set_glow(true, cx))?;
    let t0 = phase("transitions");
    this.update(cx, |s, cx| {
        s.recording = Some(Recording {
            start: t0,
            ts: Vec::with_capacity(4096),
        });
        cx.notify();
    })?;
    let end = t0 + ms;
    let mut k = 1u32;
    loop {
        let next = t0 + Duration::from_millis(1000) * k;
        if next >= end {
            break;
        }
        sleep_until(cx, next).await;
        this.update(cx, |s, cx| {
            let to = if s.target == Page::Home {
                Page::Library
            } else {
                Page::Home
            };
            s.navigate(to, cx);
        })?;
        k += 1;
    }
    sleep_until(cx, end).await;
    let ts = this.update(cx, |s, _| {
        s.recording.take().map(|r| r.ts).unwrap_or_default()
    })?;
    report(json!({"ev": "frames", "phase": "transitions", "ts": ts}));

    // 3. scroll: Library, 1200 px/s time-based, bouncing between top and bottom.
    let t0 = phase("scroll");
    this.update(cx, |s, cx| {
        s.recording = Some(Recording {
            start: t0,
            ts: Vec::with_capacity(4096),
        });
        s.scroll_start = Some(t0);
        s.navigate(Page::Library, cx);
        cx.notify();
    })?;
    sleep_until(cx, t0 + ms).await;
    let ts = this.update(cx, |s, _| {
        s.scroll_start = None;
        s.recording.take().map(|r| r.ts).unwrap_or_default()
    })?;
    report(json!({"ev": "frames", "phase": "scroll", "ts": ts}));

    // 4. minimized: glow keeps "running"; count renders to see whether frames are throttled.
    let t0 = phase("minimized");
    window.update(cx, |_, window, _| window.minimize_window())?;
    this.update(cx, |s, cx| {
        s.minimized_frames = Some(0);
        cx.notify();
    })?;
    sleep_until(cx, t0 + ms).await;
    let (count, active) = window.update(cx, |_, window, cx| {
        let n = this
            .update(cx, |s, _| s.minimized_frames.take().unwrap_or(0))
            .unwrap_or(0);
        (n, window.is_window_active())
    })?;
    report(
        json!({"ev": "info", "phase": "minimized", "raf_count": count, "window_active": active}),
    );

    // 5. destroyed: remove the window, keep the process alive (QuitMode::Explicit).
    this.update(cx, |s, cx| s.set_glow(false, cx))?;
    let t0 = phase("destroyed");
    window.update(cx, |_, window, _| window.remove_window())?;
    sleep_until(cx, t0 + ms).await;
    let windows_left = cx.update(|cx| cx.windows().len());
    report(json!({"ev": "info", "phase": "destroyed", "windows_left": windows_left}));

    // 6. exit.
    report(json!({"ev": "done", "t": epoch_ms()}));
    cx.update(|cx| cx.quit());
    Ok(())
}
