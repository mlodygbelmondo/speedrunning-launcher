"""stronghold.png - the REAL end-portal room of seed 2026 (Java 1.16.1), block for block.

Stronghold start (-216, 1400); portal room piece SHPR at [-228,21,1375]..[-213,28,1385].
Layout extracted from a locally generated world (npz kept in /tmp only); our own textures.
The real room has no torches: the warm light is its lava, the cold light the portal.
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

pal, arr, origin = mc.load("stronghold")
OX, OY, OZ = -228, 21, 1386


def P(x, y, z):  # MC point -> Blender
    return (x - OX, OZ - z, y - OY)


# fill the ring: a runner's lit portal (real seed has 2 eyes; eyes=real keeps them)
if A.get("eyes", "all") == "all":
    pal = list(pal)
    for i, p in enumerate(pal):
        if "end_portal_frame" in p and "eye=false" in p:
            pal[i] = p.replace("eye=false", "eye=true")
    ep = len(pal)
    pal.append("minecraft:end_portal")
    x0, y0, z0 = origin
    for x in range(-219, -216):
        for z in range(1379, 1382):
            arr[x - x0, 24 - y0, z - z0] = ep

vox.STAR_GAIN[0] = float(A.get("sg", 3.2))
vox.STAR_BASE[0] = float(A.get("sb", 2.0))
b = mc.Builder(pal, arr, origin, OX, OY, OZ, seed=3,
               mat_over={"lava": dict(emit=True, es=float(A.get("les", 1.6)), rough=0.6, base_mul=0.12, spec=0.2),
                         "eye_of_ender": dict(emit=True, es=float(A.get("ees", 2.5)), rough=0.25, spec=0.7),
                         "eye_of_ender_side": dict(emit=True, es=float(A.get("ees", 2.5)) * 0.35, rough=0.3, spec=0.6)})
b.build_cubes("sh")
b.build_models("sh")
print("[sh] unknown", b.unknown)
b.add_lights()

# ---------------------------------------------------------------- light
world_gradient([(-1, (0, 0, 0)), (1, (0, 0, 0))], strength=0.0)
# lava pool under the portal and the two corner pools: soft warm area lights just above the surface
area((P(-218, 22.9, 1380)[0] + 0.5, P(-218, 22.9, 1380)[1] - 0.5, P(-218, 22.9, 1380)[2]), (180, 0, 0), 2.8,
     float(A.get("lg", 60)), (1.0, 0.42, 0.12))
for zc in (1376, 1384):
    c = P(-226, 22.95, zc)
    area((c[0] + 0.5, c[1] - 0.5, c[2]), (180, 0, 0), 2.6, float(A.get("lc", 30)), (1.0, 0.42, 0.12), size_y=1.0)
# the portal surface glows teal upward
c = P(-218, 24.8, 1380)
area((c[0] + 0.5, c[1] - 0.5, c[2]), (180, 0, 0), 2.9, float(A.get("pg", 90)), (0.3, 0.95, 0.85))
# faint cold bounce so the bricks read in the dark corners
c = P(-221, 26.5, 1380)
area((c[0], c[1], c[2]), (0, 0, 0), 6.0, float(A.get("fill", 14)), (0.55, 0.65, 0.9), size_y=8.0)
fog_box((8, 5.5, 4), (16, 11, 8), float(A.get("fog", 0.03)), color=(0.85, 0.85, 0.9), anisotropy=0.35)

# ---------------------------------------------------------------- camera
cam = [float(v) for v in A.get("cam", "-227.2,26.2,1384.3").split(",")]
tgt = [float(v) for v in A.get("tgt", "-218.5,23.6,1380.0").split(",")]
camera(P(*cam), P(*tgt), lens=float(A.get("lens", 17)), fstop=float(A.get("f", 8)),
       focus=P(-218.5, 24.5, 1380.5), shift_x=float(A.get("sx", -0.12)), shift_y=float(A.get("sy", 0.0)))

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(A.get("exp", 0.0)), bloom=0.5,
             bloom_threshold=1.0, bloom_size=0.55, time_limit=float(A.get("tl", 195)), volume_bounces=1,
             res=(int(A.get("w", 1920)), int(A.get("h", 1080))))
render(A.get("out", "stronghold"), T0)
