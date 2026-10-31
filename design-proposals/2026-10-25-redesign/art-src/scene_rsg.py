"""mode-rsg.png - the REAL buried treasure of seed 2026 (Java 1.16.1), block for block.

Buried treasure chest at (-2343, 60, -2903), three blocks under the sand on the south beach of a small ocean
island (island x -2382..-2318, z -2946..-2896). The sand above the chest is dug away as a runner would
(stepped pit through the sand and a little dirt). Golden hour. Ocean beyond the extracted box is a plain water plane.
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

pal, arr, origin = mc.load("rsg")
pal = list(pal)
x0, y0, z0 = origin
nx, ny, nz = arr.shape
OX, OY, OZ = -2343, 62, -2903
CX, CY, CZ = -2343, 60, -2903  # the chest


def P(x, y, z):
    return (x - OX, OZ - z, y - OY)


# ---------------------------------------------------------------- dig the treasure out (sand, dirt, grass)
AIR = pal.index("minecraft:air")
sand = {i for i, p in enumerate(pal) if mc.parse(p)[0] in ("sand", "grass", "tall_grass", "dirt", "grass_block")}
for y, r in ((60, 1), (61, 2), (62, 3), (63, 3), (64, 2)):
    for x in range(CX - r, CX + r + 1):
        for z in range(CZ - r, CZ + r + 1):
            if r >= 2 and abs(x - CX) + abs(z - CZ) > r + r // 2:
                continue  # round the crater
            c = (x - x0, y - y0, z - z0)
            if arr[c] in sand:
                arr[c] = AIR

# sand is a falling block: whatever sand was left hanging over the dug cells comes down (we drop it away)
SANDS = {i for i, p in enumerate(pal) if mc.parse(p)[0] in ("sand", "gravel")}
for x in range(CX - 4, CX + 5):
    for z in range(CZ - 4, CZ + 5):
        for y in range(60, y0 + ny - 1):
            c = (x - x0, y - y0, z - z0)
            if arr[c] == AIR:
                yy = y + 1
                while yy < y0 + ny and arr[x - x0, yy - y0, z - z0] in SANDS:
                    arr[x - x0, yy - y0, z - z0] = AIR
                    yy += 1
b = mc.Builder(pal, arr, origin, OX, OY, OZ, seed=4,
               mat_over={"water": dict(rough=0.05, ior=1.33, alpha_const=float(A.get("wa", 0.8)), spec=0.6)})
b.build_cubes("rsg")
b.build_models("rsg")
print("[rsg] unknown", b.unknown)

# ---------------------------------------------------------------- open ocean around the box
SEA = 62 + 14 / 16
wm = b.mat("water")
fm = material("sand", tint=(0.55, 0.6, 0.62))
mb = vox.MeshBuilder()
bx0, bx1 = x0 - OX, x0 + nx - OX
by0, by1 = OZ - (z0 + nz), OZ - z0
R = 3000
for (ax0, ay0, ax1, ay1) in ((-R, -R, R, by0), (-R, by1, R, R), (-R, by0, bx0, by1), (bx1, by0, R, by1)):
    for zz, m in ((SEA - OY, wm), (44 - OY, fm)):
        mb.quad([(ax0, ay0, zz), (ax1, ay0, zz), (ax1, ay1, zz), (ax0, ay1, zz)],
                [(ax0, ay0), (ax1, ay0), (ax1, ay1), (ax0, ay1)], m)
mb.build("ocean")

# ---------------------------------------------------------------- golden hour
SUN_EL, SUN_AZ = float(A.get("el", 7.0)), float(A.get("az", 250.0))
sun_ob, sdir = add_sun(SUN_EL, SUN_AZ, float(A.get("suns", 12.0)), (1.0, 0.47, 0.19), angle=2.0)
SKY = [(-1.0, (0.10, 0.08, 0.07)), (-0.02, (0.95, 0.50, 0.22)), (0.015, (1.0, 0.64, 0.30)),
       (0.08, (1.0, 0.62, 0.40)), (0.2, (0.86, 0.56, 0.58)), (0.42, (0.46, 0.44, 0.74)), (1.0, (0.20, 0.24, 0.56))]
world_gradient(SKY, strength=float(A.get("skys", 0.5)), sun_glow=(sdir, (1.0, 0.55, 0.22), 4.0, 1.2))
cloud_layer(7, -500, 500, -500, 500, 70, cell=6, thick=2.5, cover=float(A.get("cover", 0.16)), block="cloud_warm")
fog_gradient_box((0, 0, 10), (900, 900, 40), float(A.get("fog", 0.004)), z0=-2, z1=float(A.get("fogh", 18)),
                 color=(1.0, 0.82, 0.62), anisotropy=0.5)
# a faint warm bounce into the pit so the chest reads
c = P(CX + 0.5, CY + 2.6, CZ + 0.5)
area(c, (0, 0, 0), 1.6, float(A.get("pit", 12.0)), (1.0, 0.8, 0.6))

cam = [float(v) for v in A.get("cam", "-2340,74,-2891").split(",")]
tgt = [float(v) for v in A.get("tgt", "-2343.5,61,-2903.5").split(",")]
camera(P(*cam), P(*tgt), lens=float(A.get("lens", 38)), fstop=float(A.get("f", 0)) or None,
       focus=P(CX + 0.5, CY + 0.6, CZ + 0.5), shift_x=float(A.get("sx", 0.0)), shift_y=float(A.get("sy", 0.0)))

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(A.get("exp", 0.0)), bloom=0.35,
             bloom_threshold=1.2, bloom_size=0.55, time_limit=float(A.get("tl", 195)), volume_bounces=1,
             res=(int(A.get("w", 1920)), int(A.get("h", 1080))))
render(A.get("out", "mode-rsg"), T0)
