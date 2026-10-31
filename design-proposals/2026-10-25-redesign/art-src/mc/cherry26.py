"""26.3: find a cherry grove for a seed and force-generate the area around it.

usage: python3 cherry26.py <seed> [radius]
"""
import json
import sys
import time

sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcdrive import Server

seed = sys.argv[1]
rad = int(sys.argv[2]) if len(sys.argv) > 2 else 112
JAVA = "/home/piotr/.local/share/mise/installs/java/temurin-25.0.5+8.0.LTS/bin/java"
WD = f"/tmp/srl/mcworld/26.3/w_{seed}"
s = Server(WD, "/tmp/srl/mcworld/26.3/server.jar", java=JAVA, seed=seed)
s.ready()
OV = "minecraft:overworld"
found = []
for at in [(0, 0), (1500, 0), (-1500, 0), (0, 1500), (0, -1500)]:
    pos, line = s.locate("minecraft:cherry_grove", OV, at=at, kind="biome")
    print("[cherry]", at, line, flush=True)
    if pos:
        found.append(pos)
        break
for (x, _, z) in found[:1]:
    n = s.forceload_area(OV, x - rad, z - rad, x + rad, z + rad)
    s.wait_generated(OV, x - rad, z - rad, x + rad, z + rad, timeout=1800, every=2)
    s.save()
    s.send(f"execute in {OV} run forceload remove all")
    print(f"[cherry] generated {n} chunks around {x},{z}", flush=True)
s.save()
s.stop()
json.dump({"found": found, "rad": rad}, open(f"{WD}/cherry.json", "w"))
print("[cherry] DONE")
