"""List surface (sky-exposed) blocks of a kind, clustered. usage: surf.py wd x0 z0 x1 z1 y0 y1 kind"""
import sys
import numpy as np
sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcregion import Dim, extract_box
wd = sys.argv[1]; x0, z0, x1, z1, y0, y1 = map(int, sys.argv[2:8]); kind = sys.argv[8]
pal, arr = extract_box(Dim(f"{wd}/world/region"), x0, y0, z0, x1, y1, z1)
air = [i for i, p in enumerate(pal) if p.split("[")[0] in ("minecraft:air", "minecraft:cave_air")]
solid = ~np.isin(arr, air)
nx, ny, nz = arr.shape
top = ny - 1 - np.argmax(solid[:, ::-1, :], axis=1)
blk = np.take_along_axis(arr, top[:, None, :], 1)[:, 0, :]
ids = [i for i, p in enumerate(pal) if kind in p]
m = np.isin(blk, ids)
from scipy import ndimage
lab, n = ndimage.label(m)
for k in range(1, n + 1):
    xs, zs = np.nonzero(lab == k)
    if len(xs) >= 3:
        print(f"cluster {len(xs):4d}  x {x0+xs.min()}..{x0+xs.max()}  z {z0+zs.min()}..{z0+zs.max()}  y {y0+top[xs[0],zs[0]]}")
