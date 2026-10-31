# Bake-off results: Windows (PIOTR-WINDOWS), scene v3, run 2 (1 run x 10 s)

Written from the run's JSON; the harness's own `.md` stayed on the PC.

- **Setup:** Microsoft Windows 11 Pro 25H2 (build 26200.9711), Intel(R) Core(TM) i7-14700K, 63.8 GB RAM, NVIDIA GeForce RTX 4070 SUPER. Primary monitor 180 Hz (DWM measured 179.73 Hz).
- **Run:** 1 run(s) per app, 10000 ms phases, PresentMon 2.6.0, WebView2 155.0.4283.62. Harness warnings: other heavy apps running: msedge.
- **Short, mostly clean run.** Nothing covered the test windows. Chrome was closed, Edge was open, and Piotr watched a
  video on another monitor.
- **PresentMon displayed frames are not usable.** Again about 99 % of presents are marked as never shown for every
  app, the same as in the first run, so it isn't occlusion. The likely cause is how displayed frames are detected on
  this setup (three monitors, video on another one).
- **Still usable:** producer frames, startup, memory and CPU.
- **Same picture as the first run:**
  - GPUI has the tightest producer frame pacing: p99 5.8–5.9 ms and jank 0–0.06 %.
  - GPUI uses the least CPU (19 % in scroll, against 45 % for Tauri and 54 % for Electron) and the least memory while
    visible.
  - Tauri renders at full rate while minimised: raf_count 1802 in 10 s, 18 % CPU.
  - Electron's scroll jank is 9 % here, and it takes 1.0 s to start.

## Startup (spawn -> first frame)

| app | first run ms | warm median ms | zip MB |
| --- | --- | --- | --- |
| tauri | 392.7 | – | 5.9 |
| electron | 1014.6 | – | 154.7 |
| gpui | 322.2 | – | 8.1 |

## Memory and CPU per phase (medians across runs)

private = private bytes summed over the process tree; main = host process only; CPU = % of one core.

| app | phase | private MB | main private MB | working set MB | CPU % | procs |
| --- | --- | --- | --- | --- | --- | --- |
| tauri | idle | 200.6 | 7.5 | 357.8 | 2.44 | 7 |
| tauri | transitions | 322 | 6.7 | 475.2 | 17.22 | 7 |
| tauri | scroll | 316.1 | 6.8 | 485.4 | 45.28 | 7 |
| tauri | minimized | 311.7 | 6.8 | 479.9 | 18.06 | 7 |
| tauri | destroyed | 6.8 | 6.8 | 30 | 0 | 1 |
| electron | idle | 196.4 | 33.6 | 320.9 | 0 | 4 |
| electron | transitions | 318 | 34.6 | 433.8 | 22.4 | 4 |
| electron | scroll | 315 | 34.7 | 449.1 | 53.87 | 4 |
| electron | minimized | 314.1 | 35 | 436.8 | 0.35 | 4 |
| electron | destroyed | 181.1 | 34.4 | 243.4 | 0 | 3 |
| gpui | idle | 147.5 | 147.5 | 96.1 | 0.17 | 1 |
| gpui | transitions | 164.7 | 164.7 | 106.9 | 19.45 | 1 |
| gpui | scroll | 151.4 | 151.4 | 93.9 | 19.12 | 1 |
| gpui | minimized | 152.4 | 152.4 | 95 | 0.53 | 1 |
| gpui | destroyed | 149.1 | 149.1 | 92.2 | 0 | 1 |

## Producer frames (the app's own frame timestamps, pooled across runs)

| app | phase | fps | p50 ms | p95 ms | p99 ms | max ms | jank % |
| --- | --- | --- | --- | --- | --- | --- | --- |
| tauri | transitions | 177.53 | 5.6 | 5.9 | 7.32 | 34.2 | 0.68 |
| tauri | scroll | 179.9 | 5.6 | 7.8 | 8.3 | 11.2 | 0.94 |
| electron | transitions | 177.59 | 5.6 | 5.9 | 7.12 | 59.1 | 0.56 |
| electron | scroll | 179.65 | 5.5 | 12.3 | 12.7 | 14.6 | 9.18 |
| gpui | transitions | 179.99 | 5.55 | 5.72 | 5.87 | 8.95 | 0.06 |
| gpui | scroll | 180.01 | 5.56 | 5.72 | 5.84 | 6.39 | 0 |

jank % = share of frame intervals > 1.5x the refresh interval (8.33 ms at 180 Hz).

## PresentMon displayed frames (not trusted, see above)

| app | phase | presents dropped % | displayed fps |
| --- | --- | --- | --- |
| tauri | transitions | 99.75 | 0.1 |
| tauri | scroll | 99.31 | 0.8 |
| electron | transitions | 99.31 | 0.4 |
| electron | scroll | 99.82 | 0.2 |
| gpui | transitions | 99.21 | 0.8 |
| gpui | scroll | 99.12 | 1.5 |

## Targets (PresentMon frame targets left out)

| target | tauri | electron | gpui |
| --- | --- | --- | --- |
| cold start (first run) <= 1000 ms | PASS 392.7 | FAIL 1014.6 | PASS 322.2 |
| warm start (median runs 2..N) <= 400 ms | FAIL None | FAIL None | FAIL None |
| idle CPU <= 0.5% (tree) | FAIL 2.44 | PASS 0 | PASS 0.17 |
| destroyed CPU ~0 (main <= 0.2%) | PASS 0 | PASS 0 | PASS 0 |
| destroyed memory <= 20 MB (main, private) | PASS 6.8 | FAIL 34.4 | FAIL 149.1 |
