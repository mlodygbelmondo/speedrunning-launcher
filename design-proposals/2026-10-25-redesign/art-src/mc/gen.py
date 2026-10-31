"""Force-generate block boxes in a vanilla server world.

usage: python3 gen.py <version> <seed> <jobs.json> [java]
jobs.json: [[dim, x0, z0, x1, z1], ...]
"""
import json
import os
import sys
import time

sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcdrive import Server

ver, seed, jobs = sys.argv[1], sys.argv[2], json.load(open(sys.argv[3]))
java = sys.argv[4] if len(sys.argv) > 4 else "java"
WD = f"/tmp/srl/mcworld/{ver}/w_{seed}"
# 1.16.1 races (ConcurrentModificationException) when several workers first load village pieces;
# MC_CPUS=2 gives the server a single worldgen thread.
cpus = os.environ.get("MC_CPUS")
s = Server(WD, f"/tmp/srl/mcworld/{ver}/server.jar", java=java, seed=seed,
           jvm=[f"-XX:ActiveProcessorCount={cpus}"] if cpus else [])
s.ready()
t0 = time.time()
for (dim, x0, z0, x1, z1) in jobs:
    t = time.time()
    n = s.forceload_area(dim, x0, z0, x1, z1)
    s.wait_generated(dim, x0, z0, x1, z1, timeout=3600, every=3)
    s.save()
    s.send(f"execute in {dim} run forceload remove all")
    print(f"[gen] {dim} {x0},{z0}..{x1},{z1}: {n} chunks in {time.time() - t:.0f}s", flush=True)
s.save()
s.stop()
print(f"[gen] DONE in {time.time() - t0:.0f}s")
