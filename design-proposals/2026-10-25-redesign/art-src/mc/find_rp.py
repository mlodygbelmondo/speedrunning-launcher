"""Find ruined portals on a grid, generate a box around each land one, and score them.
usage: python3 find_rp.py <seed>"""
import json
import sys
import numpy as np
sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcdrive import Server
from mcregion import Dim, structure_starts, extract_box

seed = sys.argv[1]
WD = f"/tmp/srl/mcworld/1.16.1/w_{seed}"
OV = "minecraft:overworld"
s = Server(WD, "/tmp/srl/mcworld/1.16.1/server.jar", seed=seed, jvm=["-XX:ActiveProcessorCount=2"])
s.ready()
pts = set()
for x in range(-2800, 2801, 700):
    for z in range(-2800, 2801, 700):
        pos, _ = s.locate("ruined_portal", OV, at=(x, z))
        if pos:
            pts.add((pos[0], pos[2]))
pts = sorted(pts)
print("[rp]", len(pts), "portals", flush=True)
for (x, z) in pts:
    s.send(f"execute in {OV} run forceload add {x - 24} {z - 24} {x + 24} {z + 24}")
for (x, z) in pts:
    s.wait_generated(OV, x - 24, z - 24, x + 24, z + 24, timeout=900, every=1)
s.save()
s.send(f"execute in {OV} run forceload remove all")
s.save()
s.stop()
d = Dim(f"{WD}/world/region")
out = []
for (x, z) in pts:
    st = structure_starts(d.chunk(x >> 4, z >> 4)).get("ruined_portal")
    if not st:
        continue
    c = st["Children"][0]
    bb = [int(v) for v in c["BB"]]
    pal, arr = extract_box(d, bb[0] - 6, bb[1] - 4, bb[2] - 6, bb[3] + 6, bb[4] + 4, bb[5] + 6)
    cnt = {}
    for i, n in enumerate(np.bincount(arr.ravel(), minlength=len(pal))):
        k = pal[i].split("[")[0][10:]
        cnt[k] = cnt.get(k, 0) + int(n)
    keys = ["chest", "gold_block", "crying_obsidian", "obsidian", "lava", "magma_block", "netherrack", "grass_block", "oak_leaves", "water"]
    rec = dict(pos=[x, z], bb=bb, tpl=c["Template"].split("/")[-1], vp=c["VerticalPlacement"], cold=c["Properties"]["cold"],
               **{k: cnt.get(k, 0) for k in keys})
    out.append(rec)
    print("[rp]", rec, flush=True)
json.dump(out, open(f"{WD}/ruined_portals.json", "w"), indent=1)
print("[rp] DONE")
