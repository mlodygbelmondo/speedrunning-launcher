# Speedrunluncher UI bake-off

Speedrunluncher's UI stack gets picked by measurement. Three small test apps draw the **same screen** (the
Celestial Violet Noir launcher home and a 500-row library), run the **same scripted scenario** and report
the **same events**:

| stack | dir | binary |
|---|---|---|
| Tauri 2 (system WebView: WebView2 / WebKitGTK) | `tauri/` + `scene/` | `bakeoff-tauri` |
| Electron (bundled Chromium) | `electron/` + `scene/` | `bakeoff-electron` |
| GPUI (Zed's Rust GPU UI) | `gpui/` | `bakeoff-gpui` |

A harness launches each app, samples its whole process tree, reads the app's JSONL report, and writes one
results file per machine. `SPEC.md` has the full scene, scenario and report protocol.

## The scenario

Every phase lasts `BAKEOFF_PHASE_MS` (20 s by default):

1. **startup**: launch, render Home, then report `first_frame`.
2. **idle**: Home is static, nothing animates, no frames are requested.
3. **transitions**: Home and Library swap every second with fades while the glow pulses.
4. **scroll**: the 500-row library scrolls continuously at 1200 px/s.
5. **minimized**: the window is minimised while the animation keeps "running". Does the stack throttle?
6. **destroyed**: the window is gone but the process stays alive. This is "the game is running and the
   launcher sits in the tray".

## Build locally

Prerequisites: Node LTS + pnpm, Rust stable. On Linux, also the WebKitGTK and GPUI dev packages listed in
`scene/README.md` and `gpui/README.md`. The commands are the same on Linux and Windows:

```sh
cd bakeoff/scene    && pnpm install --frozen-lockfile && pnpm build     # -> scene/dist
cd ../tauri         && pnpm install --frozen-lockfile && pnpm build     # tauri build --no-bundle
cd ../electron      && pnpm install --frozen-lockfile && pnpm package   # -> out/bakeoff-electron-<os>-x64/
cd ../gpui          && cargo build --release
```

On Ubuntu 23.10+, Electron's sandbox helper must be setuid root, or Electron aborts at startup. Fix it once
per build:

```sh
sudo chown root:root bakeoff/electron/out/bakeoff-electron-linux-x64/chrome-sandbox
sudo chmod 4755      bakeoff/electron/out/bakeoff-electron-linux-x64/chrome-sandbox
```

`run.py` checks this and prints the exact command when it's wrong.

## Run on Linux

```sh
python3 bakeoff/harness/linux/run.py                   # all three apps, 3 runs each, 20 s phases (~20 min)
python3 bakeoff/harness/linux/run.py --apps tauri,gpui --runs 1 --phase-ms 5000
python3 bakeoff/harness/linux/run.py --apps mock       # harness self-test, no real apps needed
```

The harness needs only Python 3 and its standard library. It writes
`bakeoff/results/linux-<host>-<timestamp>.json` and a `.md` summary, and prints the summary tables. Exit
code 0 means every run delivered every protocol event.

**Real session (the numbers that count).** Run from a terminal inside your GNOME session:

- Unlock the screen and keep it awake. A locked GNOME session doesn't present windows, and the harness
  prints a loud warning when `LockedHint` is set.
- Plug in the laptop and close browsers and other busy apps.
- Don't touch the mouse while windows animate.

The harness auto-detects the refresh rate through Mutter's DisplayConfig (then xrandr, then 60 Hz). It
also records which GPU each app actually used, from the open `/dev/dri` and `/dev/nvidia*` nodes and the
mapped GL/EGL/Vulkan libraries. For Tauri this answers *"does WebKitGTK take the NVIDIA EGL path?"*.
Override with `--refresh-hz 144`.

**Nested (`--nested`)** starts a headless Weston on socket `bakeoff-h`:

```
weston --backend=headless --renderer=gl --width=1920 --height=1080 --refresh-rate=<hz*1000> --socket=bakeoff-h --xwayland
```

It falls back to `--renderer=pixman` when GL fails, as on CI. The apps run inside it, so this works with a
locked screen, over SSH and in CI. GPUI goes through Weston's Xwayland, because its Wayland backend needs a
`wl_seat` and headless Weston has none. Results are labelled **"nested (not real presentation)"**: memory,
CPU and startup are meaningful, frame pacing is not.

Useful flags: `--sample-ms` (200), `--settle-ms` (skip the first 1 s of each phase for memory and CPU),
`--startup-timeout`, `--no-ctxsw`, `--env KEY=VAL`, `--tauri-bin/--electron-bin/--gpui-bin`. The harness
removes all `ELECTRON_*` variables from the apps' environment. `ELECTRON_RUN_AS_NODE`, which Electron-based
terminals and IDEs set, would turn the Electron app into plain Node.

## Run on Windows

1. Download `speedrunluncher-bakeoff-windows.zip` from the
   [`bakeoff-latest` release](https://github.com/mlodygbelmondo/speedrunluncher/releases/tag/bakeoff-latest). CI rebuilds it on every push to `bakeoff`.
2. Extract it with right-click -> Extract All. Don't run it from inside the zip.
3. Double-click **`run-bakeoff.cmd`**. It calls `powershell -ExecutionPolicy Bypass -File run-bakeoff.ps1`,
   so no policy changes are needed.
4. **Accept the admin prompt.** PresentMon needs admin to read GPU presentation events (ETW).
5. Keep hands off for ~20 minutes. Windows open, animate, minimise and close by themselves.
6. **Send back the file** named on the last line: `results-windows-<PC>-<timestamp>.json`, next to the script.

The script runs in stock Windows PowerShell 5.1. On first run it downloads **PresentMon 2.6.0** (the
official GameTechDev console build) into `tools\` and verifies it:

- size: 980320 bytes
- SHA-256: `b2a706bc6ad475749e3b7e3409263aa1e6906d45bdcf993f6dbc0f660188f1af`

PresentMon records every process for the length of each run (`--qpc_time` CSV). The harness then keeps
only the rows from the app's process tree (WebView2's GPU process for Tauri, Electron's GPU process, the
GPUI exe), maps QPC timestamps to the phase windows and computes displayed-frame intervals.

Switches: `-Apps`, `-Runs`, `-PhaseMs`, `-RefreshHz`, `-NoPresentMon` (no admin needed, fewer metrics),
`-Sampler native|cim`, `-NoSizeCheck`, `-NoPause`, `-NoElevate`. `-SelfTest` runs the summary code on
synthetic data and works in pwsh on Linux too.

## Metrics

| metric | meaning |
|---|---|
| **first run / warm start ms** | Process spawn -> `first_frame` (the first frame is on screen). "First run" is the first launch in the session, the closest to a cold start; a true cold start needs a reboot. "Warm" is the median of runs 2..N. "host start ev" is spawn -> the host's `main()`. |
| **mem (Linux)** | PSS from `/proc/<pid>/smaps_rollup`, summed over the process tree: shared pages are split fairly between processes, so multi-process stacks aren't double-counted. Falls back to RSS. |
| **private / WS (Windows)** | Private bytes and working set, summed over the tree. |
| **main** | The host process alone (Tauri/Electron main process, the GPUI exe). |
| **CPU %** | Mean CPU time / wall time over the phase, in **% of one core**, summed over the tree. |
| **ctxsw/s** | Voluntary + involuntary context switches per second over all threads of the tree: a wakeup proxy (high when idle means timers or polling). |
| **procs** | Maximum number of processes in the tree. The harness rediscovers new children every tick and cleans up orphans. |
| **producer frames** | The app's own rAF/render timestamps: fps, p50/p95/p99/max frame interval. |
| **presented frames** (Windows) | PresentMon displayed frames of the app's main swap chain: the frame intervals the screen actually showed, dropped frames (presented but never displayed) and GPU busy ms per frame. |
| **jank %** | Share of frame intervals > 1.5x the refresh interval (> 10.4 ms at 144 Hz). |
| **refreshes hit %** | Frames shown / refreshes in the phase. 100% means a new frame on every vblank. |

Memory and CPU skip the first second of each phase (`--settle-ms` / `-SettleMs`), so one phase's work doesn't
spill into the next. Frame statistics are pooled over all runs. Everything else is the median across runs.

## Pass/fail targets

| target | threshold |
|---|---|
| Smooth transitions and scroll at 144 Hz | **>= 99 %** of refreshes presented |
| Frame pacing | **p99 frame interval <= 1.5x** the refresh interval (10.4 ms at 144 Hz) |
| Startup on Windows | **warm <= 400 ms**, **cold <= 1 s** |
| Idle | **<= 0.5 %** of one core |
| Destroyed (game running, launcher in tray) | CPU **~0** (main process <= 0.2 %) and memory **<= 20 MB** (core process) |
| Windows download | **<= 25 MB** (the Windows harness zips each app folder to check) |

The summary's Targets table marks each one PASS/FAIL per app. Windows uses PresentMon frames when they're
available and producer frames otherwise.

## CI

`.github/workflows/bakeoff.yml` runs on pushes to `bakeoff` that touch `bakeoff/**` and on manual dispatch:

- **windows**: builds the scene, Tauri, Electron and GPUI, then assembles
  `dist/` (`tauri\`, `electron\`, `gpui\`, `run-bakeoff.cmd/.ps1`, `README-WINDOWS.txt`) and zips it as
  `speedrunluncher-bakeoff-windows.zip`. It smoke-tests the harness in PowerShell 5.1 (2 s phases, 1 run;
  it fails only on crashes or missing events), then tries PresentMon for information only. It uploads the
  zip as an artifact and replaces the asset on the `bakeoff-latest` prerelease.
- **linux**: builds all three and runs `run.py --nested` with 2 s phases plus the mock self-test. It
  uploads `bakeoff/results/`.

## Files

- `harness/linux/run.py`: Linux harness.
- `harness/windows/run-bakeoff.ps1` and `run-bakeoff.cmd`: Windows harness and its double-click launcher.
- `harness/windows/README-WINDOWS.txt`: instructions shipped in the zip.
- `harness/mock_app.py`: fake app that speaks the full protocol, for testing harnesses.
- `results/`: result files. Commit the ones worth keeping; `results/raw/` is ignored.
