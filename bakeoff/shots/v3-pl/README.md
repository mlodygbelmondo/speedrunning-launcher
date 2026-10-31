# Scene v3 screenshots, Polish

The same 26 screen ids as `shots/v3/`, with the UI switched to Polish (`assets/copy.pl.json`, `assets/screens.pl.json`).
The web build (Tauri + Electron, headless Chrome) is on top and GPUI (Xvfb) below, both 1280×800 at scale 1. Regenerate
them with `SHOOT_LANG=pl node bakeoff/tools/shoot-web.mjs …` and `BAKEOFF_LANG=pl bakeoff/tools/shoot-gpui.sh …`.
