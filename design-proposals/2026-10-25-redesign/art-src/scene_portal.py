"""nether-portal.png - complete lit obsidian portal at dusk in a forest clearing."""
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
MATPROPS["oak_leaves"] = dict(rough=0.8)  # opaque canopy at dusk (holes read as noise)
W = World(seed=21)
r = W.rng

X0, X1, Y0, Y1 = -60, 80, -20, 140
w, h = X1 - X0, Y1 - Y0
n1 = fbm2(w, h, 31, scale=20, octaves=3)
n2 = fbm2(w, h, 32, scale=50, octaves=2)
yy = np.arange(Y0, Y1)[:, None] * np.ones((1, w))
xx = np.ones((h, 1)) * np.arange(X0, X1)[None, :]
PX, PY = 6, 22
# clearing: round around the portal, plus a long open glade behind it (seen from the camera)
dist = np.sqrt((xx - PX - 1.5) ** 2 + ((yy - PY) * 1.1) ** 2)
vd = np.array([0.69, 0.72])
along = (xx - PX - 1.5) * vd[0] + (yy - PY) * vd[1]
across = -(xx - PX - 1.5) * vd[1] + (yy - PY) * vd[0]
glade = np.sqrt((across / 9.0) ** 2 + (np.clip(along, 0, None) / 40.0) ** 2) * 14.0
dist = np.minimum(dist, np.where(along > 0, glade, 1e9))
H = 5 + 2.2 * (n1 - 0.5) * 2 + np.clip((dist - 14) / 30, 0, 1) * 10 * np.clip(n2 - 0.3, 0, 1) - np.clip(along - 12, 0, 30) * 0.12
clear = np.exp(-(dist / 13.0) ** 2)
H = H * (1 - clear) + 5 * clear
Hi = np.round(H).astype(int)


def hat(x, y):
    return Hi[y - Y0, x - X0]


for iy in range(h):
    for ix in range(w):
        x, y = ix + X0, iy + Y0
        top = Hi[iy, ix]
        nb = [Hi[max(0, min(h - 1, iy + dy)), max(0, min(w - 1, ix + dx))] for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))]
        lo = min(min(nb), top) - 1
        for z in range(lo, top + 1):
            W.set(x, y, z, "grass" if z == top else ("dirt" if z >= top - 2 else "stone"))

g = hat(PX, PY)
# a few coarse dirt / path blocks in the clearing
for i in range(22):
    x = int(PX + 1 + r.normal(0, 3.2)); y = int(PY - 2 + r.normal(0, 3.0))
    if W.get(x, y, hat(x, y)) == "grass" and r.random() < 0.5:
        W.set(x, y, hat(x, y), "dirt")

# ---------------------------------------------------------------- portal 4x5 frame, interior 2x3
base = g + 1
for i in range(4):
    W.set(PX + i, PY, base, "obsidian")
    W.set(PX + i, PY, base + 4, "obsidian")
for k in range(1, 4):
    W.set(PX, PY, base + k, "obsidian")
    W.set(PX + 3, PY, base + k, "obsidian")
pm = material("nether_portal", es=float(A.get("pes", 1.5)), alpha_const=0.95, base_mul=0.08)
W.box((PX + 1, PY + 6 / 16, base + 1), (PX + 3, PY + 10 / 16, base + 4), pm, pm, pm, skip=("px", "nx", "pz", "nz"))

# portal particles: tiny glowing squares drifting around the frame
part = emission_material("portal_particle", (0.72, 0.22, 1.0), 1.1)
for i in range(34):
    cx = PX + 2 + r.normal(0, 1.5)
    cy = PY + 0.5 + r.normal(0, 1.1)
    cz = base + 2.5 + r.normal(0, 1.4)
    s = r.choice([1 / 16, 1 / 16, 1.5 / 16])
    W.box((cx, cy, cz), (cx + s, cy + s, cz + s), part)

# ---------------------------------------------------------------- forest


def oak(x, y, trunk=5, rad=2.4, seed=0):
    rr = np.random.default_rng(seed)
    if not (X0 + 3 < x < X1 - 3 and Y0 + 3 < y < Y1 - 3):
        return
    z0 = hat(x, y) + 1
    W.set(x, y, z0 - 1, "dirt")
    top = z0 + trunk
    for z in range(z0, top):
        W.set(x, y, z, "oak_log")
    cz = top - 0.5
    R = int(math.ceil(rad)) + 1
    for dx in range(-R, R + 1):
        for dy in range(-R, R + 1):
            for dz in range(-2, 3):
                d = math.sqrt(dx * dx + dy * dy + (dz * 1.3) ** 2)
                if d + rr.normal(0, 0.35) < rad:
                    p = (x + dx, y + dy, int(cz + dz))
                    if W.get(*p) is None:
                        W.set(*p, "oak_leaves")


n_t = 0
for i in range(3000):
    x = int(r.integers(X0 + 4, X1 - 4)); y = int(r.integers(Y0 + 4, Y1 - 4))
    d = dist[y - Y0, x - X0]
    if d < 14.0 + r.normal(0, 1.5):
        continue
    # keep the camera corridor open
    if y < PY - 4 and -18 < x - PX < 8:
        continue
    if math.hypot(x - (PX - 9.5), y - (PY - 12)) < 11:
        continue
    if r.random() < 0.13:
        oak(x, y, trunk=int(r.integers(5, 9)), rad=r.uniform(2.3, 3.4), seed=500 + i)
        n_t += 1
print("trees", n_t)

# plants in the clearing
for x in range(PX - 16, PX + 18):
    for y in range(PY - 16, PY + 14):
        t = hat(x, y)
        if W.get(x, y, t) != "grass" or W.get(x, y, t + 1) is not None:
            continue
        if PX - 1 <= x <= PX + 4 and PY - 1 <= y <= PY + 1:
            continue
        q = r.random()
        dn = math.hypot(x - PX - 1.5, y - PY + 2)
        dc = math.hypot(x - (PX - 9.5), y - (PY - 12))
        dens = 0.2 if dn > 5 else 0.06
        if dc < 9:
            dens = 0.07
        if q < dens:
            W.cross(x, y, t + 1, "tall_grass", size=0.9, height=0.9 * r.uniform(0.7, 1.0))
        elif q < dens + 0.025:
            W.cross(x, y, t + 1, "flower_red" if r.random() < 0.4 else "flower_yellow", size=0.8, height=0.8)

W.build("portal_scene")

# ---------------------------------------------------------------- light: dusk
SUN_EL, SUN_AZ = -1.5, 20.0
_, sdir = sun_dir(4, SUN_AZ)
sun_ob, _ = add_sun(2.0, SUN_AZ, float(A.get("suns", 0.9)), (1.0, 0.45, 0.25), angle=6)
world_gradient([(-1.0, (0.02, 0.02, 0.03)), (-0.02, (0.20, 0.10, 0.12)), (0.0, (0.85, 0.38, 0.24)),
                (0.05, (0.55, 0.28, 0.40)), (0.13, (0.22, 0.16, 0.42)), (0.32, (0.07, 0.08, 0.28)), (1.0, (0.015, 0.025, 0.10))],
               strength=float(A.get("skys", 0.8)), sun_glow=(sdir, (1.0, 0.40, 0.16), 28.0, 1.2),
               stars=(420, 0.06, 6.0))
pc = (PX + 2, PY + 0.5, base + 2.5)
point((PX + 2.0, PY - 2.6, base + 1.2), float(A.get("spill", 380)), (0.62, 0.12, 1.0), radius=1.2)
add_sun(55, 160, float(A.get("fill", 0.55)), (0.45, 0.55, 1.0), angle=20)  # cool twilight fill
area((PX + 2, PY - 0.35, base + 2.5), (-90, 0, 0), 1.7, float(A.get("pl", 850)), (0.7, 0.25, 1.0), size_y=2.7)
area((PX + 2, PY + 1.35, base + 2.5), (90, 0, 0), 1.7, 200, (0.7, 0.25, 1.0), size_y=2.7)
fog_gradient_box((PX, 50, 10), (150, 180, 30), float(A.get("fog", 0.014)), z0=g, z1=g + 9,
                 color=(0.85, 0.8, 1.0), anisotropy=0.35)

cam_loc = (PX - 9.5, PY - 12.0, g + 1.9)
target = (PX - 3.4, PY + 2, g + 4.4)
camera(cam_loc, target, lens=30, fstop=4.5, focus=(PX + 2, PY, g + 3))

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(A.get("exp", 0.0)), bloom=0.5,
             bloom_threshold=1.0, bloom_size=0.6, time_limit=195, volume_bounces=1)
bpy.context.scene.render.resolution_percentage = PCT
render(A.get("out", "nether-portal"), T0)
