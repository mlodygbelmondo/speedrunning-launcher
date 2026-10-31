"""26.3 force-generation of boxes. usage: gen26.py <seed> <jobs.json>"""
import json
import sys
sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcdrive import Server
JAVA = "/home/piotr/.local/share/mise/installs/java/temurin-25.0.5+8.0.LTS/bin/java"
seed, jobs = sys.argv[1], json.load(open(sys.argv[2]))
s = Server(f"/tmp/srl/mcworld/26.3/w_{seed}", "/tmp/srl/mcworld/26.3/server.jar", java=JAVA, seed=seed)
s.ready()
for (dim, x0, z0, x1, z1) in jobs:
    n = s.forceload_area(dim, x0, z0, x1, z1)
    s.wait_generated(dim, x0, z0, x1, z1, timeout=1800, every=2)
    s.save()
    s.send(f"execute in {dim} run forceload remove all")
    print(f"[gen26] {x0},{z0}..{x1},{z1}: {n} chunks", flush=True)
s.save()
s.stop()
print("[gen26] DONE")
