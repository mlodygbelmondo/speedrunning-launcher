#!/usr/bin/env bash
# Screenshots of the app's screen ids at 1280x800, scale 1, via Xvfb (no window manager, so no
# decorations). Adapted from bakeoff/tools/shoot-gpui.sh.
#
# Usage: [SRL_LANG=en|pl] [SRL_DEMO=1] launcher/tools/shoot-app.sh <outDir> [ids...]
#   SRL_LANG     UI language (default en)
#   SRL_DEMO=1   the prototype's sample data, for comparison with bakeoff/shots/v3
#   SRL_APP_BIN  the binary (default: $CARGO_TARGET_DIR/release/speedrunning-launcher, else
#                launcher/target/release/speedrunning-launcher)
#   SHOOT_FORMAT jpg (default, quality 85) or png (lossless, for pixel diffs)
#   SHOOT_SUFFIX appended to every file name (e.g. "-demo")
#   SHOOT_SETTINGS extra settings.json fields, e.g. '"draftout_extras":true' (Settings states)
#   SHOOT_HOME_ROOT where the throwaway homes go (default ${SRL_DEV_ROOT:-~/.cache/srl-dev/app}/shoot)
# Every run uses a throwaway SRL_HOME (under SHOOT_HOME_ROOT) whose settings already
# have the player name "VioletRunner", so screens other than "name" skip the first-run prompt.
# Run it through srl-cpu: it starts Xvfb and the app.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
bin=${SRL_APP_BIN:-${CARGO_TARGET_DIR:-$here/target}/release/speedrunning-launcher}
lang=${SRL_LANG:-en}
demo=${SRL_DEMO:-}
fmt=${SHOOT_FORMAT:-jpg}
suffix=${SHOOT_SUFFIX:-}
out=${1:?outDir}; shift || true
ids=("$@")
# Every screen id (crates/srl-app/README.md "Screen ids").
[ ${#ids[@]} -gt 0 ] || ids=(home home-rsg home-fsg home-ssg home-draftout instances instances-packs instances-config
  instances-screenshots instances-logs instances-settings instances-cleanup instances-draftout practice practice-end
  learn tools tools-installing tools-update tools-failed stats stats-scanning stats-empty stats-unknown
  stats-offline stats-unreachable stats-new settings settings-stats settings-credits home-stats home-rsg-stats
  name installing error crash stopped-draftout running account account-list login login-nobrowser login-steps
  login-done login-error msa-home msa-signing msa-running msa-warning msa-relogin msa-keyring msa-sign-in-first
  practice-installed practice-about practice-installing practice-update practice-reset practice-remove practice-starting
  practice-running practice-error practice-link import import-scanning import-none import-other import-importing
  import-done mods mods-ssg mods-draftout update-ready update-about update-blocked update-checking update-current
  update-downloading update-manual update-error update-local)
extra=${SHOOT_SETTINGS:+,$SHOOT_SETTINGS}
[ -x "$bin" ] || { echo "no binary at $bin (build with cargo build --release -p srl-app)" >&2; exit 1; }
mkdir -p "$out"
# SHOOT_HOME_ROOT, else SRL_DEV_ROOT/shoot: where the throwaway homes go.
home_root=${SHOOT_HOME_ROOT:-${SRL_DEV_ROOT:-$HOME/.cache/srl-dev/app}/shoot}
mkdir -p "$home_root"
srl_home=$(mktemp -d "$home_root/home.XXXXXX")
tmp_png=$(mktemp "$home_root/shot.XXXXXX.png")
# Let Xvfb pick a free display (-displayfd): a fixed number can collide with another Xvfb, and
# then the app would open on someone else's screen.
dfile=$(mktemp "$home_root/display.XXXXXX")
Xvfb -displayfd 3 -screen 0 1600x1000x24 -nolisten tcp 3>"$dfile" >/dev/null 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true; rm -rf "$dfile" "$srl_home" "$tmp_png"' EXIT
for _ in $(seq 1 50); do [ -s "$dfile" ] && break; sleep 0.1; done
disp=:$(head -1 "$dfile")
[ "$disp" != ":" ] || { echo "Xvfb did not start" >&2; exit 1; }
for id in "${ids[@]}"; do
  # A fresh home per shot: a screen never inherits settings changed by the one before.
  rm -rf "$srl_home"/*
  printf '{"player_name":"VioletRunner","language":"%s","max_memory_mb":3072%s}\n' "$lang" "$extra" \
    >"$srl_home/settings.json"
  env -u WAYLAND_DISPLAY DISPLAY=$disp GPUI_X11_SCALE_FACTOR=1 SRL_FAKE=1 SRL_HOME="$srl_home" SRL_LANG=$lang \
    SRL_DEMO=$demo SRL_SCREEN=$id "$bin" >/dev/null 2>&1 &
  app=$!
  win=""
  for _ in $(seq 1 150); do win=$(DISPLAY=$disp xdotool search --name '^Speedrunning Launcher$' 2>/dev/null | head -1 || true); [ -n "$win" ] && break; sleep 0.1; done
  # `import -window ""` would wait for a click, so a window that never shows is reported and skipped.
  if [ -z "$win" ]; then
    echo "no window for $id" >&2
    kill $app 2>/dev/null || true; wait $app 2>/dev/null || true
    continue
  fi
  # Park the pointer outside the window so nothing shows a hover state.
  DISPLAY=$disp xdotool mousemove 1590 990 || true
  sleep 1.5
  dest="$out/$id$suffix.$fmt"
  # A busy machine can leave the window black (no frame presented yet) after the first wait, so a
  # black capture is taken again, up to three times, a little later.
  shot=""
  for try in 1 2 3; do
    if DISPLAY=$disp timeout 20 import -window "$win" "$tmp_png" 2>/dev/null; then
      mean=$(convert "$tmp_png" -format '%[fx:mean]' info: 2>/dev/null || echo 0)
      if awk -v m="$mean" 'BEGIN { exit !(m > 0.004) }'; then shot=1; break; fi
    fi
    sleep $((try * 2))
  done
  if [ -z "$shot" ]; then
    echo "import failed for $id" >&2
  elif [ "$fmt" = png ]; then
    cp "$tmp_png" "$dest"
  else
    convert "$tmp_png" -quality 85 "$dest"
  fi
  kill $app 2>/dev/null || true; wait $app 2>/dev/null || true
done
echo "wrote ${#ids[@]} screenshots ($lang${demo:+, demo}) to $out"
