# Web scene + Tauri / Electron shells

`scene/` is the Svelte 5 + Vite + TypeScript UI from `../SPEC.md`. It is built once into `scene/dist`
(self-contained: fonts, hero, glow, avatars and data.json are bundled from `../assets`) and loaded by
both web shells:

- `../tauri/`: Tauri 2 shell. It embeds `scene/dist` at compile time. Binary: `bakeoff-tauri`.
- `../electron/`: Electron shell. The packager copies `scene/dist` to `resources/app/scene`. Binary: `bakeoff-electron`.

The scene detects its host at runtime. Electron's preload exposes `window.bakeoff` (contextBridge,
sandbox on). Tauri is detected via `window.__TAURI_INTERNALS__` and bridged with `invoke`. In a plain
browser, `?scenario=1&phaseMs=2000` runs the scenario and logs the JSON lines to the console.

## Prerequisites

- Node 22.12+ and pnpm 10+ (all `package.json` scripts are cross-platform; no shell-specific syntax).
- Tauri: a Rust stable toolchain. On Linux you also need `libwebkit2gtk-4.1-dev` and `libgtk-3-dev`; on
  Windows you need MSVC build tools and WebView2, which windows-latest runners already have.

## Build (Linux and Windows: same commands)

The scene must be built first; both shells read `scene/dist`.

```sh
# 1. scene -> bakeoff/scene/dist
cd bakeoff/scene
pnpm install --frozen-lockfile
pnpm build

# 2. Tauri -> bakeoff/tauri/src-tauri/target/release/bakeoff-tauri[.exe]
cd ../tauri
pnpm install --frozen-lockfile
pnpm build            # = tauri build --no-bundle (release profile: opt 3, LTO, 1 CGU, strip)

# 3. Electron -> bakeoff/electron/out/bakeoff-electron-<platform>-<arch>/bakeoff-electron[.exe]
cd ../electron
pnpm install --frozen-lockfile
pnpm package          # node scripts/package.mjs (@electron/packager, asar off, host platform/arch)
```

`pnpm build:all` in `tauri/` or `electron/` runs the scene steps and then the shell build.

Windows outputs are `bakeoff\tauri\src-tauri\target\release\bakeoff-tauri.exe` and
`bakeoff\electron\out\bakeoff-electron-win32-x64\bakeoff-electron.exe`.

## Running

```sh
# interactive (no scenario)
bakeoff/tauri/src-tauri/target/release/bakeoff-tauri
# scenario
BAKEOFF_OUT=/tmp/tauri.jsonl BAKEOFF_PHASE_MS=2000 bakeoff/tauri/src-tauri/target/release/bakeoff-tauri
```

Linux gotchas for Electron:

- On Ubuntu 23.10+ (AppArmor restricts unprivileged user namespaces), the packaged
  `chrome-sandbox` must be setuid root, as a .deb install would make it:
  `sudo chown root:root out/bakeoff-electron-linux-x64/chrome-sandbox && sudo chmod 4755 out/bakeoff-electron-linux-x64/chrome-sandbox`.
  Otherwise Electron aborts at startup. Don't use `--no-sandbox`, because that changes what is measured.
- If `ELECTRON_RUN_AS_NODE` is set in the environment (some Electron-based tools set it), the binary
  runs as plain Node. Unset it before launching.

## Scenario details (as implemented)

- Phase timers run in a tiny Web Worker (`setTimeout` there), so Chromium's 1 s timer alignment
  for hidden/minimised pages does not stretch the `minimized` phase.
- Page transition: the outgoing page animates for 150 ms ease-in (opacity 1→0, translateY 0→8 px). Then the page
  swaps, and the incoming page animates for 220 ms ease-out (0→1, 8→0 px). Both use the Web Animations API, and the
  pages are sequential, not cross-faded. Library is re-mounted on every visit (all 500 rows in the DOM).
- Glow pulse is a CSS animation that is only attached while `glowRunning` is true. Otherwise the glow is static
  (opacity 0.8, scale 1).
- At the end of `minimized`, the scene emits an extra line
  `{"ev":"info","phase":"minimized","raf_count":N,"visibility":"hidden|visible"}`. N counts rAF callbacks
  during that phase, which shows whether the host throttles hidden windows.
- Electron also emits `{"ev":"info","source":"host",...}` with GPU feature status. The scene emits
  `{"ev":"info","source":"scene","ua":...,"dpr":...}`.
