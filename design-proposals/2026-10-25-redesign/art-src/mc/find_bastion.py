"""Survey bridge bastions: locate on a grid, generate around each start, score openness over lava.
usage: python3 find_bastion.py <seed>"""
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
for x in range(-3000, 3001, 500):
    for z in range(-3000, 3001, 500):
        pos, _ = s.locate("bastion_remnant", NE, at=(x, z))
        if pos:
            pts.add((pos[0], pos[2]))
pts = sorted(pts)
print("[ba]", len(pts), "bastions", flush=True)
for (x, z) in pts:
    s.send(f"execute in {NE} run forceload add {x - 64} {z - 64} {x + 64} {z + 64}")
for (x, z) in pts:
    s.wait_generated(NE, x - 64, z - 64, x + 64, z + 64, timeout=900, every=2)
s.save()
s.send(f"execute in {NE} run forceload remove all")
s.save()
s.stop()
d = Dim(f"{WD}/world/DIM-1/region")
out = []
for (x, z) in pts:
    st = structure_starts(d.chunk(x >> 4, z >> 4)).get("bastion_remnant")
    if not st:
        continue
    ch = st["Children"]
    br = [c for c in ch if "bridge/bridge_pieces/bridge" in str(c.get("pool_element", {}).get("location", ""))]
    if not br:
        continue
    bb = [int(v) for v in br[0]["BB"]]
    wide_x = (bb[3] - bb[0]) > (bb[5] - bb[2])
    m = 24
    if wide_x:
        box = (bb[0], 32, bb[2] - m, bb[3], 100, bb[5] + m)
    else:
        box = (bb[0] - m, 32, bb[2], bb[3] + m, 100, bb[5])
    pal, arr = extract_box(d, *box)
    names = [p.split("[")[0][10:] for p in pal]
    air = np.array([n in ("air", "cave_air") for n in names])
    lava = np.array([n == "lava" for n in names])
    A = air[arr]
    open_lo = A[:, 0:30, :].mean()
    open_hi = A[:, 30:68, :].mean()
    pal2, arr2 = extract_box(d, bb[0], 31, bb[2], bb[3], 31, bb[5])
    lv = np.array([p.split("[")[0][10:] == "lava" for p in pal2])[arr2].mean()
    biome = "?"
    rec = dict(pos=[x, z], bridge=bb, open_lo=round(float(open_lo), 2), open_hi=round(float(open_hi), 2), lava=round(float(lv), 2))
    out.append(rec)
    print("[ba]", rec, flush=True)
json.dump(out, open(f"{WD}/bridge_bastions.json", "w"), indent=1)
print("[ba] DONE")
