"""mode-ranked.png - a REAL plains village of seed 2026 (Java 1.16.1), block for block, our textures.

The village starts at (-816, 32) in a plains biome, west of the world origin: houses along grass paths, the church,
two meeting points with bells, small farms (wheat, carrots, potatoes), lamp posts and hay bales, a pond on its
west side and a dark forest with huge mushrooms behind it. Extract `vBbig` is the generated world box
x -1120..-510, y 50..110, z -280..340 (mc/find_village.py, mc/gen.py, mc/extract.py). The camera stands south-west
of the village at (-870, 84, 95) and looks north-east; golden hour, the sun low in the west.

One edit to the real terrain: a cave mouth at (-850, 53) between the pond and the southern houses is filled in
(dirt, grass and tufts at the height of the meadow around it), because at this size it reads as a black hole.
Outside the box there is a flat grass plane that a distance haze fades into the sky.

blender -b -P scene_village.py -- [samples=..] [w= h=] [cam=x,y,z tgt=x,y,z lens=..] [el= az=] out=..
(MC coordinates for cam/tgt.) The defaults render the master.
"""
import sys
import time
sys.path.insert(0, "/tmp/srl/art/src")
import numpy as np
import bpy
import vox
from vox import *
import mcblocks as mc

T0 = time.time()
A = args()
D = dict(npz="vBbig", o="-816,64,32", cam="-870,84,95", tgt="-800,66,20", lens="32", sx="-0.1", sy="0",
         el="5", az="288", suns="10", sunc="1.0,0.62,0.34", skys="0.8", gs="4.0", gw="1.1", cover="0.12", cseed="5",
         cz="64", fog="0.0012", fogh="28", fogc="1.0,0.84,0.68", exp="0.25", out="mode-ranked")
G = lambda k: A.get(k, D.get(k))
SAMPLES = int(A.get("samples", 128))
reset()

LEAVES = ("oak_leaves", "birch_leaves", "spruce_leaves", "jungle_leaves", "dark_oak_leaves", "acacia_leaves")
vox.PACKED.update(LEAVES)
pal, arr, origin = mc.load(G("npz"))
pal = list(pal)
x0, y0, z0 = origin
nx, ny, nz = arr.shape
OX, OY, OZ = [int(float(v)) for v in G("o").split(",")]


def pid(state):
    if state not in pal:
        pal.append(state)
    return pal.index(state)


def fill_hole(bx0, bx1, bz0, bz1):
    """Close a cave mouth: each column in the box is raised to the ground height around it (dirt, grass on top)."""
    dirt, grass, tuft = pid("minecraft:dirt"), pid("minecraft:grass_block[snowy=false]"), pid("minecraft:grass")
    rng = np.random.default_rng(7)
    names = [p.split("[")[0][10:] for p in pal]
    airy = np.array([n in ("air", "cave_air") for n in names])
    ground = np.array([n in ("grass_block", "dirt", "stone", "granite", "diorite", "andesite", "gravel", "sand", "clay",
                             "grass_path", "coal_ore", "iron_ore") for n in names])
    i0, i1, k0, k1 = bx0 - x0, bx1 - x0 + 1, bz0 - z0, bz1 - z0 + 1
    g = ground[arr[i0 - 1:i1 + 1, :, k0 - 1:k1 + 1]]  # one column of margin
    H = np.where(g.any(1), g.shape[1] - 1 - np.argmax(g[:, ::-1, :], axis=1), 0).astype(float)
    H0 = H.copy()
    for _ in range(40):  # raise-only relaxation: the hole rises to the ground level around it
        nb = np.stack([np.roll(np.roll(H, a, 0), c, 1) for a in (-1, 0, 1) for c in (-1, 0, 1) if a or c])
        inner = H[1:-1, 1:-1]
        H[1:-1, 1:-1] = np.maximum(inner, np.median(nb, axis=0)[1:-1, 1:-1])
    n = 0
    for a in range(1, H.shape[0] - 1):
        for c in range(1, H.shape[1] - 1):
            t = int(round(H[a, c]))
            if t <= H0[a, c]:
                continue
            col = arr[i0 + a - 1, :t + 1, k0 + c - 1]
            m = airy[col]
            col[m] = dirt
            if airy[col[t]] or col[t] == dirt:
                col[t] = grass
                if rng.random() < 0.35 and airy[arr[i0 + a - 1, t + 1, k0 + c - 1]]:  # tufts like the meadow around
                    arr[i0 + a - 1, t + 1, k0 + c - 1] = tuft
            n += int(m.sum())
    print("[village] filled", n, "cells")


if G("npz") == "vBbig" and A.get("fill", "1") == "1":
    fill_hole(-858, -842, 48, 62)  # the cave mouth between the lake and the southern houses


def P(x, y, z):
    return (x - OX, OZ - z, y - OY)


b = mc.Builder(pal, arr, origin, OX, OY, OZ, seed=int(A.get("seed", 3)),
               mat_over={"water": dict(rough=0.08, ior=1.33, alpha_const=float(A.get("wa", 0.55)), spec=0.5),
                         **{l: dict(alpha=False) for l in LEAVES}})
b.build_cubes("vil")
b.build_models("vil")
print("[village] unknown", b.unknown)
if A.get("lights", "1") == "1":
    b.add_lights(radius=0.1)

# ---------------------------------------------------------------- ground beyond the box (hidden by haze)
gm = material("grass_top", tint=(0.62, 0.72, 0.5))
mb = vox.MeshBuilder()
bx0, bx1 = x0 - OX, x0 + nx - OX
by0, by1 = OZ - (z0 + nz), OZ - z0
R = 4000
GZ = float(A.get("ground", 63)) - OY
for (ax0, ay0, ax1, ay1) in ((-R, -R, R, by0), (-R, by1, R, R), (-R, by0, bx0, by1), (bx1, by0, R, by1)):
    mb.quad([(ax0, ay0, GZ), (ax1, ay0, GZ), (ax1, ay1, GZ), (ax0, ay1, GZ)],
            [(ax0, ay0), (ax1, ay0), (ax1, ay1), (ax0, ay1)], gm)
mb.build("ground")

# ---------------------------------------------------------------- sky, sun, clouds, haze
SUN_EL, SUN_AZ = float(G("el")), float(G("az"))
sunc = tuple(float(t) for t in G("sunc").split(","))
sun_ob, sdir = add_sun(SUN_EL, SUN_AZ, float(G("suns")), sunc, angle=float(A.get("sa", 2.0)))
SKY = [(-1.0, (0.06, 0.06, 0.08)), (-0.02, (0.85, 0.48, 0.24)), (0.0, (1.0, 0.58, 0.30)), (0.03, (1.0, 0.68, 0.44)),
       (0.07, (0.84, 0.64, 0.64)), (0.13, (0.50, 0.55, 0.84)), (0.26, (0.24, 0.36, 0.80)), (1.0, (0.06, 0.14, 0.50))]
world_gradient(SKY, strength=float(G("skys")),
               sun_glow=(sdir, (1.0, 0.62, 0.30), float(G("gs")), float(G("gw"))))
cloud_layer(int(G("cseed")), -1200, 1200, -1200, 1200, float(G("cz")), cell=float(A.get("cell", 12)), thick=4,
            cover=float(G("cover")), block="cloud_warm")
fog_gradient_box((0, 0, 20), (1600, 1600, 80), float(G("fog")), z0=-4, z1=float(G("fogh")),
                 color=tuple(float(t) for t in G("fogc").split(",")), anisotropy=0.55)



def far_haze(cx, cy, r0, r1, density, zmax, color):
    """Haze that only starts r0 blocks from (cx, cy): fades the flat ground beyond the box into the sky."""
    bpy.ops.mesh.primitive_cube_add(size=1, location=(cx, cy, zmax / 2 - 10))
    ob = bpy.context.active_object
    ob.name = "FarHaze"
    ob.scale = (2 * r1 + 2000, 2 * r1 + 2000, zmax + 20)
    m = bpy.data.materials.new("FarHaze_m")
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        if n.type != "OUTPUT_MATERIAL":
            nt.nodes.remove(n)
    pv = nt.nodes.new("ShaderNodeVolumePrincipled")
    pv.inputs["Color"].default_value = (*color, 1)
    pv.inputs["Anisotropy"].default_value = 0.5
    geo = nt.nodes.new("ShaderNodeNewGeometry")
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(geo.outputs["Position"], sep.inputs[0])
    comb = nt.nodes.new("ShaderNodeCombineXYZ")
    for i, (ax, c) in enumerate((("X", cx), ("Y", cy))):
        sub = nt.nodes.new("ShaderNodeMath"); sub.operation = "SUBTRACT"; sub.inputs[1].default_value = c
        nt.links.new(sep.outputs[ax], sub.inputs[0])
        nt.links.new(sub.outputs[0], comb.inputs[i])
    ln = nt.nodes.new("ShaderNodeVectorMath"); ln.operation = "LENGTH"
    nt.links.new(comb.outputs[0], ln.inputs[0])
    mr = nt.nodes.new("ShaderNodeMapRange")
    mr.inputs["From Min"].default_value = r0
    mr.inputs["From Max"].default_value = r1
    mr.inputs["To Min"].default_value = 0.0
    mr.inputs["To Max"].default_value = density
    mr.interpolation_type = "SMOOTHSTEP"
    nt.links.new(ln.outputs["Value"], mr.inputs["Value"])
    hz = nt.nodes.new("ShaderNodeMapRange")
    hz.inputs["From Min"].default_value = 0
    hz.inputs["From Max"].default_value = zmax
    hz.inputs["To Min"].default_value = 1.0
    hz.inputs["To Max"].default_value = 0.0
    hz.interpolation_type = "SMOOTHSTEP"
    nt.links.new(sep.outputs["Z"], hz.inputs["Value"])
    mul = nt.nodes.new("ShaderNodeMath"); mul.operation = "MULTIPLY"
    nt.links.new(mr.outputs["Result"], mul.inputs[0])
    nt.links.new(hz.outputs["Result"], mul.inputs[1])
    nt.links.new(mul.outputs[0], pv.inputs["Density"])
    nt.links.new(pv.outputs[0], nt.nodes["Material Output"].inputs["Volume"])
    ob.data.materials.append(m)
    ob.visible_shadow = False


cam = [float(v) for v in G("cam").split(",")]
tgt = [float(v) for v in G("tgt").split(",")]
focus = [float(v) for v in A.get("focus", G("tgt")).split(",")]
if float(A.get("hz", 0.006)) > 0:
    far_haze(cam[0] - OX, OZ - cam[2], float(A.get("hz0", 160)), float(A.get("hz1", 420)), float(A.get("hz", 0.006)),
             float(A.get("hzh", 45)), tuple(float(t) for t in A.get("hzc", "1.0,0.80,0.62").split(",")))
camera(P(*cam), P(*tgt), lens=float(G("lens")), fstop=float(A.get("f", 0)) or None, focus=P(*focus),
       shift_x=float(G("sx")), shift_y=float(G("sy")))
bpy.context.scene.camera.data.clip_end = 6000

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(G("exp")), bloom=0.3,
             bloom_threshold=1.2, bloom_size=0.55, time_limit=float(A.get("tl", 240)), volume_bounces=1,
             res=(int(A.get("w", 1920)), int(A.get("h", 1080))))
render(A.get("out", G("out")), T0)
