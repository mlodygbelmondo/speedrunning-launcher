"""the-end.png - end stone island at night, obsidian pillars with glowing crystals, starry sky."""
import sys, math, time
sys.path.insert(0, "/tmp/srl/art/src")
import importlib, vox
importlib.reload(vox)
from vox import *
from mathutils import Matrix

T0 = time.time()
A = args()
PCT = int(A.get("pct", 100))
SAMPLES = int(A.get("samples", 128))

reset()
W = World(seed=66)
r = W.rng

# ---------------------------------------------------------------- main island
IR = int(A.get("ir", 118))
n = fbm2(2 * IR + 1, 2 * IR + 1, 3, scale=22, octaves=3)
nb = fbm2(2 * IR + 1, 2 * IR + 1, 4, scale=10, octaves=2)
HT = {}
for x in range(-IR, IR + 1):
    for y in range(-IR, IR + 1):
        d = math.hypot(x, y) / IR
        edge = IR * (1 + 0.12 * (n[y + IR, x + IR] - 0.5) * 2)
        if math.hypot(x, y) > edge:
            continue
        if math.hypot(x + 80, y - 40) < 34 + 6 * (n[y + IR, x + IR] - 0.5):  # cove bitten out of the rim
            continue
        top = int(round(3.0 * (1 - d * d) + 3.4 * (n[y + IR, x + IR] - 0.5)))
        dc = math.hypot(x + 80, y - 40) - 34
        dd = min(1 - d, dc / IR)  # distance to the nearest rim (outer edge or cove)
        under = min(top - 2, int(round(-3 - 30 * max(0.0, dd) ** 0.8 * 2.2 - 6 * nb[y + IR, x + IR]))) if dd < 0.2 else top - 3
        HT[(x, y)] = top
        for z in range(under, top + 1):
            W.set(x, y, z, "end_stone")


def ground(x, y):
    return HT.get((int(x), int(y)), 0)


# ---------------------------------------------------------------- obsidian pillars + crystals
OB = material("obsidian")
CR = material("end_crystal")
CG = material("crystal_cage")
BR = material("bedrock")
PILLARS = []
heights = [36, 47, 40, 56, 50, 38, 44, 43, 47, 42]
radii = [3, 2, 3, 2, 4, 3, 2, 3, 3, 4]
for i in range(10):
    ang = math.radians(i * 36 + 8)
    px, py = int(round(42 * math.cos(ang))), int(round(42 * math.sin(ang)))
    h, rad = heights[i], radii[i]
    g = ground(px, py)
    for dx in range(-rad - 1, rad + 2):
        for dy in range(-rad - 1, rad + 2):
            if dx * dx + dy * dy <= rad * rad + rad * 0.8:
                for z in range(g - 2, g + h):
                    W.set(px + dx, py + dy, z, "obsidian")
    top = g + h
    W.set(px, py, top, "bedrock")
    PILLARS.append((px + 0.5, py + 0.5, top + 1, i))
    # crystal: two nested rotated cubes floating above the bedrock
    c = (px + 0.5, py + 0.5, top + 2.6)
    R1 = Matrix.Rotation(math.radians(45), 3, "X") @ Matrix.Rotation(math.radians(35 + i * 13), 3, "Z")
    R2 = Matrix.Rotation(math.radians(-30), 3, "Y") @ Matrix.Rotation(math.radians(20 + i * 29), 3, "Z") @ Matrix.Rotation(math.radians(45), 3, "X")
    W.obox(c, (1.0, 1.0, 1.0), R1, CR)
    W.obox(c, (1.9, 1.9, 1.9), R2, CG)
    W.cross(px, py, top + 1, "fire", size=0.8, height=0.8)
    # iron-bar cages on two of the pillars
    if i in ():
        IB = material("iron_bars")
        for dx in range(-2, 3):
            for dy in range(-2, 3):
                for dz in range(1, 5):
                    if (abs(dx) == 2 or abs(dy) == 2 or dz == 4):
                        x0, y0, z0 = px + dx, py + dy, top + dz
                        W.box((x0, y0, z0), (x0 + 1, y0 + 1, z0 + 1), IB, IB, IB)

# ---------------------------------------------------------------- distant outer islands


def islet(cx, cy, cz, rad, seed):
    rr = np.random.default_rng(seed)
    R = int(rad) + 2
    for dx in range(-R, R + 1):
        for dy in range(-R, R + 1):
            d = math.hypot(dx, dy) / rad
            if d > 1 + rr.normal(0, 0.08):
                continue
            top = int(round(1.5 * (1 - d * d) + rr.normal(0, 0.4)))
            under = int(round(-1 - rad * 0.85 * max(0.0, 1 - d) ** 0.85 - abs(rr.normal(0, 0.35))))
            for z in range(under, top + 1):
                W.set(cx + dx, cy + dy, cz + z, "end_stone")


islet(-80, 115, 30, 12, 1)
islet(-110, 230, -6, 16, 2)
islet(170, 260, 34, 14, 3)
islet(-230, 60, -14, 9, 4)

W.build("the_end")

# ---------------------------------------------------------------- light
world_gradient([(-1.0, (0.012, 0.006, 0.02)), (-0.1, (0.03, 0.015, 0.045)), (0.0, (0.06, 0.03, 0.08)),
                (0.12, (0.032, 0.016, 0.055)), (0.4, (0.009, 0.005, 0.02)), (1.0, (0.003, 0.002, 0.008))],
               nebula=(2.2, 0.45, (0.55, 0.75, 1.3)),
               strength=float(A.get("skys", 0.62)), stars=(560, 0.085, float(A.get("stars", 20.0))))
add_sun(float(A.get("mel", 26)), float(A.get("maz", -150)), float(A.get("moon", 1.3)), (0.74, 0.68, 1.0), angle=6)
for (x, y, z, i) in PILLARS:
    point((x, y, z + 1.0), float(A.get("cl", 2500)), (1.0, 0.45, 0.95), radius=0.6)
fog_gradient_box((0, 60, 20), (560, 560, 120), float(A.get("fog", 0.004)), z0=-30, z1=float(A.get("fogtop", 45)),
                 color=(0.62, 0.52, 0.85), anisotropy=0.2)
fog_gradient_box((0, 60, 5), (560, 560, 30), float(A.get("mist", 0.03)), z0=-2, z1=9, color=(0.7, 0.62, 0.9), anisotropy=0.3)

CX, CY = float(A.get("cx", -40.0)), float(A.get("cy", -92.0))
cam_loc = (CX, CY, ground(CX, CY) + float(A.get("cz", 9.0)))
target = (float(A.get("tx", 0.0)), float(A.get("ty", 10.0)), float(A.get("tz", 30.0)))
camera(cam_loc, target, lens=float(A.get("lens", 24)), fstop=None)

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(A.get("exp", 0.0)), bloom=0.6,
             bloom_threshold=1.0, bloom_size=0.65, time_limit=195, volume_bounces=1)
bpy.context.scene.render.resolution_percentage = PCT
render(A.get("out", "the-end"), T0)
