"""extract.py <worlddir> <dim n|o|e> x0 y0 z0 x1 y1 z1 <name>  -> /tmp/srl/mcworld/extract/<name>.npz"""
import sys, os, numpy as np
sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcregion import Dim, extract_box
wd, dk = sys.argv[1], sys.argv[2]
x0, y0, z0, x1, y1, z1 = map(int, sys.argv[3:9])
name = sys.argv[9]
sub = {"n": "DIM-1/region", "o": "region", "e": "DIM1/region"}[dk]
pal, arr = extract_box(Dim(f"{wd}/world/{sub}"), x0, y0, z0, x1, y1, z1)
os.makedirs("/tmp/srl/mcworld/extract", exist_ok=True)
np.savez_compressed(f"/tmp/srl/mcworld/extract/{name}.npz", arr=arr, pal=np.array(pal), origin=np.array([x0, y0, z0]))
from collections import Counter
c = Counter()
for i, n in enumerate(np.bincount(arr.ravel(), minlength=len(pal))):
    c[pal[i].split("[")[0]] += int(n)
print(name, arr.shape, len(pal), "states")
print(" ".join(f"{k[10:]}:{v}" for k, v in c.most_common()))
