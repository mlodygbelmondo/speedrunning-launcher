"""mode-ssg.png - a REAL stretch of seed 2026 (Java 1.16.1) as a top-down cartography map.

Orthographic, straight down, north up: x -1360..-610, z -2650..-2218. Coastline and coral reefs, the
shipwreck at (-912, -2608), the ruined portal at (-1200, -2320) and the lava pool at (-1150, -2235).
Blocks are our own textures; plants are left out so the map stays calm (the UI draws a route over it).
Post: a light parchment grade by grade_ssg.py (PIL).
"""
import sys
import time
sys.path.insert(0, "/tmp/srl/art/src")
import bpy
import vox
from vox import *
import mcblocks as mc

T0 = time.time()
A = args()
SAMPLES = int(A.get("samples", 64))
reset()

pal, arr, origin = mc.load("ssg")
x0, y0, z0 = origin
nx, ny, nz = arr.shape
OX, OY, OZ = x0, 62, z0 + nz


def P(x, y, z):
    return (x - OX, OZ - z, y - OY)


b = mc.Builder(pal, arr, origin, OX, OY, OZ, seed=8, fancy_leaves=False, plants=False,
               mat_over={"lava": dict(emit=True, es=float(A.get("les", 1.6)), rough=0.6, base_mul=0.15, spec=0.2),
                         "water": dict(rough=0.08, ior=1.33, alpha_const=float(A.get("wa", 0.5)), spec=0.35)})
b.build_cubes("ssg")
b.build_models("ssg")
print("[ssg] unknown", b.unknown)

# depth colour: absorption + a little blue scatter under the water surface (shallow = turquoise, deep = navy)
bpy.ops.mesh.primitive_cube_add(size=1, location=(nx / 2, nz / 2, (30 - OY + 0.86) / 2))
wv = bpy.context.active_object
wv.name = "WaterVol"
wv.scale = (nx + 2, nz + 2, 0.86 - (30 - OY))
m = bpy.data.materials.new("WaterVol_m")
m.use_nodes = True
nt = m.node_tree
for n in list(nt.nodes):
    if n.type != "OUTPUT_MATERIAL":
        nt.nodes.remove(n)
ab = nt.nodes.new("ShaderNodeVolumeAbsorption")
ab.inputs["Color"].default_value = (0.30, 0.70, 0.82, 1)
ab.inputs["Density"].default_value = float(A.get("wd", 0.25))
sc_ = nt.nodes.new("ShaderNodeVolumeScatter")
sc_.inputs["Color"].default_value = (0.12, 0.38, 0.62, 1)
sc_.inputs["Density"].default_value = float(A.get("ws_", 0.015))
ad = nt.nodes.new("ShaderNodeAddShader")
nt.links.new(ab.outputs[0], ad.inputs[0])
nt.links.new(sc_.outputs[0], ad.inputs[1])
nt.links.new(ad.outputs[0], nt.nodes["Material Output"].inputs["Volume"])
wv.data.materials.append(m)
wv.visible_shadow = False

sun_ob, sdir = add_sun(float(A.get("el", 38)), float(A.get("az", 315)), float(A.get("suns", 3.6)), (1.0, 0.96, 0.9),
                       angle=3.0)
world_gradient([(-1, (0.55, 0.6, 0.66)), (1, (0.62, 0.68, 0.76))], strength=float(A.get("ws", 0.9)))

cd = bpy.data.cameras.new("Cam")
cd.type = "ORTHO"
cd.ortho_scale = nx - 12  # trim the box border
cd.clip_end = 1000
ob = bpy.data.objects.new("Cam", cd)
bpy.context.scene.collection.objects.link(ob)
ob.location = (nx / 2, nz / 2, 160)
bpy.context.scene.camera = ob

setup_render(samples=SAMPLES, look="AgX - Medium High Contrast", exposure=float(A.get("exp", 0.0)), bloom=0.15,
             bloom_threshold=2.0, bloom_size=0.4, time_limit=float(A.get("tl", 195)), volume_bounces=1,
             res=(int(A.get("w", 1920)), int(A.get("h", 1080))))
render(A.get("out", "../work/mode-ssg_raw"), T0)  # graded into out/mode-ssg.png by grade_ssg.py
