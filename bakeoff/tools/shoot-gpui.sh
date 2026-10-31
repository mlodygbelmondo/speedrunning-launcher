#!/usr/bin/env bash
# Screenshots of every GPUI screen id at 1280x800, scale 1, via Xvfb (no window manager, so no decorations).
# Usage: [BAKEOFF_LANG=en|pl] bakeoff/tools/shoot-gpui.sh <outDir> [ids...]
# BAKEOFF_LANG picks the UI language (default en). BAKEOFF_GPUI_BIN overrides the binary.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
bin=${BAKEOFF_GPUI_BIN:-$here/gpui/target/release/bakeoff-gpui}
lang=${BAKEOFF_LANG:-en}
out=${1:?outDir}; shift || true
ids=("$@")
# All 26 ids: the 20 of SCREENS.md, then the 6 scene v3 additions (REDESIGN.md "Screen ids").
[ ${#ids[@]} -gt 0 ] || ids=(home library onboarding-welcome onboarding-signin onboarding-signin-code onboarding-mode
  onboarding-java onboarding-eye onboarding-obs onboarding-done accounts modes check practice learn tools stats
  submit settings game home-rsg home-fsg home-ssg home-draftout modes-ssg practice-end)
mkdir -p "$out"
# Let Xvfb pick a free display (-displayfd): a fixed or random number can collide with another
# Xvfb, and then the app would open on someone else's screen.
dfile=$(mktemp)
Xvfb -displayfd 3 -screen 0 1600x1000x24 -nolisten tcp 3>"$dfile" >/dev/null 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true; rm -f "$dfile"' EXIT
for _ in $(seq 1 50); do [ -s "$dfile" ] && break; sleep 0.1; done
disp=:$(head -1 "$dfile")
[ "$disp" != ":" ] || { echo "Xvfb did not start" >&2; exit 1; }
for id in "${ids[@]}"; do
  env -u WAYLAND_DISPLAY DISPLAY=$disp GPUI_X11_SCALE_FACTOR=1 BAKEOFF_LANG=$lang BAKEOFF_SCREEN=$id \
    "$bin" >/dev/null 2>&1 &
  app=$!
  win=""
  for _ in $(seq 1 150); do win=$(DISPLAY=$disp xdotool search --name bake-off 2>/dev/null | head -1 || true); [ -n "$win" ] && break; sleep 0.1; done
  # `import -window ""` would wait for a click, so a window that never shows is reported and skipped.
  if [ -z "$win" ]; then
    echo "no window for $id" >&2
    kill $app 2>/dev/null || true; wait $app 2>/dev/null || true
    continue
  fi
  # Park the pointer outside the window so nothing shows a hover state.
  DISPLAY=$disp xdotool mousemove 1590 990 || true
  sleep 1.5
  DISPLAY=$disp timeout 20 import -window "$win" "$out/gpui-$id.png" || echo "import failed for $id" >&2
  kill $app 2>/dev/null || true; wait $app 2>/dev/null || true
done
echo "wrote ${#ids[@]} screenshots ($lang) to $out"
