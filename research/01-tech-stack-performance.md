# 01: Tech stack and performance research

**Project:** Speedrunluncher, a desktop Minecraft launcher for speedrunners (MCSR: RSG, FSG, MCSR Ranked)
**Date:** 2026-10-23
**Status:** research only. Nothing was prototyped and no downloaded binary was run.

**Method**
- Seven parallel research passes covered:
  - webviews and Tauri
  - Electron, CEF and Wails
  - Rust-native GUI toolkits
  - Flutter, Qt, Avalonia and the JVM toolkits
  - existing launchers and the Celestial teardown
  - the launcher core and reusable cores
  - measurement tooling
- I then cross-checked them myself with `gh`, npm, crates.io, `apt-cache`, the WebKit source and spot checks of the cited GitHub issues.

**How to read the sources**
- Every factual claim has a URL next to it.
- "(local)" means I checked it on this Ubuntu 26.04 machine.
- **UNVERIFIED** marks anything I could not confirm.
- The shared web-search quota ran out late in the research. The final checks therefore used GitHub, crates.io, npm and direct page fetches only.

**Conventions**
- Frame budgets:
  - 144 Hz is 6.94 ms per frame.
  - 240 Hz is 4.17 ms.
  - 360 Hz is 2.78 ms.
- "PSS" means proportional set size: shared pages are divided among the processes that share them.

---

## 0. TL;DR

**Recommendation: Tauri 2 with a Svelte 5 (TypeScript) UI over a separate Rust core.**
- **What "separate" means:** the core is a set of UI-agnostic crates. Pandora Launcher already splits its backend from its UI this way, through a `bridge` crate.
- **While Minecraft runs:** by default the launcher destroys its webview and keeps only the Rust core with a tray icon.
- **Gate:** the decision is final only after the bake-off described below.

**Ranked shortlist**
1. **Tauri 2 + Svelte 5 + Rust core.**
   - Windows (the main MCSR platform):
     - It renders with Chromium's frame pacing through WebView2.
     - The installer is tiny: Celestial's is 20.6 MB.
     - The whole webview can be unloaded while the game runs.
   - Celestial, the launcher whose look Piotr likes, is itself Tauri 2 + React, and Modrinth App is Tauri 2 + Vue.
   - **The weak spot is Linux (WebKitGTK).** It has NVIDIA and Wayland breakage, and it often runs at ~60 Hz.
   - **macOS** caps `requestAnimationFrame` at ~60 Hz.
2. **Electron 45 + the same Svelte 5 UI + the Rust core as a sidecar daemon.**
   - It runs the same Chromium everywhere, so frame pacing at high refresh rates is good on all three OSes, and it has the best performance tooling.
   - Costs:
     - an installer of ~100+ MB;
     - more RAM while the UI is open on Linux and macOS;
     - two runtimes.
   - On Windows its RAM is about the same as Tauri's.
   - This is the fallback if WebKitGTK fails the bake-off. The UI code carries over unchanged.
3. **Qt 6.12 Quick (QML)** is the native challenger.
   - It is the only mature native toolkit with all of the following on every OS:
     - a vsync-locked threaded render loop
     - built-in blur and shadow effects
     - custom shaders
     - video rendered inside the scene graph
   - Costs: a C++/QML shell (Rust bindings are immature), LGPL obligations, and slower UI iteration than with web tech.
4. **Slint 1.19 (Skia renderer)** is the Rust-native option.
   - It has the best footprint and pacing control and fits a Rust core well.
   - It is weak on the rich visuals Piotr wants:
     - backdrop blur arrives only in the unreleased 1.19, Skia-only;
     - there are no custom shaders;
     - there is no video element.
   - Re-evaluate it after 1.19 ships.
5. **GPUI** (Pandora's toolkit) and **Makepad** are worth watching but not choosing now.
   - GPUI: distribution and API churn, no element-level backdrop blur, and video on macOS only.
   - Makepad: no maintained release.

**Not recommended**
- **Flutter:** it is still fixed at 60 Hz on Linux.
- **Avalonia:** it only stopped being capped at 60 Hz in 12.1 (July 2026), Wayland support is experimental, and video can't be drawn under other controls.
- **Compose Desktop and JavaFX:** JVM footprint. JavaFX also runs its animation pulse at 60 Hz by default.
- **Wails v3:** still beta, and it uses the same webviews as Tauri.
- **Electrobun:** forces X11 on Linux and has footprint bugs.
- **Iced, egui, Xilem, Godot:** each lacks visuals, accessibility, maturity or a small footprint.

**Two findings that matter more than the framework choice**
- **The in-game footprint depends on architecture, not framework.** If the UI is unloaded while Minecraft runs, every candidate drops to a few-MB Rust process.
- **Speedrun legality:** a new launcher is not on the speedrun.com allowed-launcher list. It has to:
  - reproduce the MultiMC/Prism instance layout and `INST_*` environment variables;
  - use a game command line that tools like Julti and Jingle can parse;
  - get legalised through the MCSR Discord.

  Pandora (MIT, Rust) is on that list (rule A.1.1, as reported by the coordinator; see §6). Building on its backend crates is a real option.

---

## 1. Requirements turned into measurable targets

| Requirement | What we measure | Proposed budget (tune after the bake-off) |
|---|---|---|
| Smooth animation at 144–360 Hz | Frames actually presented vs. display refreshes; p99 frame interval | ≥ 99% of refreshes presented during transitions and scrolling at 144 and 240 Hz; p99 ≤ 1.5× the refresh interval; on a 360 Hz panel, report the numbers (best effort) |
| Rich visuals | The bake-off scene (§12) renders correctly on all engines | Glass, blur, gradient, animated background and page transition all look the same on Win, Linux X11, Linux Wayland and macOS |
| Fast start | Launch → first presented frame | Windows warm ≤ 400 ms, cold ≤ 1 s |
| Low idle cost (UI open, static) | CPU %, wakeups/s, GPU %, PSS of the whole process tree | CPU ≤ 0.5% of one core, ≤ 10 wakeups/s, GPU ≈ 0, PSS ≤ 150 MB for webview stacks |
| Near-zero cost while the game runs | Same metrics with the UI hidden or destroyed | CPU ≤ 0.1%, ≤ 1 wakeup/s, GPU 0, core PSS ≤ 20 MB |
| Small download | Installer and bundle size | ≤ 25 MB on Windows |
| Cross-platform | Windows first, then Linux (X11 and Wayland, NVIDIA common), macOS a minority | Same scene, same budgets, per OS |

---

## 2. The findings that decide the choice

### 2.1 Which engines can pace above 60 Hz

| Engine / toolkit | Windows | Linux | macOS | Evidence |
|---|---|---|---|---|
| **Chromium** (Electron, CEF, WebView2) | Vsync at the display rate | Vsync at the display rate. Electron runs natively on Wayland since v38, but has an open bug on multi-monitor VRR | Vsync, including 120 Hz ProMotion | rAF matches the display refresh ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/Window/requestAnimationFrame)); WebView2 follows vsync unless `--disable-frame-rate-limit` ([WebView2Feedback #3045](https://github.com/MicrosoftEdge/WebView2Feedback/issues/3045)); a Tauri user reports 120 fps on Windows ([tauri #13978](https://github.com/tauri-apps/tauri/issues/13978)); Wayland default ([Electron breaking changes](https://www.electronjs.org/docs/latest/breaking-changes)); two maximised windows on two monitors drop to about 15 fps on Ubuntu 26.04 Wayland with VRR ([electron #54035](https://github.com/electron/electron/issues/54035)) |
| **WebKitGTK** (Tauri, Wails, Dioxus-webview on Linux) | – | **Often around 60 Hz; contested** (see below) | – | [WebKit bug 199376](https://bugs.webkit.org/show_bug.cgi?id=199376), [Vanitty PR #61](https://github.com/stasadance/vanitty/pull/61), [DisplayVBlankMonitorTimer.cpp](https://github.com/WebKit/WebKit/blob/main/Source/WebKit/UIProcess/glib/DisplayVBlankMonitorTimer.cpp) |
| **WKWebView** (Tauri, Wails on macOS) | – | – | **rAF near 60 Hz.** CSS and compositor animations can reach 120 Hz | `PreferPageRenderingUpdatesNear60FPSEnabled` defaults to on ([UnifiedWebPreferences.yaml](https://github.com/WebKit/WebKit/blob/main/Source/WTF/Scripts/Preferences/UnifiedWebPreferences.yaml)); no public opt-out ([WebKit bug 294338](https://bugs.webkit.org/show_bug.cgi?id=294338)); Tauri closed its issue as "not planned" ([tauri #13978](https://github.com/tauri-apps/tauri/issues/13978)) |
| **Flutter 3.47** | Vsync, but the VRR timestamp bug is open | **Fixed 60 Hz** | Display link | [flutter #191245](https://github.com/flutter/flutter/issues/191245) (tracking issue, open, reviewed 2026-10-14); [#183703](https://github.com/flutter/flutter/issues/183703); [#190289](https://github.com/flutter/flutter/issues/190289) |
| **Qt Quick 6** | Vsync (threaded loop) | Vsync (threaded loop) | Vsync | The frame interval comes from QScreen. Guaranteed for one window ([Qt scene graph docs](https://doc.qt.io/qt-6/qtquick-visualcanvas-scenegraph.html)) |
| **Avalonia 12.1** | DWM-timed (ANGLE). WGL and Vulkan use a sleep timer; on NVIDIA, WGL locks to the slowest monitor | X11: a software timer at the XRandR maximum. Wayland: experimental | CVDisplayLink | [PR #21643](https://github.com/AvaloniaUI/Avalonia/pull/21643), [PR #18558](https://github.com/AvaloniaUI/Avalonia/pull/18558), [12.1 blog](https://avaloniaui.net/blog/release-12-1). Before 12.1 the rate was "hardcoded to 60" ([discussion #18421](https://github.com/AvaloniaUI/Avalonia/discussions/18421)) |
| **JavaFX** | 60 Hz pulse by default | 60 Hz | 60 Hz | `-Djavafx.animation.pulse` overrides it ([QuantumToolkit.java](https://github.com/openjdk/jfx/blob/master/modules/javafx.graphics/src/main/java/com/sun/javafx/tk/quantum/QuantumToolkit.java)) |
| **Rust GPU toolkits** (Slint, GPUI, Iced, egui, Makepad, Freya) | Swapchain vsync | Swapchain vsync | Swapchain vsync | Each toolkit's renderer docs (§3.7). Little published data above 60 Hz. The only bake-off (macOS, 144 Hz) is in §3.7 |

**WebKitGTK and the 60 Hz cap: the evidence**
- **When it can exceed 60 Hz:**
  - WebKitGTK paces frames with a DRM vblank monitor (`drmWaitVBlank`) at the monitor's rate ([DisplayVBlankMonitorDRM.cpp](https://github.com/WebKit/WebKit/blob/main/Source/WebKit/UIProcess/glib/DisplayVBlankMonitorDRM.cpp)).
  - **NVIDIA's proprietary driver rejects that call.** WebKit then falls back to a timer hard-coded at 60 fps ([DisplayVBlankMonitorTimer.cpp](https://github.com/WebKit/WebKit/blob/main/Source/WebKit/UIProcess/glib/DisplayVBlankMonitorTimer.cpp)).
  - A Tauri app (Vanitty) shimmed `drmWaitVBlank`. Its rAF counter went from 61.7 to 490 fps under Xvfb with a faked 500 Hz device. On an RTX 4090 with a 500 Hz monitor the shim paced at 499.9 Hz ([vanitty #61](https://github.com/stasadance/vanitty/pull/61)).
  - So the "prefer near 60 fps" setting did **not** cap that X11 setup.
  - My reading of the code agrees: the setting only applies when the port reports a nominal display rate ([AnimationFrameRate.cpp](https://github.com/WebKit/WebKit/blob/main/Source/WebCore/platform/graphics/AnimationFrameRate.cpp)).
- **When it is capped:**
  - A 2026-10-12 retest on WebKitGTK 2.54.1, GTK 3.24.52, GNOME 50 **Wayland**, AMD at 120 Hz measured a 16.9 ms rAF interval.
  - The tester blames the GTK3 GL frame clock, which is fixed at 60 Hz.
  - Only `GDK_GL=disable` plus turning the setting off reached about 9 ms, and that path copies frames through the CPU, which is slow ([bug 199376](https://bugs.webkit.org/show_bug.cgi?id=199376)).
- **Untested:** the GTK4 port (webkitgtk-6.0). Tauri is still on GTK3.
- **Conclusion:** on Linux, Tauri may present at only ~60 Hz on both NVIDIA (any session) and GTK3 Wayland (any GPU). This is the single biggest technical risk of the recommended stack, and it has to be measured.

### 2.2 Linux WebKitGTK: compositing and stability

**Versions**
- Ubuntu 26.04 ships WebKitGTK 2.52.6 for both 4.1 and 6.0, and GTK 3.24.52 (local `apt-cache policy`).
- Fedora and Arch ship 2.54.1 ([Fedora](https://packages.fedoraproject.org/pkgs/webkitgtk/webkit2gtk4.1/), [Arch](https://archlinux.org/packages/extra/x86_64/webkit2gtk-4.1/)).

**Rendering pipeline progress**

| Release | Change | Source |
|---|---|---|
| 2.44 | DMA-BUF renderer | [2.44](https://webkitgtk.org/2024/03/16/webkitgtk2.44.0-released.html) |
| 2.46 | Skia replaces Cairo; GPU rendering by default | [Igalia](https://blogs.igalia.com/carlosgc/?p=1029) |
| 2.48 | Threaded GPU tile painting | [2.48](https://webkitgtk.org/2025/04/08/webkitgtk-2.48.html) |
| 2.50 | Damage propagation | [2.50](https://webkitgtk.org/2025/11/26/webkitgtk-2.50.html) |
| 2.52 | Non-blocking composition | [2.52](https://webkitgtk.org/2026/03/18/webkitgtk2.52.0-released.html) |
| 2.54 | Skia compositor | [2.54](https://webkitgtk.org/2026/09/16/webkitgtk2.54.0-released.html) |

Igalia's large MotionMark gains were measured on a Raspberry Pi 4 ([Igalia](https://blogs.igalia.com/carlosgc/?p=1041)).

**Known breakage**
- **NVIDIA and DMA-BUF:** blank or white windows, flicker, and "Error 71" protocol errors on Wayland. Tauri's documented workaround ladder ([Tauri Linux graphics doc](https://v2.tauri.app/develop/debug/linux-graphics/)):
  1. `nvidia_drm.modeset=1`
  2. `__NV_DISABLE_EXPLICIT_SYNC=1`
  3. `WEBKIT_DISABLE_DMABUF_RENDERER=1`
  4. `WEBKIT_DISABLE_COMPOSITING_MODE=1`

  Tauri sets none of these by default ([tauri #14424](https://github.com/tauri-apps/tauri/issues/14424)).
- **Error 71:**
  - The issue has 60 comments and is still open in Oct 2026, including a first report on an all-AMD Ubuntu 26.04 machine ([tauri #10702](https://github.com/tauri-apps/tauri/issues/10702)).
  - Its root cause is in GTK3 ([GTK #8056](https://gitlab.gnome.org/GNOME/gtk/-/work_items/8056)).
  - A community workaround crate exists ([tauri-plugin-wayland-nvidia-quirk](https://crates.io/crates/tauri-plugin-wayland-nvidia-quirk)).
- **Glass effects break under the standard workaround:** `backdrop-filter: blur()` is "not expected to work with dmabuf renderer disabled" ([WebKit bug 303811](https://bugs.webkit.org/show_bug.cgi?id=303811), which also reports RTX 50-series GBM failures). There are also hardware-specific "backdrop-filter not rendering" reports ([tauri #14040](https://github.com/tauri-apps/tauri/issues/14040)) and transparent-window ghosting on NVIDIA ([tauri #14924](https://github.com/tauri-apps/tauri/issues/14924)).
- **Real launcher evidence (Modrinth App, Tauri):**
  - The Linux mega-issue has 179 comments: about 1 s UI freezes on NVIDIA, and it needs `WEBKIT_DISABLE_DMABUF_RENDERER=1` to start ([modrinth #3057](https://github.com/modrinth/code/issues/3057)).
  - WebKitWebProcess sits at **106–107% CPU, and memory grew from 4.6 to 5.4 GB in 15 s** on Wayland. Forcing `GDK_BACKEND=x11` drops CPU to 1–3% ([modrinth #7276](https://github.com/modrinth/code/issues/7276), open).
- **Other Tauri apps:** Readest said that on WebKitGTK it "was barely usable on most Linux machines… janky", and moved its Linux builds to the CEF runtime ([tauri #14963](https://github.com/tauri-apps/tauri/issues/14963#issuecomment-5965777662)).
- **No GTK4 yet:** the wry/Tauri GTK4 port has not started on the maintainers' side ([wry #1474](https://github.com/tauri-apps/wry/issues/1474)). A community PR is open ([wry #1767](https://github.com/tauri-apps/wry/pull/1767)).
- **Engine gaps:**
  - No WebGPU in WebKitGTK builds ([OptionsGTK.cmake](https://github.com/WebKit/WebKit/blob/main/Source/cmake/OptionsGTK.cmake)).
  - Video codecs depend on the system's GStreamer plugins. AppImage media bundling is fragile ([tauri #8579](https://github.com/tauri-apps/tauri/issues/8579), [Tauri AppImage doc](https://v2.tauri.app/distribute/appimage/)).

### 2.3 On Windows, Tauri does not save RAM, but it does save download size and unloads cleanly

- **Elanis' CI comparison** (empty app, release build; noisy GitHub runners, likely without a GPU) ([repo](https://github.com/Elanis/web-to-desktop-framework-comparison)):

  | Memory (all processes summed) | Tauri | Electron | Wails v2 |
  |---|---|---|---|
  | Windows | ≈ 307 MB | ≈ 279 MB | ≈ 317 MB |
  | Linux | ≈ 94 MB | ≈ 589 MB | – |
  | macOS | ≈ 96 MB | ≈ 364 MB | – |

  Start time on Windows: Tauri ≈ 576 ms, Electron ≈ 153 ms. Build size: Tauri ≈ 3 MB, Electron ≈ 385 MB unpacked.
- **Better Stack** (the same real app in each framework, macOS, median of 10 runs) ([Better Stack](https://betterstack.com/community/guides/scaling-nodejs/tauri-vs-electron-vs-deno-vs-electrobun/)):

  | | Tauri | Electron |
  |---|---|---|
  | Bundle | 57 MB | 323 MB |
  | Startup | 311 ms | 273 ms |
  | Idle RAM | 109 MB | 128 MB |
- **Tauri's own CI** (hello world, 2026-10-23) ([benchmark_results](https://github.com/tauri-apps/benchmark_results)):
  - binary about 2.8–3.0 MB;
  - launch-to-exit 1.01 s on Windows (min 0.59 s), 0.71 s on Linux;
  - Linux peak 273 MB, measured with `mprof`, which sums RSS and so double-counts shared memory.
- **Microsoft's guidance:** don't use WebView2 for a splash screen, because of its startup cost. Startup is faster when Edge is already running ([WebView2 perf](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance)).

### 2.4 The rich effects have real costs

- **`backdrop-filter`** forces render surfaces and turns off overlay compositing. In one Electron app on macOS, a single blurred element raised GPU-process memory **from about 107 MB to 585 MB**, and even `blur(0px)` paid that cost ([measurement](https://github.com/theagenticage/hercule/issues/301)). Windows and Linux costs are UNMEASURED.
- **Compositor-only animation:** only `transform` and `opacity` (plus `filter` in Chromium) animate off the main thread ([web.dev](https://web.dev/articles/stick-to-compositor-only-properties-and-manage-layer-count), [Chrome](https://developer.chrome.com/blog/hardware-accelerated-animations)).
- **Video backgrounds on Linux:** Chromium does not enable VA-API by default, and NVIDIA's VA-API driver doesn't work with Chromium ([Chromium VA-API doc](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/docs/gpu/vaapi.md)). Expect CPU decoding on Linux with Electron or CEF.
- **How Celestial keeps its effects cheap** (§5.4):
  - particles run in Web Workers on OffscreenCanvas, capped at **20–30 fps**;
  - it has a `lowEndMode` toggle;
  - animations pause while minimised;
  - it honours `prefers-reduced-motion`.

### 2.5 The in-game footprint depends on architecture

Every web stack keeps 100–600 MB alive if the window is only minimised. The fixes:

- **WebView2:** the host must set `IsVisible=false` itself, then call `TrySuspendAsync`; `MemoryUsageTargetLevel=Low` also helps ([IsVisible](https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2controller.isvisible), [TrySuspendAsync](https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2.trysuspendasync), [MemoryUsageTargetLevel](https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2.memoryusagetargetlevel)).
- **Electron:** destroying a window ends its renderer, but the browser, GPU and utility processes keep running ([Electron process model](https://www.electronjs.org/docs/latest/tutorial/process-model)).
- **Tauri:** the app can close every window and stay alive. `ExitRequestApi::prevent_exit` "Prevents the app from exiting" ([docs.rs](https://docs.rs/tauri/latest/tauri/struct.ExitRequestApi.html)).
- **A separate tiny Rust daemon** is a few MB. Measured Rust tray apps: under 2 MB ([OLEDShift](https://github.com/Marko19907/OLEDShift)) and about 6 MB ([mulltray](https://github.com/andriamanitra/mulltray)).

---

## 3. Each candidate in detail

### 3.1 Tauri 2 (Rust core + system webview)

**Version and release pace**
- Stable is **2.12.5 (2026-10-21)**; the CLI is 2.12.4 (local `gh release list`; [release](https://github.com/tauri-apps/tauri/releases/tag/tauri-v2.12.5)).
- Minor releases arrive every 1–3 months ([releases](https://github.com/tauri-apps/tauri/releases)).
- 2.12 added macOS "Liquid Glass" window effects ([2.12.0](https://github.com/tauri-apps/tauri/releases/tag/tauri-v2.12.0)).
- v3 has been in alpha since 2026-09-13 ([v3.0.0-alpha.0](https://github.com/tauri-apps/tauri/releases/tag/tauri-v3.0.0-alpha.0)).
- wry 0.57.0 and tao 0.37.1 ([wry](https://github.com/tauri-apps/wry/releases), [tao](https://github.com/tauri-apps/tao/releases)).

**Rendering per OS**
- **Windows (WebView2):** Chromium compositor, vsync-paced. Mica, Acrylic and Blur window backdrops come from `window-vibrancy`. Acrylic and Blur "have bad performance when resizing/dragging"; Mica doesn't ([window-vibrancy](https://github.com/tauri-apps/window-vibrancy)).
- **Linux (WebKitGTK):** see §2.1 and §2.2.
- **macOS (WKWebView):** rAF near 60 Hz on ProMotion (§2.1). A community plugin flips the setting through a private API, which is risky for the App Store ([tauri-plugin-macos-fps](https://github.com/userFRM/tauri-plugin-macos-fps)).

**Hidden-window controls**
- wry exposes `set_memory_usage_level` but no TrySuspend ([wry lib.rs](https://github.com/tauri-apps/wry/blob/dev/src/lib.rs)).
- It does not set `IsVisible=false` on minimise ([wry webview2](https://github.com/tauri-apps/wry/blob/dev/src/webview2/mod.rs)).
- Both are reachable through `with_webview` → controller.
- `background_throttling` is supported on macOS only ([docs.rs WebviewWindowBuilder](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html)).

**Size, start and RAM:** §2.3. A real rich app, Celestial 2.4.1, ships a **20.6 MB** Windows installer (§5.4).

**Engine differences**

| Feature | WebView2 | WKWebView | WebKitGTK |
|---|---|---|---|
| View Transitions | Chromium 111 | Safari 18 | Unverified |
| WebGPU | Yes | macOS 26 | **No** |
| `backdrop-filter` | Yes | Yes | Only on the DMA-BUF path |
| Codecs | Chromium | System | GStreamer |

Sources: [caniuse](https://caniuse.com/view-transitions), [PlatformEnable.h](https://github.com/WebKit/WebKit/blob/main/Source/WTF/wtf/PlatformEnable.h).

**Updater, signing and accessibility**
- **Updater plugin:** signatures are mandatory. Artifacts are NSIS/MSI, `.app.tar.gz` and AppImage; **no deb/rpm** ([updater](https://v2.tauri.app/plugin/updater/)).
- **Windows signing:** OV certificates, Azure Key Vault or Azure Artifact Signing. EV no longer buys instant SmartScreen reputation ([Tauri Windows signing](https://v2.tauri.app/distribute/sign/windows/)).
- **macOS notarization:** needs the $99/yr Apple Developer account ([macOS signing](https://v2.tauri.app/distribute/sign/macos/)).
- **AppImage:** "70+ MB"; build it on the oldest glibc you support ([AppImage](https://v2.tauri.app/distribute/appimage/)).
- **Accessibility:** the platform webview's (WebKitGTK uses AT-SPI). A Linux accessibility-bus warning issue is still open ([tauri #4315](https://github.com/tauri-apps/tauri/issues/4315)).

**Licence:** MIT/Apache-2.0.

**Precedents**
- **Celestial** (Tauri 2.11.5 + React 19), §5.4.
- **Modrinth App** (Tauri 2 + Vue 3). Its source is [modrinth/code](https://github.com/modrinth/code); its Linux problems are in §2.2.

### 3.2 Linux escape hatches for Tauri: CEF runtime, Servo/Verso, others

**`tauri-runtime-cef` 3.0.0-alpha.6 (2026-10-14)**
- It bundles CEF 154.5 and ships with the Tauri v3 alpha only ([release](https://github.com/tauri-apps/tauri/releases/tag/tauri-runtime-cef-v3.0.0-alpha.6); crates.io, local).
- **Readest ships its Linux builds on it**, and says the app "runs as smoothly as on macOS and Windows". It also listed 13 pitfalls ([tauri #14963](https://github.com/tauri-apps/tauri/issues/14963#issuecomment-5965777662)):
  - **X11 only** (Wayland goes through XWayland);
  - the sandbox had to be disabled on Ubuntu 24.04+;
  - the **.deb grew from 40 to 176 MB**.
- Open blockers:
  - **The idle main thread spins at 100% of a core** ([tauri #16189](https://github.com/tauri-apps/tauri/issues/16189));
  - a white flash at startup ([#16205](https://github.com/tauri-apps/tauri/issues/16205));
  - WebGPU only on SwiftShader ([#16203](https://github.com/tauri-apps/tauri/issues/16203));
  - keyboard and focus bugs ([#16351](https://github.com/tauri-apps/tauri/issues/16351)).
- **Verdict:** this is the right long-term hatch for Linux, but not before v3 is stable and the idle-CPU bug is fixed.

**Other engines**
- **Servo/Verso:**
  - Verso has been archived since 2025-10 ([verso](https://github.com/versotile-org/verso)).
  - `tauri-runtime-verso` exists ([repo](https://github.com/versotile-org/tauri-runtime-verso)).
  - A Tauri maintainer (2026-02) called Servo "not mature enough for general use" ([tauri #14944](https://github.com/tauri-apps/tauri/issues/14944)).
  - Servo v0.6.0 LTS shipped 2026-09-29 ([release](https://github.com/servo/servo/releases/tag/v0.6.0)).
  - Not viable for us.
- **Blitz:** beta, no JavaScript ([blitz](https://github.com/DioxusLabs/blitz)). Not viable.
- **WPE WebKit backend for wry:** none found (UNVERIFIED that none exists).

### 3.3 Electron, CEF directly, Ultralight, Sciter

**Electron 45.0.0 (2026-10-21)**
- Chromium 156, Node 24.22 ([release](https://github.com/electron/electron/releases/tag/v45.0.0); `npm view electron version`, local).
- Three stable majors are supported, with a new one every 8 weeks ([timelines](https://www.electronjs.org/docs/latest/tutorial/electron-timelines)).
- v45 brings faster sandboxed-renderer startup and contentTracing in Perfetto format ([blog](https://www.electronjs.org/blog/electron-45-0)).

**Footprint**
- Empty app summed across processes:

  | Source | OS | Memory |
  |---|---|---|
  | Elanis (§2.3) | Windows | ≈ 279 MB |
  | Elanis | Linux | ≈ 589 MB |
  | Elanis | macOS | ≈ 364 MB |
  | `footprint` measurement | macOS | 124–139 MB ([source](https://github.com/theagenticage/hercule/issues/301)) |
- Runtime zip for v45: 158 MB on Windows, 123 MB on Linux, 120 MB on macOS ([release assets](https://github.com/electron/electron/releases/tag/v45.0.0)).
- No measured NSIS installer size (UNVERIFIED; likely about 80–110 MB).

**Pacing:** Chromium vsync at the display rate. Open issues:
- the Linux multi-monitor VRR bug ([#54035](https://github.com/electron/electron/issues/54035));
- anecdotal 144 Hz complaints on Linux ([#17541](https://github.com/electron/electron/issues/17541)).

There is no frame-cap API for normal windows ([#34693](https://github.com/electron/electron/issues/34693)).

**Hidden behaviour**
- `backgroundThrottling` defaults to `true` ([webPreferences](https://www.electronjs.org/docs/latest/api/structures/web-preferences)).
- Visibility is `hidden` only when minimised or after `win.hide()`; occlusion is tracked on macOS only ([BrowserWindow](https://www.electronjs.org/docs/latest/api/browser-window#page-visibility)).
- `app.getAppMetrics()` reports per-process CPU, idle wakeups and memory ([ProcessMetric](https://www.electronjs.org/docs/latest/api/structures/process-metric)).
- No one has published the footprint of the main process alone after all windows are destroyed (UNVERIFIED).

**Updater and signing**
- `electron-updater` covers NSIS, DMG+zip, AppImage, DEB, RPM and Pacman, with blockmap differential downloads ([electron-builder](https://www.electron.build/docs/features/auto-update)).
- **Azure Artifact Signing** (formerly Trusted Signing): individual developers must be in the **US or Canada**; organisations in the EU and some other regions qualify ([MS quickstart](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart)).

**Accessibility:** Chromium's, automatic when assistive tech is present ([docs](https://www.electronjs.org/docs/latest/tutorial/accessibility)).

**Licence:** MIT.

**CEF directly**
- Version 156.0.8. Minimal distributions: 175 MB (Windows), 332 MB (Linux), 135 MB (macOS arm64) ([cef-builds index](https://cef-builds.spotifycdn.com/index.json)).
- Licence BSD ([LICENSE](https://github.com/chromiumembedded/cef/blob/master/LICENSE.txt)).
- Bindings: `cef-rs` v154.5 ([tauri-apps/cef-rs](https://github.com/tauri-apps/cef-rs)) and CefSharp ([releases](https://github.com/cefsharp/CefSharp/releases)).
- Off-screen rendering defaults to 30 fps ([cef_types.h](https://github.com/chromiumembedded/cef/blob/master/include/internal/cef_types.h)).
- Using CEF directly is more work than Electron for little gain, unless you go through Tauri's CEF runtime.

**Ultralight**
- Vendor-claimed memory of 9–17 MB.
- **The free tier caps rendering at 60 fps and excludes the JIT and media** ([pricing](https://ultralig.ht/pricing)).
- Rejected.

**Sciter**
- A 4–8 MB engine with its own CSS dialect ([pysciter](https://pypi.org/project/pysciter)).
- Its public SDK repo has been stale since 2022 ([sciter-js-sdk](https://github.com/c-smile/sciter-js-sdk)).
- Rejected.

### 3.4 Wails v3, Electrobun, Deno desktop, Neutralino

**Wails v3**
- Still **beta** (v3.0.0-beta.34, 2026-10-20; local `gh release list`; [status](https://github.com/wailsapp/wails/blob/master/docs/mpress/content/status.md)).
- Uses the system webviews; on Linux the default is **GTK4 + WebKitGTK 6.0**. Whether GTK4 avoids the 60 Hz frame-clock problem is untested and could matter.
- Built-in updater supporting GitHub, keygen, Sparkle and custom feeds ([updater guide](https://github.com/wailsapp/wails/blob/master/docs/mpress/content/guides/updater.md)).
- Go core.
- Footprint per the Wails v2 numbers in Elanis: about 317 MB on Windows, about 11 MB build.
- **Verdict:** same engine risks as Tauri, beta status, and a Go core instead of Rust. No advantage for us.

**Electrobun 2.0.4**
- Vendor claims about 14 MB and 15–30 MB RAM ([docs](https://framework.blackboard.sh/electrobun/guides/what-is-electrobun/)).
- Better Stack measured **208 MB idle, 773 ms start and a 418 MB bundle** (§2.3).
- Linux forces X11 ([#568](https://github.com/blackboardsh/electrobun/issues/568)).
- Idle commit growth on a Windows tray app ([#571](https://github.com/blackboardsh/electrobun/issues/571)).
- Rejected.

**Deno desktop:** experimental, shipped in Deno 2.9 ([blog](https://deno.com/blog/v2.9)). Not yet.

**Neutralino:** about 2 MB, but the same system-webview issues ([releases](https://github.com/neutralinojs/neutralinojs/releases)).

### 3.5 Frontend frameworks for the webview UI

Versions on npm today (local): Svelte 5.57.2, Solid 1.9.17 (2.0 in RC), React 19.3.1, Vue 3.5.44 (3.6 Vapor in RC).

js-framework-benchmark, Chrome 154 run, weighted geometric mean of CPU time (lower is better). These are recomputed from raw results, so they may differ slightly from the published page ([release](https://github.com/krausest/js-framework-benchmark/releases/tag/chrome154)):

| Implementation | CPU geomean | Bundle (brotli) | First paint |
|---|---|---|---|
| vanillajs | 1.04 | – | – |
| Vue Vapor | 1.11 | – | – |
| Solid 1.9 | 1.13 | 4.5 KB | 52 ms |
| Svelte 5 | 1.17 | 9.7 KB | 71 ms |
| Vue 3.5 | 1.30 | – | – |
| React 19 hooks | 1.59 | 51 KB | 227 ms |

React Compiler shows no gain here (1.65).

**Animation tooling**
- **Svelte:** built-in `transition:`, which compiles to CSS and Web Animations that "can run off the main thread". Spring and Tween classes, plus `prefersReducedMotion` ([transition docs](https://svelte.dev/docs/svelte/transition), [svelte/motion](https://svelte.dev/docs/svelte/svelte-motion)).
- **Motion** (MIT) uses WAAPI for hardware-accelerated transform, opacity, filter and clip-path ([performance docs](https://motion.dev/docs/performance)).
- **GSAP** is now free including commercial use, under a proprietary licence ([npm](https://www.npmjs.com/package/gsap), [licence](https://scancode-licensedb.aboutcode.org/gsap-standard-no-charge-2025.html)).

**Virtualised lists:** `@tanstack/virtual-core` 3.19 and `virtua` 0.53 both have Svelte 5 and Solid adapters ([npm](https://www.npmjs.com/package/@tanstack/virtual-core), [npm](https://www.npmjs.com/package/virtua)).

**Pick: Svelte 5.**
- It is close to vanilla speed with a small bundle.
- It has first-class transitions and motion.
- It is stable, while Solid 2.0 is still in RC.
- Solid is equally good if the team prefers it.
- React costs about 1.4× CPU and 5× bundle size. Celestial shows React can still look great, but there's no reason to choose it here.

### 3.6 Flutter desktop

**Version and renderer**
- 3.47.7 stable (2026-10-08) ([releases json](https://storage.googleapis.com/flutter_infra_release/releases/releases_linux.json)).
- **Impeller is the default on all desktops since 3.47.** Skia is still an opt-out for now ([Impeller docs](https://docs.flutter.dev/perf/impeller)).
- Shaders compile at build time, so there's no first-run animation stutter ([3.47 blog](https://flutter.dev/blog/whats-new-in-flutter-3-47)).
- The blog says "Vulkan on Windows and Linux", but the code and bug stacks point to GLES via ANGLE on Windows ([#190200](https://github.com/flutter/flutter/issues/190200)). UNVERIFIED which backend is actually used.

**Frame pacing**
- **Linux: fixed 60 Hz** ([#191245](https://github.com/flutter/flutter/issues/191245)).
- Windows: vsync. With VRR, frame timestamps go backwards ([#190289](https://github.com/flutter/flutter/issues/190289)). A spinner at 165 Hz drove an AMD integrated GPU to about 80% ([#171880](https://github.com/flutter/flutter/issues/171880)).

**NVIDIA bugs**
- Black window on Wayland with Optimus ([#188966](https://github.com/flutter/flutter/issues/188966)).
- Crash on maximise on NVIDIA/Wayland ([#191775](https://github.com/flutter/flutter/issues/191775)).
- White window on Windows NVIDIA laptops ([#190200](https://github.com/flutter/flutter/issues/190200)).

**Effects and video**
- `BackdropGroup` shares a single backdrop pass ([API](https://api.flutter.dev/flutter/widgets/BackdropGroup-class.html)).
- The official `video_player` has **no Windows or Linux** support ([pub.dev](https://pub.dev/packages/video_player)). The alternative, media_kit, was last released 2025-12 and has 365 open issues ([media_kit](https://github.com/media-kit/media-kit)).

**Size and accessibility**
- LocalSend (a real Flutter app) ships an 18 MB Windows installer and a 63 MB AppImage ([release](https://github.com/localsend/localsend/releases/tag/v1.18.2)).
- Ubuntu warns that Flutter has AT-SPI problems ([Ubuntu docs](https://documentation.ubuntu.com/desktop/en/latest/explanation/accessibility-stack)).

**Updates and maintainers**
- `auto_updater` has no Linux support ([pub.dev](https://pub.dev/packages/auto_updater)).
- Canonical now maintains the desktop embedders ([OMG Ubuntu](https://www.omgubuntu.co.uk/2026/05/flutter-desktop-canonical-maintained)).

**Verdict:** the 60 Hz cap on Linux rules it out for this project.

### 3.7 Rust-native GPU toolkits

| Toolkit | Version / status | Renderer and frame model | Rich visuals | Accessibility | Licence | Notes |
|---|---|---|---|---|---|---|
| **Slint** | 1.18.1 (2026-09-21), stable 1.x since 2023 ([crates.io](https://crates.io/crates/slint)) | FemtoVG GL (default), Skia (GL/Metal/Vulkan/D3D), software; Vello experimental ([renderers](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/)). Lazy, dirty-tracked redraw | Gradients and shadows; springs and path animations in 1.18 ([blog](https://slint.dev/blog/slint-1.18-released)); **`backdrop-blur` in unreleased 1.19, Skia only** ([CHANGELOG](https://github.com/slint-ui/slint/blob/master/CHANGELOG.md), [#2066](https://github.com/slint-ui/slint/issues/2066)); **no custom shaders** ([#10887](https://github.com/slint-ui/slint/issues/10887)); video means feeding frames from ffmpeg or GStreamer yourself ([examples](https://github.com/slint-ui/slint/tree/master/examples)); virtualised ListView | AccessKit | **GPLv3, or Royalty-free 2.0** (desktop allowed; **attribution required**: AboutSlint widget in the About screen, or a badge on the download page), or commercial ([licence](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md), [pricing](https://slint.dev/pricing)) | Live preview, Figma plugin ([1.10](https://slint.dev/blog/slint-1.10-released)). Avoid the FemtoVG+wgpu path: the maintainer says it is "known not to perform well" ([#8693](https://github.com/slint-ui/slint/issues/8693)). Production user: LibrePCB 2.0 ([blog](https://librepcb.org/blog/2026-01-28_release_2.0.0/)) |
| **GPUI** (Zed) | crates.io 0.2.2 is a year stale; real use comes from git or `gpui-pre` snapshots ([crates.io](https://crates.io/crates/gpui), [gpui-component](https://github.com/longbridge/gpui-component/blob/main/Cargo.toml)); "tied to Zed" ([gpui.rs](https://www.gpui.rs/)) | Metal; D3D11 + DirectComposition; wgpu on Linux since Feb 2026 ([PR #46758](https://github.com/zed-industries/zed/pull/46758)). Hybrid immediate/retained, vsync | Box shadows, **linear gradients only**, window-level blur and Mica but **no element backdrop blur**; **video surface is macOS-only** ([surface.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/elements/surface.rs)); virtual lists | AccessKit | Apache-2.0 | Only published bake-off, macOS 144 Hz, 100k-row scroll: first frame 152 ms, p99 8.3 ms, 125 MiB peak RSS, 3.8 MiB binary, ahead of egui and Slint ([zed discussion #63832](https://github.com/zed-industries/zed/discussions/63832), single author). **Used by Pandora** (§6) |
| **Makepad** | crates.io 1.0.0 (2025-05) is stale; the dev branch is churning ([README](https://github.com/makepad/makepad)) | Own engine: Metal, D3D11, GL, Vulkan; every widget is a shader | **Best visuals:** shader DSL, element backdrop and glass blur ([backdrop.rs](https://github.com/makepad/makepad/blob/dev/widgets/core/src/backdrop.rs)), **zero-copy hardware video** (D3D11VA, DMA-BUF/GStreamer, NVIDIA path) ([video.rs](https://github.com/makepad/makepad/blob/dev/widgets/core/src/video.rs)) | Early | MIT/Apache | High ceiling, high risk |
| **Freya** | 0.4.3 stable; 0.5.0-rc.10 (2026-10-18; crates.io, local) | Skia; no longer based on Dioxus ([README](https://github.com/marc2332/freya)) | `backdrop_blur`, SkSL shaders ([rect.rs](https://github.com/marc2332/freya/blob/main/crates/freya-core/src/elements/rect.rs)); video through an ffmpeg *sidecar process* | AccessKit | MIT | Mostly a single maintainer |
| **Iced** | 0.14.0 (2025-12), "experimental" ([README](https://github.com/iced-rs/iced)) | wgpu; reactive rendering since 0.14 ([PR #2662](https://github.com/iced-rs/iced/pull/2662)) | Custom shader widget; no blur; GStreamer video player ([iced_video_player](https://github.com/jazzfool/iced_video_player)) | **None upstream** ([#3281](https://github.com/iced-rs/iced/pull/3281)) | MIT | Open bug: freezes for minutes after minimise on NVIDIA/Wayland ([#3496](https://github.com/iced-rs/iced/issues/3496)) |
| **egui** | 0.37.0 (2026-10-15) ([release](https://github.com/emilk/egui/releases/tag/0.37.0)) | Immediate mode on wgpu; repaints only on input or animation | Two-colour gradients, shadows, paint callbacks | AccessKit | MIT/Apache | wgpu `MemoryHints::Performance` reserves **~250 MB at init vs ~50 MB** with `MemoryUsage` ([#8453](https://github.com/emilk/egui/issues/8453)); a Wayland busy-spin when occluded was fixed 2026-08 ([#8434](https://github.com/emilk/egui/issues/8434)). Looks like a tool, not a game client |
| **Dioxus** | 0.7.10 stable, 0.8 alpha ([releases](https://github.com/DioxusLabs/dioxus/releases)) | Desktop = **wry webview** (same engines as Tauri); native = Blitz, in beta ([Blitz](https://github.com/DioxusLabs/blitz)) | Blitz: `backdrop-filter` Skia-only, no 3D transforms ([CSS status](https://blitz.is/status/css)) | AccessKit (Blitz) | MIT/Apache | Best hot-patching. Webview mode inherits Tauri's Linux issues |
| **Xilem / Masonry** | 0.4.0, "experimental" ([README](https://github.com/linebender/xilem)) | Vello; the Vello compute renderer is now "research" ([Vello](https://github.com/linebender/vello)) | – | AccessKit | Apache | Not ready |
| **Floem, Vizia, Ribir, Cushy** | Stale, niche or dormant ([floem](https://github.com/lapce/floem), [vizia](https://github.com/vizia/vizia), [Ribir](https://github.com/RibirX/Ribir), [cushy](https://github.com/khonsulabs/cushy)) | – | – | – | – | – |
| **Bevy UI** | 0.20.0 ([release](https://github.com/bevyengine/bevy/releases/tag/v0.20.0)) | Continuous loop by default; `desktop_app()` makes it reactive ([winit_config](https://github.com/bevyengine/bevy/blob/main/crates/bevy_winit/src/winit_config.rs)) | Shaders, gradients | Partial | MIT/Apache | About 33 MiB example binaries ([PR #14934](https://github.com/bevyengine/bevy/pull/14934)). Overkill |

**Caveat that applies to every wgpu-based toolkit on Linux:** by default wgpu loads all backends. That pulls in about 8.5 MB of Mesa GL plus about 24 MB of lavapipe/LLVM unless you restrict them ([slint #13260](https://github.com/slint-ui/slint/issues/13260)).

### 3.8 Qt 6 / QML (Qt Quick)

**Version:** Qt 6.12 LTS (2026-09-30), with QML hot reload in the engine ([blog](https://qt.io/blog/qt-6.12-released)).

**Pacing**
- The threaded render loop is the default on D3D11, Metal, Vulkan and most Linux GL setups.
- Animations advance in step with vsync, and the interval comes from QScreen (e.g. 6.94 ms at 144 Hz) ([scene graph docs](https://doc.qt.io/qt-6/qtquick-visualcanvas-scenegraph.html)).
- Caveats from the same page:
  - smooth vsync animation is guaranteed for one on-screen window only;
  - minimised or occluded windows stop presenting, and running animations fall back to a timer, so the app must stop its own animations to reach zero CPU.

**RHI backends:** D3D11 (Windows), Metal (macOS), OpenGL elsewhere; switch with `QSG_RHI_BACKEND=vulkan|d3d12` ([renderer docs](https://doc.qt.io/qt-6/qtquick-visualcanvas-scenegraph-renderer.html)).

**Effects**
- `MultiEffect` combines blur, shadow, colourise and mask. "Blur and shadow are the heaviest" ([MultiEffect](https://doc.qt.io/qt-6/qml-qtquick-effects-multieffect.html)).
- There is no CSS-style backdrop element. Glass is built by blurring a `ShaderEffectSource` of the background region (standard practice; UNVERIFIED cost).
- `ShaderEffect` takes custom shaders.

**Video:** the FFmpeg backend is the default on desktop and is bundled by the deploy tools ([Qt Multimedia](https://doc.qt.io/qt-6/qtmultimedia-index.html)).

**Sizes (real apps)**
- Moonlight-qt, Qt Quick + FFmpeg: 35.5 MB Windows zip, 51.5 MB AppImage ([releases](https://github.com/moonlight-stream/moonlight-qt/releases)).
- Prism Launcher, Qt Widgets: 22.8 MB setup ([releases](https://github.com/PrismLauncher/PrismLauncher/releases)).
- Idle RAM: no credible number (UNVERIFIED).

**Licence**
- LGPLv3 obligations: dynamic linking so users can relink, provide or offer the source, show a notice ([LGPL obligations](https://www.qt.io/licensing/open-source-lgpl-obligations)).
- **Modules that are GPL-only (avoid):** Quick 3D, Lottie, Quick Timeline, Qml Compiler, Canvas Painter, Graphs, Virtual Keyboard, and others ([Qt licensing](https://doc.qt.io/qt-6/licensing.html)).
- MultiEffect and Multimedia are not GPL-only.

**Rust bindings:** CXX-Qt 0.10.0 (2026-08-24), "in early development and the API changes frequently" ([crates.io](https://crates.io/crates/cxx-qt), [README](https://github.com/KDAB/cxx-qt)).

**Accessibility:** native platform APIs ([docs](https://doc.qt.io/qt-6/accessible.html)).

**Updater:** none built in. Velopack (MIT) works for C++ and Rust apps ([velopack](https://github.com/velopack/velopack)).

### 3.9 Avalonia 12, Uno, Compose Multiplatform Desktop, JavaFX

**Avalonia 12.1.4 (2026-10-09)**
- Skia renderer; an Impeller backend is experimental ([releases](https://github.com/AvaloniaUI/Avalonia/releases), [Impeller](https://avaloniaui.net/blog/avalonia-partners-with-google-s-flutter-t-eam-to-bring-impeller-rendering-to-net)).
- Pacing is covered in §2.1. Wayland is "experimental", opt-in, and drops to 20 fps when starved ([12.1](https://avaloniaui.net/blog/release-12-1), [WaylandWorker.RenderTimer](https://github.com/AvaloniaUI/Avalonia/blob/main/src/Avalonia.Wayland/Server/WaylandWorker.RenderTimer.cs)).
- LibVLC video is a native child window, so **no controls can be drawn over the video** ([LibVLCSharp #539](https://code.videolan.org/videolan/LibVLCSharp/-/issues/539)). The built-in MediaPlayer is paid ([docs](https://docs.avaloniaui.net/accelerate/components/media-player/mediaplayer)).
- NativeAOT gives about an 18 MB exe and about 20 MB RAM in a community template ([AvaloniaAOT](https://github.com/lixinyang123/AvaloniaAOT)).
- MIT core ([retiring Accelerate](https://avaloniaui.net/blog/retiring-accelerate)).
- **Verdict:** high-refresh and Wayland support are too new, and video backgrounds are blocked.

**Uno 6.6:** Skia; "display-synced frame pacing" since 6.4 ([6.4](https://platform.uno/blog/uno-platform-6-4/)); Linux screen reader "planned" ([InfoQ](https://infoq.com/news/2026/08/uno-platform-6-6)). Not considered further.

**Compose Multiplatform 1.12.2**
- Skiko on D3D, Metal or GL, vsync on ([SkikoProperties](https://github.com/JetBrains/skiko/blob/master/skiko/src/jvmMain/kotlin/org/jetbrains/skiko/SkikoProperties.kt)).
- Cold start: 1,356 ms on Windows, 650 ms with a JDK AOT cache ([PR #5644](https://github.com/JetBrains/compose-multiplatform/pull/5644)).
- About 100–140 MB RAM (anecdotal) and about a 50–65 MB installer ([Slack](https://slack-chats.kotlinlang.org/t/12008610/i-tried-creating-an-msi-with-just-the-hello-world-example-bu?t=v2)).
- **No Linux accessibility** ([docs](https://kotlinlang.org/docs/multiplatform/compose-desktop-accessibility.html)). Wayland goes through AWT/XWayland.
- Updates through Conveyor, which is paid for closed source ([pricing](https://hydraulic.dev/pricing.html)).

**JavaFX:** 60 Hz pulse by default (§2.1); the JVM baseline is about 44 MB for hello world ([benchmarks](https://programming-language-benchmarks.vercel.app/problem/helloworld)).

### 3.10 Others

**Godot 4.7.2**
- An empty project uses **200–400 MB** RAM ([forum](https://forum.godotengine.org/t/default-memory-usage-concerns-200mb-400mb-for-empty-project/129801)).
- Video is Ogg Theora only, decoded on the CPU ([docs](https://docs.godotengine.org/en/stable/tutorials/animation/playing_videos.html)).
- It has a low-processor mode ([docs](https://docs.godotengine.org/en/stable/classes/class_os.html)).
- Great for particles and shaders, wrong footprint for a launcher.

**Ruled out:**
- **NoesisGUI:** middleware that needs your own renderer ([changelog](https://www.noesisengine.com/docs/Gui.Core.Changelog.html)).
- **.NET MAUI:** Linux only via an Avalonia preview ([blog](https://avaloniaui.net/blog/net-maui-is-coming-to-linux-and-the-browser-powered-by-avalonia)).
- **React Native Windows:** no Linux ([blog](https://devblogs.microsoft.com/react-native/%f0%9f%9a%80react-native-windows-v0-82-is-here/)).

---

## 4. Comparison table

**Ratings:** 1 is poor and 5 is excellent. The animation column is per OS: Windows / Linux / macOS.

**Numbers:**
- They are from the cited sources, are not our own measurements, and sum all of an app's processes.
- "Idle RAM" means the UI is open.
- "Bundle" means the installer or packaged app.

| Stack | Animation at 144–360 Hz (W / L / M) | Cold start | Idle RAM | Minimised or hidden | Bundle | Consistency across OSes | Rich-UI ease (blur, shaders, video) | Dev velocity and ecosystem | Updater and signing | Accessibility | Licence |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **Tauri 2 + Svelte 5** | 5 / **2** / 3. WebKitGTK ~60 Hz on NVIDIA and GTK3 Wayland; WKWebView rAF ~60 | 3: hello world 0.6–1.0 s on Windows, 0.71 s on Linux | 3: Windows ≈ 307 MB (same as Electron); Linux and macOS ≈ 95 MB | 4: WebView2 suspend is reachable but not wrapped; **can destroy the webview and keep the Rust process** | **5**: ~3 MB hello world; Celestial 20.6 MB | **2**: three engines | 4: full CSS, but Linux blur needs DMA-BUF, no WebGPU on Linux, GStreamer codecs | **5** | 4: signed updater plugin (no deb/rpm) | 4 | MIT/Apache |
| **Tauri 3 α + CEF runtime** (Linux) | – / 5 (Chromium; X11/XWayland only) / – | ? | Chromium-like | 2: **idle 100% CPU bug** | 2: .deb +136 MB | 4 | 5 | 3 (alpha) | 3 | 5 | MIT/Apache + BSD |
| **Electron 45 + Svelte 5** | 5 / 4 / 5. Multi-monitor VRR bug on Linux | 4: ≈150–270 ms | 2: Windows ≈ 279 MB, Linux ≈ 589 MB, macOS 124–364 MB | 3: throttles; destroying windows leaves the browser and GPU processes | **2**: runtime 120–158 MB | **5** | **5** (CPU video decode on Linux) | **5** | 5: electron-updater for all formats | 5 | MIT |
| **CEF direct** | 5 / 5 / 5 | 4 | 2 | 3 | 1: 135–332 MB | 5 | 5 | 2 | 2 (DIY) | 5 | BSD |
| **Wails v3 (beta)** | 5 / 2? (GTK4 untested) / 3 | 3: ≈ 537 ms (v2) | 3: Windows ≈ 317 MB (v2) | 3 | 5: ≈ 11 MB (v2) | 2 | 4 | 3 | 4: built-in updater | 4 | MIT |
| **Electrobun 2** | 5 / 2 (X11 forced) / 3 | 2: 773 ms (Better Stack) | 2: 208 MB | 2: tray commit bug | 1–5 (418 MB measured vs 14 MB claimed) | 2 | 4 | 2 | 4 | 4 | MIT |
| **Flutter 3.47** | 4 / **1 (fixed 60 Hz)** / 5 | ? (UNVERIFIED) | ? | 4: frames stop when hidden | 4: 18 MB Windows installer (LocalSend) | 3 | 3: BackdropFilter and shaders good; desktop video weak | 4 | 2: no Linux auto-update package | 3: AT-SPI issues | BSD-3 |
| **Slint 1.18/1.19** | 4 / 4 / 4 | 4: first frame 190 ms (macOS bake-off) | **4**: 118 MiB peak RSS (macOS bake-off; idle UNVERIFIED) | 5: lazy redraw | 5: ~9 MB binary | 4 | **2**: blur only in 1.19 and Skia-only; no shaders; DIY video | 3: live preview, Figma | 2: DIY or Velopack | 4 (AccessKit) | GPLv3 / royalty-free with attribution / commercial |
| **GPUI** (Pandora) | 4 / 3 (needs Vulkan) / 5 | 4: 152 ms | 4: 125 MiB peak | 4 | **5**: Pandora 14.2 MB setup | 3 | **2**: no element blur; video macOS-only | 2: git dependencies, churn | 2: DIY (Pandora has its own) | 4 | Apache-2.0 |
| **Makepad** | 4 / 4 / 4 (UNVERIFIED) | ? | ? | ? | 5 | 4 | **5**: shaders, blur, zero-copy video | 1: no maintained release | 1 | 2 | MIT/Apache |
| **Freya 0.5** | 4 / 4 / 4 (UNVERIFIED) | ? | ? | ? | 4 | 4 | 4: backdrop blur, SkSL, ffmpeg-sidecar video | 2: one maintainer | 1 | 4 | MIT |
| **Iced 0.14** | 4 / 3 (NVIDIA minimise freeze) / 4 | ≈ 230 ms (2023) | ? | 3 | 4 | 3 | 2 | 3 | 1 | **1** | MIT |
| **egui 0.37** | 4 / 4 / 4 | ≈ 280 ms (2023) | 2–4: 50–250 MB depending on wgpu hints | 4 | 4 | 4 | 1 (tool look) | 4 | 1 | 4 | MIT/Apache |
| **Dioxus 0.7** (webview / Blitz) | = Tauri / Blitz beta | = Tauri | = Tauri | = Tauri | = Tauri | 2 | 4 / 2 | 4 | 2 | 4 | MIT/Apache |
| **Qt 6.12 Quick** | **5 / 5 / 5** (vsync; one window guaranteed) | ? (UNVERIFIED) | ? (UNVERIFIED) | 4: stops presenting; app must stop its animations | 4: ≈ 35–50 MB | **5** | **4**: MultiEffect, ShaderEffect, FFmpeg video in the scene graph; no CSS-style backdrop | 3: QML; Rust bindings immature | 2: Velopack | 5 | LGPLv3 (avoid GPL-only modules) / commercial |
| **Avalonia 12.1** | 3 / 2 (X11 timer; Wayland experimental) / 4 | ? | ≈ 20 MB (AOT template) | 3 | 4: ≈ 15 MB NSIS (AOT) | 3 | 2: no video under controls | 4 | 4: Velopack | 4: AT-SPI2 in 12 | MIT core |
| **Compose MP 1.12** | 4 / 3 (XWayland) / 4 | 2: 1.36 s, 0.65 s with AOT cache | 2: ≈ 100–140 MB | ? | 3: ≈ 50–65 MB | 3 | 3 | 4 | 2: Conveyor (paid) | **1 on Linux** | Apache-2.0 |
| **JavaFX** | 2 (60 Hz pulse) / 2 / 2 | 2 | 2 | ? | 3 | 3 | 3 | 3 | 2 | 3 | GPL+CE |
| **Godot 4.7** | 5 / 5 / 5 | 3 | **1: 200–400 MB** | 3 | 4: ≈ 30–39 MB | 4 | 4: Theora-only video | 3 | 1 | 2 (experimental) | MIT |

---

## 5. What existing launchers and game clients use

### 5.1 Minecraft launchers

| Launcher | UI stack | Core | Licence | Performance notes |
|---|---|---|---|---|
| **Modrinth App** | **Tauri 2 + Vue 3** ([modrinth/code](https://github.com/modrinth/code)) | Rust (`packages/app-lib`, a.k.a. theseus) | GPL-3.0 | Linux and WebKitGTK problems: freezes, a DMA-BUF workaround needed to start, 106% CPU and a memory loop on Wayland ([#3057](https://github.com/modrinth/code/issues/3057), [#7276](https://github.com/modrinth/code/issues/7276)). Leaks a file handle per watcher event ([#7371](https://github.com/modrinth/code/issues/7371)). Windows memory grows over days ([#3414](https://github.com/modrinth/code/issues/3414)). It can only *minimise* on launch; closing on launch is an open request ([#4278](https://github.com/modrinth/code/issues/4278)) |
| **Prism Launcher / MultiMC** | Qt 6 Widgets ([CMakeLists](https://github.com/PrismLauncher/PrismLauncher/blob/develop/CMakeLists.txt)) / Qt | C++ | GPL-3.0 / Apache-2.0 | Few launcher complaints, mostly about huge logs ([#1061](https://github.com/PrismLauncher/PrismLauncher/issues/1061)). Has "hide when the game window opens" and "quit when the game closes" ([settings UI](https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/ui/widgets/MinecraftSettingsWidget.ui)) |
| **GDLauncher Carbon** | Overwolf ow-electron 39 + SolidJS | Rust core process | **BUSL-1.1** ([LICENSE](https://github.com/gorilla-devs/GDLauncher-Carbon/blob/main/LICENSE)) | Kept Electron because there was no "production-ready alternative" ([docs](https://gdlauncher.com/docs/gdlauncher-vs-gdlauncher-carbon)) |
| **XMCL** | Electron 43 + Vue 3 + Vuetify | TypeScript/Node | MIT | An open request (2026-10-22) to drop Electron over "significant memory usage and startup overhead" ([#1807](https://github.com/Voxelum/x-minecraft-launcher/issues/1807)) |
| **HMCL** | JavaFX ([build](https://github.com/HMCL-dev/HMCL/blob/main/HMCL/build.gradle.kts)) | Java | GPL-3.0 | – |
| **ATLauncher** | Swing + FlatLaf ([build](https://github.com/ATLauncher/ATLauncher/blob/master/build.gradle)) | Java | GPL-3.0 | – |
| **Lunar Client** | ow-electron since 2024 ([Lunar](https://lunarclient.com/news/lunar-client-x-overwolf)) | Electron + Java | Proprietary | 9–25 s to open on Linux ([KDE forum](https://discuss.kde.org/t/application-rendering-delay-lunar-client/47758)). **Banned in MCSR Ranked** ([wiki](https://wiki.mcsrranked.com/install/)) |
| **Badlion** | Electron ([AUR PKGBUILD](https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h=badlion-client)) | – | Proprietary | – |
| **Feather** | UNVERIFIED (moving users to "Dawn") ([feathermc](https://feathermc.com/)) | – | Proprietary | – |
| **Official launcher** | "TypeScript, React, C++" ([minecraft.wiki](https://minecraft.wiki/w/Minecraft_Launcher)) | C++ | Proprietary | **Closes on launch by default** (same source) |
| **SKLauncher** | UNVERIFIED (Java) | – | – | – |

### 5.2 MCSR launchers and the allowlist

**The allowlist**
- **speedrun.com rule A.1.1:** according to the coordinator's other research agent, it lists the official launcher, MultiMC, Prism, Modrinth App, ATLauncher, **MCSR Launcher**, **Pandora** and **Nixcraft**. New launchers are legalised through an MCSR Discord thread.
  - I could not fetch the page myself (HTTP 403 from [speedrun.com/mc/rules](https://www.speedrun.com/mc/rules)).
  - **The community page [mc.sr/mods](https://mc.sr/mods/), fetched today, lists only six launchers:** Vanilla, MCSR Launcher, Prism, MultiMC, Modrinth App and ATLauncher. Pandora and Nixcraft are missing there.
  - Treat Pandora's and Nixcraft's status as **likely but conflicting**.
- **Nixcraft** is a declarative Nix-based launcher (GPL-3.0, 97★) ([loystonpais/nixcraft](https://github.com/loystonpais/nixcraft)).

**MCSR Launcher**
- Repo: [MCSRLauncher/Launcher](https://github.com/MCSRLauncher/Launcher). Lead developer RedLime, who is also the MCSR Ranked developer.
- Stack: **Kotlin/Java Swing + FlatLaf + MigLayout**, JVM 17, Inno Setup installer, GPL-3.0. I confirmed FlatLaf and `javax.swing` in its source with `gh` code search.
- Created 2025-07. The latest is 0.8.10-beta (2026-10-04).
- It is listed **first** in the MCSR Ranked install guide ([wiki source](https://github.com/MCSR-Ranked/Wiki/blob/main/docs/install/index.md)).
- Performance reports:
  - **memory and CPU growth after hours open next to the game**, enough to lag other programs ([#56](https://github.com/MCSRLauncher/Launcher/issues/56), closed);
  - in-game lag reports ([#24](https://github.com/MCSRLauncher/Launcher/issues/24), open);
  - a blank window on Arch tiling WMs until `_JAVA_AWT_WM_NONREPARENTING=1` is set ([#47](https://github.com/MCSRLauncher/Launcher/issues/47)).
- **Lesson:** a launcher that stays resident during runs is a real complaint source, even when it's "lightweight" Swing.

**What runners actually use:** Prism (recommended over MultiMC) with the official .mrpack, plus Ninjabrainbot, Toolscreen and others ([MCSR Ranked wiki](https://wiki.mcsrranked.com/install/install_prism)).

**Tools a new launcher must work with**
- **SeedQueue** (Java mod, MIT) replaces multi-instancing ([repo](https://github.com/KingContaria/seedqueue)).
- **Julti** (Java, MIT) last released Jan 2025 ([repo](https://github.com/DuncanRuns/Julti)):
  - it launches instances with `<multimc|prism> --launch <folder>`;
  - it finds instances by parsing `-Djava.library.path=<inst>/natives` or `--gameDir` from the command line ([InstanceInfoUtil](https://github.com/DuncanRuns/Julti/blob/main/src/main/java/xyz/duncanruns/julti/util/InstanceInfoUtil.java)).
- **Jingle** (active; v2 pre-releases Aug–Sep 2026) uses `INST_MC_DIR`, then `--gameDir`, then `java.library.path`, then `mmc-pack.json` ([InstanceInfo.java](https://github.com/DuncanRuns/Jingle/blob/v2/src/main/java/xyz/duncanruns/jingle/instance/InstanceInfo.java)).
- **waywall** (Linux, C, GPL-3.0) needs ([setup doc](https://github.com/tesselslate/waywall/blob/main/doc/00_setup.md)):
  - a per-instance wrapper command, `waywall wrap --`;
  - a custom GLFW;
  - `__GL_THREADED_OPTIMIZATIONS=0` on NVIDIA.

  **It does not work if the launcher runs inside Flatpak.**
- **Toolscreen** (Windows, DLL injection) supports Prism, MultiMC and MCSR Launcher only ([repo](https://github.com/jojoe77777/Toolscreen)).

### 5.3 Game clients

**League client**
- CEF front end with Ember JS plugins ([Riot tech blog](https://technology.riotgames.com/node/40)).
- Riot's CEF 74→91 upgrade cut median memory **from 597 to 275 MB** and median CPU from 34 to 22% ([Riot](https://www.leagueoflegends.com/en-gb/news/dev/client-cleanup-we-launched-the-updated-chromium-embedded-framework-cef/)).
- A new client is announced for 2027 ([FAQ](https://support.riotgames.com/es-es/riot/client/riot-client-faq)); its tech is undisclosed.

**Steam:** the CEF UI uses 400–500 MB typically, with outliers in the GBs ([Level1Techs](https://forum.level1techs.com/t/steam-client-webhelper-memory-usage/207918), [Steam forum](https://steamcommunity.com/discussions/forum/0/4652759797619575839)).

**EA app:** QtWebEngine; grew from 67 MB to 1.2 GB over hours ([EA forum](https://forums.ea.com/discussions/ea-app-technical-issues-en/memory-leak/12003782)).

**Battle.net:** Agent processes reached several GB on macOS ([forum](https://us.forums.blizzard.com/en/blizzard/t/macos-multiple-agent-processes-eating-32gb-of-ram/55360)). Its "Qt + CEF" stack is UNVERIFIED.

**Epic:** a 2020 idle-CPU bug ([HotHardware](https://hothardware.com/news/epic-games-launcher-resource-hog-bug)). Its Slate + CEF stack is UNVERIFIED.

**HoYoPlay and Ubisoft Connect:** stacks UNVERIFIED.

**Pattern:** web-based clients draw "grows over hours" complaints, and **closing or hiding the launcher when the game starts is standard practice.**

### 5.4 Celestial Launcher teardown (static inspection only)

**Identity:** Divine Skins' League of Legends skin manager. Official releases are at [DivineSkins/celestial-releases](https://github.com/DivineSkins/celestial-releases), linked from [divineskins.gg](https://www.divineskins.gg/install-mods). Two look-alike repos appear to be impersonations and were not touched.

**Method**
- Downloaded `Celestial_2.4.1_x64-setup.exe` and `Celestial_2.4.2_aarch64.app.tar.gz` into an isolated temp directory.
- Inspected them with `file`, `7z l`, `tar`, `strings` and `exiftool`, and decoded the embedded brotli assets as data.
- **Nothing was executed.** The directory has been deleted.

**Findings**
- **Shell: Tauri 2, not Electron.**
  - Rust crates in the binary: tauri 2.11.5, wry 0.55.1, tao 0.35.3, plus plugins (updater, deep-link, global-shortcut, single-instance, window-state).
  - `nsis_tauri_utils.dll`, and an updater signature "from tauri secret key".
  - Strings such as `migrated-from-electron.json` suggest **it was ported from Electron to Tauri** (inference).
- **Frontend:** React 19, Base UI, Tailwind CSS 4.3, Vite + Rolldown.
- **Backend:** tokio, reqwest, axum, LMDB (heed), Sentry, and the `window-vibrancy` crate.
- **Sizes:** **20.6 MB Windows installer** (69 MB unpacked exe); 62 MB macOS binary with the assets embedded.
- **How the look is built:**
  - an animated splash: letter-by-letter logo, orb, ring, particles;
  - ambient layers (starfield, fireflies, sakura, rain…) on **OffscreenCanvas in Web Workers capped at 20–30 fps**;
  - a blurred wallpaper with glassmorphism plus **native Windows Mica/Acrylic**;
  - about 50 CSS keyframe animations (page slide-in, shimmer, glitch and scan effects);
  - a Cinzel/Inter typeface pairing.
- **Performance safeguards:**
  - `lowEndMode` ("fewer effects, no skin images, no GIFs");
  - pauses animations when minimised;
  - honours `prefers-reduced-motion`.
- **Takeaway:** the look Piotr likes comes from CSS, cheap capped-fps canvas layers and native window backdrops. A webview stack reproduces it directly.

---

## 6. Pandora Launcher, and whether to build on it

**Facts** (from `gh`, checked today)
- Repo: [Moulberry/PandoraLauncher](https://github.com/Moulberry/PandoraLauncher). Rust (2.28 MB of source), **MIT**, 938★.
- Created 2025-10-24. **v6.0.0 shipped 2026-09-17**; the last push was 2026-10-07.
- **The UI is GPUI from Zed's git main** (with `x11`, `wayland` and `runtime_shaders` features) **plus Longbridge's `gpui-component` pinned to a git revision** ([Cargo.toml](https://github.com/Moulberry/PandoraLauncher/blob/master/Cargo.toml)).
- README: "Native application (no Electron/Tauri)", "designed to be performant… cpu and memory usage". The repo ships a `flamegraph.sh` ([README](https://github.com/Moulberry/PandoraLauncher)).
- **Architecture is directly reusable.** The workspace is split into crates, and **`backend` has no GPUI dependency**; it talks to the frontend through a `bridge` crate of message types ([backend Cargo.toml](https://github.com/Moulberry/PandoraLauncher/blob/master/crates/backend/Cargo.toml), [bridge](https://github.com/Moulberry/PandoraLauncher/tree/master/crates/bridge)).
  - Crates: `auth`, `backend`, `bridge`, `frontend`, `schema`, `nbt`, …
  - The backend uses reqwest, sha1/sha2, notify plus a debouncer, rusqlite, reflink-copy, and minisign-verify for updates.
- **Release sizes, v6.0.0:**

  | Asset | Size |
  |---|---|
  | Windows setup | **14.2 MB** |
  | Windows portable | 55.9 MB |
  | AppImage | 25.7 MB |
  | .deb | 24.6 MB |
  | macOS dmg | 33.3 MB |

  It has its own update manifests ([release](https://github.com/Moulberry/PandoraLauncher/releases/tag/v6.0.0)).
- **Bus factor:** Moulberry has 549 commits; the next contributor has 16.
- **Issues about GPUI on Linux:**
  - It panics at startup with "Unable to init GPU context" when Vulkan drivers are missing; the answer was to see Zed's Linux docs ([#14](https://github.com/Moulberry/PandoraLauncher/issues/14)).
  - A Mesa 25.3 GPU crash when unloading many images, through blade-graphics on the older GPUI ([#88](https://github.com/Moulberry/PandoraLauncher/issues/88)).
  - Wayland icon, focus and graphics-tablet issues ([#247](https://github.com/Moulberry/PandoraLauncher/issues/247), [#102](https://github.com/Moulberry/PandoraLauncher/issues/102), [#720](https://github.com/Moulberry/PandoraLauncher/issues/720)).
  - The unofficial Flatpak is stale ([#689](https://github.com/Moulberry/PandoraLauncher/issues/689)).
- **No published RAM, CPU or frame-time numbers.** I did not run it (we don't execute downloads in this phase). It is a prime candidate for the baseline measurements in §12.8.

**Options**

| Option | Pros | Cons |
|---|---|---|
| **A. Fork Pandora wholesale** | Working, allowlisted base; Rust; MIT; small installer | The GPUI UI can't easily reach the Celestial look (no element backdrop blur, linear gradients only, video macOS-only, git-only dependency on Zed); a fork is still a *new* launcher for legality purposes (UNVERIFIED whether a fork inherits approval); long-term divergence |
| **B. Reuse Pandora's `backend`/`auth`/`schema` crates under our own UI** | Proven, UI-agnostic core; MIT allows it if we keep the copyright notice; the bridge pattern matches our "UI can be unloaded" design | The crates aren't published, so we'd vendor or fork them; single-maintainer upstream; we must check API quality, MCSR needs (MultiMC layout, `INST_*` variables) and test coverage |
| **C. Contribute speedrun features upstream to Pandora** | Cheapest path to a legal, performant launcher | Doesn't produce the rich animated UI Piotr wants; we don't control the roadmap |
| **D. New core** (on portablemc or lighty; §7) | Full control | Most work; the legalisation burden is the same as A and B |

**Suggestion:** do **B or D, decided by a one-week code spike** on Pandora's backend against portablemc (Apache-2.0) and lighty (MIT). Option A only makes sense if Piotr drops the Celestial-style visual brief.

---

## 7. Launcher core and backend language

**Choice: Rust.** Idle footprint, measured as peak memory of a hello-world program ([programming-language-benchmarks](https://programming-language-benchmarks.vercel.app/problem/helloworld)):

| Runtime | Peak memory |
|---|---|
| C++ | 1.4 MB |
| **Rust** | **1.8 MB** |
| Go | 3.4 MB |
| .NET 9 NativeAOT | 9.5 MB |
| OpenJDK 21 | 43.9 MB |
| Kotlin/JVM | 47.6 MB |
| Node 24 | 50.7 MB |

In an HTTP-server test, Rust uses 19–26 MB against 232–257 MB for Kotlin/JVM ([http-server](https://programming-language-benchmarks.vercel.app/problem/http-server)).

**Downloads:** reqwest + tokio + RustCrypto sha1/sha2. Both Modrinth's app-lib and Pandora use these ([app-lib Cargo.toml](https://github.com/modrinth/code/blob/main/packages/app-lib/Cargo.toml), [Pandora backend](https://github.com/Moulberry/PandoraLauncher/blob/master/crates/backend/Cargo.toml)).

**File watching:**
- Rust's `notify` uses native inotify, FSEvents, ReadDirectoryChangesW and kqueue ([notify](https://github.com/notify-rs/notify)).
- Go's fsnotify has no recursive watching and no FSEvents backend ([fsnotify](https://github.com/fsnotify/fsnotify)).
- The JVM's WatchService **polls every 2 s on macOS** ([BsdFileSystem.java](https://github.com/openjdk/jdk/blob/master/src/java.base/macosx/classes/sun/nio/fs/BsdFileSystem.java)).

**Processes:**
- `process-wrap` provides Job Object, process group and session wrappers ([repo](https://github.com/watchexec/process-wrap)).
- Windows Job Objects: with `KILL_ON_JOB_CLOSE`, closing the last handle kills every process in the job ([MS docs](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)).
- **So only the long-lived core may hold the job handle.** Otherwise closing the UI would kill Minecraft.

**Java runtimes:** Mojang's `java-runtime` manifest lists alpha (16), gamma (17), delta (21) and epsilon (25) ([all.json](https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json)). Adoptium and Azul APIs are alternatives ([Adoptium](https://api.adoptium.net/v3/info/available_releases)).

**Reusable cores**

| Core | Language | Licence | Status | Use |
|---|---|---|---|---|
| **portablemc** | Rust | Apache-2.0 | v5.0.6 (2026-10-18) | Launch, loaders, Mojang JREs ([repo](https://github.com/theorzr/portablemc)) |
| **LightyLauncherLib** | Rust | MIT | 26.9.3 (2026-09) | Split crates ([repo](https://github.com/Lighty-Launcher/LightyLauncherLib)) |
| **Pandora backend** | Rust | MIT | v6.0.0 | Vendor the crates (§6) |
| Modrinth app-lib | Rust | **GPL-3.0** | Active | Bakes in Modrinth URLs; not a clean library ([app-lib](https://github.com/modrinth/code/tree/main/packages/app-lib)) |
| ferinth / furse | Rust | MIT | Active | Modrinth and CurseForge API clients |
| Prism, HMCL, MCSR Launcher | C++ / Java / Kotlin | GPL-3.0 | Active | Reference only, unless we go GPL |
| GDLauncher Carbon, gorilla daedalus | Rust | **BUSL-1.1** | – | **Avoid** |
| @xmcl/core, minecraft-launcher-core | TypeScript / JavaScript | MIT | Active / stale | Needs Node |

**Metadata sources:**
- [launcher-meta.modrinth.com](https://launcher-meta.modrinth.com/minecraft/v0/manifest.json)
- [meta.prismlauncher.org](https://meta.prismlauncher.org/v1/index.json)
- [mcsrlauncher.github.io/meta](https://mcsrlauncher.github.io/meta/index.json) (generated by MCSRLauncher's MIT-licensed "metabolism")

**MCSR requirements that shape the core**
- Verify mods against the legal-mods hashes ([legal-mods](https://github.com/Minecraft-Java-Edition-Speedrunning/legal-mods), [mcsr-meta mods.json](https://github.com/tildejustin/mcsr-meta/blob/main/mods.json), [legal DLL list](https://github.com/Minecraft-Java-Edition-Speedrunning/legal-builds/blob/main/legal-dlls.csv)).
- Keep separate mod profiles for Ranked and for leaderboard runs ([mc.sr/mods](https://mc.sr/mods/)).
- Tolerate the MCSR Ranked mod updating itself in the mods folder. Its licence forbids redistribution, so download it at install time ([Modrinth](https://modrinth.com/mod/mcsr-ranked)).
- Support:
  - the MultiMC instance layout;
  - the `INST_*` environment variables;
  - a `--launch <instance>` CLI;
  - per-instance wrapper commands (waywall);
  - **no Flatpak distribution**.

(Auth and launch internals are covered by the other research agent.)

---

## 8. Ranked shortlist: pros and cons

### 1. Tauri 2 + Svelte 5 + Rust core (recommended, pending the bake-off)

**Pros**
- On Windows (the main platform):
  - Chromium-quality pacing and effects through WebView2;
  - the **smallest installers** of any web stack (Celestial 20.6 MB);
  - the webview can be **destroyed while the game runs**, leaving a Rust process with a few MB of logic.
- **The fastest way to build a rich UI:** CSS glass, gradients, keyframes, View Transitions on Chromium and WebKit, WebGL particles in workers, `<video>`. Celestial, the reference look, is built exactly this way.
- The Rust core runs in-process, with no second runtime. The same crates can also run as a standalone daemon.
- Mature updater and bundler. Precedents in the same domain: Modrinth App, and Celestial ported from Electron.

**Cons**
- **Linux WebKitGTK** (§2.1, §2.2):
  - likely ~60 Hz on NVIDIA and on GTK3 Wayland;
  - NVIDIA blank-window and Error 71 crashes;
  - `backdrop-filter` breaks under the usual workaround;
  - real launcher reports of runaway CPU and memory.
- **macOS** rAF is capped near 60 Hz.
- Three engines mean three test targets and CSS differences.
- **Windows RAM while the UI is open ≈ Electron's**, about 300 MB summed.
- Tauri v3 migration is ahead; GTK4 is not ported.

### 2. Electron 45 + the same Svelte 5 UI + Rust core daemon

**Pros**
- One engine everywhere, so **consistent smooth animation at high refresh on every OS**, including native Wayland and macOS ProMotion.
- The best performance tooling: Perfetto `contentTracing`, `getAppMetrics`, LoAF, Playwright.
- The fastest measured cold start.
- The UI code is **identical** to option 1, so switching costs little.

**Cons**
- A **~120–160 MB runtime** (likely an ~80–110 MB installer, UNVERIFIED).
- More RAM while open on Linux and macOS.
- Two runtimes (Node and Rust).
- Destroying windows leaves the browser and GPU processes running, so a separate Rust daemon is required for a near-zero in-game footprint.
- CPU video decoding on Linux.

### 3. Qt 6.12 Quick (QML; C++ shell, Rust core over IPC or cxx-qt)

**Pros**
- **The best-documented vsync-locked animation on all three OSes**, plus MultiEffect blur and shadow, ShaderEffect and FFmpeg video inside one scene graph.
- Native Wayland, good accessibility, mature (15+ years), LTS release.
- Probably a much smaller footprint than webviews (UNVERIFIED).

**Cons**
- QML and C++ skills are needed; Rust bindings are immature (cxx-qt).
- LGPLv3 obligations, and some GPL-only modules must be avoided.
- No CSS-style backdrop blur (needs a ShaderEffectSource pattern).
- Slower design iteration than web tech.
- Smooth pacing is guaranteed for one window only.
- No built-in updater.

### 4. Slint 1.19 (Skia)

**Pros:** Rust-native, lazy redraw with a measurable performance overlay, small binaries, live preview and Figma import, stable 1.x API, AccessKit.

**Cons:**
- Blur, shaders and video are immature or missing for this brief.
- The licence needs GPL, or attribution, or payment.

### 5. GPUI or Makepad

Watch both.
- **GPUI** has the strongest published numbers and an MCSR-legal precedent (Pandora), but weak rich-visual primitives and an unstable distribution.
- **Makepad** has the best native visuals (shaders, blur, zero-copy video) but no stable release.

---

## 9. Recommendation and reasoning

**Build on Tauri 2 + Svelte 5 + a UI-agnostic Rust core, and make the bake-off (§12) a go/no-go gate before the UI build starts.**

**Why**
1. **Windows is the main platform, and there Tauri is as smooth as Electron** (both are Chromium) with a far smaller download. Its "unload the UI while the game runs" mode needs no second process.
2. **The visual brief is a web-tech brief.** Celestial is Tauri + React with CSS, workers and Mica. No native toolkit except Makepad (unstable) and Qt Quick (slower to iterate, C++) matches that expressiveness today.
3. **The UI code is portable across shells.** The same Svelte app runs in:
   - Tauri with WebView2;
   - Tauri with the CEF runtime, once v3 stabilises (it fixes Linux pacing without changing shells);
   - Electron, as a fallback.

   A native toolkit choice locks the visuals in.
4. **The in-game footprint, which matters most to speedrunners, is solved by architecture** (§11), not by the UI framework.
5. **The Rust core has the best footprint, file watcher and process control, and permissive reusable bases** (Pandora backend, portablemc, lighty).

**Gate (kill criteria), evaluated on the bake-off hardware matrix**

The bake-off compares three Linux modes:
- **(a) default** WebKitGTK;
- **(b) tuned** WebKitGTK: `__NV_DISABLE_EXPLICIT_SYNC=1` on NVIDIA, a vblank shim like Vanitty's, `GDK_BACKEND=x11` fallback;
- **(c) Tauri v3 CEF runtime**.

Then:
- **If (b) meets the pacing budget** on NVIDIA X11, NVIDIA Wayland and AMD Wayland at ≥ 144 Hz, ship Tauri 2 everywhere.
- **If only (c) meets it, and its idle-CPU bug ([#16189](https://github.com/tauri-apps/tauri/issues/16189)) is fixed,** ship Tauri with CEF on Linux.
- **If neither does,** ship the same UI in **Electron**, with the Rust core as a daemon, on all OSes. One shell is simpler than two. Electron could also be Linux-only, as a fallback.
- **Native escalation:** if the native challenger (Qt Quick, or Slint 1.19) beats the webview stacks by a wide margin on pacing *and* the visual-fidelity checklist passes, reconsider. Agree the margin as part of the bake-off; for example, p99 ≤ 1.2× the refresh interval while webviews exceed 2×, and half the PSS.

---

## 10. Risks of the recommendation and how to reduce them

| Risk | Likelihood / impact | Mitigation |
|---|---|---|
| WebKitGTK at ~60 Hz on NVIDIA and GTK3 Wayland | High / high (Linux users) | Measure first (§12). Apply the vblank shim on NVIDIA ([vanitty #61](https://github.com/stasadance/vanitty/pull/61); a hack, so isolate it behind a flag). Set `__NV_DISABLE_EXPLICIT_SYNC=1` on NVIDIA (Vanitty sets it automatically, [main.rs](https://github.com/stasadance/vanitty/blob/main/src-tauri/src/main.rs)). Track the GTK4 port ([wry #1474](https://github.com/tauri-apps/wry/issues/1474), [#1767](https://github.com/tauri-apps/wry/pull/1767)) and the CEF runtime. Electron fallback. **Read the active vblank monitor from `webkit://gpu` in a diagnostics page** ([bug 266685](https://bugs.webkit.org/show_bug.cgi?id=266685)) and show it in a perf HUD |
| NVIDIA/DMA-BUF blank windows and Error 71 crashes | Medium / high | Auto-detect NVIDIA at startup and apply the safe env vars; crash-loop detection that relaunches with `WEBKIT_DISABLE_DMABUF_RENDERER=1` and a **"safe mode" theme without backdrop-filter**. Consider [tauri-plugin-wayland-nvidia-quirk](https://crates.io/crates/tauri-plugin-wayland-nvidia-quirk) |
| Glass blur costs GPU memory and breaks without DMA-BUF | Medium / medium | Design glass as **pre-blurred background images** (blur once on the CPU, cache, use as a panel background); allow at most 1–2 live `backdrop-filter` surfaces and never animate blur; keep a fallback style for engines without backdrop-filter |
| Video backgrounds: CPU decode on Linux, GStreamer codec gaps, AppImage bundling | Medium / medium | Default to a **shader or animated-gradient background** (WebGL in a worker, capped fps, paused when hidden); video is opt-in, low bitrate, and paused when not visible; test codecs per distro; ship .deb/AUR builds alongside the AppImage |
| macOS rAF capped near 60 | High / low (minority) | Drive animations with CSS and WAAPI (compositor), not JS rAF loops; accept 60 Hz rAF; don't use the private-API plugin for an App Store build |
| Windows RAM while the UI is open ≈ Electron | Certain / medium | Unload the UI during play (§11); `MemoryUsageTargetLevel=Low` plus `TrySuspendAsync` when hidden; lazy-load heavy pages; keep the DOM small (virtualised lists) |
| Tauri v2→v3 churn | Medium / low | Isolate Tauri APIs behind a thin `shell` module; stay on 2.x until v3 is stable |
| Updater has no deb/rpm; AppImage glibc issues | Medium / low | AppImage built on an old base plus deb/AUR through the repos; **no Flatpak** (breaks waywall) |
| Code signing: Azure Artifact Signing individual accounts are US/Canada only | Medium / medium | Use an organisation account (EU orgs qualify) or an OV certificate; start identity validation early (1–20 business days) ([MS](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart)) |
| Speedrun legality | Certain / high | MultiMC-compatible instance layout and `INST_*` variables; Julti/Jingle-parsable command line; `--launch` CLI; Toolscreen compatibility; legal-mods hash checks; engage the MCSR Discord early; consider building on Pandora's backend (§6) |
| Effects bloat over time | Medium / medium | **CI performance budgets (§12.9)** and a built-in perf HUD; a "low-end mode" like Celestial's |

---

## 11. Launcher behaviour while the game runs

| Option | In-game footprint | Resume latency | Keeps features (log streaming, playtime, crash detect, multi-instance control) | Notes |
|---|---|---|---|---|
| **1. Minimise to tray, webview suspended** | Tauri/WebView2: memory stays (swappable); CPU ≈ 0 if `IsVisible=false` + `TrySuspendAsync`. WebKitGTK 2.48+ pauses rendering when the toplevel is suspended ([2.48](https://webkitgtk.org/2025/04/08/webkitgtk-2.48.html)). Electron: `backgroundThrottling` | Instant | Yes | Needs manual wiring in Tauri (wry doesn't set `IsVisible` on minimise). Occluded windows are *not* treated as hidden ([tauri #6864](https://github.com/tauri-apps/tauri/issues/6864)) |
| **2. Destroy the webview, keep the core process with a tray icon** (recommended default) | Tauri: only the Rust process. On Linux the GTK and WebKit libraries stay mapped; **measure** (UNVERIFIED, expected tens of MB RSS, much less PSS). Electron: browser and GPU processes stay | Cold webview start, ~0.3–1 s; hide it behind a native splash | Yes (the core owns processes and logs) | `prevent_exit` keeps Tauri alive ([docs.rs](https://docs.rs/tauri/latest/tauri/struct.ExitRequestApi.html)); persist UI state; recreate the window from the tray |
| **3. Exit completely** | 0 | Full cold start | No (loses log capture, playtime, "relaunch instance") | Optional "Close launcher when the game starts" setting, like the vanilla launcher and Prism |
| **4. Tiny native companion daemon; the UI process exits** | A few MB (Rust tray apps measure < 2–6 MB, [OLEDShift](https://github.com/Marko19907/OLEDShift), [mulltray](https://github.com/andriamanitra/mulltray)) | Same as option 2 | Yes | **Required for Electron.** For Tauri, use it only if option 2's measured footprint exceeds the budget. The daemon holds the Job Object and process handles; IPC through `interprocess` ([crates.io](https://crates.io/crates/interprocess)); tray through `tray-icon` ([repo](https://github.com/tauri-apps/tray-icon)) |

**While the game runs, in every option:**
- No background downloads or indexing unless the user starts them.
- Lower the core's priority:
  - Windows: EcoQoS through `SetProcessInformation(ProcessPowerThrottling)` plus `BELOW_NORMAL_PRIORITY_CLASS` ([MS](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setprocessinformation));
  - Linux: `SCHED_IDLE` or `cpu.idle` in the cgroup ([cgroup v2](https://docs.kernel.org/admin-guide/cgroup-v2.html)).
- Batch log tailing (no per-line UI updates while the UI is hidden).

Make options 2, 3 and 1 user-selectable, with option 2 as the default.

---

## 12. Bake-off benchmark plan

### 12.1 Candidates

| ID | Stack | Variants |
|---|---|---|
| **T** | Tauri 2.12 + Svelte 5 | Windows (WebView2); macOS (WKWebView); Linux: T-a default WebKitGTK, T-b tuned (NVIDIA env vars, vblank shim, `GDK_BACKEND=x11`), T-c Tauri 3 alpha with the CEF runtime |
| **E** | Electron 45 + the *same* Svelte 5 build | Linux: native Wayland and `--ozone-platform=x11` |
| **N** | Native challenger: **Qt 6.12 Quick** (primary) | Optional N2: Slint 1.19 Skia (cheap to add with `.slint` markup) |
| **B** | Real-launcher baselines, measured with the same harness | No new code: Celestial (Tauri), Modrinth App (Tauri), Pandora (GPUI), Prism (Qt Widgets), MCSR Launcher (Swing) |

### 12.2 The scene (identical in every stack)

- **Window:** 1280×800 logical px at 100% and 150% scale, plus a full-screen pass at native resolution.
- **Layer 0, animated background, two variants:**
  - (a) a looping muted 1080p30 video (H.264 and VP9 files);
  - (b) a GPU shader background (animated gradient plus 300 particles), WebGL in a worker for web stacks, ShaderEffect or Canvas for Qt.
- **Layer 1, glass:** three panels with a 24 px backdrop blur, a 1 px gradient border and a 32 px drop shadow, over the moving background.
- **Page transition:** every 2 s, switch page A↔B: 300 ms slide plus fade (transform and opacity only), with the incoming panel blurring in.
- **List:** 500 rows. Each row has:
  - a 64×64 WebP thumbnail (500 distinct local files, decoded asynchronously);
  - two text lines;
  - a badge and a hover state.

  It is virtualised. The scripted scroll runs at 3000 px/s for 10 s, then a fling, then jumps to the top and bottom.
- **Idle page:** static UI, no animation (background paused). This measures the true idle cost.
- **Assets and seed:** shared assets and a fixed random seed, so every frame's content is comparable.

### 12.3 Scenario script, driven from inside the app

Each prototype implements `--bench-scenario=full --bench-out=<file.json>` and runs these phases with timestamps written to the log:

1. **Launch to first presented frame.** Log it, and also exit early for startup runs (`--bench-exit-after-first-frame`).
2. **Warm-up**, 5 s.
3. **Background only**, 20 s (variant a, then b).
4. **Transitions**, 20 s.
5. **List scroll**, 20 s.
6. **All at once**, 20 s.
7. **Idle**, static, 60 s.
8. **Minimised**, 60 s.
9. **Hidden to tray**, 60 s.
10. **Fully occluded** by another window, 60 s.
11. **UI destroyed, core only**, 60 s (not applicable to N).
12. **Restore**, measuring time to the first frame of the restored UI.
13. **Exit.**

Drive animation by elapsed time, not frame count. Use WebDriver or CDP only for correctness tests, never during timing runs.

### 12.4 In-app instrumentation

**Per-frame record:** `{frame, cpu_ts_ns, present_ts_ns?, phase}`.

**Presentation timestamps (ground truth, where available):**
- **Wayland:** `wp_presentation` feedback, which gives presented and discarded frames, the refresh period and `vsync` flags ([protocol](https://wayland.app/protocols/presentation-time)). Supported by Mutter, KWin, Sway, Weston and niri. Native toolkits can request it directly. For webviews, rely on external tools.
- **Windows:** PresentMon's `DisplayedTime` (§12.6), joined by timestamp. Use `--qpc_time_ms`.

**Web stacks**
- A rAF-delta logger. **rAF time is not presentation time**, so use it as a correlation signal only ([MDN presentationTime](https://developer.mozilla.org/en-US/docs/Web/API/PerformancePaintTiming/presentationTime)).
- **Long Animation Frames (LoAF)** on Chromium only. It reports only frames over 50 ms, so it's useless for 144 Hz jank but good for catching main-thread stalls ([Chrome](https://developer.chrome.com/docs/web-platform/long-animation-frames)).
- `performance.mark` around page transitions.

**Native stacks**
- Tracy through `tracing-tracy` ([rust_tracy_client](https://github.com/nagisa/rust_tracy_client)), plus `wgpu-profiler` for GPU scopes ([repo](https://github.com/Wumpf/wgpu-profiler)).
- Qt: `QT_LOGGING_RULES="qt.scenegraph.time.renderloop.debug=true"` and `QSG_INFO=1` ([Qt](https://doc.qt.io/qt-6/qtquick-visualcanvas-scenegraph.html)).
- Slint: `SLINT_DEBUG_PERFORMANCE=refresh_lazy,console`, which counts redraws ([Slint](https://docs.slint.dev/latest/docs/slint/guide/development/debugging_techniques/)).

**Diagnostics page:** shows engine version, GPU, refresh rate, the WebKit vblank monitor type (`webkit://gpu` on Linux), DMA-BUF on or off, and the scale factor. Every result file records these.

### 12.5 Metrics and how each is computed

**Frame pacing,** per phase, using presentation timestamps:
- presented frames ÷ (phase duration × refresh rate);
- the share of frames shown longer than 1.5× the refresh interval;
- p50, p95 and p99 of the presented interval;
- the longest stall;
- PresentMon `MsAnimationError` p99 on Windows.

**Cold and warm start:**
- Run with hyperfine `-N --warmup 3 --runs 30 --export-json`.
- Cold on Linux: `--prepare 'sync; echo 3 | sudo tee /proc/sys/vm/drop_caches'`.
- Cold on Windows: `RAMMap -Et`. Close Edge, and record whether it was running, because WebView2 starts faster when Edge is warm ([MS perf](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance)).
- Sources: [hyperfine](https://github.com/sharkdp/hyperfine) (v2.0.0, 2026-10-07; Ubuntu ships 1.19, local).

**Memory: sum PSS across the whole process tree.**
- **Linux:** run under `systemd-run --user --scope -p MemoryAccounting=yes` and read `memory.current`, `memory.peak` and `memory.stat`. Also sum `Pss`, `Pss_Anon` and `Pss_File` from `/proc/<pid>/smaps_rollup` for every descendant: `WebKitWebProcess`, `WebKitNetworkProcess`, `WebKitGPUProcess`; Electron's browser, renderer, GPU and utility processes ([kernel proc docs](https://docs.kernel.org/filesystems/proc.html), [cgroup v2](https://docs.kernel.org/admin-guide/cgroup-v2.html)).
  - **Why PSS:** shared libraries are counted once across processes. RSS sums over-count them, and USS drops them entirely.
- **Windows:** sum `Working Set - Private` and `Private Bytes` for the app and every `msedgewebview2.exe` or Electron child. Give the benchmark its own WebView2 user-data folder ([MS memory counters](https://learn.microsoft.com/en-us/windows/win32/memory/memory-performance-information), [WebView2 process model](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model)). VMMap for deep dives.
- **macOS:** `footprint -p <pid> -j out.json` for the app and its WebContent XPC processes, by name ([man page](https://leancrew.com/all-this/man/man1/footprint.html)).
- **Electron:** also record `app.getAppMetrics()` ([docs](https://www.electronjs.org/docs/latest/api/structures/process-metric)).

**CPU and wakeups:**
- Linux: cgroup `cpu.stat` `usage_usec` deltas for the whole tree, and `pidstat -u -w -h 1` for per-process context switches.
- Windows: the `\Process(*)\% Processor Time` counters for every PID in the tree (map instance names to PIDs). Electron's `idleWakeupsPerSecond`.

**GPU:**
- Linux, cross-vendor: per-process `/proc/<pid>/fdinfo` `drm-engine-*` ns deltas, plus `drm-resident-*` for GPU memory ([kernel DRM usage stats](https://docs.kernel.org/gpu/drm-usage-stats.html)).
- NVIDIA: `nvidia-smi pmon -s u -d 1` (UNVERIFIED flags).
- Intel: `intel_gpu_top -J` ([man page](https://manpages.debian.org/testing/intel-gpu-tools/intel_gpu_top.1.en.html)).
- AMD: `amdgpu_top -J` ([repo](https://github.com/Umio-Yasuno/amdgpu_top)).
- Windows: `\GPU Engine(pid_<PID>_*engtype_3D)\Utilization Percentage` ([MS Q&A](https://learn.microsoft.com/en-us/answers/questions/5641645/how-to-get-the-special-process-gpu-usage-with-the), [DirectX blog](https://devblogs.microsoft.com/directx/gpus-in-the-task-manager/)).

**Size:** installer, installed size, and the size of the update delta.

### 12.6 Tools per OS

**Windows (primary)**
- **PresentMon 2.6.0** console ([releases](https://github.com/GameTechDev/PresentMon/releases), [console README](https://github.com/GameTechDev/PresentMon/blob/main/README-ConsoleApplication.md)):

  ```
  PresentMon-2.6.0-x64.exe --process_name msedgewebview2.exe --process_name speedrunluncher.exe ^
    --output_file run.csv --multi_csv --delay 2 --timed 30 --terminate_after_timed --qpc_time_ms
  ```

  - Needs admin, or membership of "Performance Log Users".
  - Use the columns `DisplayedTime`, `MsBetweenDisplayChange`, `MsAnimationError` and `PresentMode`.
  - **First validation task:** confirm PresentMon sees Chromium's DirectComposition presents from the WebView2 and Electron GPU processes (UNVERIFIED). If it doesn't, use `dwm.exe` rows plus a Chromium trace.
- **WPR/WPA** with the "GPU activity" and "Desktop composition activity" profiles, plus **GPUView** for the flip queue against vsync ([WPR profiles](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/built-in-recording-profiles), [GPUView](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/using-gpuview)). The WebView2 ETW profile is at [aka.ms/wv2etw](https://aka.ms/wv2etw).
- **Chromium tracing:**
  - WebView2: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`, then Playwright `connectOverCDP` ([Playwright](https://playwright.dev/docs/webview2)). Or Tauri's `additional_browser_args`, re-adding wry's default flags ([docs.rs](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html)).
  - Electron: `contentTracing` writes Perfetto protobuf ([docs](https://www.electronjs.org/docs/latest/api/content-tracing)).
  - In the trace, read the `FrameSequenceTracker`, `PipelineReporter` and `FramePresented` gaps ([debug-janks.md](https://chromium.googlesource.com/chromium/src/+/main/docs/speed/debug-janks.md)).
- **Counters:** `typeperf` / `Get-Counter` for Process and GPU Engine counters (exact syntax UNVERIFIED).
- **Cold cache:** `RAMMap -Et`.
- **Startup:** hyperfine.

**Linux**
- **Ground truth:** `wp_presentation` in native prototypes. For webviews, use **Sysprof**, where WebKit emits marks and Mesa adds marks, giving vblank → compositor → GTK → WebKit ([Sysprof marks](https://feaneron.com/2025/09/08/marks-and-counters-in-sysprof/), [WebKit profiling](https://feaneron.com/2024/07/12/profiling-a-web-engine/)). Ubuntu ships Sysprof 50 (local).
- **Perfetto `tracebox`** with ftrace `drm:drm_vblank_event`, `dma_fence:*` and `gpu_scheduler:*` ([Perfetto system tracing](https://perfetto.dev/docs/getting-started/system-tracing)). Ubuntu ships perfetto 53 (local).
- **MangoHud** v0.8.4 for Vulkan and GL apps (native toolkits; Electron with `--ozone-platform=x11`). Logging: `MANGOHUD_CONFIG="output_folder=/tmp/mh,autostart_log=1,log_duration=30,log_interval=0"`. There is no `MANGOHUD_LOG` variable ([README](https://github.com/flightlessmango/MangoHud), [#2018](https://github.com/flightlessmango/MangoHud/issues/2018)). Its use with WebKitGTK is UNVERIFIED.
- **WebKitGTK overlays and tooling:** `WEBKIT_SHOW_FPS=1` and `WEBKIT_SHOW_DAMAGE=1` (present in libwebkit2gtk 2.52.6, local `strings`; [bug 176928](https://bugs.webkit.org/show_bug.cgi?id=176928)); `webkit://gpu`.
- **Tracy** v0.14.1 for native stacks ([releases](https://github.com/wolfpld/tracy/releases)). Zed's recipe is a good model ([zed.dev](https://zed.dev/docs/performance)).
- **Memory, CPU and GPU:** cgroup scope plus `smaps_rollup`, `pidstat`, DRM fdinfo, and the vendor tools above.

**macOS**
- `xcrun xctrace record --template 'Animation Hitches'` (or `'Metal System Trace'`) `--time-limit 30s --output run.trace --launch -- <app>`, then `xctrace export --toc` / `--xpath` ([xctrace man](https://keith.github.io/xcode-man-pages/xctrace.1.html)). Whether Animation Hitches works for Mac apps is UNVERIFIED.
- `footprint -j`.
- Safari Web Inspector Timelines → Frames for WKWebView ([WebKit](https://webkit.org/web-inspector/timelines-tab/)).
- `powermetrics` (flags UNVERIFIED).

### 12.7 Hardware matrix (minimum)

| Machine | OS / session | Display | Purpose |
|---|---|---|---|
| W1: Windows 11, NVIDIA RTX (desktop) | Windows | 240 Hz (+ a 144 Hz second monitor) | Main platform; mixed refresh rates; HAGS on and off |
| W2: Windows 11 laptop, iGPU | Windows | 144 Hz | Low end; integrated GPU |
| L1: Linux, NVIDIA proprietary (≥ 555) | GNOME Wayland **and** X11 | 144/240 Hz | The WebKitGTK worst case |
| L2: Linux, AMD | KDE Wayland | 144 Hz | GTK3 frame-clock question |
| L3: this machine (Intel Tiger Lake + RTX 3050 Ti Mobile, Ubuntu 26.04 GNOME Wayland) | Wayland | Panel rate | Optimus / PRIME |
| M1: Mac with ProMotion | macOS 26 | 120 Hz | WKWebView rAF cap |

Every run records driver versions, compositor, VRR state and power plan.

**Statistics:**
- At least 10 runs per cell.
- Report the median and p95, p99 of the run medians.
- Compare stacks with Mann-Whitney U **plus** a minimum effect size, because Mann-Whitney alone over-flags noise ([Mozilla bug 2056802](https://bugzilla.mozilla.org/show_bug.cgi?id=2056802)).

### 12.8 Real-launcher baselines

**These are executed in the bake-off phase, not now.**

Run Celestial, Modrinth App, Pandora, Prism and MCSR Launcher through the idle, minimised, hidden and "game running" phases. This gives:
- real-world anchors, e.g. how much a Tauri launcher (Modrinth) costs on Windows versus Pandora (GPUI) and Prism (Qt);
- a direct answer to "beat the launchers runners use today".

### 12.9 Wiring performance budgets into CI

**Tier 1: every PR, on GitHub-hosted runners**
- **Context:** public-repo runners have 4 vCPUs and 16 GB, with **no GPU** ([GitHub docs](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)).
- **Checks:**
  - **Size budgets:** installer and binary size (`cargo bloat`, JS bundle size through size-limit; neither tool re-checked this session, UNVERIFIED).
  - **Startup to first frame and PSS** under Xvfb or headless Weston with llvmpipe or lavapipe (`LIBGL_ALWAYS_SOFTWARE=true`, `VK_DRIVER_FILES=…/lvp_icd.json`) ([Mesa env vars](https://docs.mesa3d.org/envvars.html), [Vulkan loader](https://github.com/KhronosGroup/Vulkan-Loader/blob/main/docs/LoaderInterfaceArchitecture.md)). **Use these as relative-regression signals only.**
  - **Instruction counts** through `hyperfine --metrics` (2.x) for deterministic CPU signals ([hyperfine](https://github.com/sharkdp/hyperfine)).
  - **Microbenchmarks:** the Rust core (download, hash, file scanning) through divan or criterion; the Svelte list render through a JS benchmark.
- **Thresholds:** report to **Bencher** with `t_test`, `percentage` or `iqr` thresholds and `--error-on-alert` ([Bencher thresholds](https://bencher.dev/docs/explanation/thresholds), [hyperfine adapter](https://bencher.dev/docs/explanation/adapters/)). Or use **github-action-benchmark** with `customSmallerIsBetter`, `alert-threshold` and `fail-on-alert` ([repo](https://github.com/benchmark-action/github-action-benchmark)). CodSpeed walltime needs its own bare-metal runners ([docs](https://codspeed.io/docs/instruments/walltime)).

**Tier 2: nightly, plus pre-release, on self-hosted machines with real GPUs and high-refresh panels (W1 and L1 at least)**
- Run the full §12.3 scenario through PresentMon or Sysprof/MangoHud and post the results to Bencher.
- **Fail the release if a frame-pacing or idle budget from §1 is breached.**
- Keep the panels awake and unlocked, and pin the power plans.
- This follows how Firefox (Perfherder, about 2% alert thresholds, [docs](https://firefox-source-docs.mozilla.org/testing/perfdocs/perf-sheriffing.html)) and Chromium (Pinpoint bisection, [docs](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/docs/speed/addressing_performance_regressions.md)) run performance CI.

**Field data**
- Ship the perf HUD and `--bench-scenario` in release builds, hidden behind a setting.
- An opt-in "send performance report" posts the JSON (frame histogram, engine and GPU info). This is how Linux and NVIDIA regressions get caught on hardware we don't own.

### 12.10 Decision rule and deliverables

**Score per OS, weighted by user share.** Suggested weights: Windows 0.6, Linux 0.3, macOS 0.1.

| Factor | Weight |
|---|---|
| Pacing | 35% |
| Idle and hidden cost | 25% |
| Visual fidelity checklist (pass/fail per effect) | 15% |
| Cold start | 10% |
| Size | 5% |
| Dev effort observed while building the scene | 10% |

**Deliverables:**
- a results table;
- frame-time plots (median and p99 per phase);
- the go/no-go result against the §9 gate.

---

## 13. Still uncertain or UNVERIFIED

1. **WebKitGTK above 60 Hz:**
   - Exactly which setups reach the display rate and which are capped. The evidence conflicts: Vanitty ran at ~490 fps on X11 with a shim, while GTK3 Wayland measured 60 Hz.
   - Whether the GTK4 port (webkitgtk-6.0; Wails v3 uses it) avoids the cap.
2. **Whether PresentMon captures WebView2 and Electron DirectComposition frames.**
3. **Real idle and hidden RAM and CPU** for Qt Quick, Slint, Flutter and Avalonia apps, and for the Tauri main process after its webview is destroyed (Linux keeps GTK and WebKit mapped). There are no credible published numbers. The bake-off measures them.
4. **The Electron installer size** for v45, and Electron's footprint with all windows destroyed.
5. **Whether speedrun.com rule A.1.1 lists Pandora and Nixcraft.** The coordinator's source says yes; mc.sr/mods, fetched today, says no. Also how the legalisation process works for forks.
6. **Celestial's Electron→Tauri migration** (inferred from strings).
7. **Which Impeller backend Flutter uses** on Windows and Linux (Vulkan vs GLES/ANGLE).
8. **Exact tool flags:** `nvidia-smi pmon`, `wpr -start GPU`, `powermetrics`, and Windows counter syntax; whether MangoHud works with WebKitGTK.
9. **Code-signing eligibility for Piotr specifically** (individual vs organisation, country).
10. **Stacks of** Feather/Dawn, SKLauncher, HoYoPlay, Ubisoft Connect, Battle.net and Epic.
11. **Elanis and Better Stack numbers** come from noisy or atypical setups. Use them only to rank relatively.
