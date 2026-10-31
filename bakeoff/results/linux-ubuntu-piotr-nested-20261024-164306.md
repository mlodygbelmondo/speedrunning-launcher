# Bake-off results: Linux (ubuntu-piotr)

- Mode: **nested headless weston (not real presentation)**
- Refresh: 144.00 Hz (nested weston --refresh-rate); phase 20000 ms; 3 run(s) per app; sampling every 200 ms
- Ubuntu 26.04.1 LTS, kernel 7.0.0-38-generic, wayland, 11th Gen Intel(R) Core(TM) i7-11800H @ 2.30GHz
- GPU: VGA compatible controller [0300]: Intel Corporation TigerLake-H GT1 [UHD Graphics] [8086:9a60] (rev 01) [i915]; 3D controller [0302]: NVIDIA Corporation GA107M [GeForce RTX 3050 Ti Mobile] [10de:25a0] (rev a1) [nvidia] ; NVIDIA driver 595.99.02
- Warning: gpui ran through nested Xwayland :2 (GPUI's Wayland backend needs a wl_seat, which headless weston lacks)

## Startup (spawn -> first_frame)

| app | runs ok | first run ms | warm median ms | median ms | host start ev ms |
|---|---|---|---|---|---|
| tauri | 3/3 | 819 | 514 | 517 | 25.1 |
| electron | 3/3 | 530 | 357 | 374 | 88.8 |
| gpui | 3/3 | 290 | 348 | 345 | 5.5 |

## Memory and CPU per phase (medians across runs)

mem = PSS summed over the process tree; main = host process only; CPU = % of one core.

| app | phase | mem med MB | mem max MB | main MB | CPU % | main CPU % | ctxsw/s | procs |
|---|---|---|---|---|---|---|---|---|
| tauri | startup | 215 | 247 | 86.2 | 94.99 | 53.32 | 2825 | 6 |
| tauri | idle | 233 | 251 | 87.2 | 0.32 | 0 | 27.3 | 6 |
| tauri | transitions | 297 | 348 | 90.8 | 50.32 | 9.84 | 1114 | 6 |
| tauri | scroll | 342 | 350 | 90.8 | 56.21 | 14.63 | 1398 | 3 |
| tauri | minimized | 344 | 362 | 90.8 | 38.32 | 5.84 | 1186 | 3 |
| tauri | destroyed | 122 | 351 | 95.9 | 1.79 | 1.79 | 7.9 | 3 |
| electron | startup | 324 | 445 | 100 | 155 | 84.96 | 7691 | 11 |
| electron | idle | 419 | 445 | 101 | 0.21 | 0.05 | 8.2 | 11 |
| electron | transitions | 514 | 532 | 100 | 37.74 | 4.79 | 2664 | 11 |
| electron | scroll | 530 | 533 | 104 | 64.9 | 5.37 | 3038 | 8 |
| electron | minimized | 455 | 523 | 103 | 0.42 | 0.05 | 12.6 | 8 |
| electron | destroyed | 326 | 434 | 115 | 0.16 | 0.11 | 12.2 | 7 |
| gpui | startup | 118 | 141 | 118 | 82.46 | 82.46 | 1302 | 1 |
| gpui | idle | 141 | 141 | 141 | 2.47 | 2.47 | 260 | 1 |
| gpui | transitions | 144 | 144 | 144 | 42.26 | 42.26 | 488 | 1 |
| gpui | scroll | 147 | 149 | 147 | 43.37 | 43.37 | 525 | 1 |
| gpui | minimized | 149 | 149 | 149 | 1.51 | 1.51 | 156 | 1 |
| gpui | destroyed | 147 | 147 | 147 | 0.79 | 0.79 | 117 | 1 |

## Frames (producer timestamps, pooled across runs)

| app | phase | frames | fps | p50 ms | p95 ms | p99 ms | max ms | jank % | refreshes hit % |
|---|---|---|---|---|---|---|---|---|---|
| tauri | transitions | 1197 | 59.83 | 16 | 17 | 45.14 | 93 | 97.04 | 41.56 |
| tauri | scroll | 1242 | 62.07 | 16 | 17 | 17 | 17 | 100 | 43.12 |
| electron | transitions | 2508 | 125 | 7 | 8.8 | 13.1 | 1430 | 1.39 | 87.08 |
| electron | scroll | 2523 | 126 | 7 | 15.4 | 23.32 | 32.3 | 12.89 | 87.6 |
| gpui | transitions | 2519 | 126 | 6.95 | 13.9 | 20.86 | 605 | 8.44 | 87.45 |
| gpui | scroll | 2610 | 130 | 6.95 | 13.89 | 20.86 | 21 | 7.95 | 90.61 |

jank % = share of frame intervals > 1.5x the refresh interval (10.42 ms).

## Targets

| target | tauri | electron | gpui |
|---|---|---|---|
| transitions refreshes presented >= 99% | FAIL 41.56 | FAIL 87.08 | FAIL 87.45 |
| transitions p99 <= 1.5x refresh (10.42 ms) | FAIL 45.14 | FAIL 13.1 | FAIL 20.86 |
| scroll refreshes presented >= 99% | FAIL 43.12 | FAIL 87.6 | FAIL 90.61 |
| scroll p99 <= 1.5x refresh (10.42 ms) | FAIL 17 | FAIL 23.32 | FAIL 20.86 |
| cold start (first run) <= 1000 ms | PASS 819 | PASS 530 | PASS 290 |
| warm start (median runs 2..N) <= 400 ms | FAIL 514 | PASS 357 | PASS 348 |
| idle CPU <= 0.5% (tree) | PASS 0.32 | PASS 0.21 | FAIL 2.47 |
| destroyed CPU ~0 (main <= 0.2%) | FAIL 1.79 | PASS 0.11 | FAIL 0.79 |
| destroyed memory <= 20 MB (main) | FAIL 95.9 | FAIL 115 | FAIL 147 |

## App info events (first ok run)

- tauri: source=scene, ua=Mozilla/5.0 (X11; Ubuntu; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/60.5 Safari/605.1.15, dpr=1
- tauri: phase=minimized, raf_count=1419, visibility=visible
- electron: source=host, platform=linux-x64
- electron: source=scene, ua=Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) bakeoff-electron/0.1.0 Chrome/156.0.8078.48 Electron/45.0.0 Safari/537.36, dpr=1
- electron: phase=minimized, raf_count=0, visibility=hidden
- gpui: source=host, gpu=NVIDIA GeForce RTX 3050 Ti Laptop GPU, driver=NVIDIA 595.99.02, software_gpu=False, scale_factor=1.0, compositor=X11
- gpui: phase=minimized, raf_count=22, window_active=False
- gpui: phase=destroyed, windows_left=0

## GPU path per app (first run, sampled during transitions)

- tauri (run 1): drivers opened: **nvidia**; WebKitGTK NVIDIA EGL path: **yes**
  - `1508303:bakeoff-tauri`: /dev/dri/renderD129, /dev/nvidia-modeset, /dev/nvidia0, /dev/nvidiactl, libEGL_mesa, libEGL_nvidia, libgallium-26.0.8-1ubuntu0.3.so, libnvidia-eglcore, libnvidia-gpucomp, libwebkit2gtk-4.1.so.0.21.10
  - `1508484:WebKitNetworkPr`: libwebkit2gtk-4.1.so.0.21.10
  - `1508493:WebKitWebProces`: /dev/dri/renderD129, /dev/nvidia0, /dev/nvidiactl, libEGL_mesa, libEGL_nvidia, libgallium-26.0.8-1ubuntu0.3.so, libnvidia-eglcore, libnvidia-gpucomp, libwebkit2gtk-4.1.so.0.21.10, libwebkit2gtkinjectedbundle.so
- electron (run 1): drivers opened: **i915, nvidia**
  - `1513327:bakeoff-electro`: /dev/dri/renderD128
  - `1513368:bakeoff-electro`: /dev/dri/renderD128, /dev/dri/renderD129, /dev/nvidia-modeset, /dev/nvidia0, /dev/nvidiactl, libEGL_mesa, libEGL_nvidia, libGLX_nvidia, libgallium-26.0.8-1ubuntu0.3.so, libnvidia-eglcore, libnvidia-glcore, libnvidia-gpucomp, libvulkan.so, libvulkan_intel, libvulkan_lvp
- gpui (run 1): drivers opened: **i915, nvidia**
  - `1515875:bakeoff-gpui`: /dev/dri/card1, /dev/dri/renderD128, /dev/dri/renderD129, /dev/nvidia-modeset, /dev/nvidia0, /dev/nvidiactl, libEGL_mesa, libEGL_nvidia, libGLX_nvidia, libgallium-26.0.8-1ubuntu0.3.so, libnvidia-eglcore, libnvidia-glcore, libnvidia-gpucomp, libvulkan.so, libvulkan_intel, libvulkan_lvp

