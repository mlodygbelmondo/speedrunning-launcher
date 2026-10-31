"""isair.py <npz> x,y,z [x,y,z ...] -> air? and free radius (cube) around each point"""
import sys
import numpy as np
d = np.load(f"/tmp/srl/mcworld/extract/{sys.argv[1]}.npz")
arr, pal, o = d["arr"], [str(p) for p in d["pal"]], d["origin"]
air = np.array([p.split("[")[0][10:] in ("air", "cave_air") for p in pal])
A = air[arr]
for s in sys.argv[2:]:
    x, y, z = [int(float(v)) for v in s.split(",")]
    i, j, k = x - o[0], y - o[1], z - o[2]
    r = 0
    while r < 30:
        sub = A[max(0, i - r - 1):i + r + 2, max(0, j - r - 1):j + r + 2, max(0, k - r - 1):k + r + 2]
        if not sub.all():
            break
        r += 1
    print(s, "air" if A[i, j, k] else "SOLID", "free radius", r)
