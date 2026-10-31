#!/bin/bash
# round 3: bastion (long bridge), mode-rsg (island, no chest), mode-ssg (beached shipwreck). usage: render_batch3.sh <pass>
B=/tmp/srl/art/blender-dl/blender-5.2.3-linux-x64/blender
S=/tmp/srl/art/src
P=${1:-p1}
mkdir -p /tmp/srl/art/work/pass4
cd /tmp/srl/art/work
run() { local name=$1; local tag=$2; shift 2
  local t0=$(date +%s)
  timeout 900 $B -b -P $S/$name.py -- "$@" > /tmp/srl/art/work/log_${tag}.txt 2>&1
  echo "$tag $(( $(date +%s) - t0 ))s $(grep -c RENDER /tmp/srl/art/work/log_${tag}.txt)" >> /tmp/srl/art/work/batch_times.txt
}
run scene_bastion2 bastion_r3$P out=../work/pass4/bastion_$P "${@:2}"
run scene_coast rsg_r3$P preset=rsg out=../work/pass4/mode-rsg_$P
run scene_coast ssg_r3$P preset=ssg out=../work/pass4/mode-ssg_$P
echo "DONE3 $P" >> /tmp/srl/art/work/batch_times.txt
