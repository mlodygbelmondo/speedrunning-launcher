#!/bin/bash
# second full passes of the mode images
B=/tmp/srl/art/blender-dl/blender-5.2.3-linux-x64/blender
S=/tmp/srl/art/src
cd /tmp/srl/art/work
run() { local name=$1; local tag=$2; shift 2
  local t0=$(date +%s)
  timeout 900 $B -b -P $S/$name.py -- "$@" > /tmp/srl/art/work/log_${tag}.txt 2>&1
  echo "$tag $(( $(date +%s) - t0 ))s $(grep -c Saved /tmp/srl/art/work/log_${tag}.txt)" >> /tmp/srl/art/work/batch_times.txt
}
run scene_draftout draftout_p2 out=mode-draftout
cp /tmp/srl/art/out/mode-draftout.png pass3/mode-draftout_p2.png
run scene_fsg fsg_p2 out=mode-fsg
cp /tmp/srl/art/out/mode-fsg.png pass3/mode-fsg_p2.png
run scene_ranked ranked_p2 out=mode-ranked
cp /tmp/srl/art/out/mode-ranked.png pass3/mode-ranked_p2.png
run scene_ssg ssg_p2 samples=128
(cd $S && uv run --with numpy --with pillow python grade_ssg.py /tmp/srl/art/work/mode-ssg_raw.png /tmp/srl/art/out/mode-ssg.png)
cp /tmp/srl/art/out/mode-ssg.png pass3/mode-ssg_p2.png
echo DONE2 >> /tmp/srl/art/work/batch_times.txt
