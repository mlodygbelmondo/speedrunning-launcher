"""mode-fsg.png - a REAL ruined portal of seed 2026 (Java 1.16.1), block for block.

Ruined portal (portal_5 template) at (-1072, 1616), BB [-1071,63,1616]..[-1062,72,1622]: broken obsidian frame
with one standing pillar, crying obsidian, three gold blocks, its loot chest (-1066,66,1620), netherrack and
magma spread, and a lava pool just west of it (x -1086..-1077, z 1620..1627) - in an oak/birch meadow by a lake.
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

LEAVES = ("oak_leaves", "birch_leaves")
if A.get("lalpha", "0") != "1":
    vox.PACKED.update(LEAVES)  # solid canopies read cleaner at tile size
pal, arr, origin = mc.load("fsg")
x0, y0, z0 = origin
nx, ny, nz = arr.shape
OX, OY, OZ = -1068, 64, 1620


def P(x, y, z):
    return (x - OX, OZ - z, y - OY)


b = mc.Builder(pal, arr, origin, OX, OY, OZ, seed=6,
               mat_over={"lava": dict(emit=True, es=float(A.get("les", 2.5)), rough=0.6, base_mul=0.15, spec=0.2),
                         "magma": dict(emit_mask="magma_e", es=float(A.get("mes", 2.0))),
                         "crying_obsidian": dict(emit_mask="crying_obsidian_e", es=float(A.get("ces", 3.0))),
                         "gold_block": dict(metal=1.0, rough=0.3),
                         **{l: dict(alpha=A.get("lalpha", "0") == "1") for l in LEAVES}})
b.build_cubes("fsg")
b.build_models("fsg")
b.add_lights()
print("[fsg] unknown", b.unknown)

# lava pool glow and the crying obsidian's purple
area(P(-1081, 62.2, 1623.5), (180, 0, 0), 8, float(A.get("lg", 300)), (1.0, 0.45, 0.12), size_y=6)
point(P(-1068.5, 67.2, 1621.5), float(A.get("cg", 25)), (0.7, 0.3, 1.0), radius=0.5)

# ---------------------------------------------------------------- afternoon light
SUN_EL, SUN_AZ = float(A.get("el", 24.0)), float(A.get("az", 210.0))
sun_ob, sdir = add_sun(SUN_EL, SUN_AZ, float(A.get("suns", 5.5)), (1.0, 0.86, 0.68), angle=2.0)
SKY = [(-1.0, (0.12, 0.12, 0.12)), (-0.02, (0.70, 0.74, 0.78)), (0.02, (0.80, 0.84, 0.88)),
       (0.12, (0.62, 0.74, 0.92)), (0.4, (0.34, 0.52, 0.88)), (1.0, (0.16, 0.32, 0.74))]
world_gradient(SKY, strength=float(A.get("skys", 1.0)), sun_glow=(sdir, (1.0, 0.85, 0.6), 3.0, 0.6))
cloud_layer(3, -400, 400, -400, 400, 70, cell=6, thick=2.5, cover=float(A.get("cover", 0.22)), block="cloud")
fog_gradient_box((0, 0, 10), (600, 600, 50), float(A.get("fog", 0.006)), z0=-6, z1=float(A.get("fogh", 30)),
                 color=(0.86, 0.9, 0.96), anisotropy=0.4)

cam = [float(v) for v in A.get("cam", "-1072,80,1628").split(",")]
tgt = [float(v) for v in A.get("tgt", "-1067,66,1620").split(",")]
camera(P(*cam), P(*tgt), lens=float(A.get("lens", 28)), fstop=float(A.get("f", 0)) or None,
       focus=P(-1066.5, 66.5, 1620.5), shift_x=float(A.get("sx", 0.0)), shift_y=float(A.get("sy", 0.0)))

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(A.get("exp", 0.0)), bloom=0.3,
             bloom_threshold=1.3, bloom_size=0.5, time_limit=float(A.get("tl", 195)), volume_bounces=1,
             res=(int(A.get("w", 1920)), int(A.get("h", 1080))))
render(A.get("out", "mode-fsg"), T0)
