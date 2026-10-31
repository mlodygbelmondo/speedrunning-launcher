"""mode-ranked.png - six tier blocks as rising pillars in a dark Nether arena (synthetic, our textures).

Coal < Iron < Gold < Emerald < Diamond < Netherite (tallest), each a 3x3 pillar of its block, on a polished
blackstone floor ringed by lava, under spotlights in haze.
"""
import math
import sys
import time
sys.path.insert(0, "/tmp/srl/art/src")
import bpy
import numpy as np
import vox
from vox import *
import mcblocks as mc

T0 = time.time()
A = args()
SAMPLES = int(A.get("samples", 128))
reset()

TIERS = [("coal_block", dict(rough=0.55, spec=0.3)), ("iron_block", dict(metal=1.0, rough=0.35)),
         ("gold_block", dict(metal=1.0, rough=0.28)), ("emerald_block", dict(rough=0.22, spec=0.8)),
         ("diamond_block", dict(rough=0.18, spec=0.9)), ("netherite_block", dict(metal=0.8, rough=0.38))]
for t, m in TIERS:
    vox.BLOCKS[t] = (t, t, t, {"mat": m})
vox.BLOCKS["pbb"] = ("polished_blackstone_bricks",) * 3 + ({},)
vox.BLOCKS["cpbb"] = ("cracked_polished_blackstone_bricks",) * 3 + ({},)
vox.BLOCKS["gilded"] = ("gilded_blackstone",) * 3 + ({},)

W = World(11)
r = np.random.default_rng(5)
R_FLOOR, R_LAVA = 19, 22
for x in range(-46, 47):
    for y in range(-30, 47):
        d = math.hypot(x, y - 6)
        if d <= R_FLOOR:
            W.set(x, y, 0, "cpbb" if r.random() < 0.12 else "pbb")
            W.set(x, y, -1, "blackstone")
        elif d <= R_FLOOR + 1.2:
            W.set(x, y, 0, "gilded" if r.random() < 0.25 else "blackstone")
            W.set(x, y, 1, "blackstone" if r.random() < 0.7 else None)
        elif d <= R_LAVA:
            W.set(x, y, -1, "lava")
            W.set(x, y, -2, "netherrack")
        else:
            h = int(1 + 2.5 * r.random() + max(0, d - 30) * 0.25)
            for z in range(-2, h):
                W.set(x, y, z, "netherrack" if r.random() > 0.08 else "magma")

# the six pillars on a gentle arc, rising left to right
XS = [-13, -8, -3, 2, 7, 12]
HS = [3, 5, 7, 9, 11, 14]
tops = []
for (t, _), xc, h in zip(TIERS, XS, HS):
    yc = 6 + 0.04 * (xc + 0.5) ** 2
    xi, yi = int(xc), int(round(yc))
    for x in range(xi - 1, xi + 2):
        for y in range(yi - 1, yi + 2):
            for z in range(1, h + 1):
                W.set(x, y, z, t)
    tops.append((xi + 0.5, yi + 0.5, h + 1))

# basalt columns far behind, lost in the haze
for k in range(16):
    a = math.radians(20 + 140 * k / 15 + r.uniform(-4, 4))
    d = r.uniform(34, 44)
    x, y = int(d * math.cos(a)), int(6 + d * math.sin(a))
    hh = int(r.uniform(8, 26))
    for z in range(0, hh):
        W.set(x, y, z, "basalt")
        if r.random() < 0.6:
            W.set(x + 1, y, z, "basalt")
W.build("arena")

# ---------------------------------------------------------------- light
world_gradient([(-1.0, (0.05, 0.012, 0.008)), (0.0, (0.16, 0.035, 0.02)), (0.2, (0.06, 0.014, 0.012)),
                (1.0, (0.01, 0.003, 0.004))], strength=float(A.get("ws", 1.5)))
mc.world_camera_color((0.03, 0.006, 0.006), 1.0)
fog_box((0, 8, 14), (110, 90, 30), float(A.get("fog", 0.010)), color=(0.95, 0.88, 0.84), anisotropy=0.45)
# lava ring glow
for k in range(10):
    a = 2 * math.pi * k / 10
    point((math.cos(a) * 20.6, 6 + math.sin(a) * 20.6, 0.3), float(A.get("lr", 70)), (1.0, 0.4, 0.1), radius=2.0)


def spot(loc, target, energy, color, size_deg=16, blend=0.35, radius=0.4, name="Spot"):
    ld = bpy.data.lights.new(name, "SPOT")
    ld.energy = energy
    ld.color = color
    ld.spot_size = math.radians(size_deg)
    ld.spot_blend = blend
    ld.shadow_soft_size = radius
    ob = bpy.data.objects.new(name, ld)
    ob.location = loc
    d = vox.Vector(target) - vox.Vector(loc)
    ob.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()
    bpy.context.scene.collection.objects.link(ob)
    return ob


SE = float(A.get("spot", 22000))
COLS = [(1.0, 0.85, 0.7), (0.9, 0.94, 1.0), (1.0, 0.86, 0.55), (0.75, 1.0, 0.82), (0.7, 0.96, 1.0), (1.0, 0.82, 0.95)]
for i, ((x, y, z), c) in enumerate(zip(tops, COLS)):
    gain = 1.0 + 0.5 * (i == 5)
    spot((x - 2, y - 6, z + 26), (x, y, z - 1.0), SE * gain, c, size_deg=9 + 2 * (i == 5), blend=0.25, name=f"Spot{i}")
    # key from the front so each tier's colour reads
    kg = {0: 2.2, 5: 3.0}.get(i, 1.0)
    spot((x + 1, y - 17, z + 8), (x, y, z * 0.55), float(A.get("key", 5000)) * kg, c, size_deg=13, blend=0.4,
         name=f"Key{i}")
# the champion: a cool rim behind netherite
xn, yn, zn = tops[5]
spot((xn + 7, yn + 9, zn + 6), (xn, yn, zn * 0.6), float(A.get("nrim", 9000)), (0.78, 0.62, 1.0), size_deg=20, name="NRim")
# soft front fill so the tier colours read
area((0, -18, 12), (62, 0, 0), 20, float(A.get("fill", 1200)), (1.0, 0.8, 0.7), size_y=6)
# rim from behind
area((0, 34, 20), (-60, 0, 0), 30, float(A.get("rim", 1500)), (1.0, 0.45, 0.2), size_y=10)

# embers
em = emission_material("ember_m", (1.0, 0.45, 0.12), float(A.get("ember", 9)))
mb = vox.MeshBuilder()
for _ in range(int(A.get("nember", 90))):
    p = (r.uniform(-26, 26), r.uniform(-8, 30), r.uniform(1, 22))
    s = r.uniform(0.02, 0.05)
    mb.quad([(p[0] - s, p[1], p[2] - s), (p[0] + s, p[1], p[2] - s), (p[0] + s, p[1], p[2] + s), (p[0] - s, p[1], p[2] + s)],
            [(0, 0), (1, 0), (1, 1), (0, 1)], em)
eo = mb.build("embers")
eo.visible_shadow = False

cam = [float(v) for v in A.get("cam", "-0.5,-18,4.5").split(",")]
tgt = [float(v) for v in A.get("tgt", "-0.5,6,7.2").split(",")]
camera(cam, tgt, lens=float(A.get("lens", 26)), shift_x=float(A.get("sx", 0.0)), shift_y=float(A.get("sy", 0.0)))

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(A.get("exp", 0.0)), bloom=0.45,
             bloom_threshold=1.2, bloom_size=0.6, time_limit=float(A.get("tl", 195)), volume_bounces=1,
             res=(int(A.get("w", 1920)), int(A.get("h", 1080))))
render(A.get("out", "mode-ranked"), T0)
