"""stronghold.png - end-portal room: stone bricks, stairs, 3x3 ring of frames, starfield portal."""
import sys, math, time
sys.path.insert(0, "/tmp/srl/art/src")
import importlib, vox
importlib.reload(vox)
from vox import *
vox.STAR_GAIN[0] = 3.2
vox.STAR_BASE[0] = 2.2
from mathutils import Matrix

T0 = time.time()
A = args()
PCT = int(A.get("pct", 100))
SAMPLES = int(A.get("samples", 128))

reset()
MATPROPS["lava"]["es"] = float(A.get("les", 0.8))
W = World(seed=55)
r = W.rng

RX, RY, RZ = 16, 27, 11  # interior: x 0..15, y 0..26, z 0..10


def sb():
    q = r.random()
    return "stone_bricks" if q < 0.58 else ("mossy_stone_bricks" if q < 0.83 else "cracked_stone_bricks")


# shell
for x in range(-2, RX + 2):
    for y in range(-2, RY + 2):
        for z in range(-2, RZ + 2):
            inside = 0 <= x < RX and 0 <= y < RY and 0 <= z < RZ
            if not inside:
                W.set(x, y, z, sb())
# wall pilasters with arches between them
for y in range(1, RY, 5):
    for x in (0, RX - 1):
        for z in range(0, RZ):
            W.set(x, y, z, "stone_bricks")
for y in range(0, RY):
    for x in (0, RX - 1):
        if (y - 1) % 5 != 0:
            W.set(x, y, RZ - 1, sb())
            if (y - 1) % 5 in (1, 4):
                W.set(x, y, RZ - 2, sb())
# ceiling beams
for y in range(1, RY, 5):
    for x in range(0, RX):
        W.set(x, y, RZ - 1, "stone_bricks")

# raised platform at the back
PZ = 3  # platform top surface height
for x in range(2, RX - 2):
    for y in range(14, RY - 1):
        for z in range(0, PZ):
            W.set(x, y, z, sb())
# lava pools in the floor either side of the stairs
LAVA = []
for (x0, x1) in ((1, 3), (12, 14)):
    for x in range(x0, x1 + 1):
        for y in range(9, 14):
            W.set(x, y, -1, "lava")
            LAVA.append((x, y))
# stone-brick curb around the pools
for (x0, x1) in ((1, 4), (11, 14)):
    for x in range(x0 - 1, x1 + 2):
        for y in (5, 13):
            if 0 < x < RX - 1 and W.get(x, y, 0) is None:
                pass

# ---------------------------------------------------------------- stairs (half-step geometry)
SB = material("stone_bricks")
SX0, SX1 = 5, 10
for i, (y, z) in enumerate([(11, 0), (12, 1), (13, 2)]):
    for zz in range(0, z):
        for x in range(SX0, SX1 + 1):
            W.set(x, y, zz, "stone_bricks")
    W.box((SX0, y, z), (SX1 + 1, y + 1, z + 0.5), SB, SB, SB, skip=("nz",))
    W.box((SX0, y + 0.5, z + 0.5), (SX1 + 1, y + 1, z + 1), SB, SB, SB, skip=("nz", "py"))
# side walls of the stair run so nothing floats
for (y, z) in [(11, 0), (12, 1), (13, 2)]:
    for x in (SX0 - 1, SX1 + 1):
        for zz in range(0, z + 1):
            W.set(x, y, zz, "stone_bricks")

# ---------------------------------------------------------------- end portal ring
CX0, CY0 = 6, 18  # portal cells x 6..8, y 18..20
FT, FS, ES = material("end_portal_frame_top"), material("end_portal_frame_side"), material("end_stone")
EYE, EYES = material("eye_of_ender"), material("eye_of_ender_side")
frames = [(x, CY0 - 1) for x in range(CX0, CX0 + 3)] + [(x, CY0 + 3) for x in range(CX0, CX0 + 3)] + \
         [(CX0 - 1, y) for y in range(CY0, CY0 + 3)] + [(CX0 + 3, y) for y in range(CY0, CY0 + 3)]
with_eye = {0, 1, 2, 4, 6, 7, 9, 10}
for i, (x, y) in enumerate(frames):
    W.box((x, y, PZ), (x + 1, y + 1, PZ + 13 / 16), FT, FS, ES, skip=("nz",))
    if i in with_eye:
        W.box((x + 4 / 16, y + 4 / 16, PZ + 13 / 16), (x + 12 / 16, y + 12 / 16, PZ + 1.0), EYE, EYES, EYES, skip=("nz",))
EPM = end_portal_material()
pz = PZ + 12 / 16
W.quad_plane([(CX0, CY0, pz), (CX0 + 3, CY0, pz), (CX0 + 3, CY0 + 3, pz), (CX0, CY0 + 3, pz)], EPM,
             uvs=((0, 0), (3, 0), (3, 3), (0, 3)))

# ---------------------------------------------------------------- spawner cage on the floor
IB = material("iron_bars")
W.box((12, 1, 0), (13, 2, 1), IB, IB, IB, skip=("nz",))
core = emission_material("spawner_core", (0.9, 0.35, 0.12), 1.2)
W.box((12.35, 1.35, 0.2), (12.65, 1.65, 0.5), core)

# ---------------------------------------------------------------- torches
STICK = material("oak_log")
FLAME = emission_material("torch_flame", (1.0, 0.45, 0.12), 3.6)
TORCHES = []


def obox(c, size, R, mats):
    """Oriented box: centre c, size (sx,sy,sz), rotation matrix R."""
    sx, sy, sz = size[0] / 2, size[1] / 2, size[2] / 2
    C = Vector(c)
    for d, (off, corners) in FACES.items():
        pts, uvs = [], []
        for k in corners:
            lp = Vector(((k[0] * 2 - 1) * sx, (k[1] * 2 - 1) * sy, (k[2] * 2 - 1) * sz))
            pts.append(tuple(C + R @ lp))
            u, v = face_uv(d, (k[0], k[1], k[2]))
            uvs.append((0.45 + u * 0.12, v * 0.6))
        W.extra.quad(pts, uvs, mats)


def wall_torch(x, y, z, facing, axis="Y"):
    """Torch leaning 22 deg out of a wall (axis Y: leans along x, axis X: leans along y)."""
    ang = math.radians(22) * facing
    R = Matrix.Rotation(ang, 3, axis)
    base = Vector((x, y, z))
    stick_c = base + R @ Vector((0, 0, 0.3))
    obox(stick_c, (0.125, 0.125, 0.62), R, STICK)
    fc = base + R @ Vector((0, 0, 0.66))
    obox(fc, (0.14, 0.14, 0.14), R, FLAME)
    TORCHES.append(tuple(base + R @ Vector((0, 0, 0.8))))


for y in (11, 16, 21):
    wall_torch(1.06, y + 0.5, 4.3 if y < 14 else 5.6, +1)
    wall_torch(RX - 1.06, y + 0.5, 4.3 if y < 14 else 5.6, -1)
wall_torch(4.5, RY - 1.06, 5.0, +1, axis="X")
wall_torch(11.5, RY - 1.06, 5.0, +1, axis="X")
# standing torches on the platform front corners
for (x, y) in ((3.5, 14.5), (12.5, 14.5)):
    obox((x, y, PZ + 0.31), (0.125, 0.125, 0.62), Matrix.Identity(3), STICK)
    obox((x, y, PZ + 0.68), (0.14, 0.14, 0.14), Matrix.Identity(3), FLAME)
    TORCHES.append((x, y, PZ + 0.85))

W.build("stronghold")

# ---------------------------------------------------------------- light
world_gradient([(-1, (0, 0, 0)), (1, (0, 0, 0))], strength=0.0)
for t in TORCHES:
    point(t, float(A.get("tw", 45)), (1.0, 0.6, 0.28), radius=0.15)
# portal glow (teal) up from the surface
area((CX0 + 1.5, CY0 + 1.5, pz + 0.05), (0, 0, 0), 3.0, float(A.get("pg", 140)), (0.25, 0.95, 0.85))
# lava pool glow
for (x0, x1) in ((1, 3), (12, 14)):
    area(((x0 + x1 + 1) / 2, 11.5, 0.3), (0, 0, 0), 2.5, float(A.get("lg", 35)), (1.0, 0.4, 0.1), size_y=4.5)
fog_box((RX / 2, RY / 2, RZ / 2), (RX, RY, RZ), float(A.get("fog", 0.012)), color=(0.8, 0.85, 0.9), anisotropy=0.3)

cam_loc = (float(A.get("cx", 2.3)), float(A.get("cy", 5.0)), float(A.get("cz", 6.3)))
target = (float(A.get("tx", 5.6)), 19.5, float(A.get("tz", 2.6)))
camera(cam_loc, target, lens=float(A.get("lens", 26)), fstop=5.6, focus=(CX0 + 1.5, CY0 + 1.5, PZ))

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(A.get("exp", 0.0)), bloom=0.5,
             bloom_threshold=1.0, bloom_size=0.6, time_limit=195, volume_bounces=1)
bpy.context.scene.render.resolution_percentage = PCT
render(A.get("out", "stronghold"), T0)
