#!/usr/bin/env bash
# Usage: shoot.sh <name> — renders <name>.html at 1280x800 to renders/<name>.png and a phone JPEG.
set -euo pipefail
cd "$(dirname "$0")"; mkdir -p renders
prof=$(mktemp -d /tmp/srl-rd-XXXX)
timeout 60 google-chrome --headless=new --no-first-run --disable-gpu --hide-scrollbars --user-data-dir="$prof" --allow-file-access-from-files \
  --window-size=1280,800 --force-device-scale-factor=1 --virtual-time-budget=4000 --screenshot="renders/$1.png" "file://$PWD/$1.html" >/dev/null 2>&1
rm -rf "$prof"
magick "renders/$1.png" -resize 960x -strip -quality 82 "renders/$1.jpg"
echo "renders/$1.png"
