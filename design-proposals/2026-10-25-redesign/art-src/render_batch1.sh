#!/bin/bash
# full 1920x1080 passes, one after the other (each Cycles render is capped at 195 s)
B=/tmp/srl/art/blender-dl/blender-5.2.3-linux-x64/blender
S=/tmp/srl/art/src
cd /tmp/srl/art/work
run() { local name=$1; local tag=$2; shift 2
  local t0=$(date +%s)
  timeout 900 $B -b -P $S/$name.py -- "$@" > /tmp/srl/art/work/log_${tag}.txt 2>&1
  echo "$tag $(( $(date +%s) - t0 ))s $(grep -c Saved /tmp/srl/art/work/log_${tag}.txt)" >> /tmp/srl/art/work/batch_times.txt
}
run scene_draftout draftout_p1 out=mode-draftout
cp /tmp/srl/art/out/mode-draftout.png pass3/mode-draftout_p1.png
run scene_fsg fsg_p1 out=mode-fsg
cp /tmp/srl/art/out/mode-fsg.png pass3/mode-fsg_p1.png
run scene_ranked ranked_p1 out=mode-ranked
cp /tmp/srl/art/out/mode-ranked.png pass3/mode-ranked_p1.png
run scene_ssg ssg_p1 samples=128
(cd $S && uv run --with numpy --with pillow python grade_ssg.py /tmp/srl/art/work/mode-ssg_raw.png /tmp/srl/art/out/mode-ssg.png)
cp /tmp/srl/art/out/mode-ssg.png pass3/mode-ssg_p1.png
run scene_stronghold2 stronghold_p2 samples=128 cam=-224.5,26.5,1384.7 tgt=-218,23.5,1379.5 lens=16 exp=0.75 fill=8 les=2.0 lg=70 ees=3.4 out=stronghold
cp /tmp/srl/art/out/stronghold.png pass3/stronghold_p2.png
run scene_rsg rsg_p2 lens=34 pit=25 out=mode-rsg
cp /tmp/srl/art/out/mode-rsg.png pass3/mode-rsg_p2.png
echo DONE >> /tmp/srl/art/work/batch_times.txt
