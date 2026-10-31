"""bastion.png - a REAL Bridge bastion remnant of seed 2026 (Java 1.16.1), block for block.

Bastion start (-2544, -1536), rotated 180: bridge/bridge [-2543,53,-1558]..[-2513,71,-1543] runs east
from the gate (entrance_face [-2543,72,-1567]..[-2539,95,-1536]); legs at x -2539/-2521 stand in the
lava sea. Picked from 20 bridge bastions of the seed as the one in the most open lava cavern (every Bridge bastion uses the
same 31-block bridge piece). Round 3: seen from the north-east, a little above the deck, so the whole span with
both legs in the lava lake leads into the gate tower.
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

pal, arr, origin = mc.load("bastion2")
OX, OY, OZ = -2528, 31, -1550


def P(x, y, z):
    return (x - OX, OZ - z, y - OY)


CUT = int(A.get("cut", 104))
b = mc.Builder(pal, arr, origin, OX, OY, OZ, seed=7, light_scale=float(A.get("ls", 1.0)),
               mat_over={"lava": dict(emit=True, es=float(A.get("les", 0.25)), rough=0.6, base_mul=0.15, spec=0.2),
                         "gold_block": dict(metal=1.0, rough=0.28, emit=True, es=float(A.get("gold", 0.6))),
                         "magma": dict(emit_mask="magma_e", es=float(A.get("mes", 3.0))),
                         "shroomlight": dict(emit=True, es=float(A.get("ses", 3.0))),
                         "glowstone": dict(emit=True, es=float(A.get("ges", 1.6))),
                         "fire": dict(es=float(A.get("fes", 0.7)))})
b.build_cubes("ba", top_cut=CUT - origin[1])
b.build_models("ba", top_cut=CUT - origin[1])
n = b.add_lights()
print("[ba] lights", n, "unknown", b.unknown)

mc.embers(P, (-2515, 66, -1552), (30, 18, 22), n=int(A.get("nember", 150)), strength=float(A.get("ember", 14)), seed=9,
          smin=0.02, smax=0.05)

# ---------------------------------------------------------------- light, haze
world_gradient([(-1.0, (0.10, 0.025, 0.01)), (0.0, (0.26, 0.06, 0.03)), (0.15, (0.12, 0.03, 0.02)),
                (0.5, (0.035, 0.009, 0.008)), (1.0, (0.012, 0.004, 0.004))], strength=float(A.get("ws", 12.0)))
mc.world_camera_color((0.09, 0.018, 0.012), float(A.get("cs", 1.0)))
fog_gradient_box((0, 0, 40), (380, 360, 100), float(A.get("fog", 0.003)), z0=-2, z1=70, color=(0.75, 0.34, 0.24),
                 anisotropy=0.25)
# warm bounce from the lava sea onto the bridge underside and gate
area(P(-2515, 33, -1555), (180, 0, 0), 80, float(A.get("key", 90000)), (1.0, 0.38, 0.1), size_y=60)

# two warm, fairly narrow fills from the north-east: one on the bridge, one on the gate tower, so the bastion
# (not the lava) is the brightest thing in the frame
import math
FILLS = [("Fill", "fillp", "-2515,92,-1592", "fillt", "-2528,62,-1556", "fills", 18, "fill", 16000, "fc", "1.0,0.47,0.25"),
         ("Fill2", "fill2p", "-2505,96,-1585", "fill2t", "-2546,80,-1560", "fill2s", 8, "fill2", 14000, "f2c", "1.0,0.45,0.27")]
for nm, kp, dp, kt, dt, ks, ds, ke, de, kc, dc in FILLS:
    fo = mc.aim_area(P(*[float(t) for t in A.get(kp, dp).split(",")]), P(*[float(t) for t in A.get(kt, dt).split(",")]),
                     float(A.get(ks, ds)), float(A.get(ke, de)), tuple(float(t) for t in A.get(kc, dc).split(",")), name=nm)
    fo.data.spread = math.radians(float(A.get("fspread", 60)))

# camera north-east of the bridge, a little above the deck: the arched bridge runs from the left into the gate
cam = [float(v) for v in A.get("cam", "-2512,78,-1606").split(",")]
tgt = [float(v) for v in A.get("tgt", "-2530,62,-1552").split(",")]
camera(P(*cam), P(*tgt), lens=float(A.get("lens", 24)), fstop=float(A.get("f", 0)) or None,
       focus=P(-2530, 66, -1550), shift_x=float(A.get("sx", -0.05)), shift_y=float(A.get("sy", 0.0)))

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(A.get("exp", 0.0)), bloom=0.45,
             bloom_threshold=float(A.get("bt", 1.5)), bloom_size=0.6, time_limit=float(A.get("tl", 195)),
             volume_bounces=1, res=(int(A.get("w", 1920)), int(A.get("h", 1080))))
render(A.get("out", "bastion"), T0)
