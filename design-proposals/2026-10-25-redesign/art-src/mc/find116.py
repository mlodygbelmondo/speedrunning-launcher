"""Find candidate structures in a 1.16.1 world and identify their types from region data.

usage: python3 find116.py <seed>
"""
import json
import sys
import time

sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcdrive import Server

seed = sys.argv[1]
WD = f"/tmp/srl/mcworld/1.16.1/w_{seed}"
s = Server(WD, "/tmp/srl/mcworld/1.16.1/server.jar", seed=seed)
s.ready()
found = {}
NETHER, OVER = "minecraft:the_nether", "minecraft:overworld"
grid_n = [(x, z) for x in (-1200, -400, 400, 1200) for z in (-1200, -400, 400, 1200)]
grid_o = [(x, z) for x in (-2000, -700, 700, 2000) for z in (-2000, -700, 700, 2000)]
jobs = [("bastion_remnant", NETHER, grid_n), ("fortress", NETHER, grid_n[::2]),
        ("buried_treasure", OVER, grid_o), ("ruined_portal", OVER, grid_o[::2]),
        ("village", OVER, grid_o[::2]), ("shipwreck", OVER, grid_o[::2]), ("stronghold", OVER, [(0, 0)])]
for what, dim, grid in jobs:
    pts = set()
    for (x, z) in grid:
        pos, line = s.locate(what, dim, at=(x, z))
        if pos:
            pts.add((pos[0], pos[2]))
    found[what] = sorted(pts)
    print(what, len(pts), sorted(pts)[:20], flush=True)

# generate the start chunk of each candidate so its Starts NBT exists
for what, dim, _ in jobs:
    for (x, z) in found[what]:
        s.send(f"execute in {dim} run forceload add {x} {z}")
time.sleep(5)
for what, dim, _ in jobs:
    pts = found[what]
    for (x, z) in pts:
        pass
# wait for all to be generated
for what, dim, _ in jobs:
    for (x, z) in found[what]:
        s.wait_generated(dim, x, z, x, z, timeout=900, every=1)
s.save()
for what, dim, _ in jobs:
    for (x, z) in found[what]:
        s.send(f"execute in {dim} run forceload remove {x} {z}")
s.save()
s.stop()
json.dump(found, open(f"{WD}/candidates.json", "w"), indent=1)
print("DONE")
