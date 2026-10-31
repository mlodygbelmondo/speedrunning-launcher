#!/bin/bash
# Render all six key-art scenes at 1920x1080 (sequentially, all CPU threads each).
B=/tmp/srl/art/blender-dl/blender-5.2.3-linux-x64/blender
SRC=/tmp/srl/art/src
cd /tmp/srl/art/work
SCENES="${@:-overworld portal bastion fortress stronghold end}"
for s in $SCENES; do
  case $s in
    overworld) f=scene_overworld.py; o=overworld;;
    portal) f=scene_portal.py; o=nether-portal;;
    bastion) f=scene_bastion.py; o=bastion;;
    fortress) f=scene_fortress.py; o=fortress;;
    stronghold) f=scene_stronghold.py; o=stronghold;;
    end) f=scene_end.py; o=the-end;;
  esac
  extra=""; [ "$s" = end ] && extra="lens=22"
  $B -b -P $SRC/$f -- samples=${SAMPLES:-128} out=$o $extra > /tmp/srl/art/work/log_$o.txt 2>&1
  grep -E "RENDER|Error|Traceback" /tmp/srl/art/work/log_$o.txt
done
echo BATCH_DONE
