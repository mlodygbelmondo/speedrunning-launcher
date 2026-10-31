"""fortress.png - a REAL nether fortress of seed 2026 (Java 1.16.1), block for block.

Fortress start (-1584, 2336), crimson forest, BB [-1673,48,2317]..[-1469,86,2401]. Bridge crossings
(nebcr) at y 69-78 forty blocks above the lava sea, blaze-spawner platforms (nemt) at
[-1576,72,2376]..[-1570,79,2384] and [-1595,72,2376]..[-1589,79,2384]. Picked from 46 fortresses
of the seed as one of the most open. Nether roof above y=CUT removed (hidden in haze).
"""
import sys
import time
sys.path.insert(0, "/tmp/srl/art/src")
import vox
from vox import *
import mcblocks as mc

T0 = time.time()
A = args()
SAMPLES = int(A.get("samples", 128))
reset()

pal, arr, origin = mc.load("fortress2")
OX, OY, OZ = -1580, 31, 2370


def P(x, y, z):
    return (x - OX, OZ - z, y - OY)


CUT = int(A.get("cut", 104))
b = mc.Builder(pal, arr, origin, OX, OY, OZ, seed=11, light_scale=float(A.get("ls", 1.0)),
               mat_over={"lava": dict(emit=True, es=float(A.get("les", 0.8)), rough=0.6, base_mul=0.15, spec=0.2),
                         "magma": dict(emit_mask="magma_e", es=float(A.get("mes", 3.0))),
                         "shroomlight": dict(emit=True, es=float(A.get("ses", 3.0))),
                         "glowstone": dict(emit=True, es=float(A.get("ges", 1.6))),
                         "fire": dict(es=float(A.get("fes", 0.9)))})
b.build_cubes("fo", top_cut=CUT - origin[1])
mc.SPAWNER_FX[0] = float(A.get("sfx", 6.0))
b.build_models("fo", top_cut=CUT - origin[1])
for sx_, sy_, sz_ in ((-1592, 77, 2381), (-1573, 77, 2381)):  # the two blaze spawners: a warm glow in the cage
    vox.point(P(sx_ + 0.5, sy_ + 0.5, sz_ + 0.5), float(A.get("spl", 40)), (1.0, 0.55, 0.2), radius=0.3, name="L_spawner")
n = b.add_lights()
print("[fo] lights", n, "unknown", b.unknown)

EC = [float(t) for t in A.get("embers", "-1590,80,2385,25,15,25").split(",")]
mc.embers(P, EC[:3], EC[3:], n=int(A.get("nember", 120)), strength=float(A.get("ember", 10)), seed=5, smin=0.02, smax=0.05)

world_gradient([(-1.0, (0.10, 0.025, 0.01)), (0.0, (0.24, 0.055, 0.03)), (0.15, (0.11, 0.028, 0.02)),
                (0.5, (0.03, 0.008, 0.007)), (1.0, (0.01, 0.003, 0.003))], strength=float(A.get("ws", 12.0)))
mc.world_camera_color((0.09, 0.018, 0.012), float(A.get("cs", 1.0)))
fog_gradient_box((0, 0, 40), (360, 300, 100), float(A.get("fog", 0.003)), z0=-2, z1=70, color=(0.78, 0.36, 0.24),
                 anisotropy=0.25)
key = A.get("keyp", "-1580,33,2380")
kx, ky, kz = [float(v) for v in key.split(",")]
area(P(kx, ky, kz), (180, 0, 0), 80, float(A.get("key", 60000)), (1.0, 0.38, 0.1), size_y=60)
# soft warm fill from above the camera side so the nether bricks read
fp = [float(t) for t in A.get("fillp", "-1560,95,2395").split(",")]
ft = [float(t) for t in A.get("fillt", "-1585,74,2370").split(",")]
if float(A.get("fill", 25000)) > 0:
    mc.aim_area(P(*fp), P(*ft), float(A.get("fills", 12)), float(A.get("fill", 25000)), (1.0, 0.62, 0.42))

cam = [float(v) for v in A.get("cam", "-1600,88,2398").split(",")]
tgt = [float(v) for v in A.get("tgt", "-1583,74,2372").split(",")]
camera(P(*cam), P(*tgt), lens=float(A.get("lens", 20)), shift_x=float(A.get("sx", -0.14)), shift_y=float(A.get("sy", 0.0)))

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(A.get("exp", 0.0)), bloom=0.45,
             bloom_threshold=float(A.get("bt", 1.5)), bloom_size=0.6, time_limit=float(A.get("tl", 195)),
             volume_bounces=1, res=(int(A.get("w", 1920)), int(A.get("h", 1080))))
render(A.get("out", "fortress"), T0)
