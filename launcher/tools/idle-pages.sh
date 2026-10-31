#!/usr/bin/env bash
# Counts the frames the app draws while idle on each given screen id (SRL_TRACE_RENDERS=1 logs one
# line per frame): opens the screen in Xvfb with the fake backend, waits for it to settle, then
# counts frames for IDLE_SECS (default 8). An idle page must draw 0. Exit code 1 if one doesn't.
# Usage: [SRL_DEMO=1] [SRL_LANG=pl] launcher/tools/idle-pages.sh <screen ids...>  (run it through
# srl-cpu). See idle-check.sh for the Play flow on Home.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
bin=${SRL_APP_BIN:-${CARGO_TARGET_DIR:-$here/target}/release/speedrunning-launcher}
secs=${IDLE_SECS:-8}
[ $# -gt 0 ] || { echo "usage: $0 <screen ids...>" >&2; exit 2; }
work="${SRL_DEV_ROOT:-$HOME/.cache/srl-dev/app}/idle"
mkdir -p "$work"
srl_home=$(mktemp -d "$work/home.XXXXXX")
dfile=$(mktemp "$work/display.XXXXXX")
Xvfb -displayfd 3 -screen 0 1600x1000x24 -nolisten tcp 3>"$dfile" >/dev/null 2>&1 &
xvfb=$!
app=""
trap '[ -n "$app" ] && kill $app 2>/dev/null; kill $xvfb 2>/dev/null || true; rm -rf "$dfile" "$srl_home"' EXIT
for _ in $(seq 1 50); do [ -s "$dfile" ] && break; sleep 0.1; done
disp=:$(head -1 "$dfile")
[ "$disp" != ":" ] || { echo "Xvfb did not start" >&2; exit 1; }
bad=0
for id in "$@"; do
  log="$srl_home/renders-$id.log"
  printf '{"player_name":"VioletRunner","language":"%s","max_memory_mb":3072}\n' "${SRL_LANG:-en}" >"$srl_home/settings.json"
  env -u WAYLAND_DISPLAY DISPLAY=$disp GPUI_X11_SCALE_FACTOR=1 SRL_FAKE=1 SRL_HOME="$srl_home" SRL_SCREEN=$id \
    SRL_DEMO=${SRL_DEMO:-} SRL_TRACE_RENDERS=1 "$bin" >/dev/null 2>"$log" &
  app=$!
  win=""
  for _ in $(seq 1 150); do win=$(DISPLAY=$disp xdotool search --name '^Speedrunning Launcher$' 2>/dev/null | head -1 || true); [ -n "$win" ] && break; sleep 0.1; done
  [ -n "$win" ] || { echo "$id: no window" >&2; bad=1; kill $app; wait $app 2>/dev/null || true; app=""; continue; }
  DISPLAY=$disp xdotool mousemove 1590 990
  sleep 3
  r0=$(grep -c '^render' "$log" || true)
  sleep "$secs"
  r1=$(grep -c '^render' "$log" || true)
  echo "$id: $((r1 - r0)) frames in $secs s idle ($r0 to settle)"
  [ $((r1 - r0)) -eq 0 ] || bad=1
  kill $app 2>/dev/null || true; wait $app 2>/dev/null || true; app=""
done
exit $bad
