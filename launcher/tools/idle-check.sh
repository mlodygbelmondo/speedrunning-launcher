#!/usr/bin/env bash
# Checks that the app draws nothing while idle, and walks the fake backend's Play flow once:
# Home idle -> click Play (install with progress) -> game screen idle -> Stop -> Home idle.
# Counts rendered frames (SRL_TRACE_RENDERS=1 logs one line per frame) and the process's CPU time
# (utime + stime from /proc, in 1/100 s) in each phase. Xvfb at 1600x1000, window 1280x800 at
# scale 1, no window manager (the window sits at 160,100).
# Usage: [SRL_APP_BIN=...] launcher/tools/idle-check.sh [shotDir]   (run it through srl-cpu)
# With shotDir, it also saves a JPEG of the live install and of the game screen there.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
bin=${SRL_APP_BIN:-${CARGO_TARGET_DIR:-$here/target}/release/speedrunning-launcher}
shots=${1:-}
work="$HOME/.cache/srl-dev/app/idle"
mkdir -p "$work"
srl_home=$(mktemp -d "$work/home.XXXXXX")
log="$srl_home/renders.log"
printf '{"player_name":"VioletRunner","language":"en","max_memory_mb":3072}\n' >"$srl_home/settings.json"
dfile=$(mktemp "$work/display.XXXXXX")
Xvfb -displayfd 3 -screen 0 1600x1000x24 -nolisten tcp 3>"$dfile" >/dev/null 2>&1 &
xvfb=$!
app=""
trap '[ -n "$app" ] && kill $app 2>/dev/null; kill $xvfb 2>/dev/null || true; rm -rf "$dfile" "$srl_home"' EXIT
for _ in $(seq 1 50); do [ -s "$dfile" ] && break; sleep 0.1; done
disp=:$(head -1 "$dfile")
env -u WAYLAND_DISPLAY DISPLAY=$disp GPUI_X11_SCALE_FACTOR=1 SRL_FAKE=1 SRL_HOME="$srl_home" SRL_SCREEN=home \
  SRL_TRACE_RENDERS=1 "$bin" >/dev/null 2>"$log" &
app=$!
win=""
for _ in $(seq 1 150); do win=$(DISPLAY=$disp xdotool search --name '^Speedrunning Launcher$' 2>/dev/null | head -1 || true); [ -n "$win" ] && break; sleep 0.1; done
[ -n "$win" ] || { echo "no window" >&2; exit 1; }
DISPLAY=$disp xdotool mousemove 1590 990
sleep 3

renders() { grep -c '^render' "$log" || true; }
cpu() { awk '{print $14 + $15}' "/proc/$app/stat"; }
phase() {
  local r0 c0
  r0=$(renders); c0=$(cpu)
  sleep "$2"
  echo "$1: $(( $(renders) - r0 )) frames, $(( $(cpu) - c0 )) CPU ticks in $2 s"
}
click() { DISPLAY=$disp xdotool mousemove --window "$win" "$1" "$2" click 1 mousemove 1590 990; }
shoot() { [ -z "$shots" ] || { mkdir -p "$shots"; DISPLAY=$disp import -window "$win" "$work/s.png" && convert "$work/s.png" -quality 85 "$shots/$1.jpg"; rm -f "$work/s.png"; }; }

phase "idle on Home" 10
click 208 398   # Play (Ranked, not installed: installs first)
sleep 5   # mid-way through the assets stage
shoot installing-live
phase "installing (frames come from progress events)" 3
for _ in $(seq 1 200); do grep -q 'page=Game' "$log" && break; sleep 0.1; done
grep -q 'page=Game' "$log" || { echo "the game screen never came up" >&2; exit 1; }
sleep 1
shoot running-live
phase "idle on the game screen" 10
click 208 370   # Stop game
sleep 1.5
phase "idle on Home after Stop" 10
echo "total frames: $(renders)"
cp "$log" "$work/renders-last.log"
