"""fortress.png - nether fortress bridges and pillars over lava, soul sand, fire, embers."""
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
MATPROPS["glowstone"] = dict(emit=True, es=2.2)
W = World(seed=44)
r = W.rng
Z = 26  # deck height

# lava sea far below
for x in range(-110, 150):
    for y in range(-30, 230):
        W.set(x, y, 0, "lava")


def bridge(axis, c0, a0, a1, width=5, pillar_every=11, pillar_off=0):
    """Bridge along `axis` ('x' or 'y'); c0 = lowest coordinate across; a0..a1 along."""
    def P(a, c, z):
        return (a, c, z) if axis == "x" else (c, a, z)
    for a in range(a0, a1 + 1):
        ph = (a - a0 + pillar_off) % pillar_every
        for c in range(c0, c0 + width):
            W.set(*P(a, c, Z), "nether_bricks")
            W.set(*P(a, c, Z - 1), "nether_bricks")
        # pillars (3 long) down to the lava, arches in between
        if ph < 3:
            for c in range(c0, c0 + width):
                for z in range(1, Z - 1):
                    W.set(*P(a, c, z), "nether_bricks")
        else:
            gap = pillar_every - 3
            i = ph - 3
            depth = int(round(1 + 4 * (abs(i - (gap - 1) / 2) / ((gap - 1) / 2)) ** 2.2))
            for c in range(c0, c0 + width):
                for z in range(Z - 1 - depth, Z - 1):
                    W.set(*P(a, c, z), "nether_bricks")
        # fence rails on both sides
        for c in (c0, c0 + width - 1):
            fence_at(P(a, c, Z + 1), axis)


FENCE = []


def fence_at(p, axis):
    FENCE.append((p, axis))


def build_fences():
    nb = material("nether_bricks")
    have = set(f[0] for f in FENCE)
    for (p, axis) in FENCE:
        x, y, z = p
        if W.get(x, y, z) is not None:
            continue
        W.box((x + 6 / 16, y + 6 / 16, z), (x + 10 / 16, y + 10 / 16, z + 1), nb, nb, nb, skip=("nz",))
        # bars to the next post along the axis
        nxt = (x + 1, y, z) if axis == "x" else (x, y + 1, z)
        if nxt in have and W.get(*nxt) is None:
            for (b0, b1) in ((6 / 16, 9 / 16), (12 / 16, 15 / 16)):
                if axis == "x":
                    W.box((x + 10 / 16, y + 7 / 16, z + b0), (x + 1 + 6 / 16, y + 9 / 16, z + b1), nb, nb, nb)
                else:
                    W.box((x + 7 / 16, y + 10 / 16, z + b0), (x + 9 / 16, y + 1 + 6 / 16, z + b1), nb, nb, nb)


# main bridge running away from camera, cross bridge in the distance
bridge("y", 10, -4, 150, pillar_off=6)
bridge("x", 70, -90, 120, pillar_off=2)
bridge("y", -40, 75, 200, pillar_off=4)
# soul-sand garden balcony on the west side of the near bridge
for x in range(4, 10):
    for y in range(16, 25):
        W.set(x, y, Z, "nether_bricks")
        W.set(x, y, Z - 1, "nether_bricks")
for x in range(5, 8):
    for y in range(17, 24):
        W.set(x, y, Z, "soul_sand")
        if r.random() < 0.8:
            W.cross(x, y, Z + 1, "nether_wart", size=0.9, height=0.9)
for x in range(5, 9):  # support pier with a stepped corbel
    for y in range(19, 22):
        for z in range(1, Z - 1):
            W.set(x, y, z, "nether_bricks")
for y in range(16, 25):
    for z in (Z - 2, Z - 3):
        W.set(4 + (Z - 2 - z), y, z, "nether_bricks") if 18 <= y <= 22 else None
for y in range(16, 25):
    fence_at((4, y, Z + 1), "y")
for x in range(4, 10):
    fence_at((x, 16, Z + 1), "x")
    fence_at((x, 24, Z + 1), "x")
for (x, y) in ((8, 17), (8, 23)):
    W.set(x, y, Z + 1, "netherrack")
# remove fences where bridges meet
for (p, axis) in list(FENCE):
    x, y, z = p
    if (62 <= y <= 82 and 2 <= x <= 22) or (x == 10 and 17 <= y <= 23):
        FENCE.remove((p, axis))

# junction keep at the crossing: two tiers, glowing slit windows, fire on the roof
GLOW = []


def keep_tier(x0, x1, y0, y1, z0, z1, win_rows):
    for x in range(x0, x1 + 1):
        for y in range(y0, y1 + 1):
            edge = x in (x0, x1) or y in (y0, y1)
            for z in range(z0, z1 + 1):
                if edge:
                    W.set(x, y, z, "nether_bricks")
            W.set(x, y, z1, "nether_bricks")
            if edge and (x + y) % 2 == 0:
                W.set(x, y, z1 + 1, "nether_bricks")
    for wz in win_rows:
        for x in range(x0 + 2, x1 - 1, 3):
            for z in (wz, wz + 1):
                W.set(x, y0, z, None)
                W.set(x, y0 + 1, z, "glowstone")
            GLOW.append((x + 0.5, y0 - 0.6, wz + 1.0))
        for y in range(y0 + 2, y1 - 1, 3):
            for z in (wz, wz + 1):
                W.set(x0, y, z, None)
                W.set(x0 + 1, y, z, "glowstone")
            GLOW.append((x0 - 0.6, y + 0.5, wz + 1.0))


keep_tier(4, 20, 64, 80, Z + 1, Z + 10, [Z + 6])
keep_tier(8, 16, 68, 76, Z + 11, Z + 20, [Z + 15])
# gate openings where the bridges enter
for x in range(10, 15):
    for z in range(Z + 1, Z + 5):
        W.set(x, 64, z, None)
for y in range(70, 75):
    for z in range(Z + 1, Z + 5):
        W.set(4, y, z, None)
# netherrack + fire on the roofs
ROOF_FIRES = [(6, 66, Z + 11), (18, 78, Z + 11), (6, 78, Z + 11), (11, 70, Z + 21), (14, 74, Z + 21)]
for (x, y, z) in ROOF_FIRES:
    W.set(x, y, z, "netherrack")

# soul sand + nether wart bed on the near bridge deck
for x in (11, 12, 13):
    for y in range(26, 33):
        W.set(x, y, Z, "soul_sand")
        if r.random() < 0.85:
            W.cross(x, y, Z + 1, "nether_wart", size=0.9, height=0.9)
for x in (10, 14):
    for y in range(7, 13):
        pass
# burning netherrack heaps on the deck towards the tower
for (x, y) in [(12, 47), (13, 55), (11, 60)]:
    W.set(x, y, Z + 1, "netherrack")
W.set(13, 56, Z + 1, "netherrack")
W.set(12, 48, Z + 1, "soul_sand")

# netherrack outcrops with fire (left foreground and far)
def rock(cx, cy, cz, rad, seed):
    """Netherrack spire rising out of the lava, with a lumpy top at height cz."""
    rr = np.random.default_rng(seed)
    R = int(rad) + 3
    for dx in range(-R, R + 1):
        for dy in range(-R, R + 1):
            d = math.hypot(dx, dy)
            for z in range(1, cz + R):
                # radius grows a little towards the lava, rounded cap on top
                rz = rad * (1.0 + 0.25 * (cz - z) / max(cz, 1)) if z <= cz else math.sqrt(max(0, rad * rad - ((z - cz) * 1.6) ** 2))
                if d + rr.normal(0, 0.45) < rz:
                    q = rr.random()
                    W.set(cx + dx, cy + dy, z, "netherrack" if q < 0.9 else ("magma" if q < 0.95 else "soul_sand"))


rock(-30, 40, Z + 6, 5.0, 2)

fires = []
for (x, y) in [(-31, 38), (-28, 41), (12, 47), (13, 55), (11, 60), (13, 56), (8, 17), (8, 23)] + [(a, b) for (a, b, c) in ROOF_FIRES]:
    zt = max([z for z in range(Z - 15, Z + 26) if W.get(x, y, z) is not None], default=None)
    if zt is not None and W.get(x, y, zt) in ("netherrack", "magma", "soul_sand") and W.get(x, y, zt + 1) is None:
        W.cross(x, y, zt + 1, "fire" if r.random() < 0.5 else "fire_b")
        fires.append((x + 0.5, y + 0.5, zt + 1.6))

# soul fire burning on soul sand at the balcony's outer corners
SOUL = []
for (x, y) in ((5, 17), (5, 23)):
    W.set(x, y, Z, "soul_sand")
    W.cross(x, y, Z + 1, "soul_fire")
    SOUL.append((x + 0.5, y + 0.5, Z + 1.6))
build_fences()

# floating embers (fire particles), drifting up from fires and lava
ember = emission_material("ember", (1.0, 0.30, 0.05), 2.2)
ember2 = emission_material("ember2", (1.0, 0.5, 0.12), 2.0)
for i in range(44):
    if i < 16 and fires:
        fx, fy, fz = fires[i % min(3, len(fires))]
        cx, cy, cz = fx + r.normal(0, 0.9), fy + r.normal(0, 0.9), fz + abs(r.normal(1.2, 1.4))
    else:
        cx, cy, cz = r.uniform(-2, 26), r.uniform(12, 60), r.uniform(Z - 12, Z + 8)
    s = r.choice([1 / 16, 1.5 / 16, 2 / 16])
    W.box((cx, cy, cz), (cx + s, cy + s, cz + s), ember if r.random() < 0.6 else ember2)

W.build("fortress")

# ---------------------------------------------------------------- light, haze
world_gradient([(-1.0, (0.30, 0.06, 0.02)), (-0.2, (0.22, 0.05, 0.02)), (0.1, (0.12, 0.03, 0.02)),
                (1.0, (0.03, 0.008, 0.008))], strength=1.0)
fog_box((20, 100, 40), (280, 280, 100), float(A.get("fog", 0.0035)), color=(0.40, 0.17, 0.11), anisotropy=0.25)
for f in fires:
    point(f, 35, (1.0, 0.5, 0.15), radius=0.3)
point((6.5, 20.5, Z + 3.2), float(A.get("gw", 110)), (1.0, 0.55, 0.25), radius=0.8)  # garden glow
for sp in SOUL:
    point(sp, 30, (0.3, 0.85, 1.0), radius=0.3)
for gp in GLOW:
    point(gp, 18, (1.0, 0.72, 0.38), radius=0.4)
# glow from the lava far below, coming up under the bridges
# lava glow from below on pillars / arch undersides, and a dim fill from above
area((12, 40, 2.0), (180, 0, 0), 40, float(A.get("key", 30000)), (1.0, 0.36, 0.08), size_y=90)
area((-10, 72, 2.0), (180, 0, 0), 80, float(A.get("key2", 15000)), (1.0, 0.36, 0.08), size_y=14)
add_sun(70, 200, float(A.get("fill", 0.25)), (1.0, 0.45, 0.3), angle=30)

cam_loc = (float(A.get("cx", -11.0)), float(A.get("cy", -6.0)), Z + float(A.get("cz", 10.0)))
target = (8.0, 40.0, Z + 2.5)
camera(cam_loc, target, lens=26, fstop=8.0, focus=(12, 30, Z))

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(A.get("exp", 0.0)), bloom=0.5,
             bloom_threshold=1.0, bloom_size=0.6, time_limit=195, volume_bounces=1)
bpy.context.scene.render.resolution_percentage = PCT
render(A.get("out", "fortress"), T0)
