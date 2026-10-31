"""campick.py <npz> tx,ty,tz[;...] ring: search camera spots with clear line of sight to all targets.
args: npz targets xmin,xmax,zmin,zmax ymin,ymax step"""
import sys
import numpy as np
d = np.load(f"/tmp/srl/mcworld/extract/{sys.argv[1]}.npz")
arr, pal, o = d["arr"], [str(p) for p in d["pal"]], d["origin"]
names = [p.split("[")[0][10:] for p in pal]
clear = np.array([n in ("air", "cave_air", "lava", "fire", "soul_fire", "weeping_vines", "weeping_vines_plant", "twisting_vines",
                        "twisting_vines_plant", "crimson_roots", "warped_roots", "nether_sprouts", "chain", "lantern") for n in names])
C = clear[arr]
air = np.array([n in ("air", "cave_air") for n in names])[arr]
T = [tuple(float(v) for v in t.split(",")) for t in sys.argv[2].split(";")]
x0, x1, z0, z1 = [int(v) for v in sys.argv[3].split(",")]
y0, y1 = [int(v) for v in sys.argv[4].split(",")]
st = int(sys.argv[5]) if len(sys.argv) > 5 else 4


def los(a, b):
    a, b = np.array(a), np.array(b)
    n = int(np.linalg.norm(b - a) * 2) + 1
    p = a[None, :] + (b - a)[None, :] * np.linspace(0, 1, n)[:, None]
    q = np.floor(p).astype(int) - o[None, :]
    q = q[2:-2]
    ok = (q >= 0).all(1) & (q < np.array(arr.shape)).all(1)
    if not ok.all():
        return 0.0
    return C[q[:, 0], q[:, 1], q[:, 2]].mean()


res = []
for x in range(x0, x1 + 1, st):
    for z in range(z0, z1 + 1, st):
        for y in range(y0, y1 + 1, st):
            i, j, k = x - o[0], y - o[1], z - o[2]
            if not (0 < i < arr.shape[0] - 3 and 0 < j < arr.shape[1] - 3 and 0 < k < arr.shape[2] - 3):
                continue
            if not air[i - 2:i + 3, j - 2:j + 3, k - 2:k + 3].all():
                continue
            s = [los((x + .5, y + .5, z + .5), t) for t in T]
            if min(s) > 0.999:
                res.append((x, y, z))
print(len(res), "clear spots")
for r in res[:: max(1, len(res) // 60)]:
    print(r)
