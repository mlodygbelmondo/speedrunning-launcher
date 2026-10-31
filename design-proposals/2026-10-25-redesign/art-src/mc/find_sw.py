"""Survey shipwrecks (beached / shallow) and villages on a grid, and generate extra boxes in the same server run.
usage: python3 find_sw.py <seed> [extra_jobs.json]"""
import json
import math
import sys
import numpy as np
sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcdrive import Server
from mcregion import Dim, structure_starts, extract_box

seed = sys.argv[1]
extra = json.load(open(sys.argv[2])) if len(sys.argv) > 2 else []
WD = f"/tmp/srl/mcworld/1.16.1/w_{seed}"
OV = "minecraft:overworld"
s = Server(WD, "/tmp/srl/mcworld/1.16.1/server.jar", seed=seed, jvm=["-XX:ActiveProcessorCount=2"])
s.ready()
found = {}
for what in ("shipwreck", "village"):
    pts = set()
    for x in range(-3200, 3201, 640):
        for z in range(-3200, 3201, 640):
            pos, _ = s.locate(what, OV, at=(x, z))
            if pos:
                pts.add((pos[0], pos[2]))
    found[what] = sorted(pts)
    print(f"[sw] {what}: {len(pts)}", flush=True)
for (x, z) in found["shipwreck"]:
    s.send(f"execute in {OV} run forceload add {x - 40} {z - 40} {x + 40} {z + 40}")
for (x, z) in found["shipwreck"]:
    s.wait_generated(OV, x - 40, z - 40, x + 40, z + 40, timeout=900, every=2)
s.save()
s.send(f"execute in {OV} run forceload remove all")
for (dim, x0, z0, x1, z1) in extra:
    n = s.forceload_area(dim, x0, z0, x1, z1)
    s.wait_generated(dim, x0, z0, x1, z1, timeout=3600, every=3)
    s.save()
    s.send(f"execute in {dim} run forceload remove all")
    print(f"[sw] gen {dim} {x0},{z0}..{x1},{z1}: {n} chunks", flush=True)
s.save()
s.stop()

d = Dim(f"{WD}/world/region")
rps = json.load(open(f"{WD}/ruined_portals.json"))
surf_rp = [r for r in rps if r["vp"] in ("on_land_surface", "partly_buried", "on_ocean_floor")]
out = []
for (x, z) in found["shipwreck"]:
    st = structure_starts(d.chunk(x >> 4, z >> 4)).get("shipwreck")
    if not st:
        continue
    c = st["Children"][0]
    bb = [int(v) for v in c["BB"]]
    pal, arr = extract_box(d, bb[0], 40, bb[2], bb[3], 120, bb[5])
    names = [p.split("[")[0][10:] for p in pal]
    air = np.array([n in ("air", "cave_air") for n in names])
    water = np.array([n in ("water", "seagrass", "tall_seagrass", "kelp", "kelp_plant") for n in names])
    wood = np.array([("spruce" in n or "oak" in n or "jungle" in n or "birch" in n or "dark_oak" in n or "acacia" in n)
                     and "leaves" not in n for n in names])
    solid = ~(air | water)
    A = arr
    nx, ny, nz = A.shape
    top = ny - 1 - np.argmax(solid[A][:, ::-1, :], axis=1)
    topblk = np.take_along_axis(A, top[:, None, :], 1)[:, 0, :]
    wood_top = wood[topblk].mean()
    wcol = water[A].sum(1)  # water blocks above in each column
    dmin = lambda P: min((math.hypot(P[0] - x, P[1] - z) for P in []), default=0)
    near_rp = min(((math.hypot(r["pos"][0] - x, r["pos"][1] - z), r["pos"], r["vp"]) for r in surf_rp), default=None)
    near_v = min(((math.hypot(v[0] - x, v[1] - z), v) for v in found["village"]), default=None)
    rec = dict(pos=[x, z], bb=bb, tpl=c["Template"].split("/")[-1], beached=int(c["isBeached"]),
               wood_top=round(float(wood_top), 2), water_over=round(float(wcol.mean()), 1),
               rp=[round(near_rp[0]), near_rp[1], near_rp[2]] if near_rp else None,
               village=[round(near_v[0]), near_v[1]] if near_v else None)
    out.append(rec)
    print("[sw]", rec, flush=True)
json.dump(dict(shipwrecks=out, villages=found["village"]), open(f"{WD}/shipwrecks.json", "w"), indent=1)
print("[sw] DONE")
