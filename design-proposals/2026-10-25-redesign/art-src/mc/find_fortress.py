"""Survey fortresses: generate around each start and score how open (air around) the bridges are.
usage: python3 find_fortress.py <seed>"""
import json
import sys
import numpy as np
sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcdrive import Server
from mcregion import Dim, structure_starts, extract_box

seed = sys.argv[1]
WD = f"/tmp/srl/mcworld/1.16.1/w_{seed}"
NE = "minecraft:the_nether"
s = Server(WD, "/tmp/srl/mcworld/1.16.1/server.jar", seed=seed, jvm=["-XX:ActiveProcessorCount=2"])
s.ready()
pts = set()
for x in range(-2400, 2401, 800):
    for z in range(-2400, 2401, 800):
        pos, _ = s.locate("fortress", NE, at=(x, z))
        if pos:
            pts.add((pos[0], pos[2]))
pts = sorted(pts)
print("[fo]", len(pts), "fortresses", flush=True)
for (x, z) in pts:
    s.send(f"execute in {NE} run forceload add {x - 112} {z - 112} {x + 112} {z + 112}")
for (x, z) in pts:
    s.wait_generated(NE, x - 112, z - 112, x + 112, z + 112, timeout=1200, every=3)
s.save()
s.send(f"execute in {NE} run forceload remove all")
s.save()
s.stop()
d = Dim(f"{WD}/world/DIM-1/region")
out = []
for (x, z) in pts:
    st = structure_starts(d.chunk(x >> 4, z >> 4)).get("fortress")
    if not st:
        continue
    bb = [int(v) for v in st["BB"]]
    pal, arr = extract_box(d, bb[0], 40, bb[2], bb[3], 110, bb[5])
    names = [p.split("[")[0][10:] for p in pal]
    air = np.array([n in ("air", "cave_air") for n in names])[arr]
    lava = np.array([n == "lava" for n in names])[arr]
    nb = np.array([n.startswith("nether_brick") for n in names])[arr]
    scores = []
    nmt = 0
    for c in st["Children"]:
        cid = c["id"].split(":")[-1].lower()
        if cid == "nemt":
            nmt += 1
        if cid not in ("nebs", "nebcr", "nemt"):
            continue
        b = [int(v) for v in c["BB"]]
        i0, i1 = b[0] - bb[0], b[3] - bb[0] + 1
        k0, k1 = b[2] - bb[2], b[5] - bb[2] + 1
        # air above deck (y 80..100) and beside it, and depth of open air below the deck
        above = air[max(0, i0 - 8):i1 + 8, 80 - 40:100 - 40, max(0, k0 - 8):k1 + 8].mean()
        below = air[i0:i1, 45 - 40:68 - 40, k0:k1].mean()
        lv = lava[i0:i1, 0:31, k0:k1].mean()
        scores.append((round(float(above), 2), round(float(below), 2), cid, b))
    scores.sort(reverse=True)
    rec = dict(pos=[x, z], bb=bb, nemt=nmt, best=scores[:4], mean_above=round(float(np.mean([t[0] for t in scores])), 2))
    out.append(rec)
    print("[fo]", rec, flush=True)
json.dump(out, open(f"{WD}/fortresses.json", "w"), indent=1)
print("[fo] DONE")
