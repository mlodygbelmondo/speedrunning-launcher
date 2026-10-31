# bakeoff-gpui

The GPUI contender for the Speedrunluncher UI bake-off (see `../SPEC.md`). It's a standalone Cargo
project, not a workspace member, and it builds the binary `bakeoff-gpui`.

- GPUI comes from Zed git at `fc9258b4632544c0819953c59e2c31b9e92ea362`, the same revision Pandora
  Launcher's `Cargo.lock` pins. It uses `gpui` plus `gpui_platform` with the features
  `font-kit`, `x11`, `wayland` and `runtime_shaders`, as Pandora does. No gpui-component.
- `Cargo.lock` was seeded from Pandora's lockfile, so the crates.io versions match a known-good set.
- All assets are compiled in: the fonts via `include_bytes!` and `text_system().add_fonts`, the images
  and icons through an embedded `AssetSource` (`build.rs` generates the 500-avatar table, the
  `assets/icons/*.svg` table and the `assets/art` table), and `data.json`, `copy.json`, `copy.pl.json`,
  `screens.json` and `screens.pl.json` via `include_str!`. `build.rs` lists `assets/art/hero/*` and
  `assets/art/card/*` at build time and reruns when a file there is added or replaced, so new key
  art only needs a rebuild.
  The binary never reads the working directory.

## Code layout

| File | What |
|---|---|
| `src/main.rs` | entry point: report init, `BAKEOFF_SCREEN`, `BAKEOFF_LANG`, window options |
| `src/app.rs` | the root view `Scene`: state, deep links, navigation + page transition, page header v2, toast, frame bookkeeping |
| `src/shell.rs` | scene v3 top nav (REDESIGN.md "Shell") |
| `src/v3.rs` | scene v3 tokens and primitives: Archivo/Plex text, tracked text, wrapping, overlays, outline rings, Play button, pixel avatars |
| `src/scenario.rs` | the measured scenario (SPEC.md), unchanged in behaviour from scene v1 |
| `src/report.rs` | JSON Lines report |
| `src/ui.rs` | shared components (SCREENS.md): card, buttons, chip, badge, toggle, segmented control, checkbox, radio, progress, status icon, setting row, QR tile, spinner |
| `src/screens/*.rs` | one file per screen: home, library, onboarding, signin (shared by onboarding + Accounts), accounts, modes, check, practice, learn, tools, stats, submit, settings, game. Home, Modes and Practice are scene v3 (REDESIGN.md); the rest are scene v2 pages under the v3 top nav |
| `src/copy.rs` | the current language (en, pl) and `t(key)` / `tf(key, &[(k, v)])` over `copy.json` / `copy.pl.json` (current language, then English, then the key); every UI string goes through it |
| `src/data.rs` | typed `data.json`, and one typed `screens.json` per language (the Polish one is `screens.json` deep-merged with the `screens.pl.json` overlay as `serde_json::Value`, then deserialized) |
| `src/theme.rs` | colour tokens, fonts, CSS cubic-bezier easing |
| `src/assets.rs` | embedded `AssetSource` |

## Build

Linux (Ubuntu/Debian packages). The binary links only libxcb and libxkbcommon(-x11). Wayland,
Vulkan and fontconfig are loaded at runtime.

```sh
sudo apt install build-essential pkg-config libxkbcommon-dev libxkbcommon-x11-dev \
  libxcb1-dev libwayland-dev libfontconfig-dev libfreetype-dev libvulkan1
cargo build --release          # -> target/release/bakeoff-gpui
```

Windows (GitHub Actions `windows-latest`, MSVC toolchain):

```powershell
rustup toolchain install stable --profile minimal   # x86_64-pc-windows-msvc
cargo build --release          # -> target\release\bakeoff-gpui.exe
```

Windows-specific notes:
- In release builds, `gpui_windows`'s build script compiles its HLSL shaders with **`fxc.exe`** from the
  Windows 10/11 SDK. `windows-latest` ships the SDK, so it's found through the registry. Elsewhere,
  put `fxc.exe` on `PATH` or set `GPUI_FXC_PATH`.
- `gpui` embeds an application manifest through `embed-resource`, so the MSVC `rc.exe` from the
  same SDK must be available. It is on `windows-latest`.
- Release builds use `windows_subsystem = "windows"`, so no console window opens. Reports go to
  `BAKEOFF_OUT`, never to stdout.
- With fat LTO and `codegen-units = 1`, a clean build takes a few minutes. Cache `~/.cargo` and
  `target/` in CI.

## Run

```sh
target/release/bakeoff-gpui                                   # interactive
BAKEOFF_SCREEN=modes target/release/bakeoff-gpui              # open on a screen id (SCREENS.md + REDESIGN.md)
BAKEOFF_LANG=pl target/release/bakeoff-gpui                   # start in Polish (default en)
BAKEOFF_OUT=/tmp/gpui.jsonl BAKEOFF_PHASE_MS=2000 target/release/bakeoff-gpui   # scenario
```

`BAKEOFF_SCREEN` and `BAKEOFF_LANG` are ignored in scenario mode, which always runs in English. An
unknown id prints a warning and opens Home; an unknown language prints a warning and uses English.
Settings → Language switches every string, the sample content, an open toast and the window title
at once (`window.set_window_title`).

### Screenshots (Linux, headless)

`../tools/shoot-gpui.sh <outDir> [ids...]` shoots all 26 screen ids (or the ids given) to
`<outDir>/gpui-<id>.png`; `BAKEOFF_LANG=pl ../tools/shoot-gpui.sh /tmp/srl/v3/gpui-pl` shoots Polish.
By hand:

GPUI picks Wayland when `WAYLAND_DISPLAY` is set, so unset it and use an Xvfb screen larger than the
window (with no window manager the window has no decorations; capture it by window id):

```sh
Xvfb :57 -screen 0 1600x1000x24 &
env -u WAYLAND_DISPLAY DISPLAY=:57 GPUI_X11_SCALE_FACTOR=1 BAKEOFF_SCREEN=stats target/release/bakeoff-gpui &
sleep 1.5; DISPLAY=:57 import -window "$(DISPLAY=:57 xdotool search --name bake-off | head -1)" gpui-stats.png
```

## Implementation notes and GPUI gaps (bake-off findings)

- **Animations** are time-driven by hand. `render()` reads `Instant::now()`, computes the page and
  glow transforms, and calls `window.request_animation_frame()` only while something moves: a page
  transition, `glowRunning`, a recorded phase or the scroll. When none of these is active, the app
  requests no frames. The easing is a CSS `cubic-bezier` port (ease-in, ease-out and ease-in-out),
  so the curves match the web scene. GPUI's `with_animation` wasn't used because it can't be driven
  by `glowRunning` and is skipped under `reduce_motion`.
- **No CSS transforms on divs.** GPUI has `Transformation` only for `svg()`.
  - Page translate: `position: relative; top: Ypx`. This is a paint-equivalent offset that doesn't
    affect siblings. GPUI re-lays out the visible tree every frame anyway.
  - Glow scale: the absolutely positioned 900 px `img` is resized to `900*s` around the same centre,
    which gives the same pixels as `scale(s)`.
- **Opacity is per primitive, not a group layer.** `.opacity()` multiplies the alpha of every quad,
  glyph and sprite underneath. CSS `opacity` composites the subtree first. Mid-fade, overlapping
  children such as the hero text over the image blend slightly differently.
- **No rounded-rect clipping.** Content masks are rectangular, so the hero image carries its own
  corner radius and the border is drawn as an overlay. The hover background of the first and last
  library rows isn't clipped to the list's 14 px corners.
- **Inactive-window throttling.** By default, GPUI caps unfocused windows at about 30 fps
  (`WindowOptions::inactive_frame_interval`). Browsers don't do this for visible windows, so this app
  sets it to `None` to keep the transitions and scroll phases comparable.
- **Native decorations on Wayland.** `WindowDecorations::Server` needs `xdg-decoration`. GNOME/Mutter
  and Weston don't provide it, so the window has no title bar there. A real launcher would have to
  draw client-side decorations. X11, Windows and KDE are fine.
- **Library:** a `uniform_list` with a `UniformListScrollHandle`. The scroll phase sets
  `base_handle.set_offset()` each frame from the elapsed time, bouncing between 0 and `max_offset`.
  Only the visible rows are built. GPUI decodes the avatars lazily on a background thread and caches
  them. There is no scrollbar, because GPUI doesn't draw one by itself.
- **Scenario timing** uses `cx.spawn` plus `background_executor().timer()` with absolute deadlines.
  `first_frame` comes from the second `on_next_frame` callback after the first render, because
  frame N's callbacks run just before frame N is presented. This mirrors the web double-rAF.
- **minimized:** `window.minimize_window()`. Renders are counted and reported as
  `{"ev":"info","phase":"minimized","raf_count":N}`. **destroyed:** `window.remove_window()` with
  `QuitMode::Explicit`, so the process stays alive. After the phase it emits `done` and calls
  `cx.quit()`, which exits with code 0.
- **Scenario on scene v3:** it still toggles Home and Library. Home is a key-art page without the glow
  (as in the web build), Library keeps it, so the glow pulse runs on every Library leg.

### Prototype screens (scene v2, SCREENS.md)

- **Icons:** the Lucide stroke SVGs (`stroke="currentColor"`, `fill="none"`) render as-is through
  `svg().path()`. usvg resolves `currentColor` to black, GPUI keeps only the alpha channel and tints
  it with the element's own `text_color`, so no preprocessing is needed. An `svg()` doesn't inherit
  the parent's text colour: every icon sets it, and hover colours use `group_hover`.
- **Animations** follow the same rule as the scene: frames are requested only while something moves
  (page transition, toggle knob 120 ms, spinner, toast fades, modal fade, fake-install bar). Holds
  without motion (toast on screen, "Add account" highlight, the 400 ms "Fix and launch" wait) use
  executor timers. Idle CPU on every static screen matches scene v1. The spinner runs as long as
  something is "working", so the device-code panel keeps a frame loop while it's open.
- **Modal fade:** `.opacity()` multiplies the alpha of every primitive in the subtree, which is the
  "interpolate bg/border/text alpha" option from SCREENS.md; overlapping children blend slightly
  differently mid-fade than a composited group would. The overlay and modal use `occlude()` so
  clicks don't fall through.
- **No rounded clipping:** tutorial thumbnails carry their own top radii (13 px inside the 1 px
  border), the welcome banner carries its own radius plus a border overlay, list rows that get a
  highlight round their own first/last corners.
- **No letter-spacing:** the device code is drawn without the 3 px tracking (as SCREENS.md allows).
- **Text input:** the Submit video field is a click-to-focus box; Paste fills the sample link.
- **QR code:** painted on a `canvas` as run-length merged quads, module edges rounded to whole
  pixels (4.5 px modules → 149 px code, 185 px tile with the 4-module quiet zone).
- **No scrollbars:** Practice, Learn and the Modes layer list scroll (`overflow_y_scroll` with an
  id), but GPUI draws no scrollbar, so cards use the full width where the web build leaves a
  gutter.
- **Avatars** use GPUI's default (linear) image sampling; there's no nearest-neighbour option.
- **Language:** the current language is one process-wide value in `copy.rs` that `t()` and
  `screens()` read, so render helpers need no extra parameter; `Scene::set_language` is its only
  writer after startup. Both languages are parsed at startup, so a switch only redraws. Toasts keep
  their copy key, not the text, so an open toast follows a switch. Polish layout fixes: the onboarding
  mode list and the sign-in option cards wrap and grow; a long mode name, practice category or mode
  subtitle ends in an ellipsis instead of pushing a badge out; the Tools auto-update column is 144 px
  so the pinned badge fits.

### Scene v3 (REDESIGN.md)

- **Letter-spacing** (`v3::spaced`): GPUI has none, and a spacer glyph after every letter breaks
  kerning pairs and rounds to the spacer's width. So the label is shaped once in one run (kerning
  and fallback included) through a `WindowTextSystem` on the app's text system, and a `canvas`
  paints each glyph moved right by `track` em per preceding character, in a box as wide as CSS
  makes it (trailing tracking included). Caps labels, the big titles (negative tracking), the Play
  hint (0.04 em) and the Home stats (−0.01 em) go through it. Shaped labels are cached per string.
  Painted text doesn't wrap and can't take a hover colour from a style, so hover colours stack a
  second copy at opacity 0 that `group_hover` shows.
- **Line wrapping** (`v3::wrap`): GPUI's wrapper counts the space after a line's last word and
  breaks before "·", so its lines come out a word shorter than Chromium's. Paragraphs on the v3
  pages get their line breaks from a greedy wrap on the shaped width instead, then render nowrap.
  The Practice tile lines use the tile width of the 1280 px window.
- **Fallback glyphs:** ▲ ▼ aren't in Plex Mono. Shaped text falls back to DejaVu Sans Mono (Plex)
  or DejaVu Sans (others), which is what Chromium's `monospace` / `sans-serif` give on Linux. On a
  system without DejaVu the platform fallback applies.
- **Outline with offset:** `ring()` is a 2 px border 5 px outside the tile; the 1 px hover outline
  at offset 3 is `hover_ring()`. **Pixel corners:** the Play button is two overlapping rects.
  **Gradients:** 2-stop layers stacked; the Home hero uses `shade-hero.png`. **No filters:** dimming
  is overlay rects.
- **Pixel avatars:** GPUI samples images linearly, so the 64×64 avatars (8×8 cells) are decoded once
  with the `png` crate, cached as 64 colours and painted as whole-pixel quads on a `canvas`.
  Library avatars still use linear sampling.
- **Background position:** GPUI's cover fit centres. The Practice background (`center 40%`) is a
  1280×720 image placed at top −144 inside a clipped 360 px box.
- **Shared mode selection:** Home and Modes share one selected mode. `home` selects
  `v3.default_mode`, `modes` selects Ranked, `home-<mode>` and `modes-ssg` select theirs.
- **Idle:** switching modes, Practice tiles and hover rings are plain re-renders; frames run only
  while the fake-install bar or a page transition moves.

## Testing notes (Linux)

- **GPUI panics on a Wayland compositor without a `wl_seat`.** `gpui_linux/src/linux/wayland/client.rs`
  calls `seat.unwrap()`. `weston --backend=headless` exposes no seat, so the native Wayland path
  can't run there. Real desktops always have a seat. The smoke runs used the same headless Weston
  through its Xwayland (`DISPLAY=:N`, GPUI's X11 backend). The Wayland backend was checked separately on
  `weston --backend=x11` nested in Xvfb, which has a seat. All events came through and it exited 0, but
  that compositor renders on llvmpipe at about 35 fps, so those frame numbers mean nothing.
- **Weston blanks idle outputs after 300 s** (the default `--idle-time`). Frame callbacks then drop to
  about 1 Hz, which affects every stack, so start the harness compositor with `--idle-time=0` or a
  fresh one per run.
- **The Windows target type-checks from Linux** with
  `cargo check --target x86_64-pc-windows-msvc` (dev profile; needs `llvm-rc` for the manifest).
  The release build has to run on Windows because of `fxc.exe`.
- **Memory:** GPUI/wgpu enumerates *all* backends (Vulkan + GL) on all adapters, so the Mesa (libLLVM,
  llvmpipe threads) and NVIDIA GL/Vulkan libraries are mapped into the process. Most of the
  ~270 MB idle RSS on the NVIDIA laptop is shared driver code; the heap is ~45 MB. Compare PSS or
  private memory, not just RSS.
