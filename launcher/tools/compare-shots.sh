#!/usr/bin/env bash
# Pixel comparison of the app's SRL_DEMO screenshots with the prototype's, both lossless PNG:
#   BAKEOFF_GPUI_BIN=... bakeoff/tools/shoot-gpui.sh <protoDir> <ids...>          (gpui-<id>.png)
#   SRL_DEMO=1 SHOOT_FORMAT=png launcher/tools/shoot-app.sh <appDir> <ids...>      (<id>.png)
# Usage: launcher/tools/compare-shots.sh <protoDir> <appDir> <diffDir> <ids...>
# Prints, per id, how many pixels differ by more than 2 % (ImageMagick `compare -metric AE
# -fuzz 2%`), and writes <diffDir>/<id>.png with the differing pixels in red.
set -euo pipefail
proto=${1:?protoDir}; app=${2:?appDir}; diff=${3:?diffDir}; shift 3
mkdir -p "$diff"
for id in "$@"; do
  # `compare` exits 1 when the images differ; the count is on stderr.
  n=$(compare -metric AE -fuzz 2% "$proto/gpui-$id.png" "$app/$id.png" "$diff/$id.png" 2>&1 || true)
  echo "$id: $n px differ"
done
