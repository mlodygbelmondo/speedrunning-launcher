"""Find villages on a grid, generate a box around each and list their pieces (type, houses, bell, well).
usage: python3 find_village.py <seed> [radius] [step]
Writes /tmp/srl/mcworld/1.16.1/w_<seed>/villages.json and a top-down preview per village into
/tmp/srl/mcworld/previews/village_<x>_<z>.png."""
import json
import os
import sys
from collections import Counter
sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcdrive import Server
from mcregion import Dim, structure_starts

seed = sys.argv[1]
RAD = int(sys.argv[2]) if len(sys.argv) > 2 else 1200
STEP = int(sys.argv[3]) if len(sys.argv) > 3 else 600
WD = f"/tmp/srl/mcworld/1.16.1/w_{seed}"
OV = "minecraft:overworld"
H = 96  # half box: village pieces stay within ~80 blocks of the start
# 1.16.1 races when several workers load village pieces at once: one worldgen thread.
s = Server(WD, "/tmp/srl/mcworld/1.16.1/server.jar", seed=seed, jvm=["-XX:ActiveProcessorCount=2"])
s.ready()
pts = set()
for x in range(-RAD, RAD + 1, STEP):
    for z in range(-RAD, RAD + 1, STEP):
        pos, _ = s.locate("village", OV, at=(x, z))
        if pos and abs(pos[0]) <= RAD + 300 and abs(pos[2]) <= RAD + 300:
            pts.add((pos[0], pos[2]))
pts = sorted(pts)
print("[v]", len(pts), "villages", pts, flush=True)
for (x, z) in pts:
    s.forceload_area(OV, x - H, z - H, x + H, z + H)
    s.wait_generated(OV, x - H, z - H, x + H, z + H, timeout=1800, every=2)
    s.save()
    s.send(f"execute in {OV} run forceload remove all")
    print("[v] generated", x, z, flush=True)
s.save()
s.stop()

d = Dim(f"{WD}/world/region")
out = []
for (x, z) in pts:
    st = structure_starts(d.chunk(x >> 4, z >> 4)).get("village")
    if not st:
        print("[v] no start at", x, z)
        continue
    kids = st.get("Children", [])
    tpls = []
    bbs = []
    for c in kids:
        el = c.get("pool_element", {})
        loc = el.get("location") or ""
        if not loc and "elements" in el:
            loc = ";".join(e.get("location", "") for e in el["elements"])
        tpls.append(str(loc).replace("minecraft:village/", ""))
        bbs.append([int(v) for v in c["BB"]])
    kinds = Counter(t.split("/")[0] for t in tpls if t)
    houses = [t for t in tpls if "/houses/" in t]
    bb = [min(b[0] for b in bbs), min(b[1] for b in bbs), min(b[2] for b in bbs),
          max(b[3] for b in bbs), max(b[4] for b in bbs), max(b[5] for b in bbs)]
    rec = dict(pos=[x, z], kind=kinds.most_common(1)[0][0] if kinds else "?", n=len(kids), houses=len(houses),
               bb=bb, pieces=tpls)
    out.append(rec)
    print("[v]", x, z, rec["kind"], "pieces", len(kids), "houses", len(houses), "bb", bb, flush=True)
    print("     ", ", ".join(sorted(set(t.split("/")[-1] for t in tpls))), flush=True)
json.dump(out, open(f"{WD}/villages.json", "w"), indent=1)
print("[v] DONE")
