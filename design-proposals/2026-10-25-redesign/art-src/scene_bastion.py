"""bastion.png - bastion remnant (blackstone, gold) above a lava sea, basalt pillars, red haze."""
import sys, math, time
sys.path.insert(0, "/tmp/srl/art/src")
import importlib, vox
importlib.reload(vox)
from vox import *

T0 = time.time()
A = args()
PCT = int(A.get("pct", 100))
SAMPLES = int(A.get("samples", 128))

reset()
MATPROPS["lava"]["es"] = float(A.get("les", 1.0))
MATPROPS["gold_block"] = dict(metal=1.0, rough=0.28, emit=True, es=0.22)
MATPROPS["magma"] = dict(emit_mask="magma_e", es=2.2)
W = World(seed=33)
r = W.rng

# ---------------------------------------------------------------- lava sea
X0, X1, Y0, Y1 = -110, 160, -20, 240
for x in range(X0, X1):
    for y in range(Y0, Y1):
        W.set(x, y, 0, "lava")

# coherent material patches for walls
PATCH = fbm2(300, 300, 77, scale=5, octaves=2)


def wall(x, y, z):
    p = PATCH[(y + 50) % 300, (x + 120) % 300]
    if z <= 2:
        return "basalt"
    if p > 0.62:
        return "blackstone"
    if p < 0.3 and r.random() < 0.18:
        return "gilded"
    return "pbb"


def put(x, y, z, b=None):
    W.set(x, y, z, b or wall(x, y, z))


# ---------------------------------------------------------------- the keep
KX0, KX1, KY0, KY1 = 30, 53, 52, 71


def level(x0, x1, y0, y1, z0, z1, arches=None, windows=None, rim_gold=True):
    for x in range(x0, x1 + 1):
        for y in range(y0, y1 + 1):
            outer = x in (x0, x1) or y in (y0, y1)
            inner = x in (x0 + 1, x1 - 1) or y in (y0 + 1, y1 - 1)
            for z in range(z0, z1 + 1):
                if outer or inner:
                    put(x, y, z)
            put(x, y, z1, "pbb")  # floor slab at the top of the level
    # crenellations + gold trim on the rim
    for x in range(x0, x1 + 1):
        for y in range(y0, y1 + 1):
            if x in (x0, x1) or y in (y0, y1):
                if (x + y) % 2 == 0:
                    put(x, y, z1 + 1, "pbb")
                if rim_gold and (x + y) % 4 == 0 and y == y0:
                    put(x, y, z1, "gold")
    # big arches on the front (y0) face
    for (cx, wdt, base, hgt) in (arches or []):
        rad = wdt / 2
        for x in range(int(cx - rad) - 1, int(cx + rad) + 2):
            for z in range(base, base + hgt + int(rad) + 2):
                dx = (x + 0.5) - cx
                top = base + hgt + math.sqrt(max(0, rad * rad - dx * dx))
                if abs(dx) <= rad and z + 0.5 < top:
                    for y in (y0, y0 + 1):
                        W.set(x, y, z, None)
        for x in range(int(cx - rad) - 1, int(cx + rad) + 2):  # gilded ring with a gold keystone
            dx = (x + 0.5) - cx
            if abs(dx) <= rad + 1:
                ztop = int(base + hgt + math.sqrt(max(0, (rad + 0.6) ** 2 - dx * dx)))
                if W.get(x, y0, ztop) is not None:
                    W.set(x, y0, ztop, "gilded" if abs(dx) > 0.6 else "gold")
    for (wx, wz, ww, wh) in (windows or []):
        for x in range(wx, wx + ww):
            for z in range(wz, wz + wh):
                for y in (y0, y0 + 1):
                    W.set(x, y, z, None)
        for x in range(wx - 1, wx + ww + 1):
            W.set(x, y0, wz + wh, "gilded")


level(KX0, KX1, KY0, KY1, 1, 15, arches=[(35.5, 5, 1, 6), (41.5, 5, 1, 7), (47.5, 5, 1, 6)])
level(KX0 + 3, KX1 - 3, KY0 + 3, KY1 - 3, 16, 27, windows=[(36, 19, 2, 4), (40, 19, 3, 5), (45, 19, 2, 4)])
level(KX0 + 7, KX1 - 7, KY0 + 6, KY1 - 6, 28, 36, windows=[(40, 30, 3, 3)])
# hollow out the keep interior so the arches show glowing lava inside
for x in range(KX0 + 2, KX1 - 1):
    for y in range(KY0 + 2, KY1 - 1):
        for z in range(1, 15):
            W.set(x, y, z, None)
# ruin the top: knock out chunks of the upper levels
for (x, y, z, rad) in [(KX1 - 7, KY0 + 6, 36, 2.8), (KX0 + 3, KY0 + 3, 27, 2.2), (KX1, KY0, 15, 1.8)]:
    R = int(rad) + 1
    for dx in range(-R, R + 1):
        for dy in range(-R, R + 1):
            for dz in range(-R, R + 1):
                if dx * dx + dy * dy + dz * dz < rad * rad + r.normal(0, 0.6):
                    W.set(x + dx, y + dy, z + dz, None)
# gold crown + hoard on the terraces
for x in range(KX0 + 8, KX1 - 7):
    if x % 2 == 0 and W.get(x, KY0 + 6, 36) is not None:
        W.set(x, KY0 + 6, 37, "gold")
for (x, y, z) in [(33, 53, 16), (34, 53, 16), (33, 54, 16), (34, 54, 17), (50, 54, 16), (37, 56, 28), (38, 56, 28)]:
    W.set(x, y, z, "gold")
# lava fall pouring out of the middle window onto the terrace and over the rim
for z in range(16, 19):
    W.set(41, KY0 + 3, z, "lava")
for z in range(1, 17):
    W.set(41, KY0 - 1, z, "lava")
W.set(41, KY0, 16, "lava")
W.set(41, KY0 + 1, 16, "lava")
W.set(41, KY0 + 2, 16, "lava")
LAVAFALL = (41.5, KY0 - 2.5)

# ---------------------------------------------------------------- broken arched bridge toward the camera side
BZ = 15
for x in range(6, KX0):
    seg = (KX0 - 1 - x) % 9  # piers every 9
    for y in range(58, 63):
        put(x, y, BZ, "pbb")
        put(x, y, BZ - 1)
        if seg in (0, 1):
            for z in range(1, BZ - 1):
                put(x, y, z)
        else:
            t = (seg - 1.0) / 8.0
            depth = int(round(1 + 7 * (1 - math.sin(math.pi * t))))
            for z in range(BZ - 1 - depth, BZ - 1):
                put(x, y, z)
    for y in (58, 62):
        if r.random() < 0.85:
            put(x, y, BZ + 1, "pbb")
# crumbling far end
for x in range(6, 13):
    for y in range(57, 64):
        for z in range(0, BZ + 2):
            if W.get(x, y, z) and r.random() < (13 - x) / 7.0 * 0.9:
                W.set(x, y, z, None)
for (x, y) in [(5, 59), (4, 61), (7, 57), (3, 60), (8, 63)]:
    W.set(x, y, 1, "blackstone" if r.random() < 0.6 else "pbb")
W.set(4, 60, 2, "blackstone")
for (x, y) in [(20, 60), (21, 60), (21, 61)]:
    W.set(x, y, BZ + 1, "gold")

# ---------------------------------------------------------------- basalt columns


def column(cx, cy, rad, h, seed):
    rr = np.random.default_rng(seed)
    R = int(rad) + 2
    for dx in range(-R, R + 1):
        for dy in range(-R, R + 1):
            d = math.hypot(dx, dy)
            hh = int(h * (1 - 0.25 * (d / (rad + 0.5)) ** 2) + rr.normal(0, 2))
            if d < rad + rr.normal(0, 0.45):
                for z in range(1, max(2, hh)):
                    W.set(cx + dx, cy + dy, z, "basalt")


for i, (x, y, rad, hgt) in enumerate([(-16, 58, 2.6, 70), (8, 78, 1.6, 34), (-22, 62, 3.2, 90), (-40, 40, 2.0, 40),
                                       (-14, 110, 3.5, 90), (70, 40, 2.2, 28), (80, 88, 3.4, 90), (24, 120, 3.0, 70),
                                       (110, 130, 4.0, 90), (-60, 150, 4.0, 90), (55, 160, 4.5, 90), (-30, 90, 2.4, 55)]):
    column(x, y, rad, hgt, 900 + i)

cl = fbm2(X1 - X0, 1, 9, scale=14, octaves=3)[0]
for x in range(X0, X1):
    top = int(40 + 50 * cl[x - X0])
    for y in range(200, 206):
        for z in range(1, top - (y - 200) * 2):
            W.set(x, y, z, "netherrack")

# ---------------------------------------------------------------- basalt delta shore, left foreground
fires = []
SH = fbm2(60, 50, 12, scale=7, octaves=3)
for x in range(-40, 14):
    for y in range(14, 60):
        # shore occupies the left, its edge runs diagonally away from the camera
        edge = (x + 40) / 54.0 - (y - 14) / 46.0 * 0.55 + 0.35 * (SH[y - 14, x + 40] - 0.5)
        if edge > 0.42:
            continue
        hgt = 1 + int(2.6 * SH[y - 14, x + 40] + (0.42 - edge) * 3)
        if r.random() < 0.06:
            hgt += int(r.integers(1, 4))  # stray basalt columns
        for z in range(0, hgt + 1):
            q = r.random()
            top = z == hgt
            b2 = "basalt" if (q < 0.55 or not top) else ("blackstone" if q < 0.88 else "magma")
            W.set(x, y, z, b2)

W.build("bastion")

# ---------------------------------------------------------------- light, haze
world_gradient([(-1.0, (0.10, 0.025, 0.01)), (0.0, (0.30, 0.07, 0.03)), (0.2, (0.13, 0.03, 0.02)),
                (0.5, (0.04, 0.01, 0.01)), (1.0, (0.015, 0.004, 0.004))], strength=1.0)
fog_box((25, 110, 45), (280, 270, 110), float(A.get("fog", 0.003)), color=(0.62, 0.30, 0.20), anisotropy=0.2)
for f in fires:
    point(f, 30, (1.0, 0.5, 0.15), radius=0.3)
point(LAVAFALL + (8.0,), 120, (1.0, 0.42, 0.1), radius=1.5)
for cx in (35.5, 41.5, 47.5):  # glow inside the arches
    point((cx, KY0 + 4.5, 4.0), 160, (1.0, 0.4, 0.1), radius=1.5)

# lava glow on the bastion's facade
area((40, 34, 2.0), (105, 0, 0), 34, float(A.get("key", 15000)), (1.0, 0.36, 0.08), size_y=6)
cam_loc = (A_f := float(A.get("cx", 2.0)), 6.0, float(A.get("cz", 4.5)))
target = (22.0, 55.0, 16.5)
camera(cam_loc, target, lens=28, fstop=8.0, focus=(36, 52, 14))

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(A.get("exp", 0.0)), bloom=0.4,
             bloom_threshold=float(A.get("bt", 2.0)), bloom_size=0.6, time_limit=195, volume_bounces=1)
bpy.context.scene.render.resolution_percentage = PCT
render(A.get("out", "bastion"), T0)
