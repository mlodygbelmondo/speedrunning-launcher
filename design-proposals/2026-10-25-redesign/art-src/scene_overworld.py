"""overworld.png - rolling grass hills, oak trees, golden hour, ruined nether portal."""
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
MATPROPS["lava"]["es"] = 1.5
W = World(seed=7)
r = W.rng

# ---------------------------------------------------------------- terrain
X0, X1, Y0, Y1 = -70, 120, -12, 230
w, h = X1 - X0, Y1 - Y0
n1 = fbm2(w, h, 11, scale=28, octaves=4)
n2 = fbm2(w, h, 12, scale=80, octaves=3)
yy = np.arange(Y0, Y1)[:, None] * np.ones((1, w))
xx = np.ones((h, 1)) * np.arange(X0, X1)[None, :]
far = np.clip((yy - 64) / 60, 0, 1) * np.clip(1.15 - np.abs(xx - 10) / 120, 0, 1)
H = 5 + 3.2 * (n1 - 0.5) * 2 + far * 24 * np.clip(n2 - 0.32, 0, 1) * 1.5
# a gentle lake basin to the left-middle
lake = np.exp(-(((xx + 10) / 16) ** 2 + ((yy - 52) / 9) ** 2))
H = H - 0.0 * lake
# flatten the portal plateau
PX, PY = 8, 24
pd = np.sqrt(((xx - PX + 3) / 11.0) ** 2 + ((yy - PY + 6) / 12.0) ** 2)
plate = np.exp(-pd ** 2 * 1.4)
H = H * (1 - plate) + 6.0 * plate
Hi = np.round(H).astype(int)
WATER = -100


def hat(x, y):
    return Hi[y - Y0, x - X0]


for iy in range(h):
    for ix in range(w):
        x, y = ix + X0, iy + Y0
        top = Hi[iy, ix]
        nb = [Hi[max(0, min(h - 1, iy + dy)), max(0, min(w - 1, ix + dx))] for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))]
        lo = min(min(nb), top) - 1
        for z in range(lo, top + 1):
            if z == top:
                b = "grass"
                if top <= WATER + 1 and lake[iy, ix] > 0.12:
                    b = "sand"
            elif z >= top - 2:
                b = "dirt"
            else:
                b = "stone"
            W.set(x, y, z, b)
        if top < WATER:
            for z in range(top + 1, WATER + 1):
                W.set(x, y, z, "water")
            W.set(x, y, top, "sand")

hp = hat(PX, PY)

# ---------------------------------------------------------------- ruined portal
# netherrack corruption patch
for x in range(PX - 7, PX + 9):
    for y in range(PY - 6, PY + 7):
        d = math.hypot((x - PX - 1.5) / 1.1, y - PY)
        if d + r.normal(0, 0.9) < 4.6:
            t = hat(x, y)
            W.set(x, y, t, "netherrack")
            if r.random() < 0.5:
                W.set(x, y, t - 1, "netherrack")
# frame: 4 wide x 5 tall in the x-z plane at y=PY, standing on the plateau
fx0 = PX
base = hp + 1
frame = []
for i in range(4):
    frame += [(fx0 + i, base), (fx0 + i, base + 4)]
for k in range(1, 4):
    frame += [(fx0, base + k), (fx0 + 3, base + k)]
missing = {(fx0 + 3, base + 4), (fx0 + 2, base + 4), (fx0 + 3, base + 3)}
crying = {(fx0, base + 2), (fx0 + 3, base + 1)}
for (x, z) in frame:
    if (x, z) in missing:
        continue
    W.set(x, PY, z, "crying_obsidian" if (x, z) in crying else "obsidian")
# stone-brick plinth under the frame
for x in range(fx0 - 1, fx0 + 5):
    for y in (PY - 1, PY, PY + 1):
        if r.random() < 0.75:
            W.set(x, y, hp, "mossy_stone_bricks" if r.random() < 0.45 else ("cracked_stone_bricks" if r.random() < 0.3 else "stone_bricks"))
# fallen obsidian chunks
W.set(fx0 + 5, PY - 1, hat(fx0 + 5, PY - 1) + 1, "obsidian")
W.set(fx0 + 4, PY + 2, hat(fx0 + 4, PY + 2) + 1, "crying_obsidian")
W.set(fx0 - 2, PY - 2, hat(fx0 - 2, PY - 2) + 1, "obsidian")
W.set(fx0 + 6, PY + 1, hat(fx0 + 6, PY + 1) + 1, "netherrack")
W.set(fx0 + 6, PY + 1, hat(fx0 + 6, PY + 1) + 2, "obsidian") if False else None
# gold block half-buried
W.set(fx0 - 3, PY + 1, hat(fx0 - 3, PY + 1), "gold")
# magma
for (x, y) in [(fx0 + 2, PY - 2), (fx0 - 1, PY + 2), (fx0 + 6, PY - 2)]:
    W.set(x, y, hat(x, y), "magma")
# lava pool (sunk one block into the plateau)
lava_cells = [(fx0 - 1, PY - 4), (fx0, PY - 4), (fx0 + 1, PY - 4), (fx0, PY - 3), (fx0 + 1, PY - 3)]
for (x, y) in lava_cells:
    t = hat(x, y)
    W.set(x, y, t, "lava")
    W.set(x, y, t - 1, "netherrack")
# fire on some netherrack
fires = []
for (x, y) in [(fx0 - 1, PY - 1), (fx0 + 5, PY + 1), (fx0 + 6, PY - 1), (fx0 + 1, PY + 3)]:
    t = hat(x, y)
    if W.get(x, y, t) in ("netherrack", "magma") and W.get(x, y, t + 1) is None:
        W.cross(x, y, t + 1, "fire" if r.random() < 0.5 else "fire_b", size=1.0, height=1.0)
        fires.append((x + 0.5, y + 0.5, t + 1.5))


# ---------------------------------------------------------------- trees
def oak(x, y, trunk=5, rad=2.4, seed=0, big=False):
    rr = np.random.default_rng(seed)
    z0 = hat(x, y) + 1
    if W.get(x, y, z0 - 1) not in ("grass", "dirt"):
        return
    W.set(x, y, z0 - 1, "dirt")
    top = z0 + trunk
    for z in range(z0, top):
        W.set(x, y, z, "oak_log")
    cz = top - 0.5
    R = int(math.ceil(rad)) + 1
    for dx in range(-R, R + 1):
        for dy in range(-R, R + 1):
            for dz in range(-2, 3):
                d = math.sqrt(dx * dx + dy * dy + (dz * 1.35) ** 2)
                if d + rr.normal(0, 0.35) < rad:
                    p = (x + dx, y + dy, int(cz + dz))
                    if W.get(*p) is None:
                        W.set(*p, "oak_leaves")
    if big:  # side branches
        for (bx, by) in [(1, 0), (-1, 1)]:
            W.set(x + bx, y + by, top - 2, "oak_log")


oak(PX + 13, PY + 9, trunk=6, rad=3.4, seed=1, big=True)
oak(PX - 17, PY + 30, trunk=5, rad=2.6, seed=2)
placed = 0
for i in range(400):
    x = int(r.integers(X0 + 5, X1 - 5))
    y = int(r.integers(36, Y1 - 10))
    if abs(x - PX) < 10 and y < 40:
        continue
    if lake[y - Y0, x - X0] > 0.05:
        continue
    if r.random() < 0.45:
        oak(x, y, trunk=int(r.integers(4, 7)), rad=r.uniform(2.0, 3.0), seed=100 + i)
        placed += 1
    if placed > 70:
        break

# ---------------------------------------------------------------- plants
for x in range(-30, 60):
    for y in range(-6, 90):
        t = hat(x, y)
        if W.get(x, y, t) != "grass" or W.get(x, y, t + 1) is not None:
            continue
        q = r.random()
        dc = math.hypot(x - (PX - 6), y - (PY - 13))
        dens = 0.16 if dc > 7 else 0.05
        if q < dens:
            W.cross(x, y, t + 1, "tall_grass", size=0.9, height=0.9 * r.uniform(0.7, 1.0))
        elif q < dens + 0.022:
            W.cross(x, y, t + 1, "flower_red" if r.random() < 0.5 else "flower_yellow", size=0.8, height=0.8)

W.build("overworld")

# ---------------------------------------------------------------- light
SUN_EL, SUN_AZ = float(A.get("el", 5.0)), float(A.get("az", -115.0))
sun_ob, sdir = add_sun(SUN_EL, SUN_AZ, float(A.get("suns", 9.0)), (1.0, 0.50, 0.21), angle=2.0)
SKIES = {
    "a": [(-1.0, (0.10, 0.08, 0.07)), (-0.02, (0.85, 0.50, 0.30)), (0.02, (1.0, 0.68, 0.40)),
          (0.09, (0.98, 0.72, 0.52)), (0.22, (0.62, 0.62, 0.78)), (0.5, (0.30, 0.46, 0.82)), (1.0, (0.14, 0.28, 0.66))],
    "c": [(-1.0, (0.10, 0.08, 0.07)), (-0.02, (0.9, 0.42, 0.18)), (0.015, (1.0, 0.55, 0.24)),
          (0.07, (1.0, 0.58, 0.36)), (0.17, (0.62, 0.50, 0.70)), (0.34, (0.24, 0.34, 0.80)), (1.0, (0.07, 0.15, 0.55))],
    "b": [(-1.0, (0.10, 0.08, 0.07)), (-0.02, (0.9, 0.55, 0.32)), (0.02, (1.0, 0.75, 0.45)),
          (0.12, (0.95, 0.80, 0.62)), (0.3, (0.45, 0.60, 0.88)), (1.0, (0.18, 0.36, 0.78))],
}
world_gradient(SKIES[A.get("sky", "c")], strength=float(A.get("skys", 1.0)), sun_glow=(sdir, (1.0, 0.5, 0.2), 4.0, 1.2))
cloud_layer(5, -300, 360, 170, 560, 82, cell=5, thick=2.5, cover=float(A.get("cover", 0.2)), block="cloud_warm")
for f in fires:
    point(f, 12, (1.0, 0.5, 0.15), radius=0.3)
lx = sum(c[0] for c in lava_cells) / len(lava_cells) + 0.5
ly = sum(c[1] for c in lava_cells) / len(lava_cells) + 0.5
point((lx, ly, hp + 1.6), 22, (1.0, 0.42, 0.1), radius=0.8)
# purple glow from crying obsidian
point((fx0 + 1.5, PY - 1.2, base + 2.2), 45, (0.7, 0.3, 1.0), radius=1.0)

fog_gradient_box((25, 110, 15), (260, 280, 40), float(A.get("fog", 0.006)), z0=2, z1=float(A.get("fogh", 26)),
                 color=(1.0, 0.80, 0.60), anisotropy=0.45)

cam_loc = (PX - 8.5, PY - 15.5, hp + 3.3)
target = (PX - 3.2, PY + 2, hp + 3.6)
camera(cam_loc, target, lens=30, fstop=5.6, focus=(PX + 1.5, PY, hp + 3))

setup_render(samples=SAMPLES, look=A.get("look", "AgX - Medium High Contrast"), exposure=float(A.get("exp", 0.0)), bloom=0.35,
             bloom_threshold=1.2, bloom_size=0.55, time_limit=195, volume_bounces=1)
bpy.context.scene.render.resolution_percentage = PCT
render(A.get("out", "overworld"), T0)
