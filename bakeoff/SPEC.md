# Bake-off spec: Tauri vs Electron vs GPUI

Goal: decide the UI stack for Speedrunluncher using measurements, not opinions. Three test apps draw the
**same scene** in the chosen visual style (A + D: Celestial Violet Noir + speedrun data), run the **same
scripted scenario**, and report the **same events**. Harness scripts launch each app, sample memory and CPU,
and summarise everything per phase.

Every app must stay small and readable. This is a benchmark, not the product: no routing libraries, no state
libraries and no UI kits beyond what's listed.

## Layout

```
bakeoff/
  SPEC.md            this file
  assets/            shared; fonts, avatars, images and data.json come from tools/gen_assets.py
    fonts/           Inter (Regular/Medium/SemiBold/Bold), JetBrains Mono (Regular/Bold) + licences
    avatars/000..499.png   64x64 pixel-art
    hero.jpg         1600x480 banner
    glow.png         1200x1200 pre-rendered violet radial glow with alpha
    data.json        { rows[500], pace[8], tiles{...} }
    copy.json        every UI string (hand-edited, see SCREENS.md)
    screens.json     sample data for the prototype screens (hand-edited)
    icons/           Lucide SVG icons (ISC) + logo.svg
  scene/             Svelte 5 + Vite + TypeScript UI, shared by Tauri and Electron
  tauri/             Tauri 2 shell (src-tauri), loads scene/dist
  electron/          Electron shell, loads scene/dist
  gpui/              Rust + GPUI app (Zed rev fc9258b4632544c0819953c59e2c31b9e92ea362, same as Pandora Launcher)
  harness/linux/     Python 3 (stdlib only) runner + summariser
  harness/windows/   PowerShell 5.1 runner + summariser (+ PresentMon)
  tools/             asset generator etc.
```

Binary names: `bakeoff-tauri`, `bakeoff-electron`, `bakeoff-gpui` (`.exe` on Windows).

## The scene (identical in all three)

Window: 1280×800 logical px, resizable, native decorations, title "Speedrunluncher bake-off (<stack>)".
Fonts: Inter for UI text, JetBrains Mono for times and numbers. Load the bundled TTFs. Don't rely on system fonts.

Colours:

| Token | Hex |
|---|---|
| bg | `#0A090E` |
| surface | `#15131D` |
| raised | `#1B1327` |
| border | `#26222F` |
| accent | `#8B5CF6` |
| accent-hover | `#9D74F8` |
| text | `#ECEAF2` |
| muted | `#8A8796` |
| ahead (green) | `#4ADE80` |
| behind (red) | `#F87171` |
| pb | `#C084FC` |

**Scene v3:** REDESIGN.md replaces the icon rail with a top nav and gives Home a new full-window layout.
Library keeps the layout below under the new nav. Where REDESIGN.md differs from this section, it wins. The
scene v2 structure follows for reference.

Structure, left to right:

1. **Icon rail:** 72 px wide with icons and labels, as described in SCREENS.md ("Rail v2"). Scene v1 used a
   64 px rail with letters; results from v1 and v2 builds are not directly comparable, but all stacks in one run use
   the same scene.
2. **Content area** (rest of the window, bg `bg`):
   - `glow.png` is drawn behind the content: 900×900 px, centred on the content area's top-right corner, so it is partly off-screen. It sits under all content. Use the PNG; don't use CSS radial gradients, so all stacks draw the same thing.
   - Padding 28 px.
   - Header row: page title in Inter Bold 26 px plus a muted subtitle in 13 px. All texts come from
     `assets/copy.json` (`home.*`, `library.*`).

**Home page**
- **Hero:**
  - Full width, 220 px tall, radius 16, `hero.jpg` as cover, 1 px border `border`.
  - Text overlay at the bottom-left: "MCSR Ranked · Season 9" (muted, 13 px) and "Ready to run?" (Inter Bold 30 px).
  - Bottom-right: a **LAUNCH** button (accent bg, radius 12, 48 px tall, 160 px wide, Inter Bold 15 px, letter-spacing 1 px). Hovering switches it to `accent-hover`.
- **Tile row** (3 equal tiles, gap 16, height 150, bg `surface`, radius 14, 1 px border `border`, padding 18):
  1. "Personal best": `tiles.pb` in JetBrains Mono Bold 34 px, colour `pb`, plus `tiles.pb_category` muted.
  2. "Runs this week": 7 vertical bars from `tiles.today_runs`. Each bar is 18 px wide with a gap of 10. The height is proportional to the value, max 80 px, radius 4, colour `accent`.
  3. "Ranked elo": `tiles.elo` in Mono Bold 34 px, plus 24 thin bars (4 px wide, gap 3) from `tiles.elo_history`, scaled between min and max to 10..60 px, colour `pb`.
- **Live pace table** (bg `surface`, radius 14, border, padding 8 18), with the first 6 rows from `pace` (8 overflowed the 800 px window):
  - Each row is 44 px tall: 28×28 avatar (radius 6), name (Inter Medium 14), split (muted 13), time (Mono 14) and delta (Mono 14, green if `ahead`, else red).
  - Rows are separated by 1 px lines in `border`.

**Library page**
- A scroll container that fills the rest of the height, bg `surface`, radius 14, border.
- 500 rows from `rows`, each **72 px tall**, padding 0 18, with 1 px separators:
  - rank (Mono 13, muted, width 44);
  - 48×48 avatar (radius 8, pixelated/nearest-neighbour scaling is fine);
  - a name column (Inter SemiBold 15) with the category below it (muted 12);
  - on the right: time (Mono 15), delta (Mono 13, green/red) and elo (Mono 13, `pb`).
- Row hover: bg `raised`.
- Web: render all 500 rows in the DOM (no virtualisation library); `loading="lazy"` and `decoding="async"` on images are fine.
- GPUI: use `uniform_list`, which is the idiomatic choice. That difference is intended: each stack runs its natural approach.

**Animations**
- **Page transition:**
  - The outgoing page fades to opacity 0 and moves +8 px on Y over 150 ms (ease-in).
  - The incoming page fades from 0 and moves from +8 px to 0 over 220 ms (ease-out).
  - Only opacity and translate change. No layout animations.
- **Glow pulse:**
  - A 6 s loop: opacity 0.65 → 1.0 → 0.65 and scale 1.0 → 1.08 → 1.0 (ease-in-out).
  - **It runs only while `glowRunning` is true** (see the scenario). When it isn't running, the glow is static at opacity 0.8 and scale 1.0, and the app must not request any frames.

## Scenario mode

Scenario mode is on when the env var `BAKEOFF_OUT` is set to a file path. Without it, the app is just an
interactive UI with no scenario, which is handy for eyeballing.

The phase duration is `BAKEOFF_PHASE_MS` (default 20000; CI smoke tests use 2000).

| # | Phase | What the app does | Frames recorded? |
|---|---|---|---|
| 0 | startup | Launch and render Home. Once the first frame is on screen, emit `first_frame`. | – |
| 1 | `idle` | Home is static, `glowRunning=false`, nothing animates, no frames requested. | no |
| 2 | `transitions` | `glowRunning=true`. Toggle Home ↔ Library every 1000 ms using the page transition. Keep a frame loop running for the whole phase. | **yes** |
| 3 | `scroll` | Library is shown and `glowRunning=true`. Scroll continuously at 1200 px/s (time-based: `offset = f(elapsed)`), bouncing between top and bottom. Keep a frame loop running. | **yes** |
| 4 | `minimized` | Minimise the window (Tauri `window.minimize()`, Electron `win.minimize()`, GPUI `window.minimize_window()`). Keep the glow animation "running", which tests whether the toolkit throttles hidden windows. | no (report counter if possible) |
| 5 | `destroyed` | Destroy/close the UI window, **keeping the process alive**. This simulates "game is running, launcher sits in the tray": Tauri destroys the webview window and prevents exit; Electron calls `win.destroy()` and doesn't quit on window-all-closed; GPUI removes the window and keeps the app running. | no |
| 6 | exit | Emit `done`, flush and exit with code 0. | – |

Phases run back to back; each lasts `BAKEOFF_PHASE_MS`. Timing must be driven by wall-clock timers in the host
or UI, not by frame counts.

**Frame loop and timestamps (what "frames recorded" means):**
- Web: a `requestAnimationFrame` loop for the whole phase. Store each callback's `performance.now()` timestamp,
  relative to the phase start.
- GPUI: call `window.request_animation_frame()` (or the rev's equivalent) on every render while the phase is
  active. Record `Instant::now()` at the start of each render/paint (or a per-frame callback if the API has
  one), relative to the phase start, in fractional ms.
- These are *producer* timestamps. The Windows harness adds presentation truth via PresentMon.

## Report protocol (JSON Lines, appended to `$BAKEOFF_OUT`)

Every line is one JSON object, flushed immediately. `t` is the Unix epoch in milliseconds (float, sub-ms
precision if available).

```jsonl
{"ev":"start","t":1729780000123.4,"stack":"tauri","pid":1234,"version":"tauri 2.x / webview <ua>"}
{"ev":"first_frame","t":1729780000456.7}
{"ev":"phase","name":"idle","t":...}
{"ev":"phase","name":"transitions","t":...}
{"ev":"frames","phase":"transitions","ts":[0.0,6.94,13.89, ...]}
{"ev":"phase","name":"scroll","t":...}
{"ev":"frames","phase":"scroll","ts":[...]}
{"ev":"phase","name":"minimized","t":...}
{"ev":"phase","name":"destroyed","t":...}
{"ev":"done","t":...}
```

- `start` is emitted as early as possible in the host process (Rust `main` / Electron main) with the host pid.
- `first_frame` (web):
  - Use a double `requestAnimationFrame` after the Home page mounts.
  - Convert to epoch with `performance.timeOrigin + performance.now()`.
  - Send it through the host bridge; the host appends the line.
- `first_frame` (GPUI): the first frame-completion callback after the first render (`on_next_frame` or similar).
- Optional extra info line: `{"ev":"info", ...}` with anything useful (user agent, GPU adapter, scale factor, refresh rate if known).
- Writing is done by the host (Tauri command / Electron IPC via preload / GPUI directly). The web scene calls
  `window.bakeoff.report(obj)`. Tauri uses `invoke('report', {line})` and Electron uses
  `contextBridge.exposeInMainWorld('bakeoff', ...)`.
- Phase actions that need the host (minimise, destroy, exit) are host commands that the scene calls:
  `window.bakeoff.minimize()`, `.destroy()`. After destroy, the **host** owns the timer for the rest of the
  `destroyed` phase, then emits `done` and exits.

## Build outputs

- Linux:
  - `bakeoff/tauri/src-tauri/target/release/bakeoff-tauri`
  - `bakeoff/electron/out/bakeoff-electron-linux-x64/bakeoff-electron`
  - `bakeoff/gpui/target/release/bakeoff-gpui`
- Windows (CI): the same names with `.exe`, collected into `dist/` along with the Windows harness.
- `scene/dist` is built once by `pnpm build` and loaded by both web shells (Tauri embeds it; Electron loads it from
  `resources/app/scene`).

## Out of scope

Custom title bars, real tray icons, real launcher logic, auth, networking. The other launcher screens exist only as
clickable prototypes (SCREENS.md) and are not part of the measured scenario.
