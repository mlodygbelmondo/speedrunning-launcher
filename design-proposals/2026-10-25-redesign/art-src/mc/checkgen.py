import sys, json
sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcregion import Dim, chunk_status
from collections import Counter
WD, jobs = sys.argv[1], json.load(open(sys.argv[2]))
dd = {"minecraft:the_nether": Dim(f"{WD}/world/DIM-1/region"), "minecraft:overworld": Dim(f"{WD}/world/region"), "minecraft:the_end": Dim(f"{WD}/world/DIM1/region")}
for (dim, x0, z0, x1, z1) in jobs:
    c = Counter()
    for cx in range(x0 >> 4, (x1 >> 4) + 1):
        for cz in range(z0 >> 4, (z1 >> 4) + 1):
            ch = dd[dim].chunk(cx, cz)
            c[chunk_status(ch) if ch else "missing"] += 1
    print(dim[10:], x0, z0, dict(c))
