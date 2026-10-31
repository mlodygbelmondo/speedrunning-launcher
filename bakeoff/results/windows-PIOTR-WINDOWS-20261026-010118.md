# Bake-off results: Windows (PIOTR-WINDOWS), scene v3, run 1 (3 runs x 20 s)

Written from the run's JSON; the harness's own `.md` stayed on the PC.

- **Setup:** Microsoft Windows 11 Pro 25H2 (build 26200.9711), Intel(R) Core(TM) i7-14700K, 63.8 GB RAM, NVIDIA GeForce RTX 4070 SUPER. Primary monitor 180 Hz (DWM measured 178.6 Hz).
- **Run:** 3 run(s) per app, 20000 ms phases, PresentMon 2.6.0, WebView2 155.0.4283.62. Harness warnings: other heavy apps running: chrome, msedge.
- **Not a clean run.**
  - Chrome (playing YouTube on another monitor) and Edge were open.
  - The test windows were partly covered by the PowerShell and Explorer windows.
- **PresentMon displayed frames are not usable.** About 99 % of presents are marked as never shown, for all three
  apps. A later run with nothing over the windows (`windows-PIOTR-WINDOWS-20261026-012945`) shows the same, so the
  likely cause is how displayed frames are detected on this PC: three monitors, with a video playing on another one.
  Covered windows are not the explanation. This needs a look before frames on screen can be compared.
- **Still usable:** producer frames, startup, memory and CPU.
- **Electron GPU status:** the logged feature status says "disabled_software". That is logged before the GPU process
  is up, and Linux shows the same while it uses the NVIDIA GPU. PresentMon saw Flip presents with GPU time from
  Electron, so it most likely ran on the GPU.

## Notes

- **Tauri keeps rendering while minimised.** raf_count is 3600 in 20 s, visibility stays "visible", and it uses 22 %
  CPU. WebView2 isn't told the window is hidden; a real app would have to hide the webview on minimise.
- **GPUI keeps about 150 MB after its window closes** (destroyed phase), with the decoded key art and GPU resources
  still allocated. Tauri drops to 6.5 MB because the WebView2 processes exit. Electron keeps 174 MB across 3
  processes.
- **GPUI** has the lowest CPU while animating: 18 % in transitions and 16 % in scroll, against about 22 % and 50 % for
  both Tauri and Electron. It also has the tightest producer frame pacing: p99 6.0 ms and jank 0–0.06 %.
- **Electron** fails the startup and download targets: about 1.0 s warm and a 155 MB zip.

## Startup (spawn -> first frame)

| app | first run ms | warm median ms | zip MB |
| --- | --- | --- | --- |
| tauri | 569.6 | 384.8 | 5.9 |
| electron | 1108.8 | 1004.8 | 154.7 |
| gpui | 662 | 315.9 | 8.1 |

## Memory and CPU per phase (medians across runs)

private = private bytes summed over the process tree; main = host process only; CPU = % of one core.

| app | phase | private MB | main private MB | working set MB | CPU % | procs |
| --- | --- | --- | --- | --- | --- | --- |
| tauri | idle | 202.4 | 7.2 | 363.1 | 1.07 | 7 |
| tauri | transitions | 328.4 | 6.7 | 495.9 | 22.61 | 7 |
| tauri | scroll | 329.7 | 6.6 | 513.7 | 49.95 | 7 |
| tauri | minimized | 295.7 | 6.5 | 454.5 | 22.08 | 7 |
| tauri | destroyed | 6.5 | 6.5 | 27.9 | 0 | 1 |
| electron | idle | 196.5 | 33.2 | 318.3 | 0.17 | 4 |
| electron | transitions | 313.4 | 33.5 | 438.9 | 21.99 | 4 |
| electron | scroll | 317.2 | 33.7 | 452.9 | 51.3 | 4 |
| electron | minimized | 258.1 | 32 | 371.6 | 0.41 | 4 |
| electron | destroyed | 174.2 | 31.8 | 234.6 | 0.16 | 3 |
| gpui | idle | 147.8 | 147.8 | 94.6 | 0.17 | 1 |
| gpui | transitions | 149.9 | 149.9 | 90.7 | 18.18 | 1 |
| gpui | scroll | 153.1 | 153.1 | 93.9 | 15.98 | 1 |
| gpui | minimized | 155.5 | 155.5 | 96.6 | 1.66 | 1 |
| gpui | destroyed | 152.3 | 152.3 | 93.8 | 0 | 1 |

## Producer frames (the app's own frame timestamps, pooled across runs)

| app | phase | fps | p50 ms | p95 ms | p99 ms | max ms | jank % |
| --- | --- | --- | --- | --- | --- | --- | --- |
| tauri | transitions | 177.8 | 5.6 | 5.9 | 11.1 | 62.8 | 1.45 |
| tauri | scroll | 180 | 5.6 | 7.9 | 12.3 | 14.3 | 3.54 |
| electron | transitions | 177.9 | 5.6 | 5.9 | 7.1 | 50.1 | 0.6 |
| electron | scroll | 179.5 | 5.6 | 7.9 | 9.3 | 11.3 | 1.73 |
| gpui | transitions | 179.99 | 5.57 | 5.84 | 5.97 | 10.62 | 0.06 |
| gpui | scroll | 180 | 5.57 | 5.84 | 6.03 | 7.24 | 0 |

jank % = share of frame intervals > 1.5x the refresh interval (8.33 ms at 180 Hz).

## PresentMon displayed frames (not trusted, see above)

| app | phase | presents dropped % | displayed fps |
| --- | --- | --- | --- |
| tauri | transitions | 98.9 | 1.3 |
| tauri | scroll | 98 | 3.6 |
| electron | transitions | 98.08 | 2 |
| electron | scroll | 97.64 | 3.9 |
| gpui | transitions | 99.08 | 1.65 |
| gpui | scroll | 98.69 | 2.35 |

## Targets (PresentMon frame targets left out)

| target | tauri | electron | gpui |
| --- | --- | --- | --- |
| cold start (first run) <= 1000 ms | PASS 569.6 | FAIL 1108.8 | PASS 662 |
| warm start (median runs 2..N) <= 400 ms | PASS 384.8 | FAIL 1004.8 | PASS 315.9 |
| idle CPU <= 0.5% (tree) | FAIL 1.07 | PASS 0.17 | PASS 0.17 |
| destroyed CPU ~0 (main <= 0.2%) | PASS 0 | PASS 0 | PASS 0 |
| destroyed memory <= 20 MB (main, private) | PASS 6.5 | FAIL 31.8 | FAIL 152.3 |
