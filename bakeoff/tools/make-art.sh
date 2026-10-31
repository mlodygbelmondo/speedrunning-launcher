#!/usr/bin/env bash
# Usage: bakeoff/tools/make-art.sh <masters dir>
# Turns the 1920x1080 key-art renders into the two sizes the app ships (SCREENS.md "Scene v3"):
#   assets/art/hero/<id>.jpg  1280x800, cropped so the subject sits right of centre (text goes on the left)
#   assets/art/card/<id>.jpg  640x360, centre crop, for tiles, rows and split segments
# Masters are PNG (fresh renders) or JPEG (design-proposals/2026-10-25-redesign/art/masters). Mode art is read from
# mode-<id>; until a mode render exists, the listed fallback render is used.
set -euo pipefail
src=${1:?masters dir}
out=$(cd "$(dirname "$0")/.." && pwd)/assets/art
mkdir -p "$out/hero" "$out/card"
# id:master:fallback:focal-x (0 = keep left edge, 1 = keep right edge)
list=(
  ranked:mode-ranked:bastion:0.3
  rsg:mode-rsg:overworld:0
  fsg:mode-fsg:nether-portal:0
  ssg:mode-ssg:stronghold:0
  draftout:mode-draftout:the-end:0.6
  overworld:overworld::0.62
  nether-portal:nether-portal::0.7
  bastion:bastion::0.5
  fortress:fortress::0.6
  stronghold:stronghold::0.5
  the-end:the-end::0.55
)
for e in "${list[@]}"; do
  IFS=: read -r id master fallback fx <<<"$e"
  f=
  for n in "$master" "$fallback"; do
    for ext in png jpg; do [[ -z $f && -n $n && -f $src/$n.$ext ]] && f=$src/$n.$ext; done
  done
  [[ -n $f ]] || { echo "missing $master" >&2; exit 1; }
  # Scale to 800 px high, then crop 1280 wide at the focal point.
  w=$(magick "$f" -resize x800 -format '%w' info:)
  x=$(python3 -c "print(round(($w-1280)*$fx))")
  magick "$f" -resize x800 -crop 1280x800+"$x"+0 +repage -strip -interlace none -quality 84 "$out/hero/$id.jpg"
  magick "$f" -resize 640x360^ -gravity center -extent 640x360 -strip -quality 82 "$out/card/$id.jpg"
  echo "$id <- $(basename "$f")"
done
du -sh "$out"
