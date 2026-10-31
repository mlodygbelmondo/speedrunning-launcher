"""Quick look at an extracted box: plain daylight, orthographic top-down or a given camera.

blender -b -P mcoverview.py -- npz=fortress cut=90 w=1200 h=1200 [cam=x,y,z tgt=x,y,z lens=..] out=../work/t/name
(MC coordinates for cam/tgt.)
"""
import sys
import time
sys.path.insert(0, "/tmp/srl/art/src")
import vox
from vox import *
import mcblocks as mc
import bpy

T0 = time.time()
A = args()
reset()
pal, arr, origin = mc.load(A["npz"])
x0, y0, z0 = origin
nx, ny, nz = arr.shape
OX, OY, OZ = x0, y0, z0 + nz
cut = int(A.get("cut", y0 + ny)) - y0
skip = A.get("skip", "").split(",") if A.get("skip") else ()
if A.get("only"):  # keep only block names containing one of these substrings
    keep = A["only"].split(",")
    skip = {mc.parse(p)[0] for p in pal if not any(k in p for k in keep)}
b = mc.Builder(pal, arr, origin, OX, OY, OZ, plants=A.get("plants", "1") == "1", skip=skip)
b.build_cubes("ov", top_cut=cut)
if A.get("models", "1") == "1":
    b.build_models("ov", top_cut=cut)
print("[ov] unknown", b.unknown)
world_gradient([(-1, (0.3, 0.3, 0.3)), (1, (0.5, 0.55, 0.6))], strength=float(A.get("ws", 1.0)))
add_sun(55, 135, float(A.get("sun", 3.5)), (1, 0.97, 0.92), angle=2)


def P(x, y, z):
    return (x - OX, OZ - z, y - OY)


if "cam" in A:
    cam = [float(v) for v in A["cam"].split(",")]
    tgt = [float(v) for v in A["tgt"].split(",")]
    camera(P(*cam), P(*tgt), lens=float(A.get("lens", 30)))
else:
    cd = bpy.data.cameras.new("Cam")
    cd.type = "ORTHO"
    cd.ortho_scale = max(nx, nz)
    cd.clip_end = 2000
    ob = bpy.data.objects.new("Cam", cd)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = (nx / 2, nz / 2, ny + 50)
    bpy.context.scene.camera = ob
setup_render(samples=int(A.get("samples", 16)), exposure=float(A.get("exp", 0)), time_limit=60,
             res=(int(A.get("w", 1000)), int(A.get("h", 1000))))
bpy.context.scene.cycles.use_denoising = True
render(A.get("out", "../work/t/overview"), T0)
