"""mode-draftout.png - a REAL cherry grove of seed 2026 (Java 26.3), block for block.

Generated with the 26.3 server (Java 25); the grove covers about x -300..-30, z -430..-100 on a plateau at
y 115..128. Pink petals carpet the grass; a few petals drift in the air (ours); soft late light and haze.
"""
import math
import sys
import time
sys.path.insert(0, "/tmp/srl/art/src")
import numpy as np
import vox
from vox import *
import mcblocks as mc

T0 = time.time()
A = args()
SAMPLES = int(A.get("samples", 128))
reset()

if A.get("lalpha", "0") != "1":
    vox.PACKED.add("cherry_leaves")  # solid canopy: keep the pink of the cut-out pixels
pal, arr, origin = mc.load("draftout")
x0, y0, z0 = origin
nx, ny, nz = arr.shape
cam = [float(v) for v in A.get("cam", "-160,126.5,-330").split(",")]
tgt = [float(v) for v in A.get("tgt", "-130,121,-290").split(",")]
OX, OY, OZ = int(cam[0]), int(cam[1]) - 4, int(cam[2])


def P(x, y, z):
    return (x - OX, OZ - z, y - OY)


b = mc.Builder(pal, arr, origin, OX, OY, OZ, seed=12,
               mat_over={"water": dict(rough=0.05, ior=1.33, alpha_const=0.75, spec=0.6),
                         "cherry_leaves": dict(alpha=A.get("lalpha", "0") == "1", tint=(1.06, 1.0, 1.03))})
b.build_cubes("dr")
b.build_models("dr")
print("[dr] unknown", b.unknown)

# ---------------------------------------------------------------- soft late light
SUN_EL, SUN_AZ = float(A.get("el", 20.0)), float(A.get("az", 250.0))
sun_ob, sdir = add_sun(SUN_EL, SUN_AZ, float(A.get("suns", 8.0)), (1.0, 0.78, 0.6), angle=float(A.get("sa", 4.0)))
SKY = [(-1.0, (0.12, 0.1, 0.1)), (-0.02, (0.92, 0.78, 0.74)), (0.02, (1.0, 0.86, 0.8)),
       (0.12, (0.88, 0.80, 0.86)), (0.4, (0.56, 0.62, 0.86)), (1.0, (0.30, 0.42, 0.78))]
world_gradient(SKY, strength=float(A.get("skys", 0.8)), sun_glow=(sdir, (1.0, 0.75, 0.55), 3.0, 0.9))
cloud_layer(9, -500, 500, -500, 500, 90, cell=6, thick=2.5, cover=float(A.get("cover", 0.15)), block="cloud_warm")
fog_gradient_box((0, 0, 10), (700, 700, 80), float(A.get("fog", 0.004)), z0=-40, z1=float(A.get("fogh", 40)),
                 color=(1.0, 0.86, 0.86), anisotropy=0.55)

# drifting petals near the camera (no shadows)
r = np.random.default_rng(3)
pm = material("pink_petals", tint=(1.1, 1.0, 1.05))
mb = vox.MeshBuilder()
cx, cy, cz = P(*cam)
tx, ty, tz = P(*tgt)
fwd = np.array([tx - cx, ty - cy, tz - cz]); fwd /= np.linalg.norm(fwd)
for _ in range(int(A.get("npetal", 220))):
    d = r.uniform(3, 30)
    side = np.cross(fwd, [0, 0, 1]); side /= np.linalg.norm(side)
    p = np.array([cx, cy, cz]) + fwd * d + side * r.uniform(-0.6, 0.6) * d + np.array([0, 0, 1]) * r.uniform(-0.35, 0.3) * d
    s = r.uniform(0.07, 0.12)
    a, bb = r.uniform(0, math.pi), r.uniform(-0.6, 0.6)
    u = np.array([math.cos(a), math.sin(a), bb]); u /= np.linalg.norm(u)
    v = np.cross(u, [0.2, 0.3, 1.0]); v /= np.linalg.norm(v)
    q = [tuple(p + (u * i + v * j) * s) for i, j in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
    mb.quad(q, [(0.0, 0.0), (0.5, 0.0), (0.5, 0.5), (0.0, 0.5)], pm)
po = mb.build("petals_air")
if po:
    po.visible_shadow = False

camera(P(*cam), P(*tgt), lens=float(A.get("lens", 28)), fstop=float(A.get("f", 0)) or None,
       focus=P(*[float(v) for v in A.get("focus", A.get("tgt", "-130,121,-290")).split(",")]),
       shift_x=float(A.get("sx", 0.0)), shift_y=float(A.get("sy", 0.0)))

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(A.get("exp", 0.0)), bloom=0.3,
             bloom_threshold=1.2, bloom_size=0.6, time_limit=float(A.get("tl", 195)), volume_bounces=1,
             res=(int(A.get("w", 1920)), int(A.get("h", 1080))))
render(A.get("out", "mode-draftout"), T0)
