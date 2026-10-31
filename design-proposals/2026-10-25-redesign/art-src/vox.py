"""Shared voxel scene toolkit for Blender (Cycles).

Blocks are unit cubes on an integer grid (z up). Faces between opaque blocks are
culled, so there are no coincident faces and no z-fighting. Every face gets the
full 0..1 UV square of its 16x16 texture, sampled with Closest interpolation.
"""
import bpy
import math
import os
import sys
import time
import numpy as np
from mathutils import Vector, Euler

TEX = "/tmp/srl/art/out/textures"
OUT = "/tmp/srl/art/out"


# ------------------------------------------------------------------ noise
def fbm2(w, h, seed, scale=32.0, octaves=4, persistence=0.5):
    """Smooth 2D fBm value noise (not tileable), returns array [h, w] in ~0..1."""
    r = np.random.default_rng(seed)
    out = np.zeros((h, w))
    amp, tot = 1.0, 0.0
    for o in range(octaves):
        cell = scale / (2 ** o)
        gw, gh = int(w / cell) + 3, int(h / cell) + 3
        g = r.random((gh, gw))
        y = np.arange(h) / cell
        x = np.arange(w) / cell
        y0 = np.floor(y).astype(int)
        x0 = np.floor(x).astype(int)
        fy, fx = y - y0, x - x0
        fy = fy * fy * (3 - 2 * fy)
        fx = fx * fx * (3 - 2 * fx)
        a = g[y0][:, x0]
        b = g[y0][:, x0 + 1]
        c = g[y0 + 1][:, x0]
        d = g[y0 + 1][:, x0 + 1]
        FX, FY = fx[None, :], fy[:, None]
        out += amp * ((a * (1 - FX) + b * FX) * (1 - FY) + (c * (1 - FX) + d * FX) * FY)
        tot += amp
        amp *= persistence
    return out / tot


# ------------------------------------------------------------------ scene
def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    sc = bpy.context.scene
    sc.render.engine = "CYCLES"
    return sc


_IMG = {}


PACKED = set()  # textures whose colour must survive alpha 0 (used opaque): load channel-packed


def image(name, data=False):
    if name in _IMG:
        return _IMG[name]
    im = bpy.data.images.load(f"{TEX}/{name}.png", check_existing=True)
    if data:
        im.colorspace_settings.name = "Non-Color"
    if name in PACKED:
        im.alpha_mode = "CHANNEL_PACKED"
    _IMG[name] = im
    return im


def _tex_node(nt, name, data=False, loc=(-600, 0)):
    n = nt.nodes.new("ShaderNodeTexImage")
    n.image = image(name, data)
    n.interpolation = "Closest"
    n.extension = "REPEAT"
    n.location = loc
    return n


# material properties per texture
MATPROPS = {
    "water": dict(rough=0.04, ior=1.33, alpha_const=0.78, spec=0.6),
    "lava": dict(emit=True, es=2.2, rough=0.6, base_mul=0.15, spec=0.2),
    "magma": dict(emit_mask="magma_e", es=6.0),
    "crying_obsidian": dict(emit_mask="crying_obsidian_e", es=4.0, rough=0.28, spec=0.6),
    "obsidian": dict(rough=0.3, spec=0.6),
    "gold_block": dict(metal=1.0, rough=0.32),
    "gilded_blackstone": dict(metal_mask="gilded_blackstone_m", rough=0.7),
    "fire": dict(alpha=True, emit=True, es=3.2, shadow=False),
    "fire_b": dict(alpha=True, emit=True, es=3.2, shadow=False),
    "soul_fire": dict(alpha=True, emit=True, es=2.6, shadow=False),
    "nether_portal": dict(emit=True, es=6.0, alpha_const=0.88, shadow=False),
    "glowstone": dict(emit=True, es=5.0),
    "end_crystal": dict(emit=True, es=10.0, shadow=False, emit_tint=(1.0, 0.55, 0.95)),
    "crystal_cage": dict(alpha=True, emit=True, es=1.5, shadow=False),
    "oak_leaves": dict(alpha=True, rough=0.8, sss=True),
    "tall_grass": dict(alpha=True, rough=0.8),
    "flower_red": dict(alpha=True),
    "flower_yellow": dict(alpha=True),
    "nether_wart": dict(alpha=True),
    "iron_bars": dict(alpha=True, metal=0.8, rough=0.45),
    "eye_of_ender": dict(rough=0.25, spec=0.7, emit=True, es=0.6),
    "end_stone": dict(rough=0.9),
    "cloud": dict(rough=1.0, spec=0.0),
}

_MAT = {}
_ALPHA = {}


def png_has_alpha(path):
    """PNG colour type 4/6 => has an alpha channel (our textures are RGB unless they need cutout)."""
    if path not in _ALPHA:
        try:
            with open(path, "rb") as f:
                h = f.read(26)
            _ALPHA[path] = h[25] in (4, 6)
        except OSError:
            _ALPHA[path] = False
    return _ALPHA[path]


def material(tex, **override):
    key = tex + (str(sorted(override.items())) if override else "")
    if key in _MAT:
        return _MAT[key]
    p = dict(rough=0.85, spec=0.35, metal=0.0)
    if png_has_alpha(f"{TEX}/{tex}.png"):
        p["alpha"] = True
    p.update(MATPROPS.get(tex, {}))
    p.update(override)
    m = bpy.data.materials.new(key)
    m.use_nodes = True
    nt = m.node_tree
    bs = nt.nodes["Principled BSDF"]
    t = _tex_node(nt, tex)
    if "tint" in p:  # multiply base colour (biome tints, generic colours)
        tm = nt.nodes.new("ShaderNodeMix"); tm.data_type = "RGBA"; tm.blend_type = "MULTIPLY"
        tm.inputs["Factor"].default_value = 1.0
        nt.links.new(t.outputs["Color"], tm.inputs[6]); tm.inputs[7].default_value = (*p["tint"], 1)
        nt.links.new(tm.outputs[2], bs.inputs["Base Color"])
    elif "base_mul" in p:  # darken diffuse (emissive fluids should not also reflect light)
        mm0 = nt.nodes.new("ShaderNodeMix"); mm0.data_type = "RGBA"; mm0.blend_type = "MULTIPLY"
        mm0.inputs["Factor"].default_value = 1.0
        nt.links.new(t.outputs["Color"], mm0.inputs[6])
        mm0.inputs[7].default_value = (p["base_mul"],) * 3 + (1,)
        nt.links.new(mm0.outputs[2], bs.inputs["Base Color"])
    else:
        nt.links.new(t.outputs["Color"], bs.inputs["Base Color"])
    bs.inputs["Roughness"].default_value = p["rough"]
    bs.inputs["Specular IOR Level"].default_value = p["spec"]
    if "ior" in p:
        bs.inputs["IOR"].default_value = p["ior"]
    bs.inputs["Metallic"].default_value = p["metal"]
    if "metal_mask" in p:
        mm = _tex_node(nt, p["metal_mask"], data=True, loc=(-600, -300))
        nt.links.new(mm.outputs["Color"], bs.inputs["Metallic"])
        mix = nt.nodes.new("ShaderNodeMapRange")
        mix.inputs["To Min"].default_value = p["rough"]
        mix.inputs["To Max"].default_value = 0.3
        nt.links.new(mm.outputs["Color"], mix.inputs["Value"])
        nt.links.new(mix.outputs["Result"], bs.inputs["Roughness"])
    if "emit_color" in p:
        bs.inputs["Emission Color"].default_value = (*p["emit_color"], 1)
        bs.inputs["Emission Strength"].default_value = p["es"]
    elif p.get("emit") or "emit_mask" in p:
        if "emit_tint" in p:
            et = nt.nodes.new("ShaderNodeMix"); et.data_type = "RGBA"; et.blend_type = "MULTIPLY"
            et.inputs["Factor"].default_value = 1.0
            nt.links.new(t.outputs["Color"], et.inputs[6]); et.inputs[7].default_value = (*p["emit_tint"], 1)
            nt.links.new(et.outputs[2], bs.inputs["Emission Color"])
        else:
            nt.links.new(t.outputs["Color"], bs.inputs["Emission Color"])
        if "emit_mask" in p:
            em = _tex_node(nt, p["emit_mask"], data=True, loc=(-600, -600))
            mul = nt.nodes.new("ShaderNodeMath")
            mul.operation = "MULTIPLY"
            mul.inputs[1].default_value = p["es"]
            nt.links.new(em.outputs["Color"], mul.inputs[0])
            nt.links.new(mul.outputs[0], bs.inputs["Emission Strength"])
        else:
            bs.inputs["Emission Strength"].default_value = p["es"]
    if p.get("alpha"):
        nt.links.new(t.outputs["Alpha"], bs.inputs["Alpha"])
    if "alpha_const" in p:
        bs.inputs["Alpha"].default_value = p["alpha_const"]
    if p.get("sss"):
        bs.inputs["Subsurface Weight"].default_value = 0.0
    if p.get("shadow") is False:
        # do not cast shadows (fire, portal sheets, crystals) -> cleaner light
        m["noshadow"] = True
    _MAT[key] = m
    return m


def emission_material(name, color, strength):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        if n.type != "OUTPUT_MATERIAL":
            nt.nodes.remove(n)
    e = nt.nodes.new("ShaderNodeEmission")
    e.inputs["Color"].default_value = (*color, 1)
    e.inputs["Strength"].default_value = strength
    nt.links.new(e.outputs[0], nt.nodes["Material Output"].inputs["Surface"])
    return m


STAR_GAIN = [6.0]
STAR_BASE = [3.0]


def end_portal_material():
    """Starfield with parallax layers (fake depth) for the end portal surface."""
    if "end_portal_sf" in _MAT:
        return _MAT["end_portal_sf"]
    m = bpy.data.materials.new("end_portal_sf")
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        if n.type != "OUTPUT_MATERIAL":
            nt.nodes.remove(n)
    out = nt.nodes["Material Output"]
    geo = nt.nodes.new("ShaderNodeNewGeometry")
    sep_i = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(geo.outputs["Incoming"], sep_i.inputs[0])
    # 1 / max(I.z, 0.25)
    mx = nt.nodes.new("ShaderNodeMath"); mx.operation = "MAXIMUM"; mx.inputs[1].default_value = 0.25
    nt.links.new(sep_i.outputs["Z"], mx.inputs[0])
    inv = nt.nodes.new("ShaderNodeMath"); inv.operation = "DIVIDE"; inv.inputs[0].default_value = 1.0
    nt.links.new(mx.outputs[0], inv.inputs[1])
    total = None
    layers = [  # scale, depth, tint, strength, rotation
        (1.0, 0.0, (0.45, 1.0, 0.85), 1.0, 0.0),
        (1.5, 0.7, (0.25, 0.9, 0.6), 0.75, 0.6),
        (2.2, 1.6, (0.3, 0.55, 1.0), 0.6, 1.3),
        (3.1, 3.0, (0.55, 0.85, 1.0), 0.45, 2.1),
    ]
    tex_img = image("end_portal")
    mask_img = image("end_portal_e", data=True)
    for i, (sc, depth, tint, st, rot) in enumerate(layers):
        # uv = pos.xy * sc + I.xy * depth / I.z   (rotated)
        off = nt.nodes.new("ShaderNodeVectorMath"); off.operation = "SCALE"
        nt.links.new(geo.outputs["Incoming"], off.inputs[0])
        dmul = nt.nodes.new("ShaderNodeMath"); dmul.operation = "MULTIPLY"; dmul.inputs[1].default_value = -depth
        nt.links.new(inv.outputs[0], dmul.inputs[0])
        nt.links.new(dmul.outputs[0], off.inputs["Scale"])
        ps = nt.nodes.new("ShaderNodeVectorMath"); ps.operation = "SCALE"; ps.inputs["Scale"].default_value = sc
        nt.links.new(geo.outputs["Position"], ps.inputs[0])
        off2 = nt.nodes.new("ShaderNodeVectorMath"); off2.operation = "SCALE"; off2.inputs["Scale"].default_value = sc
        nt.links.new(off.outputs[0], off2.inputs[0])
        add = nt.nodes.new("ShaderNodeVectorMath"); add.operation = "ADD"
        nt.links.new(ps.outputs[0], add.inputs[0]); nt.links.new(off2.outputs[0], add.inputs[1])
        rotn = nt.nodes.new("ShaderNodeVectorRotate"); rotn.rotation_type = "Z_AXIS"; rotn.inputs["Angle"].default_value = rot
        nt.links.new(add.outputs[0], rotn.inputs["Vector"])
        t = nt.nodes.new("ShaderNodeTexImage"); t.image = tex_img; t.interpolation = "Closest"
        tm = nt.nodes.new("ShaderNodeTexImage"); tm.image = mask_img; tm.interpolation = "Closest"
        nt.links.new(rotn.outputs[0], t.inputs[0]); nt.links.new(rotn.outputs[0], tm.inputs[0])
        cm = nt.nodes.new("ShaderNodeMix"); cm.data_type = "RGBA"; cm.blend_type = "MULTIPLY"
        cm.inputs["Factor"].default_value = 1.0
        nt.links.new(t.outputs["Color"], cm.inputs[6]); cm.inputs[7].default_value = (*tint, 1)
        sm = nt.nodes.new("ShaderNodeVectorMath"); sm.operation = "SCALE"
        nt.links.new(cm.outputs[2], sm.inputs[0])
        k = nt.nodes.new("ShaderNodeMath"); k.operation = "MULTIPLY"; k.inputs[1].default_value = st * STAR_GAIN[0]
        nt.links.new(tm.outputs["Color"], k.inputs[0])
        nt.links.new(k.outputs[0], sm.inputs["Scale"])
        if total is None:
            total = sm
        else:
            a2 = nt.nodes.new("ShaderNodeVectorMath"); a2.operation = "ADD"
            nt.links.new(total.outputs[0], a2.inputs[0]); nt.links.new(sm.outputs[0], a2.inputs[1])
            total = a2
        if i == 0:  # dark teal base so the surface reads as a deep void
            base = nt.nodes.new("ShaderNodeVectorMath"); base.operation = "SCALE"; base.inputs["Scale"].default_value = STAR_BASE[0]
            nt.links.new(t.outputs["Color"], base.inputs[0])
            a3 = nt.nodes.new("ShaderNodeVectorMath"); a3.operation = "ADD"
            nt.links.new(total.outputs[0], a3.inputs[0]); nt.links.new(base.outputs[0], a3.inputs[1])
            total = a3
    em = nt.nodes.new("ShaderNodeEmission")
    nt.links.new(total.outputs[0], em.inputs["Color"])
    em.inputs["Strength"].default_value = 1.0
    nt.links.new(em.outputs[0], out.inputs["Surface"])
    _MAT["end_portal_sf"] = m
    return m


# ------------------------------------------------------------------ blocks
# block -> (top, side, bottom, flags)
BLOCKS = {
    "grass": ("grass_top", "grass_side", "dirt", {"rot_top": True}),
    "dirt": ("dirt", "dirt", "dirt", {"rot_top": True}),
    "stone": ("stone", "stone", "stone", {}),
    "deepslate": ("deepslate", "deepslate", "deepslate", {}),
    "sand": ("sand", "sand", "sand", {"rot_top": True}),
    "water": ("water", "water", "water", {"fluid": True}),
    "oak_log": ("oak_log_top", "oak_log", "oak_log_top", {}),
    "oak_leaves": ("oak_leaves", "oak_leaves", "oak_leaves", {"leaves": True, "rot_top": True}),
    "obsidian": ("obsidian", "obsidian", "obsidian", {"rot_top": True}),
    "crying_obsidian": ("crying_obsidian", "crying_obsidian", "crying_obsidian", {}),
    "netherrack": ("netherrack", "netherrack", "netherrack", {"rot_top": True}),
    "nether_bricks": ("nether_bricks", "nether_bricks", "nether_bricks", {}),
    "blackstone": ("blackstone_top", "blackstone", "blackstone_top", {"rot_top": True}),
    "pbb": ("polished_blackstone_bricks",) * 3 + ({},),
    "gilded": ("gilded_blackstone",) * 3 + ({},),
    "gold": ("gold_block",) * 3 + ({},),
    "basalt": ("basalt_top", "basalt_side", "basalt_top", {}),
    "soul_sand": ("soul_sand",) * 3 + ({"rot_top": True},),
    "lava": ("lava",) * 3 + ({"fluid": True, "rot_top": True},),
    "magma": ("magma",) * 3 + ({"rot_top": True},),
    "glowstone": ("glowstone",) * 3 + ({"rot_top": True},),
    "stone_bricks": ("stone_bricks",) * 3 + ({},),
    "mossy_stone_bricks": ("mossy_stone_bricks",) * 3 + ({},),
    "cracked_stone_bricks": ("cracked_stone_bricks",) * 3 + ({},),
    "end_stone": ("end_stone",) * 3 + ({"rot_top": True},),
    "bedrock": ("bedrock",) * 3 + ({"rot_top": True},),
    "gravel": ("stone",) * 3 + ({},),
    "cloud": ("cloud",) * 3 + ({},),
    "cloud_warm": ("cloud",) * 3 + ({"mat": dict(emit_color=(1.0, 0.62, 0.48), es=0.42)},),
}

# direction: (offset, corner list (local unit cube), uv function index)
FACES = {
    "px": ((1, 0, 0), [(1, 0, 0), (1, 1, 0), (1, 1, 1), (1, 0, 1)]),
    "nx": ((-1, 0, 0), [(0, 1, 0), (0, 0, 0), (0, 0, 1), (0, 1, 1)]),
    "py": ((0, 1, 0), [(1, 1, 0), (0, 1, 0), (0, 1, 1), (1, 1, 1)]),
    "ny": ((0, -1, 0), [(0, 0, 0), (1, 0, 0), (1, 0, 1), (0, 0, 1)]),
    "pz": ((0, 0, 1), [(0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)]),
    "nz": ((0, 0, -1), [(0, 1, 0), (1, 1, 0), (1, 0, 0), (0, 0, 0)]),
}


def face_uv(d, p):
    """UV for local point p (0..1 cube coords) on face d, unmirrored from outside."""
    x, y, z = p
    if d == "px":
        return (y, z)
    if d == "nx":
        return (1 - y, z)
    if d == "py":
        return (1 - x, z)
    if d == "ny":
        return (x, z)
    if d == "pz":
        return (x, y)
    return (x, 1 - y)


class MeshBuilder:
    """Accumulates quads per material, then builds one mesh object."""

    def __init__(self):
        self.verts, self.uvs, self.mats = [], [], []
        self.mat_index = {}
        self.mat_list = []

    def mi(self, mat):
        if mat.name not in self.mat_index:
            self.mat_index[mat.name] = len(self.mat_list)
            self.mat_list.append(mat)
        return self.mat_index[mat.name]

    def quad(self, pts, uvs, mat):
        self.verts.extend(pts)
        self.uvs.extend(uvs)
        self.mats.append(self.mi(mat))

    def build(self, name, smooth=False):
        if not self.mats:
            return None
        nv = len(self.verts)
        nf = nv // 4
        me = bpy.data.meshes.new(name)
        me.vertices.add(nv)
        me.vertices.foreach_set("co", np.asarray(self.verts, np.float32).ravel())
        me.loops.add(nv)
        me.loops.foreach_set("vertex_index", np.arange(nv, dtype=np.int32))
        me.polygons.add(nf)
        me.polygons.foreach_set("loop_start", np.arange(0, nv, 4, dtype=np.int32))
        uvl = me.uv_layers.new(name="UVMap")
        uvl.data.foreach_set("uv", np.asarray(self.uvs, np.float32).ravel())
        for m in self.mat_list:
            me.materials.append(m)
        me.polygons.foreach_set("material_index", np.asarray(self.mats, np.int32))
        me.update(calc_edges=True)
        me.validate()
        # merge coincident verts so the mesh is watertight-ish (keeps UVs per-loop)
        ob = bpy.data.objects.new(name, me)
        bpy.context.scene.collection.objects.link(ob)
        return ob


class World:
    def __init__(self, seed=0):
        self.v = {}
        self.rng = np.random.default_rng(seed)
        self.extra = MeshBuilder()  # boxes / crosses / special geometry
        self.glow = MeshBuilder()   # geometry that must not cast shadows

    def set(self, x, y, z, b):
        if b is None:
            self.v.pop((x, y, z), None)
        else:
            self.v[(int(x), int(y), int(z))] = b

    def get(self, x, y, z):
        return self.v.get((x, y, z))

    def fill(self, x0, y0, z0, x1, y1, z1, b):
        for x in range(min(x0, x1), max(x0, x1) + 1):
            for y in range(min(y0, y1), max(y0, y1) + 1):
                for z in range(min(z0, z1), max(z0, z1) + 1):
                    self.set(x, y, z, b)

    # -- geometry
    def _opaque(self, b):
        if b is None:
            return False
        f = BLOCKS[b][3]
        return not (f.get("fluid") or f.get("leaves"))

    def build(self, name="voxels"):
        mb = MeshBuilder()
        mats = {}
        r = self.rng
        for (x, y, z), b in self.v.items():
            top, side, bottom, fl = BLOCKS[b]
            fluid = fl.get("fluid")
            leaves = fl.get("leaves")
            for d, (off, corners) in FACES.items():
                if fluid and d == "nz":
                    continue
                nb = self.v.get((x + off[0], y + off[1], z + off[2]))
                if nb is not None:
                    if self._opaque(nb):
                        continue
                    if nb == b and (fluid or leaves):
                        continue
                    if fluid and nb is not None and not BLOCKS[nb][3].get("fluid") and not BLOCKS[nb][3].get("leaves"):
                        continue
                tex = top if d == "pz" else bottom if d == "nz" else side
                mk = (tex, b)
                if mk not in mats:
                    mats[mk] = material(tex, **fl.get("mat", {}))
                h = 1.0
                if fluid and self.v.get((x, y, z + 1)) != b:
                    h = 0.875
                pts, uvs = [], []
                rot = int(r.integers(0, 4)) if (d in ("pz", "nz") and fl.get("rot_top")) else 0
                for c in corners:
                    lp = (c[0], c[1], c[2] * h)
                    u, vv = face_uv(d, lp)
                    for _ in range(rot):
                        u, vv = 1 - vv, u
                    pts.append((x + lp[0], y + lp[1], z + lp[2]))
                    uvs.append((u, vv))
                mb.quad(pts, uvs, mats[mk])
        ob = mb.build(name)
        print(f"[vox] {name}: {len(self.v)} voxels, {len(mb.mats)} faces")
        ex = self.extra.build(name + "_extra")
        gl = self.glow.build(name + "_glow")
        if gl:
            gl.visible_shadow = False
        return ob, ex

    # -- special pieces into self.extra
    def box(self, p0, p1, tex_top, tex_side=None, tex_bottom=None, skip=(), uv_cell=None):
        """Axis-aligned box from p0 to p1 (world coords). UVs come from the
        position inside the unit cell (uv_cell = integer cell origin)."""
        tex_side = tex_side or tex_top
        tex_bottom = tex_bottom or tex_top
        x0, y0, z0 = p0
        x1, y1, z1 = p1
        cell = uv_cell or (math.floor(x0 + 1e-6), math.floor(y0 + 1e-6), math.floor(z0 + 1e-6))
        for d, (off, corners) in FACES.items():
            if d in skip:
                continue
            tex = tex_top if d == "pz" else tex_bottom if d == "nz" else tex_side
            m = tex if isinstance(tex, bpy.types.Material) else material(tex)
            pts, uvs = [], []
            for c in corners:
                wp = (x0 + (x1 - x0) * c[0], y0 + (y1 - y0) * c[1], z0 + (z1 - z0) * c[2])
                lp = (wp[0] - cell[0], wp[1] - cell[1], wp[2] - cell[2])
                pts.append(wp)
                uvs.append(face_uv(d, lp))
            self._mb(m).quad(pts, uvs, m)

    def _mb(self, m):
        return self.glow if m.get("noshadow") else self.extra

    def cross(self, x, y, z, tex, size=1.0, height=1.0, rot=None):
        """Two crossed vertical quads (plants, fire)."""
        m = material(tex)
        a = self.rng.uniform(0, math.pi / 2) if rot is None else rot
        cx, cy = x + 0.5, y + 0.5
        for k in range(2):
            ang = a + k * math.pi / 2
            dx, dy = math.cos(ang) * size * 0.5 * 1.414, math.sin(ang) * size * 0.5 * 1.414
            pts = [(cx - dx, cy - dy, z), (cx + dx, cy + dy, z), (cx + dx, cy + dy, z + height), (cx - dx, cy - dy, z + height)]
            uvs = [(0, 0), (1, 0), (1, 1), (0, 1)]
            self._mb(m).quad(pts, uvs, m)

    def obox(self, c, size, R, mat, uvscale=None):
        """Oriented box: centre c, size (sx, sy, sz), rotation matrix R (mathutils.Matrix 3x3)."""
        sx, sy, sz = size[0] / 2, size[1] / 2, size[2] / 2
        C = Vector(c)
        for d, (off, corners) in FACES.items():
            pts, uvs = [], []
            for k in corners:
                lp = Vector(((k[0] * 2 - 1) * sx, (k[1] * 2 - 1) * sy, (k[2] * 2 - 1) * sz))
                pts.append(tuple(C + R @ lp))
                u, vv = face_uv(d, (k[0], k[1], k[2]))
                if uvscale:
                    u, vv = uvscale[0] + u * uvscale[1], vv * uvscale[2]
                uvs.append((u, vv))
            self._mb(mat).quad(pts, uvs, mat)

    def quad_plane(self, pts, mat, uvs=((0, 0), (1, 0), (1, 1), (0, 1))):
        self._mb(mat).quad(pts, list(uvs), mat)


def cloud_layer(seed, x0, x1, y0, y1, z, cell=6.0, thick=3.0, cover=0.42, name="clouds", block="cloud"):
    """Blocky cloud deck: a coarse voxel layer, scaled up (cell x cell x thick)."""
    cw = World(seed)
    nx, ny = int((x1 - x0) / cell), int((y1 - y0) / cell)
    n = fbm2(nx, ny, seed, scale=5, octaves=2)
    thr = np.quantile(n, 1 - cover)
    for j in range(ny):
        for i in range(nx):
            if n[j, i] > thr:
                cw.set(i, j, 0, block)
    ob, _ = cw.build(name)
    ob.scale = (cell, cell, thick)
    ob.location = (x0, y0, z)
    ob.visible_shadow = False
    return ob


# ------------------------------------------------------------------ lights / camera
def sun(rot_deg, strength, color, angle_deg=1.5, name="Sun"):
    ld = bpy.data.lights.new(name, "SUN")
    ld.energy = strength
    ld.color = color
    ld.angle = math.radians(angle_deg)
    ob = bpy.data.objects.new(name, ld)
    ob.rotation_euler = Euler([math.radians(a) for a in rot_deg])
    bpy.context.scene.collection.objects.link(ob)
    return ob


def sun_dir(elev_deg, azim_deg):
    """Euler for a sun lamp shining FROM direction (elev, azim). azim 0 = +Y, 90 = +X."""
    # a sun lamp points along its local -Z.
    el, az = math.radians(elev_deg), math.radians(azim_deg)
    d = Vector((math.sin(az) * math.cos(el), math.cos(az) * math.cos(el), math.sin(el)))  # towards the sun
    q = (-d).to_track_quat("-Z", "Y")
    return q.to_euler(), d


def add_sun(elev, azim, strength, color, angle=1.5):
    e, d = sun_dir(elev, azim)
    ob = sun((0, 0, 0), strength, color, angle)
    ob.rotation_euler = e
    return ob, d


def point(loc, energy, color, radius=0.1, name="Pt", shadow_soft=True):
    ld = bpy.data.lights.new(name, "POINT")
    ld.energy = energy
    ld.color = color
    ld.shadow_soft_size = radius
    ob = bpy.data.objects.new(name, ld)
    ob.location = loc
    bpy.context.scene.collection.objects.link(ob)
    return ob


def area(loc, rot, size, energy, color, shape="RECTANGLE", size_y=None, name="Area"):
    ld = bpy.data.lights.new(name, "AREA")
    ld.energy = energy
    ld.color = color
    ld.shape = shape
    ld.size = size
    if size_y is not None:
        ld.size_y = size_y
    ob = bpy.data.objects.new(name, ld)
    ob.location = loc
    ob.rotation_euler = Euler([math.radians(a) for a in rot])
    bpy.context.scene.collection.objects.link(ob)
    return ob


def camera(loc, target, lens=35, fstop=None, focus=None, shift_x=0.0, shift_y=0.0):
    cd = bpy.data.cameras.new("Cam")
    cd.lens = lens
    cd.sensor_width = 36
    cd.shift_x = shift_x
    cd.shift_y = shift_y
    cd.clip_start = 0.1
    cd.clip_end = 2000
    ob = bpy.data.objects.new("Cam", cd)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = loc
    d = Vector(target) - Vector(loc)
    ob.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()
    if fstop:
        cd.dof.use_dof = True
        cd.dof.aperture_fstop = fstop
        cd.dof.focus_distance = (Vector(focus or target) - Vector(loc)).length
    bpy.context.scene.camera = ob
    return ob


# ------------------------------------------------------------------ world
def world_nodes():
    w = bpy.data.worlds.new("World")
    bpy.context.scene.world = w
    w.use_nodes = True
    nt = w.node_tree
    for n in list(nt.nodes):
        if n.type != "OUTPUT_WORLD":
            nt.nodes.remove(n)
    return w, nt, nt.nodes["World Output"]


def world_gradient(stops, strength=1.0, stars=None, sun_glow=None, nebula=None):
    """Vertical gradient sky. stops: list of (z_dir in -1..1, rgb). Optional square
    stars (density, size, brightness) and a sun glow (dir, rgb, power, strength)."""
    w, nt, out = world_nodes()
    tc = nt.nodes.new("ShaderNodeTexCoord")
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(tc.outputs["Generated"], sep.inputs[0])
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    mr = nt.nodes.new("ShaderNodeMapRange")
    mr.inputs["From Min"].default_value = -1
    mr.inputs["From Max"].default_value = 1
    nt.links.new(sep.outputs["Z"], mr.inputs["Value"])
    nt.links.new(mr.outputs["Result"], ramp.inputs["Fac"])
    el = ramp.color_ramp.elements
    for i, (z, c) in enumerate(stops):
        pos = (z + 1) / 2
        if i < len(el):
            el[i].position = pos
            el[i].color = (*c, 1)
        else:
            e = el.new(pos)
            e.color = (*c, 1)
    ramp.color_ramp.interpolation = "EASE"
    col = ramp.outputs["Color"]
    if nebula:  # large-scale brightness / tint variation across the sky
        scale, amount, tint = nebula
        nz = nt.nodes.new("ShaderNodeTexNoise")
        nz.inputs["Scale"].default_value = scale
        nz.inputs["Detail"].default_value = 4.0
        nz.inputs["Roughness"].default_value = 0.6
        nt.links.new(tc.outputs["Generated"], nz.inputs["Vector"])
        mr2 = nt.nodes.new("ShaderNodeMapRange")
        mr2.inputs["From Min"].default_value = 0.35; mr2.inputs["From Max"].default_value = 0.7
        mr2.inputs["To Min"].default_value = 1 - amount; mr2.inputs["To Max"].default_value = 1 + amount
        nt.links.new(nz.outputs["Fac"], mr2.inputs["Value"])
        tmix = nt.nodes.new("ShaderNodeMix"); tmix.data_type = "RGBA"; tmix.blend_type = "MIX"
        nt.links.new(nz.outputs["Fac"], tmix.inputs["Factor"])
        nt.links.new(col, tmix.inputs[6])
        tm2 = nt.nodes.new("ShaderNodeMix"); tm2.data_type = "RGBA"; tm2.blend_type = "MULTIPLY"
        tm2.inputs["Factor"].default_value = 1.0
        nt.links.new(col, tm2.inputs[6]); tm2.inputs[7].default_value = (*tint, 1)
        nt.links.new(tm2.outputs[2], tmix.inputs[7])
        sc2 = nt.nodes.new("ShaderNodeVectorMath"); sc2.operation = "SCALE"
        nt.links.new(tmix.outputs[2], sc2.inputs[0]); nt.links.new(mr2.outputs["Result"], sc2.inputs["Scale"])
        col = sc2.outputs[0]
    if sun_glow:
        d, rgb, pw, st = sun_glow
        dot = nt.nodes.new("ShaderNodeVectorMath"); dot.operation = "DOT_PRODUCT"
        nt.links.new(tc.outputs["Generated"], dot.inputs[0]); dot.inputs[1].default_value = tuple(Vector(d).normalized())
        cl = nt.nodes.new("ShaderNodeMath"); cl.operation = "MAXIMUM"; cl.inputs[1].default_value = 0
        nt.links.new(dot.outputs["Value"], cl.inputs[0])
        pwn = nt.nodes.new("ShaderNodeMath"); pwn.operation = "POWER"; pwn.inputs[1].default_value = pw
        nt.links.new(cl.outputs[0], pwn.inputs[0])
        mul = nt.nodes.new("ShaderNodeMath"); mul.operation = "MULTIPLY"; mul.inputs[1].default_value = st
        nt.links.new(pwn.outputs[0], mul.inputs[0])
        mix = nt.nodes.new("ShaderNodeMix"); mix.data_type = "RGBA"; mix.blend_type = "ADD"
        mix.clamp_result = False
        nt.links.new(mul.outputs[0], mix.inputs["Factor"])
        nt.links.new(col, mix.inputs[6]); mix.inputs[7].default_value = (*rgb, 1)
        col = mix.outputs[2]
    if stars:
        dens, size, bright = stars
        vor = nt.nodes.new("ShaderNodeTexVoronoi")
        vor.distance = "CHEBYCHEV"
        vor.inputs["Scale"].default_value = dens
        vor.inputs["Randomness"].default_value = 0.9
        nt.links.new(tc.outputs["Generated"], vor.inputs["Vector"])
        lt = nt.nodes.new("ShaderNodeMath"); lt.operation = "LESS_THAN"; lt.inputs[1].default_value = size
        nt.links.new(vor.outputs["Distance"], lt.inputs[0])
        # only some cells get a star, with random brightness
        sepc = nt.nodes.new("ShaderNodeSeparateColor")
        nt.links.new(vor.outputs["Color"], sepc.inputs[0])
        gt = nt.nodes.new("ShaderNodeMath"); gt.operation = "GREATER_THAN"; gt.inputs[1].default_value = 0.55
        nt.links.new(sepc.outputs[0], gt.inputs[0])
        m1 = nt.nodes.new("ShaderNodeMath"); m1.operation = "MULTIPLY"
        nt.links.new(lt.outputs[0], m1.inputs[0]); nt.links.new(gt.outputs[0], m1.inputs[1])
        pw = nt.nodes.new("ShaderNodeMath"); pw.operation = "POWER"; pw.inputs[1].default_value = 3
        nt.links.new(sepc.outputs[1], pw.inputs[0])
        m2 = nt.nodes.new("ShaderNodeMath"); m2.operation = "MULTIPLY"
        nt.links.new(m1.outputs[0], m2.inputs[0]); nt.links.new(pw.outputs[0], m2.inputs[1])
        # fade stars near horizon
        hz = nt.nodes.new("ShaderNodeMapRange")
        hz.inputs["From Min"].default_value = 0.0; hz.inputs["From Max"].default_value = 0.35
        nt.links.new(sep.outputs["Z"], hz.inputs["Value"])
        m3 = nt.nodes.new("ShaderNodeMath"); m3.operation = "MULTIPLY"
        nt.links.new(m2.outputs[0], m3.inputs[0]); nt.links.new(hz.outputs["Result"], m3.inputs[1])
        m4 = nt.nodes.new("ShaderNodeMath"); m4.operation = "MULTIPLY"; m4.inputs[1].default_value = bright
        nt.links.new(m3.outputs[0], m4.inputs[0])
        add = nt.nodes.new("ShaderNodeMix"); add.data_type = "RGBA"; add.blend_type = "ADD"
        add.clamp_result = False
        add.inputs["Factor"].default_value = 1.0
        nt.links.new(col, add.inputs[6])
        sc = nt.nodes.new("ShaderNodeVectorMath"); sc.operation = "SCALE"
        nt.links.new(m4.outputs[0], sc.inputs["Scale"]); sc.inputs[0].default_value = (0.85, 0.9, 1.0)
        nt.links.new(sc.outputs[0], add.inputs[7])
        col = add.outputs[2]
    bg = nt.nodes.new("ShaderNodeBackground")
    bg.inputs["Strength"].default_value = strength
    nt.links.new(col, bg.inputs["Color"])
    nt.links.new(bg.outputs[0], out.inputs["Surface"])
    return w


def world_sky(elev, azim, strength=1.0, sky_type="MULTIPLE_SCATTERING", air=1.0, dust=1.0, ozone=1.0, tint=None):
    w, nt, out = world_nodes()
    sky = nt.nodes.new("ShaderNodeTexSky")
    sky.sky_type = sky_type
    sky.sun_elevation = math.radians(elev)
    sky.sun_rotation = math.radians(-azim + 90)  # match sun_dir convention
    for k, v in (("air_density", air), ("aerosol_density", dust), ("dust_density", dust), ("ozone_density", ozone)):
        if hasattr(sky, k):
            setattr(sky, k, v)
    if hasattr(sky, "sun_disc"):
        sky.sun_disc = True
    col = sky.outputs["Color"]
    if tint:
        mix = nt.nodes.new("ShaderNodeMix"); mix.data_type = "RGBA"; mix.blend_type = "MULTIPLY"
        mix.inputs["Factor"].default_value = 1.0
        nt.links.new(col, mix.inputs[6]); mix.inputs[7].default_value = (*tint, 1)
        col = mix.outputs[2]
    bg = nt.nodes.new("ShaderNodeBackground")
    bg.inputs["Strength"].default_value = strength
    nt.links.new(col, bg.inputs["Color"])
    nt.links.new(bg.outputs[0], out.inputs["Surface"])
    return w, sky


def fog_box(center, size, density, color=(1, 1, 1), anisotropy=0.3, emission=None, name="Fog"):
    """Homogeneous scattering volume in a box (sky stays visible outside it)."""
    bpy.ops.mesh.primitive_cube_add(size=1, location=center)
    ob = bpy.context.active_object
    ob.name = name
    ob.scale = size
    m = bpy.data.materials.new(name + "_m")
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        if n.type != "OUTPUT_MATERIAL":
            nt.nodes.remove(n)
    pv = nt.nodes.new("ShaderNodeVolumePrincipled")
    pv.inputs["Color"].default_value = (*color, 1)
    pv.inputs["Density"].default_value = density
    pv.inputs["Anisotropy"].default_value = anisotropy
    if emission:
        pv.inputs["Emission Color"].default_value = (*emission[0], 1)
        pv.inputs["Emission Strength"].default_value = emission[1]
    nt.links.new(pv.outputs[0], nt.nodes["Material Output"].inputs["Volume"])
    ob.data.materials.append(m)
    ob.visible_shadow = False
    return ob


def fog_gradient_box(center, size, density, z0, z1, color=(1, 1, 1), anisotropy=0.2, name="FogG", emission=None):
    """Volume whose density fades from `density` at z0 to 0 at z1 (ground haze)."""
    bpy.ops.mesh.primitive_cube_add(size=1, location=center)
    ob = bpy.context.active_object
    ob.name = name
    ob.scale = size
    m = bpy.data.materials.new(name + "_m")
    m.use_nodes = True
    nt = m.node_tree
    for n in list(nt.nodes):
        if n.type != "OUTPUT_MATERIAL":
            nt.nodes.remove(n)
    pv = nt.nodes.new("ShaderNodeVolumePrincipled")
    pv.inputs["Color"].default_value = (*color, 1)
    pv.inputs["Anisotropy"].default_value = anisotropy
    tc = nt.nodes.new("ShaderNodeTexCoord")
    # object coords are -0.5..0.5 * size? use world position via Geometry
    geo = nt.nodes.new("ShaderNodeNewGeometry")
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(geo.outputs["Position"], sep.inputs[0])
    mr = nt.nodes.new("ShaderNodeMapRange")
    mr.inputs["From Min"].default_value = z0
    mr.inputs["From Max"].default_value = z1
    mr.inputs["To Min"].default_value = density
    mr.inputs["To Max"].default_value = 0.0
    mr.interpolation_type = "SMOOTHSTEP"
    nt.links.new(sep.outputs["Z"], mr.inputs["Value"])
    nt.links.new(mr.outputs["Result"], pv.inputs["Density"])
    if emission:
        pv.inputs["Emission Color"].default_value = (*emission[0], 1)
        mul = nt.nodes.new("ShaderNodeMath"); mul.operation = "MULTIPLY"; mul.inputs[1].default_value = emission[1]
        nt.links.new(mr.outputs["Result"], mul.inputs[0])
        nt.links.new(mul.outputs[0], pv.inputs["Emission Strength"])
    nt.links.new(pv.outputs[0], nt.nodes["Material Output"].inputs["Volume"])
    ob.data.materials.append(m)
    ob.visible_shadow = False
    return ob


# ------------------------------------------------------------------ render
def setup_render(samples=128, look="AgX - Medium High Contrast", exposure=0.0, bloom=0.0,
                 bloom_threshold=1.0, bloom_size=0.5, time_limit=195, volume_bounces=1, res=(1920, 1080),
                 gamma=1.0):
    sc = bpy.context.scene
    sc.render.engine = "CYCLES"
    c = sc.cycles
    c.device = "CPU"
    c.samples = samples
    c.use_adaptive_sampling = True
    c.adaptive_threshold = 0.02
    c.use_denoising = True
    c.denoiser = "OPENIMAGEDENOISE"
    c.denoising_input_passes = "RGB_ALBEDO_NORMAL"
    c.denoising_prefilter = "ACCURATE"
    if hasattr(c, "denoising_quality"):
        c.denoising_quality = "HIGH"
    c.max_bounces = 8
    c.diffuse_bounces = 3
    c.glossy_bounces = 3
    c.transmission_bounces = 4
    c.transparent_max_bounces = 24
    c.volume_bounces = volume_bounces
    c.volume_step_rate = 4.0
    c.caustics_reflective = False
    c.caustics_refractive = False
    c.sample_clamp_indirect = 8.0
    c.blur_glossy = 1.0
    c.time_limit = time_limit
    c.film_exposure = 1.0
    c.filter_width = 1.2
    sc.render.resolution_x, sc.render.resolution_y = res
    sc.render.resolution_percentage = 100
    sc.render.threads_mode = "AUTO"
    sc.render.image_settings.file_format = "PNG"
    sc.render.image_settings.color_depth = "8"
    sc.render.image_settings.compression = 40
    vs = sc.view_settings
    sc.display_settings.display_device = "sRGB"
    vs.view_transform = "AgX"
    try:
        vs.look = look
    except TypeError:
        print("[vox] look not available:", look)
    vs.exposure = exposure
    vs.gamma = gamma
    # compositor: bloom + optional vignette
    if bloom > 0:
        g = bpy.data.node_groups.new("Comp", "CompositorNodeTree")
        g.interface.new_socket(name="Image", in_out="OUTPUT", socket_type="NodeSocketColor")
        rl = g.nodes.new("CompositorNodeRLayers")
        go = g.nodes.new("NodeGroupOutput")
        last = rl.outputs["Image"]
        if True:
            gl = g.nodes.new("CompositorNodeGlare")
            gl.inputs["Type"].default_value = "Bloom"
            gl.inputs["Quality"].default_value = "High"
            gl.inputs["Threshold"].default_value = bloom_threshold
            gl.inputs["Strength"].default_value = bloom
            gl.inputs["Size"].default_value = bloom_size
            g.links.new(last, gl.inputs["Image"])
            last = gl.outputs["Image"]
        g.links.new(last, go.inputs[0])
        sc.compositing_node_group = g
        sc.render.use_compositing = True


def render(name, t0=None):
    sc = bpy.context.scene
    A = args()
    if "crop" in A:  # fractional border render, e.g. crop=0.5,0.2,0.9,0.8 (x0,y0,x1,y1; y from bottom)
        x0, y0, x1, y1 = [float(t) for t in A["crop"].split(",")]
        sc.render.use_border = True
        sc.render.use_crop_to_border = True
        sc.render.border_min_x, sc.render.border_min_y = x0, y0
        sc.render.border_max_x, sc.render.border_max_y = x1, y1
    path = f"{OUT}/{name}.png"
    sc.render.filepath = path
    t = time.time()
    bpy.ops.render.render(write_still=True)
    dt = time.time() - t
    tot = time.time() - t0 if t0 else dt
    print(f"[vox] RENDER {name}: render {dt:.1f}s, total {tot:.1f}s -> {path}")
    with open(f"/tmp/srl/art/work/times.txt", "a") as f:
        f.write(f"{name}\t{dt:.1f}\t{tot:.1f}\t{time.strftime('%H:%M:%S')}\n")
    return path


def args():
    a = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = {}
    for x in a:
        if "=" in x:
            k, v = x.split("=", 1)
            out[k] = v
    return out
