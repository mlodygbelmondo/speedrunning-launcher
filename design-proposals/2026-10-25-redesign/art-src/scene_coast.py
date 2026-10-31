"""Overworld coast scenes from REAL seed-2026 (Java 1.16.1) terrain, block for block, our textures.

Used for two key-art images (pick with preset=):
  rsg   mode-rsg.png  - a small ocean island at golden hour, "fresh world, you spawn and look around"
  ssg   mode-ssg.png  - the beached shipwreck of a known seed, on its sand spit by the shore

Everything outside the extracted box is a flat ocean (water plane + dim sea floor) out to the horizon.
"""
import math
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
PRESET = A.get("preset", "rsg")
D = {
    # island x 2067..2152, z 183..271: an S-shaped sand island with one clump of oak and birch, open ocean around
    "rsg": dict(npz="islC", o="2110,62,227", cam="2044,78,276", tgt="2110,65,234", lens="26", sx="0.04", sy="0",
                el="6", az="300", suns="11", sunc="1.0,0.50,0.22", skys="1.0", sky="b", cover="0.12", cseed="11",
                cz="85", fog="0.001", fogh="14", exp="-0.2", wd="0.3", wcol="0.15,0.45,0.7", wa="0.5", out="mode-rsg"),
    # shipwreck rightsideup_backhalf_degraded [1636,57,-1093]..[1651,65,-1085], beached on a sand spit
    "ssg": dict(npz="wreck_b", o="1643,62,-1089", cam="1670,71,-1068", tgt="1642,63,-1089", lens="30", sx="-0.07",
                sy="0.04", el="12", az="235", suns="8", sunc="1.0,0.58,0.32", skys="0.7", sky="d", cover="0.22",
                cseed="7", cz="70", fog="0.0009", fogh="22", exp="0", wd="0.3", wcol="0.15,0.5,0.7", wa="0.55",
                out="mode-ssg"),
}[PRESET]
G = lambda k: A.get(k, D.get(k))
SAMPLES = int(A.get("samples", 128))
reset()

LEAVES = ("oak_leaves", "birch_leaves", "spruce_leaves", "jungle_leaves", "dark_oak_leaves", "acacia_leaves")
if A.get("lalpha", "0") != "1":
    vox.PACKED.update(LEAVES)
pal, arr, origin = mc.load(G("npz"))
x0, y0, z0 = origin
nx, ny, nz = arr.shape
OX, OY, OZ = [int(float(v)) for v in G("o").split(",")]


def P(x, y, z):
    return (x - OX, OZ - z, y - OY)


b = mc.Builder(pal, arr, origin, OX, OY, OZ, seed=int(A.get("seed", 4)),
               mat_over={"water": dict(rough=float(A.get("wr", 0.1)), ior=1.33, alpha_const=float(G("wa")), spec=0.6),
                         **{l: dict(alpha=A.get("lalpha", "0") == "1") for l in LEAVES}})
b.build_cubes("co")
b.build_models("co")
print("[coast] unknown", b.unknown)

# ---------------------------------------------------------------- open ocean around the box
SEA = 62 + 14 / 16
wm = b.mat("water")
fm = material("sand", tint=(0.42, 0.5, 0.55))
mb = vox.MeshBuilder()
bx0, bx1 = x0 - OX, x0 + nx - OX
by0, by1 = OZ - (z0 + nz), OZ - z0
R = 4000
for (ax0, ay0, ax1, ay1) in ((-R, -R, R, by0), (-R, by1, R, R), (-R, by0, bx0, by1), (bx1, by0, R, by1)):
    for zz, m in ((SEA - OY, wm), (float(A.get("floor", 46)) - OY, fm)):
        mb.quad([(ax0, ay0, zz), (ax1, ay0, zz), (ax1, ay1, zz), (ax0, ay1, zz)],
                [(ax0, ay0), (ax1, ay0), (ax1, ay1), (ax0, ay1)], m)
mb.build("ocean")

# water body: absorption + a little blue scatter below the surface (shallows turquoise, deep water blue)
WD = float(G("wd"))
if WD > 0:
    fl = float(A.get("floor", 46)) - OY - 6
    top = SEA - OY - 0.02
    bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, (fl + top) / 2))
    wv = bpy.context.active_object
    wv.name = "WaterVol"
    wv.scale = (2 * R, 2 * R, top - fl)
    m = bpy.data.materials.new("WaterVol_m")
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        if n.type != "OUTPUT_MATERIAL":
            nt.nodes.remove(n)
    ab = nt.nodes.new("ShaderNodeVolumeAbsorption")
    ab.inputs["Color"].default_value = (*[float(t) for t in G("wcol").split(",")], 1)
    ab.inputs["Density"].default_value = WD
    sc_ = nt.nodes.new("ShaderNodeVolumeScatter")
    sc_.inputs["Color"].default_value = (0.10, 0.34, 0.58, 1)
    sc_.inputs["Density"].default_value = float(A.get("wsc", 0.01))
    ad = nt.nodes.new("ShaderNodeAddShader")
    nt.links.new(ab.outputs[0], ad.inputs[0])
    nt.links.new(sc_.outputs[0], ad.inputs[1])
    nt.links.new(ad.outputs[0], nt.nodes["Material Output"].inputs["Volume"])
    wv.data.materials.append(m)
    wv.visible_shadow = False

# ---------------------------------------------------------------- sky, sun, clouds, haze
SUN_EL, SUN_AZ = float(G("el")), float(G("az"))
sunc = tuple(float(t) for t in G("sunc").split(","))
sun_ob, sdir = add_sun(SUN_EL, SUN_AZ, float(G("suns")), sunc, angle=float(A.get("sa", 2.0)))
SKIES = {
    "a": [(-1.0, (0.10, 0.08, 0.07)), (-0.02, (0.9, 0.46, 0.22)), (0.015, (1.0, 0.60, 0.30)),
          (0.07, (1.0, 0.64, 0.42)), (0.17, (0.66, 0.54, 0.72)), (0.34, (0.28, 0.38, 0.82)), (1.0, (0.08, 0.17, 0.56))],
    "b": [(-1.0, (0.06, 0.06, 0.08)), (-0.02, (0.85, 0.42, 0.20)), (0.0, (1.0, 0.55, 0.25)), (0.04, (1.0, 0.62, 0.36)),
          (0.10, (0.82, 0.56, 0.55)), (0.18, (0.42, 0.46, 0.80)), (0.35, (0.20, 0.32, 0.75)), (1.0, (0.06, 0.14, 0.50))],
    "c": [(-1.0, (0.06, 0.07, 0.10)), (-0.02, (0.70, 0.62, 0.58)), (0.0, (0.95, 0.78, 0.60)), (0.05, (0.78, 0.80, 0.86)),
          (0.14, (0.46, 0.62, 0.92)), (0.35, (0.24, 0.44, 0.88)), (1.0, (0.10, 0.26, 0.72))],
    "d": [(-1.0, (0.12, 0.12, 0.12)), (-0.02, (0.80, 0.74, 0.66)), (0.02, (0.92, 0.84, 0.74)),
          (0.10, (0.72, 0.78, 0.90)), (0.35, (0.40, 0.56, 0.88)), (1.0, (0.18, 0.34, 0.74))],
}
SKY = SKIES[G("sky")]
glow = (1.0, 0.58, 0.26) if PRESET == "rsg" else (1.0, 0.75, 0.45)
ccol = "cloud_warm"
world_gradient(SKY, strength=float(G("skys")), sun_glow=(sdir, glow, float(A.get("gs", 4.0)), float(A.get("gw", 1.1))))
cloud_layer(int(G("cseed")), -900, 900, -900, 900, float(G("cz")), cell=6, thick=2.5,
            cover=float(G("cover")), block=ccol)
fog_gradient_box((0, 0, 10), (1400, 1400, 50), float(G("fog")), z0=-2, z1=float(G("fogh")),
                 color=tuple(float(t) for t in A.get("fogc", "1.0,0.84,0.68").split(",")), anisotropy=0.55)

cam = [float(v) for v in G("cam").split(",")]
tgt = [float(v) for v in G("tgt").split(",")]
focus = [float(v) for v in A.get("focus", G("tgt")).split(",")]
camera(P(*cam), P(*tgt), lens=float(G("lens")), fstop=float(A.get("f", 0)) or None, focus=P(*focus),
       shift_x=float(G("sx")), shift_y=float(G("sy")))
bpy.context.scene.camera.data.clip_end = 6000

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(G("exp")), bloom=0.35,
             bloom_threshold=1.2, bloom_size=0.55, time_limit=float(A.get("tl", 195)), volume_bounces=1,
             res=(int(A.get("w", 1920)), int(A.get("h", 1080))))
render(A.get("out", G("out")), T0)
