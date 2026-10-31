import sys
sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcregion import Dim, structure_starts
from collections import Counter
wd, dk, x, z, key = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), sys.argv[5]
sub = {"n": "DIM-1/region", "o": "region", "e": "DIM1/region"}[dk]
d = Dim(f"{wd}/world/{sub}")
st = structure_starts(d.chunk(x >> 4, z >> 4))[key]
print("BB", [int(v) for v in st["BB"]])
for c in st["Children"]:
    pe = c.get("pool_element")
    loc = pe.get("location") if isinstance(pe, dict) else None
    if isinstance(loc, dict): loc = loc
    print(c.get("id"), loc or c.get("Template") or "", [int(v) for v in c["BB"]], c.get("rotation", c.get("Rot", "")), c.get("O", ""))
