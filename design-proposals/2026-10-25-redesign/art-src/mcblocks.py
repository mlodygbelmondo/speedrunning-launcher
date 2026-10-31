"""Build Blender geometry from an extracted Minecraft block box, using ONLY our own textures.

The block layout comes from /tmp/srl/mcworld/extract/<name>.npz (palette of block-state strings +
uint16 index array [nx, ny, nz] in MC x, y, z order). Nothing from Mojang is shipped: these npz
files stay in /tmp; the renders use our procedural textures from /tmp/srl/art/out/textures.

Coordinates: a point (Mx, My, Mz) in Minecraft maps to Blender (Mx-OX, OZ-Mz, My-OY):
MC east (+x) = Blender +X, MC north (-z) = Blender +Y, MC up = Blender +Z.
"""
import math
import os
import zlib
import numpy as np
import bpy
from mathutils import Vector, Matrix

import vox
from vox import material, TEX, FACES, face_uv

EXTRACT = "/tmp/srl/mcworld/extract"


def load(name):
    d = np.load(f"{EXTRACT}/{name}.npz", allow_pickle=False)
    return [str(p) for p in d["pal"]], d["arr"], tuple(int(v) for v in d["origin"])


def parse(s):
    s = s.replace("minecraft:", "")
    if "[" in s:
        n, p = s[:-1].split("[", 1)
        return n, dict(kv.split("=", 1) for kv in p.split(","))
    return s, {}


def has_tex(n):
    return os.path.exists(f"{TEX}/{n}.png")


AIR = {"air", "cave_air", "void_air", "structure_void", "barrier", "light", "moving_piston"}
DIRS = ["px", "nx", "py", "ny", "pz", "nz"]
# neighbour offsets in array index space (i = MC x, j = MC y, k = MC z)
AOFF = {"px": (1, 0, 0), "nx": (-1, 0, 0), "py": (0, 0, -1), "ny": (0, 0, 1), "pz": (0, 1, 0), "nz": (0, -1, 0)}
MC2B = {"east": "px", "west": "nx", "north": "py", "south": "ny", "up": "pz", "down": "nz"}
B2MC = {v: k for k, v in MC2B.items()}
HOR = ["north", "east", "south", "west"]  # clockwise seen from above
OPP = {"north": "south", "south": "north", "east": "west", "west": "east", "up": "down", "down": "up"}


def cw(d, n=1):
    return HOR[(HOR.index(d) + n) % 4] if d in HOR else d


# ------------------------------------------------------------------ textures per block
ALIAS = {
    "magma_block": "magma", "infested_stone_bricks": "stone_bricks", "infested_stone": "stone",
    "infested_cobblestone": "cobblestone", "infested_mossy_stone_bricks": "mossy_stone_bricks",
    "infested_cracked_stone_bricks": "cracked_stone_bricks", "chiseled_stone_bricks": "stone_bricks",
    "cut_sandstone": "sandstone", "chiseled_sandstone": "sandstone", "smooth_sandstone": "sandstone_top",
    "snow_block": "snow", "powder_snow": "snow", "packed_ice": "ice", "blue_ice": "ice",
    "chiseled_quartz_block": "quartz_block", "quartz_bricks": "quartz_block",
    "smooth_quartz": "smooth_quartz", "chiseled_nether_bricks": "nether_bricks", "cracked_nether_bricks": "nether_bricks",
    "red_nether_bricks": "nether_bricks", "polished_blackstone": "polished_blackstone",
    "gilded_blackstone": "gilded_blackstone", "spawner": "spawner", "dead_bush": "dead_bush",
    "dripstone_block": "granite", "copper_ore": "iron_ore", "raw_copper_block": "granite",
    "calcite": "diorite", "tuff": "andesite", "smooth_basalt": "basalt_top", "cobbled_deepslate": "deepslate",
    "polished_deepslate": "deepslate", "deepslate_tiles": "deepslate", "deepslate_bricks": "deepslate",
    "chiseled_deepslate": "deepslate", "cracked_deepslate_bricks": "deepslate", "cracked_deepslate_tiles": "deepslate",
    "pumpkin": "terracotta", "carved_pumpkin": "terracotta", "melon": "moss_block",
    "furnace": "cobblestone", "smoker": "cobblestone", "blast_furnace": "smooth_stone", "dispenser": "cobblestone",
    "dropper": "cobblestone", "lectern": "oak_planks", "cartography_table": "dark_oak_planks",
    "fletching_table": "birch_planks", "smithing_table": "dark_oak_planks", "loom": "oak_planks",
    "stonecutter": "smooth_stone", "note_block": "oak_planks", "jukebox": "oak_planks",
    "brain_coral_block": "pink_wool", "bubble_coral_block": "purple_wool", "fire_coral_block": "red_wool",
    "horn_coral_block": "yellow_wool", "tube_coral_block": "blue_wool", "dead_fire_coral_block": "andesite",
    "moss_carpet": "moss_block", "rooted_dirt": "coarse_dirt", "mud": "coarse_dirt", "packed_mud": "coarse_dirt",
    "sea_lantern": "quartz_block", "prismarine_bricks": "prismarine", "dark_prismarine": "prismarine",
    "bricks": "terracotta", "red_sand": "terracotta", "end_stone_bricks": "end_stone",
}
TRIPLE = {
    "grass_block": ("grass_top", "grass_side", "dirt"),
    "crimson_nylium": ("crimson_nylium", "crimson_nylium_side", "netherrack"),
    "warped_nylium": ("warped_nylium", "warped_nylium_side", "netherrack"),
    "sandstone": ("sandstone_top", "sandstone", "sandstone_bottom"),
    "cut_sandstone": ("sandstone_top", "sandstone", "sandstone_bottom"),
    "chiseled_sandstone": ("sandstone_top", "sandstone", "sandstone_bottom"),
    "grass_path": ("grass_path_top", "grass_path_side", "dirt"),
    "dirt_path": ("grass_path_top", "grass_path_side", "dirt"),
    "mycelium": ("mycelium_top", "dirt", "dirt"),
    "podzol": ("podzol_top", "dirt", "dirt"),
    "farmland": ("farmland", "dirt", "dirt"),
    "crafting_table": ("crafting_table_top", "oak_planks", "oak_planks"),
    "bookshelf": ("oak_planks", "bookshelf", "oak_planks"),
    "bee_nest": ("bee_nest_top", "bee_nest_side", "bee_nest_top"),
    "blackstone": ("blackstone_top", "blackstone", "blackstone_top"),
    "barrel": ("oak_planks", "barrel_side", "oak_planks"),
    "ancient_debris": ("ancient_debris_top", "ancient_debris_side", "ancient_debris_top"),
    "composter": ("composter", "composter", "composter"),
    "smooth_stone_slab_double": ("smooth_stone", "smooth_stone_slab_side", "smooth_stone"),
}
PILLAR = {
    "basalt": ("basalt_top", "basalt_side"), "polished_basalt": ("polished_basalt_top", "polished_basalt_side"),
    "crimson_stem": ("crimson_stem_top", "crimson_stem"), "warped_stem": ("warped_stem_top", "warped_stem"),
    "crimson_hyphae": ("crimson_stem", "crimson_stem"), "warped_hyphae": ("warped_stem", "warped_stem"),
    "bone_block": ("bone_block_top", "bone_block_side"), "hay_block": ("hay_block_top", "hay_block_side"),
    "quartz_pillar": ("quartz_block", "quartz_block"), "purpur_pillar": ("prismarine", "prismarine"),
}
for _w in ("oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "cherry", "mangrove"):
    _l = _w if has_tex(f"{_w}_log") else "oak"
    PILLAR[f"{_w}_log"] = (f"{_l}_log_top", f"{_l}_log")
    PILLAR[f"stripped_{_w}_log"] = (f"{_l}_log_top", f"{_l}_planks" if has_tex(f"{_l}_planks") else f"{_l}_log")
    PILLAR[f"{_w}_wood"] = (f"{_l}_log", f"{_l}_log")
    PILLAR[f"stripped_{_w}_wood"] = (f"{_l}_planks", f"{_l}_planks")
    if not has_tex(f"{_w}_planks"):
        ALIAS[f"{_w}_planks"] = "oak_planks"
    if not has_tex(f"{_w}_leaves"):
        ALIAS[f"{_w}_leaves"] = "oak_leaves"

# random top/bottom rotation (as vanilla does for natural blocks)
ROT_TOP = {"grass_block", "dirt", "sand", "red_sand", "netherrack", "soul_sand", "soul_soil", "crimson_nylium",
           "warped_nylium", "end_stone", "bedrock", "magma_block", "gravel", "coarse_dirt", "podzol", "mycelium",
           "snow_block", "stone", "deepslate", "clay", "moss_block"}

BASE = {"stone_brick": "stone_bricks", "nether_brick": "nether_bricks", "red_nether_brick": "nether_bricks",
        "polished_blackstone_brick": "polished_blackstone_bricks", "mossy_stone_brick": "mossy_stone_bricks",
        "quartz": "quartz_block", "smooth_quartz": "smooth_quartz", "prismarine_brick": "prismarine",
        "dark_prismarine": "prismarine", "end_stone_brick": "end_stone", "brick": "terracotta",
        "petrified_oak": "oak_planks", "deepslate_brick": "deepslate", "deepslate_tile": "deepslate",
        "polished_deepslate": "deepslate", "cobbled_deepslate": "deepslate", "tuff_brick": "andesite",
        "polished_tuff": "andesite", "cut_copper": "terracotta", "waxed_cut_copper": "terracotta"}


def textures_of(name, props=None):
    """(top, side, bottom) texture names for a full-cube-ish block, or None if unknown."""
    if name in TRIPLE:
        t = TRIPLE[name]
        if name == "grass_block" and props and props.get("snowy") == "true":
            t = ("snow", "grass_side", "dirt")
        return t
    if name in PILLAR:
        e, s = PILLAR[name]
        return (e, s, e)
    n = ALIAS.get(name, name)
    if n in TRIPLE:
        return TRIPLE[n]
    if has_tex(n):
        return (n, n, n)
    for w in ("oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "cherry", "mangrove"):
        if n == f"{w}_planks":
            return ("oak_planks",) * 3
    for c in ("white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan",
              "purple", "blue", "brown", "green", "red", "black"):
        if n == f"{c}_wool" or n == f"{c}_concrete":
            return (f"{c}_wool",) * 3 if has_tex(f"{c}_wool") else ("white_wool",) * 3
        if n == f"{c}_terracotta":
            return ("terracotta",) * 3
    return None


def base_textures(name):
    """Texture triple for the base material of stairs / slabs / walls / fences."""
    b = name
    for suf in ("_stairs", "_slab", "_wall", "_fence_gate", "_fence", "_pressure_plate", "_button", "_trapdoor", "_door"):
        if name.endswith(suf):
            b = name[: -len(suf)]
            break
    if b == "smooth_stone":
        return ("smooth_stone", "smooth_stone_slab_side", "smooth_stone")
    if b in BASE:
        b = BASE[b]
    for w in ("oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "cherry", "mangrove", "crimson", "warped"):
        if b == w:
            return (f"{w}_planks",) * 3 if has_tex(f"{w}_planks") else ("oak_planks",) * 3
    t = textures_of(b)
    if t:
        return t
    t = textures_of(b + "s")
    return t or ("stone", "stone", "stone")


# ------------------------------------------------------------------ classification
# kinds
K_AIR, K_CUBE, K_CUT, K_WATER, K_LAVA, K_MODEL, K_GLASS = 0, 1, 2, 3, 4, 5, 6
WATERY = {"seagrass", "tall_seagrass", "kelp", "kelp_plant", "bubble_column", "water"}
CUTOUT_CUBES = {"spawner"}  # + flames via m_spawner_fx
GLASSY = {"glass", "ice", "tinted_glass"}
MODEL_SUFFIX = ("_stairs", "_slab", "_wall", "_fence", "_fence_gate", "_door", "_trapdoor", "_carpet", "_pane",
                "_button", "_pressure_plate", "_bed", "_banner", "_sign", "_torch", "_candle", "_coral_fan",
                "_coral_wall_fan", "_head", "_skull", "_tulip", "_sapling", "_mushroom")
CROSS = {"grass", "short_grass", "fern", "dead_bush", "dandelion", "poppy", "blue_orchid", "allium", "azure_bluet",
         "red_tulip", "orange_tulip", "white_tulip", "pink_tulip", "oxeye_daisy", "cornflower", "lily_of_the_valley",
         "wither_rose", "brown_mushroom", "red_mushroom", "crimson_roots", "warped_roots", "nether_sprouts",
         "crimson_fungus", "warped_fungus", "weeping_vines", "weeping_vines_plant", "twisting_vines",
         "twisting_vines_plant", "sugar_cane", "sweet_berry_bush", "seagrass", "kelp", "kelp_plant", "cobweb",
         "oak_sapling", "spruce_sapling", "birch_sapling", "jungle_sapling", "acacia_sapling", "dark_oak_sapling",
         "cherry_sapling", "bush", "firefly_bush", "short_dry_grass", "tall_dry_grass", "nether_wart",
         "tube_coral", "brain_coral", "bubble_coral", "fire_coral", "horn_coral", "sea_pickle", "glow_lichen",
         "pointed_dripstone", "dead_bush", "fire", "soul_fire", "leaf_litter", "wildflowers"}
DOUBLE = {"tall_grass": ("tall_grass_bottom", "tall_grass_top"), "large_fern": ("large_fern_bottom", "large_fern_top"),
          "tall_seagrass": ("tall_seagrass_bottom", "tall_seagrass_top"), "sunflower": ("tall_grass_bottom", "dandelion"),
          "lilac": ("tall_grass_bottom", "allium"), "rose_bush": ("tall_grass_bottom", "poppy"),
          "peony": ("tall_grass_bottom", "pink_tulip")}
CROSS_TEX = {"grass": "tall_grass", "short_grass": "tall_grass", "bush": "tall_grass", "firefly_bush": "fern",
             "short_dry_grass": "dead_bush", "tall_dry_grass": "dead_bush", "nether_wart": "nether_wart",
             "tube_coral": "seagrass", "brain_coral": "seagrass", "bubble_coral": "seagrass", "fire_coral": "seagrass",
             "horn_coral": "seagrass", "sea_pickle": "lily_pad", "glow_lichen": "vine", "pointed_dripstone": "dead_bush",
             "spruce_sapling": "oak_sapling", "birch_sapling": "oak_sapling", "jungle_sapling": "oak_sapling",
             "acacia_sapling": "oak_sapling", "dark_oak_sapling": "oak_sapling", "cherry_sapling": "oak_sapling",
             "leaf_litter": "dead_bush", "wildflowers": "dandelion"}
LIGHTS = {  # emitters that also get a small point light: (energy W, colour, z-offset)
    "lantern": (6.0, (1.0, 0.62, 0.28), 0.3), "soul_lantern": (4.0, (0.35, 0.85, 1.0), 0.3),
    "torch": (5.0, (1.0, 0.6, 0.25), 0.62), "wall_torch": (5.0, (1.0, 0.6, 0.25), 0.7),
    "soul_torch": (3.0, (0.35, 0.85, 1.0), 0.62), "soul_wall_torch": (3.0, (0.35, 0.85, 1.0), 0.7),
    "fire": (9.0, (1.0, 0.5, 0.18), 0.5), "soul_fire": (6.0, (0.3, 0.8, 1.0), 0.5),
    "campfire": (8.0, (1.0, 0.55, 0.2), 0.4),
}


class Builder:
    """Classify a palette, then emit culled cube faces (vectorised), fluids and special models."""

    def __init__(self, pal, arr, origin, OX, OY, OZ, seed=1, mat_over=None, fancy_leaves=True, plants=True,
                 skip=(), border_open=False, light_scale=1.0, tint=None):
        self.pal, self.arr, self.origin = pal, arr, origin
        self.O = (OX, OY, OZ)
        self.rng = np.random.default_rng(seed)
        self.mat_over = mat_over or {}
        self.fancy_leaves = fancy_leaves
        self.plants = plants
        self.skip = set(skip)
        self.border_open = border_open
        self.light_scale = light_scale
        self.tint = tint or {}
        self.unknown = {}
        self.lights = []
        self.parsed = [parse(p) for p in pal]
        n = len(pal)
        self.kind = np.zeros(n, np.int8)
        self.water = np.zeros(n, bool)
        self.ftex = [[None] * 6 for _ in range(n)]
        self.frot = np.zeros((n, 6), np.int8)
        self.frand = np.zeros(n, bool)
        self.fluid_h = np.ones(n, np.float32)
        for pid, (name, props) in enumerate(self.parsed):
            self._classify(pid, name, props)
        self.occl = self.kind == K_CUBE
        self.models = vox.MeshBuilder()
        self.glow = vox.MeshBuilder()
        self.cut = vox.MeshBuilder()

    # -------------------------------------------------------------- classification
    def _set_faces(self, pid, top, side, bottom, axis="y"):
        f = self.ftex[pid]
        if axis == "y":
            f[0] = f[1] = f[2] = f[3] = side
            f[4], f[5] = top, bottom
        elif axis == "x":  # ends on east/west
            f[0], f[1] = top, bottom
            f[2] = f[3] = f[4] = f[5] = side
            self.frot[pid, [2, 3, 4, 5]] = 1
        else:  # axis z (MC) = Blender y: ends on north/south
            f[2], f[3] = top, bottom
            f[0] = f[1] = f[4] = f[5] = side
            self.frot[pid, [0, 1]] = 1

    def _classify(self, pid, name, props):
        if name in AIR or name in self.skip:
            self.kind[pid] = K_AIR
            return
        wl = props.get("waterlogged") == "true" or name in WATERY
        self.water[pid] = wl
        if name == "water" or name == "bubble_column":
            self.kind[pid] = K_WATER
            lv = int(props.get("level", 0))
            self.fluid_h[pid] = 1.0 if lv >= 8 else (14 / 16 if lv == 0 else max(2, 14 - 2 * lv) / 16)
            return
        if name == "lava":
            self.kind[pid] = K_LAVA
            lv = int(props.get("level", 0))
            self.fluid_h[pid] = 1.0 if lv >= 8 else (14 / 16 if lv == 0 else max(2, 14 - 2 * lv) / 16)
            return
        if name.endswith("_slab") and props.get("type") == "double":
            t = base_textures(name)
            self.kind[pid] = K_CUBE
            self._set_faces(pid, *t)
            return
        if name.endswith("leaves"):
            self.kind[pid] = K_CUT
            t = name if has_tex(name) else ALIAS.get(name, "oak_leaves")
            self._set_faces(pid, t, t, t)
            self.frand[pid] = True
            return
        if name in CUTOUT_CUBES:
            self.kind[pid] = K_CUT
            self._set_faces(pid, name, name, name)
            return
        if name in GLASSY:
            self.kind[pid] = K_GLASS
            self._set_faces(pid, name, name, name)
            return
        if (name in CROSS or name in DOUBLE or name.endswith(MODEL_SUFFIX) or name in MODELS) and name not in TRIPLE:
            self.kind[pid] = K_MODEL
            return
        t = textures_of(name, props)
        if t is None:
            self.unknown[name] = self.unknown.get(name, 0) + 1
            t = ("stone", "stone", "stone")
        self.kind[pid] = K_CUBE
        axis = props.get("axis", "y")
        self._set_faces(pid, *t, axis=axis)
        if name in ROT_TOP:
            self.frand[pid] = True

    # -------------------------------------------------------------- helpers
    def cell_min(self, i, j, k):
        x0, y0, z0 = self.origin
        OX, OY, OZ = self.O
        return (x0 + i - OX, OZ - (z0 + k) - 1, y0 + j - OY)

    def mat(self, tex, **kw):
        o = dict(self.mat_over.get(tex, {}))
        if tex in self.tint:
            o["tint"] = self.tint[tex]
        o.update(kw)
        return material(tex, **o)

    def pid_at(self, i, j, k):
        a = self.arr
        if 0 <= i < a.shape[0] and 0 <= j < a.shape[1] and 0 <= k < a.shape[2]:
            return int(a[i, j, k])
        return 0

    def name_at(self, i, j, k):
        return self.parsed[self.pid_at(i, j, k)][0]

    def occl_at(self, i, j, k):
        return bool(self.occl[self.pid_at(i, j, k)])

    # -------------------------------------------------------------- vectorised faces
    def _shift(self, arr, d, fill):
        """Value of the neighbour in direction d for every cell (fill outside the box)."""
        di, dj, dk = AOFF[d]
        out = np.full(arr.shape, fill, dtype=arr.dtype)
        sl_dst, sl_src = [], []
        for ax, o in enumerate((di, dj, dk)):
            n = arr.shape[ax]
            if o == 1:
                sl_dst.append(slice(0, n - 1)); sl_src.append(slice(1, n))
            elif o == -1:
                sl_dst.append(slice(1, n)); sl_src.append(slice(0, n - 1))
            else:
                sl_dst.append(slice(0, n)); sl_src.append(slice(0, n))
        out[tuple(sl_dst)] = arr[tuple(sl_src)]
        return out

    def build_cubes(self, name="mc", top_cut=None):
        """Full cubes, cutout cubes (leaves, spawner), glass and fluids -> separate objects."""
        a = self.arr
        kind = self.kind[a]
        occ = self.occl[a]
        water = self.water[a]
        lava = kind == K_LAVA
        if top_cut is not None:  # drop everything at/above this array y (open the nether roof etc.)
            kind[:, top_cut:, :] = K_AIR
            occ[:, top_cut:, :] = False
            water[:, top_cut:, :] = False
            lava[:, top_cut:, :] = False
        groups = {"cubes": [], "cutout": [], "fluid": []}
        x0, y0, z0 = self.origin
        OX, OY, OZ = self.O
        uvtab = {}
        for d in DIRS:
            corners = np.array(FACES[d][1], np.float32)
            base = np.array([face_uv(d, c) for c in corners], np.float32)
            rots = [base]
            for r in range(3):
                u, v = rots[-1][:, 0], rots[-1][:, 1]
                rots.append(np.stack([1 - v, u], 1))
            uvtab[d] = (corners, np.stack(rots))
        for di, d in enumerate(DIRS):
            corners, uvr = uvtab[d]
            # outside the box counts as solid (no cut walls), except above the top
            edge = (not self.border_open) and d != "pz"
            nocc = self._shift(occ, d, edge)
            nid = self._shift(a, d, 0)
            nwater = self._shift(water, d, edge)
            nlava = self._shift(lava, d, edge)
            # opaque cubes
            m_cube = (kind == K_CUBE) & ~nocc
            # cutout cubes: leaves cull against opaque and (if not fancy) same block
            m_cut = (kind == K_CUT) & ~nocc
            if not self.fancy_leaves:
                m_cut &= nid != a
            m_glass = (kind == K_GLASS) & ~nocc & (nid != a)
            # fluids (waterlogged cells count as water)
            m_w = water & ~nocc & ~nwater
            m_l = lava & ~nocc & ~nlava
            if d == "nz":
                m_w &= nid != 0  # no water bottoms over air (or below the box)
            for gname, m, fl in (("cubes", m_cube, None), ("cutout", m_cut | m_glass, None),
                                 ("fluid", m_w, "water"), ("fluid", m_l, "lava")):
                idx = np.nonzero(m)
                if len(idx[0]) == 0:
                    continue
                ii, jj, kk = idx
                pids = a[idx]
                bx = (x0 + ii - OX).astype(np.float32)
                by = (OZ - (z0 + kk) - 1).astype(np.float32)
                bz = (y0 + jj - OY).astype(np.float32)
                P = np.stack([bx, by, bz], 1)[:, None, :] + corners[None, :, :]
                if fl:
                    # top height of fluid cells that have no same fluid above
                    above = self._shift(water if fl == "water" else lava, "pz", False)[idx]
                    if fl == "water":
                        h = np.where(above, 1.0, np.where(kind[idx] == K_WATER, self.fluid_h[pids], 14 / 16))
                    else:
                        h = np.where(above, 1.0, self.fluid_h[pids])
                    P[:, :, 2] = bz[:, None] + corners[None, :, 2] * h[:, None]
                    rot = np.zeros(len(ii), np.int64)
                    if d in ("pz", "nz"):
                        rot = ((ii * 7 + kk * 13 + jj * 3) % 4).astype(np.int64)
                    UV = uvr[rot]
                    tex = fl
                    mi = np.zeros(len(ii), np.int32)
                    groups[gname].append((P, UV, [self.mat(tex)], mi))
                    continue
                rot = self.frot[pids, di].astype(np.int64)
                if d in ("pz", "nz"):
                    h = (ii * 3129871) ^ (kk * 116129781) ^ (jj * 7)
                    rr = ((h >> 4) & 3)
                    rot = np.where(self.frand[pids], (rot + rr) % 4, rot)
                UV = uvr[rot]
                # material per pid
                up, inv = np.unique(pids, return_inverse=True)
                mats = []
                for p in up:
                    t = self.ftex[p][di] or "stone"
                    mats.append(self.mat(t))
                groups[gname].append((P, UV, mats, inv.astype(np.int32)))
        obs = {}
        for gname, parts in groups.items():
            if not parts:
                continue
            ob = np_mesh(f"{name}_{gname}", parts)
            obs[gname] = ob
            print(f"[mc] {name}_{gname}: {sum(len(p[0]) for p in parts)} faces")
        return obs

    # -------------------------------------------------------------- models
    def build_models(self, name="mc", top_cut=None, region=None):
        a = self.arr
        ids = np.nonzero(self.kind == K_MODEL)[0]
        m = np.isin(a, ids)
        if top_cut is not None:
            m[:, top_cut:, :] = False
        if region is not None:  # (i0, i1, k0, k1) array-index window
            i0, i1, k0, k1 = region
            mm = np.zeros_like(m)
            mm[i0:i1, :, k0:k1] = m[i0:i1, :, k0:k1]
            m = mm
        sp = [i for i, (n, _) in enumerate(self.parsed) if n == "spawner"]
        if sp:
            for (i, j, k) in np.argwhere(np.isin(a, sp)):
                if top_cut is not None and j >= top_cut:
                    continue
                m_spawner_fx(self, (int(i), int(j), int(k)), "spawner", {})
                c = self.cell_min(i, j, k)
                self.lights.append(((c[0] + 0.5, c[1] + 0.5, c[2] + 0.5), 2.5 * self.light_scale, (1.0, 0.55, 0.2), "spawner"))
        cells = np.argwhere(m)
        cnt = 0
        for (i, j, k) in cells:
            pid = int(a[i, j, k])
            nm, props = self.parsed[pid]
            fn = MODELS.get(nm)
            if fn is None:
                for suf, f in SUFFIX_MODELS:
                    if nm.endswith(suf):
                        fn = f
                        break
            if fn is None and (nm in CROSS or nm in DOUBLE):
                fn = m_cross
            if fn is None:
                self.unknown[nm] = self.unknown.get(nm, 0) + 1
                continue
            if not self.plants and fn in (m_cross,) and nm not in ("fire", "soul_fire"):
                continue
            fn(self, (int(i), int(j), int(k)), nm, props)
            cnt += 1
            if nm in LIGHTS:
                e, col, dz = LIGHTS[nm]
                c = self.cell_min(i, j, k)
                self.lights.append(((c[0] + 0.5, c[1] + 0.5, c[2] + dz), e * self.light_scale, col, nm))
        out = {}
        for nme, mb, shadow in (("models", self.models, True), ("glow", self.glow, False), ("cut", self.cut, True)):
            ob = mb.build(f"{name}_{nme}")
            if ob:
                if not shadow:
                    ob.visible_shadow = False
                out[nme] = ob
        print(f"[mc] {name}: {cnt} model blocks, unknown: {self.unknown}")
        return out

    def add_lights(self, max_n=400, radius=0.12):
        for n, (loc, e, col, nm) in enumerate(self.lights[:max_n]):
            vox.point(loc, e, col, radius=radius, name=f"L_{nm}")
        return len(self.lights)

    # ---- primitive emitters (MC pixel coordinates, MC orientation)
    def box(self, cell, p0, p1, tex, rot=0, cull=True, uv=None, mb=None, skip=(), uvlock=True):
        """Axis aligned MC element p0..p1 (pixels, x east, y up, z south) rotated by rot*90 deg cw (seen
        from above) about the block centre. tex: str | Material | {mc_dir: tex}. uv: {mc_dir: (u0,v0,u1,v1)}
        in pixels (image space, v down) for non-auto UVs."""
        x0, y0, z0 = p0
        x1, y1, z1 = p1
        for _ in range(rot % 4):  # (x, z) -> (16 - z, x)
            x0, z0, x1, z1 = 16 - z1, x0, 16 - z0, x1
        bmin = (x0 / 16, 1 - z1 / 16, y0 / 16)
        bmax = (x1 / 16, 1 - z0 / 16, y1 / 16)
        c = self.cell_min(*cell)
        i, j, k = cell
        for d, (off, corners) in FACES.items():
            mcd = B2MC[d]
            src = HOR[(HOR.index(mcd) - rot) % 4] if mcd in HOR else mcd  # face of the unrotated element
            if src in skip:
                continue
            ext = [bmax[a] - bmin[a] for a in range(3)]
            ax = {"px": 0, "nx": 0, "py": 1, "ny": 1, "pz": 2, "nz": 2}[d]
            if ext[(ax + 1) % 3] <= 1e-6 or ext[(ax + 2) % 3] <= 1e-6:
                continue
            if cull:
                on_edge = (off[ax] > 0 and bmax[ax] >= 1 - 1e-6) or (off[ax] < 0 and bmin[ax] <= 1e-6)
                if on_edge:
                    ai, aj, ak = AOFF[d]
                    if self.occl_at(i + ai, j + aj, k + ak):
                        continue
            t = tex.get(src, tex.get("*")) if isinstance(tex, dict) else tex
            if t is None:
                continue
            mt = t if isinstance(t, bpy.types.Material) else self.mat(t)
            pts, uvs = [], []
            for cc in corners:
                lp = tuple(bmin[a] + (bmax[a] - bmin[a]) * cc[a] for a in range(3))
                pts.append((c[0] + lp[0], c[1] + lp[1], c[2] + lp[2]))
                if uv and src in uv:
                    fu, fv = face_uv(d, cc)
                    u0, v0, u1, v1 = uv[src]
                    uvs.append(((u0 + (u1 - u0) * fu) / 16, 1 - (v0 + (v1 - v0) * (1 - fv)) / 16))
                else:
                    uvs.append(face_uv(d, lp))
            target = mb or (self.glow if mt.get("noshadow") else self.models)
            target.quad(pts, uvs, mt)

    def plane(self, cell, pts_px, tex, uvs=((0, 0), (1, 0), (1, 1), (0, 1)), mb=None):
        """Quad from 4 points in MC pixel coords (x, y, z)."""
        c = self.cell_min(*cell)
        mt = tex if isinstance(tex, bpy.types.Material) else self.mat(tex)
        P = [(c[0] + x / 16, c[1] + 1 - z / 16, c[2] + y / 16) for (x, y, z) in pts_px]
        (mb or (self.glow if mt.get("noshadow") else self.models)).quad(P, list(uvs), mt)


def np_mesh(name, parts):
    """parts: list of (P[N,4,3], UV[N,4,2], mats[list], mat_index[N]) -> one mesh object."""
    allP, allUV, allM = [], [], []
    mats, mindex = [], {}
    for P, UV, ml, mi in parts:
        remap = np.zeros(len(ml), np.int32)
        for n, m in enumerate(ml):
            if m.name not in mindex:
                mindex[m.name] = len(mats)
                mats.append(m)
            remap[n] = mindex[m.name]
        allP.append(P.reshape(-1, 3))
        allUV.append(UV.reshape(-1, 2))
        allM.append(remap[mi])
    P = np.concatenate(allP).astype(np.float32)
    UV = np.concatenate(allUV).astype(np.float32)
    M = np.concatenate(allM).astype(np.int32)
    nv = len(P)
    nf = nv // 4
    me = bpy.data.meshes.new(name)
    me.vertices.add(nv)
    me.vertices.foreach_set("co", P.ravel())
    me.loops.add(nv)
    me.loops.foreach_set("vertex_index", np.arange(nv, dtype=np.int32))
    me.polygons.add(nf)
    me.polygons.foreach_set("loop_start", np.arange(0, nv, 4, dtype=np.int32))
    uvl = me.uv_layers.new(name="UVMap")
    uvl.data.foreach_set("uv", UV.ravel())
    for m in mats:
        me.materials.append(m)
    me.polygons.foreach_set("material_index", M)
    me.update(calc_edges=True)
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    if any(m.get("noshadow") for m in mats) and all(m.get("noshadow") for m in mats):
        ob.visible_shadow = False
    return ob


# ------------------------------------------------------------------ models
ROT_OF = {"north": 0, "east": 1, "south": 2, "west": 3}  # cw quarter turns from a north-facing model


def _hash(cell, salt=0):
    return zlib.crc32(f"{cell[0]},{cell[1]},{cell[2]},{salt}".encode()) & 0xFFFFFFFF


def m_stairs(b, cell, nm, p):
    t = base_textures(nm)
    tex = {"up": t[0], "down": t[2], "*": t[1]}
    top = p.get("half") == "top"
    shape = p.get("shape", "straight")
    # quadrants for facing=east, as (qx, qz) with 0 = west/north half
    Q = {"straight": {(1, 0), (1, 1)}, "outer_right": {(1, 1)}, "outer_left": {(1, 0)},
         "inner_right": {(1, 0), (1, 1), (0, 1)}, "inner_left": {(1, 0), (1, 1), (0, 0)}}[shape]
    steps = (ROT_OF[p.get("facing", "east")] - 1) % 4
    for _ in range(steps):
        Q = {(1 - qz, qx) for (qx, qz) in Q}
    sy0, sy1 = (8, 16) if top else (0, 8)
    qy0, qy1 = (0, 8) if top else (8, 16)
    b.box(cell, (0, sy0, 0), (16, sy1, 16), tex)
    for (qx, qz) in Q:
        b.box(cell, (qx * 8, qy0, qz * 8), (qx * 8 + 8, qy1, qz * 8 + 8), tex)


def m_slab(b, cell, nm, p):
    t = base_textures(nm)
    tex = {"up": t[0], "down": t[2], "*": t[1]}
    if p.get("type") == "top":
        b.box(cell, (0, 8, 0), (16, 16, 16), tex)
    else:
        b.box(cell, (0, 0, 0), (16, 8, 16), tex)


def _conn(p, d):
    v = p.get(d, "false")
    return v not in ("false", "none")


def m_wall(b, cell, nm, p):
    t = base_textures(nm)
    tex = {"up": t[0], "down": t[2], "*": t[1]}
    if p.get("up", "true") == "true":
        b.box(cell, (4, 0, 4), (12, 16, 12), tex)
    for d in HOR:
        v = p.get(d, "none")
        if v in ("none", "false"):
            continue
        h = 16 if v == "tall" else 14
        b.box(cell, (5, 0, 0), (11, h, 8), tex, rot=ROT_OF[d])


def m_fence(b, cell, nm, p):
    t = base_textures(nm)
    tex = {"up": t[0], "down": t[2], "*": t[1]}
    b.box(cell, (6, 0, 6), (10, 16, 10), tex)
    for d in HOR:
        if _conn(p, d):
            b.box(cell, (7, 12, 0), (9, 15, 6), tex, rot=ROT_OF[d])
            b.box(cell, (7, 6, 0), (9, 9, 6), tex, rot=ROT_OF[d])


def m_fence_gate(b, cell, nm, p):
    t = base_textures(nm)
    tex = {"up": t[0], "down": t[2], "*": t[1]}
    r = ROT_OF[p.get("facing", "north")]
    yo = -3 if p.get("in_wall") == "true" else 0
    b.box(cell, (0, 5 + yo, 7), (2, 16 + yo, 9), tex, rot=r)
    b.box(cell, (14, 5 + yo, 7), (16, 16 + yo, 9), tex, rot=r)
    if p.get("open") != "true":
        b.box(cell, (2, 6 + yo, 7), (14, 9 + yo, 9), tex, rot=r)
        b.box(cell, (2, 12 + yo, 7), (14, 15 + yo, 9), tex, rot=r)


def m_pane(b, cell, nm, p):
    tx = "iron_bars" if nm == "iron_bars" else "glass"
    conns = [d for d in HOR if _conn(p, d)]
    if not conns:  # lone post: show both full planes like vanilla's cross
        b.box(cell, (7, 0, 0), (9, 16, 16), tx, cull=False)
        b.box(cell, (0, 0, 7), (16, 16, 9), tx, cull=False)
        return
    b.box(cell, (7, 0, 7), (9, 16, 9), tx, cull=False)
    for d in conns:
        b.box(cell, (7, 0, 0), (9, 16, 7), tx, rot=ROT_OF[d], cull=False, skip=("south",))


def m_chain(b, cell, nm, p):
    ax = p.get("axis", "y")
    c = b.cell_min(*cell)
    mt = b.mat("chain", metal=0.9, rough=0.4)
    h = 1.5 / 16
    for k, (u0, u1) in enumerate(((0, 3 / 16), (3 / 16, 6 / 16))):
        a = math.pi / 4 + k * math.pi / 2
        dx, dy = math.cos(a) * h, math.sin(a) * h
        q = [(-dx, -dy, 0), (dx, dy, 0), (dx, dy, 1), (-dx, -dy, 1)]
        P = []
        for (x, y, z) in q:
            v = Vector((x, y, z - 0.5))
            if ax == "x":
                v = Vector((v.z, v.y, -v.x))
            elif ax == "z":
                v = Vector((v.x, v.z, -v.y))
            P.append((c[0] + 0.5 + v.x, c[1] + 0.5 + v.y, c[2] + 0.5 + v.z))
        b.models.quad(P, [(u0, 0), (u1, 0), (u1, 1), (u0, 1)], mt)


def m_lantern(b, cell, nm, p):
    tx = "soul_lantern" if nm == "soul_lantern" else "lantern"
    hang = p.get("hanging") == "true"
    o = 1 if hang else 0
    mt = b.mat(tx, emit_mask="lantern_e", es=8.0, rough=0.5, metal=0.4)
    side = {"*": mt}
    uv = {d: (0, 0, 6, 7) for d in HOR}
    uv["up"] = uv["down"] = (0, 8, 4, 12)
    b.box(cell, (5, o, 5), (11, 7 + o, 11), side, uv=uv, cull=False)
    uv2 = {d: (0, 8, 4, 10) for d in HOR}
    uv2["up"] = uv2["down"] = (4, 8, 8, 12)
    b.box(cell, (6, 7 + o, 6), (10, 9 + o, 10), side, uv=uv2, cull=False)
    if hang:
        b.box(cell, (7.5, 10, 7.5), (8.5, 16, 8.5), b.mat("chain", metal=0.9, rough=0.4), cull=False)


def m_chest(b, cell, nm, p):
    f = p.get("facing", "north")
    typ = p.get("type", "single")
    r = ROT_OF[f]
    tex = {"up": "chest_top", "down": "chest_top", "north": "chest_front", "*": "chest_side"}
    # model faces north in its own frame; partner of LEFT is clockwise of facing (vanilla ChestBlock)
    x0, x1 = 1, 15
    if typ == "left":   # partner to the facing's clockwise side -> in a north frame that is east (x+)
        x1 = 16
    elif typ == "right":
        x0 = 0
    b.box(cell, (x0, 0, 1), (x1, 14, 15), tex, rot=r)


def m_end_portal_frame(b, cell, nm, p):
    b.box(cell, (0, 0, 0), (16, 13, 16), {"up": "end_portal_frame_top", "down": "end_stone", "*": "end_portal_frame_side"})
    if p.get("eye") == "true":
        b.box(cell, (4, 13, 4), (12, 16, 12), {"up": "eye_of_ender", "down": "eye_of_ender_side", "*": "eye_of_ender_side"},
              uv={d: (4, 0, 12, 3) for d in HOR})


def m_end_portal(b, cell, nm, p):
    m = vox.end_portal_material()
    b.plane(cell, [(0, 12, 16), (16, 12, 16), (16, 12, 0), (0, 12, 0)], m, mb=b.glow)


def m_torch(b, cell, nm, p):
    tx = "torch"
    mt = b.mat(tx, emit=True, es=4.0)
    if "wall" in nm:
        f = p.get("facing", "north")
        # stick leaning away from the wall: base at the wall (opposite of facing), tip toward facing
        c = b.cell_min(*cell)
        dvec = {"north": (0, 1), "south": (0, -1), "east": (1, 0), "west": (-1, 0)}[f]
        base = Vector((c[0] + 0.5 - dvec[0] * 0.5, c[1] + 0.5 - dvec[1] * 0.5, c[2] + 0.2))
        tip = base + Vector((dvec[0] * 0.25, dvec[1] * 0.25, 0.62))
        ctr = (base + tip) / 2
        ax = (tip - base).normalized()
        R = Vector((0, 0, 1)).rotation_difference(ax).to_matrix()
        vox_obox(b, tuple(ctr), (2 / 16, 2 / 16, (tip - base).length), R, mt)
    else:
        b.box(cell, (7, 0, 7), (9, 10, 9), mt, cull=False)


def vox_obox(b, c, size, R, mat):
    sx, sy, sz = size[0] / 2, size[1] / 2, size[2] / 2
    C = Vector(c)
    for d, (off, corners) in FACES.items():
        pts, uvs = [], []
        for k in corners:
            lp = Vector(((k[0] * 2 - 1) * sx, (k[1] * 2 - 1) * sy, (k[2] * 2 - 1) * sz))
            pts.append(tuple(C + R @ lp))
            u, vv = face_uv(d, k)
            uvs.append((7 / 16 + u * 2 / 16, vv * 10 / 16))
        b.models.quad(pts, uvs, mat)


def m_door(b, cell, nm, p):
    tx = "iron_door" if nm.startswith("iron") else "oak_door"
    f = p.get("facing", "north")
    if p.get("open") == "true":
        f = cw(f, -1) if p.get("hinge") == "right" else cw(f, 1)
    edge = OPP[f]  # the panel sits on the edge opposite to the shape direction
    r = (ROT_OF[edge] + 0) % 4
    # north-edge panel: z 0..3; uv mirrored for the upper half so the window shows on top
    b.box(cell, (0, 0, 0), (16, 16, 3), tx, rot=r)


def m_trapdoor(b, cell, nm, p):
    t = "oak_door" if not nm.startswith("iron") else "iron_door"
    if p.get("open") == "true":
        edge = OPP[p.get("facing", "north")]
        b.box(cell, (0, 0, 0), (16, 16, 3), t, rot=ROT_OF[edge])
    elif p.get("half") == "top":
        b.box(cell, (0, 13, 0), (16, 16, 16), t)
    else:
        b.box(cell, (0, 0, 0), (16, 3, 16), t)


def m_carpet(b, cell, nm, p):
    col = nm[: -len("_carpet")]
    t = f"{col}_wool" if has_tex(f"{col}_wool") else ("moss_block" if col == "moss" else "white_wool")
    b.box(cell, (0, 0, 0), (16, 1, 16), t)


def m_snow(b, cell, nm, p):
    h = 2 * int(p.get("layers", 1))
    b.box(cell, (0, 0, 0), (16, h, 16), "snow")


def m_lily(b, cell, nm, p):
    r = _hash(cell) % 4
    b.plane(cell, [(0, 0.3, 16), (16, 0.3, 16), (16, 0.3, 0), (0, 0.3, 0)], "lily_pad",
            uvs=[((0, 0), (1, 0), (1, 1), (0, 1))[(n + r) % 4] for n in range(4)])


def m_vine(b, cell, nm, p):
    tx = "vine"
    for d in HOR:
        if p.get(d) == "true":
            r = ROT_OF[d]
            # plane 0.8px inside the north edge of a north-frame block, rotated
            pts = [(0, 0, 0.8), (16, 0, 0.8), (16, 16, 0.8), (0, 16, 0.8)]
            rp = []
            for (x, y, z) in pts:
                for _ in range(r):
                    x, z = 16 - z, x
                rp.append((x, y, z))
            b.plane(cell, rp, tx)
    if p.get("up") == "true":
        b.plane(cell, [(0, 15.2, 16), (16, 15.2, 16), (16, 15.2, 0), (0, 15.2, 0)], tx)


def m_petals(b, cell, nm, p):
    n = int(p.get("flower_amount", p.get("segment_amount", 4)))
    r = ROT_OF[p.get("facing", "north")]
    tx = "pink_petals" if nm == "pink_petals" else ("dandelion" if nm == "wildflowers" else "dead_bush")
    quads = [(0, 0), (1, 0), (1, 1), (0, 1)][:n]  # NW, NE, SE, SW for a north-facing patch
    hh = 1.5 + (_hash(cell) % 3) * 0.3
    for (qx, qz) in quads:
        for _ in range(r):
            qx, qz = 1 - qz, qx
        x0, z0 = qx * 8, qz * 8
        P = [(x0, hh, z0 + 8), (x0 + 8, hh, z0 + 8), (x0 + 8, hh, z0), (x0, hh, z0)]
        U = [(x0 / 16, 1 - (z0 + 8) / 16), ((x0 + 8) / 16, 1 - (z0 + 8) / 16), ((x0 + 8) / 16, 1 - z0 / 16), (x0 / 16, 1 - z0 / 16)]
        if nm == "leaf_litter":
            continue
        b.plane(cell, P, tx, uvs=U)


def m_crop(b, cell, nm, p):
    tx = {"wheat": "wheat", "carrots": "carrots", "potatoes": "potatoes", "beetroots": "beetroots"}.get(nm, "wheat")
    age = int(p.get("age", 7))
    mx = 7 if nm != "beetroots" else 3
    h = 16 * (0.35 + 0.65 * age / mx)
    for x in (4, 12):
        b.plane(cell, [(x, -1, 16), (x, -1, 0), (x, h - 1, 0), (x, h - 1, 16)], tx)
    for z in (4, 12):
        b.plane(cell, [(0, -1, z), (16, -1, z), (16, h - 1, z), (0, h - 1, z)], tx)


def m_cross(b, cell, nm, p):
    if nm in DOUBLE:
        lo, hi = DOUBLE[nm]
        tx = hi if p.get("half") == "upper" else lo
    else:
        tx = CROSS_TEX.get(nm, nm)
        if not has_tex(tx):
            b.unknown[nm] = b.unknown.get(nm, 0) + 1
            return
    if nm in ("fire", "soul_fire"):
        m = b.mat(tx)
        for k in range(2):
            a = math.pi / 4 + k * math.pi / 2
            dx, dy = math.cos(a) * 8 * 1.3, math.sin(a) * 8 * 1.3
            b.plane(cell, [(8 - dx, 0, 8 - dy), (8 + dx, 0, 8 + dy), (8 + dx, 22, 8 + dy), (8 - dx, 22, 8 - dy)], m, mb=b.glow)
        return
    h = _hash(cell)
    ox = oz = 0.0
    if nm not in ("sugar_cane", "kelp", "kelp_plant", "weeping_vines", "weeping_vines_plant", "twisting_vines",
                  "twisting_vines_plant", "cobweb", "tall_seagrass", "seagrass", "nether_wart") and nm not in DOUBLE:
        ox = ((h & 15) / 15 - 0.5) * 6
        oz = (((h >> 4) & 15) / 15 - 0.5) * 6
    elif nm in DOUBLE:
        hb = _hash((cell[0], 0, cell[2]))
        ox = ((hb & 15) / 15 - 0.5) * 6
        oz = (((hb >> 4) & 15) / 15 - 0.5) * 6
    s = 8 * 1.414 * (0.9 if nm in ("tall_seagrass", "seagrass", "kelp", "kelp_plant") else 1.0)
    for k in range(2):
        a = math.pi / 4 + k * math.pi / 2
        dx, dz = math.cos(a) * s / 1.414 * 1.0, math.sin(a) * s / 1.414
        b.plane(cell, [(8 + ox - dx, 0, 8 + oz - dz), (8 + ox + dx, 0, 8 + oz + dz),
                       (8 + ox + dx, 16, 8 + oz + dz), (8 + ox - dx, 16, 8 + oz - dz)], tx)


def m_flat_cross(b, cell, nm, p):
    m_cross(b, cell, nm, p)


def m_bed(b, cell, nm, p):
    col = nm[: -len("_bed")]
    t = f"{col}_wool" if has_tex(f"{col}_wool") else "red_wool"
    b.box(cell, (0, 3, 0), (16, 9, 16), {"up": t, "down": "oak_planks", "*": t})


def m_small(tex, p0, p1):
    def f(b, cell, nm, p):
        b.box(cell, p0, p1, tex)
    return f


def m_coral_fan(b, cell, nm, p):
    col = {"tube": "blue_wool", "brain": "pink_wool", "bubble": "purple_wool", "fire": "red_wool", "horn": "yellow_wool"}
    t = "seagrass"
    for k, v in col.items():
        if nm.startswith(k) and has_tex(v):
            t = v
    if "wall" in nm:
        r = ROT_OF[OPP[p.get("facing", "north")]]
        b.box(cell, (2, 4, 0), (14, 12, 1), t, rot=r)
    else:
        b.box(cell, (3, 0, 3), (13, 1, 13), t)


def m_sea_pickle(b, cell, nm, p):
    n = int(p.get("pickles", 1))
    for q in range(n):
        x = 4 + (q % 2) * 6
        z = 4 + (q // 2) * 6
        b.box(cell, (x, 0, z), (x + 3, 5, z + 3), b.mat("lily_pad", emit=True, es=1.5))


def m_spawner_fx(b, cell, nm, p):
    """The cage itself is a cutout cube; vanilla also shows small flames inside it."""
    m = b.mat("fire", es=float(SPAWNER_FX[0]))
    h = _hash(cell)
    for k in range(3):
        x = 4 + (h >> (k * 4) & 7)
        z = 4 + (h >> (k * 4 + 2) & 7)
        y = 3 + k * 3
        s = 2.2
        for a in (0.6, 2.17):
            dx, dz = math.cos(a) * s, math.sin(a) * s
            b.plane(cell, [(x - dx, y, z - dz), (x + dx, y, z + dz), (x + dx, y + 2 * s, z + dz), (x - dx, y + 2 * s, z - dz)],
                    m, mb=b.glow)


SPAWNER_FX = [2.5]


def m_skip(b, cell, nm, p):
    pass


def m_campfire(b, cell, nm, p):
    b.box(cell, (1, 0, 1), (15, 4, 15), "oak_log")
    if p.get("lit", "true") == "true":
        m_cross(b, cell, "fire", {})


def m_bell(b, cell, nm, p):
    b.box(cell, (5, 4, 5), (11, 11, 11), b.mat("gold_block"))


def m_cauldron(b, cell, nm, p):
    t = "cobblestone"
    b.box(cell, (0, 3, 0), (16, 16, 2), t)
    b.box(cell, (0, 3, 14), (16, 16, 16), t)
    b.box(cell, (0, 3, 2), (2, 16, 14), t)
    b.box(cell, (14, 3, 2), (16, 16, 14), t)
    b.box(cell, (2, 3, 2), (14, 4, 14), t)
    b.box(cell, (0, 0, 0), (4, 3, 4), t)
    b.box(cell, (12, 0, 12), (16, 3, 16), t)


def m_anvil(b, cell, nm, p):
    b.box(cell, (2, 0, 2), (14, 4, 14), "iron_block")
    b.box(cell, (4, 4, 3), (12, 10, 13), "iron_block")
    b.box(cell, (3, 10, 0), (13, 16, 16), "iron_block")


def m_flower_pot(b, cell, nm, p):
    b.box(cell, (5, 0, 5), (11, 6, 11), "terracotta")


def m_ladder(b, cell, nm, p):
    r = ROT_OF[OPP[p.get("facing", "north")]]
    b.box(cell, (0, 0, 0), (16, 16, 1), "oak_planks", rot=r)


def m_rail(b, cell, nm, p):
    b.box(cell, (0, 0, 0), (16, 1, 16), b.mat("oak_planks", alpha_const=1.0))


def m_dripleaf(b, cell, nm, p):
    m_cross(b, cell, "fern", p)


def m_cactus(b, cell, nm, p):
    b.box(cell, (1, 0, 1), (15, 16, 15), "moss_block")


MODELS = {
    "iron_bars": m_pane, "glass_pane": m_pane, "chain": m_chain, "iron_chain": m_chain, "lantern": m_lantern,
    "soul_lantern": m_lantern, "chest": m_chest, "trapped_chest": m_chest, "ender_chest": m_chest,
    "end_portal_frame": m_end_portal_frame, "end_portal": m_end_portal, "torch": m_torch, "wall_torch": m_torch,
    "soul_torch": m_torch, "soul_wall_torch": m_torch, "redstone_torch": m_torch, "redstone_wall_torch": m_torch,
    "snow": m_snow, "lily_pad": m_lily, "vine": m_vine, "pink_petals": m_petals, "wildflowers": m_petals,
    "leaf_litter": m_skip, "wheat": m_crop, "carrots": m_crop, "potatoes": m_crop, "beetroots": m_crop,
    "sea_pickle": m_sea_pickle, "campfire": m_campfire, "soul_campfire": m_campfire, "bell": m_bell,
    "cauldron": m_cauldron, "water_cauldron": m_cauldron, "anvil": m_anvil, "chipped_anvil": m_anvil,
    "damaged_anvil": m_anvil, "flower_pot": m_flower_pot, "ladder": m_ladder, "rail": m_rail,
    "powered_rail": m_rail, "detector_rail": m_rail, "activator_rail": m_rail, "cactus": m_cactus,
    "brewing_stand": m_small("cobblestone", (6, 0, 6), (10, 14, 10)), "grindstone": m_small("stone", (2, 4, 2), (14, 16, 14)),
    "lever": m_skip, "tripwire": m_skip, "tripwire_hook": m_skip, "redstone_wire": m_skip, "glow_lichen": m_skip,
    "candle": m_small("white_wool", (7, 0, 7), (9, 6, 9)), "white_candle": m_small("white_wool", (7, 0, 7), (9, 6, 9)),
    "big_dripleaf": m_dripleaf, "small_dripleaf": m_dripleaf, "heavy_core": m_skip, "vault": m_small("deepslate", (0, 0, 0), (16, 16, 16)),
    "fire": m_cross, "soul_fire": m_cross, "pointed_dripstone": m_skip,
    "turtle_egg": m_skip, "scaffolding": m_skip,
    "skeleton_skull": m_small("bone_block_side", (4, 0, 4), (12, 8, 12)),
}
for _f in ("dandelion", "poppy", "blue_orchid", "allium", "azure_bluet", "red_tulip", "orange_tulip", "white_tulip",
           "pink_tulip", "oxeye_daisy", "cornflower", "lily_of_the_valley", "wither_rose", "oak_sapling",
           "spruce_sapling", "birch_sapling", "jungle_sapling", "acacia_sapling", "dark_oak_sapling", "fern",
           "dead_bush", "cactus", "bamboo", "red_mushroom", "brown_mushroom", "crimson_fungus", "warped_fungus",
           "crimson_roots", "warped_roots"):
    MODELS[f"potted_{_f}"] = m_flower_pot
SUFFIX_MODELS = [
    ("_stairs", m_stairs), ("_slab", m_slab), ("_wall", m_wall), ("_fence_gate", m_fence_gate), ("_fence", m_fence),
    ("_pane", m_pane), ("_door", m_door), ("_trapdoor", m_trapdoor), ("_carpet", m_carpet), ("_bed", m_bed),
    ("_coral_wall_fan", m_coral_fan), ("_coral_fan", m_coral_fan), ("_button", m_skip), ("_pressure_plate", m_skip),
    ("_banner", m_skip), ("_sign", m_skip), ("_candle", m_small("white_wool", (7, 0, 7), (9, 6, 9))),
    ("_head", m_skip), ("_skull", m_skip), ("_wall_torch", m_torch), ("_torch", m_torch),
    ("_tulip", m_cross), ("_sapling", m_cross), ("_mushroom", m_cross),
]


def embers(P, centre, extent, n=260, strength=30.0, seed=5, color=(1.0, 0.45, 0.12), smin=0.04, smax=0.1, name="embers"):
    """Small glowing specks floating in the air (MC coords via P); they do not cast shadows."""
    rng = np.random.default_rng(seed)
    em = vox.emission_material(name + "_m", color, strength)
    gm = vox.MeshBuilder()
    for _ in range(n):
        c = [centre[a] + rng.uniform(-extent[a], extent[a]) for a in range(3)]
        s = rng.uniform(smin, smax)
        p = P(*c)
        gm.quad([(p[0] - s, p[1], p[2] - s), (p[0] + s, p[1], p[2] - s), (p[0] + s, p[1], p[2] + s), (p[0] - s, p[1], p[2] + s)],
                [(0, 0), (1, 0), (1, 1), (0, 1)], em)
        gm.quad([(p[0], p[1] - s, p[2] - s), (p[0], p[1] + s, p[2] - s), (p[0], p[1] + s, p[2] + s), (p[0], p[1] - s, p[2] + s)],
                [(0, 0), (1, 0), (1, 1), (0, 1)], em)
    ob = gm.build(name)
    if ob:
        ob.visible_shadow = False
    return ob


def world_camera_color(color, strength=1.0):
    """Keep the world's lighting, but show the camera a different (darker) background colour."""
    w = bpy.context.scene.world
    nt = w.node_tree
    out = [n for n in nt.nodes if n.type == "OUTPUT_WORLD"][0]
    src = out.inputs["Surface"].links[0].from_socket
    lp = nt.nodes.new("ShaderNodeLightPath")
    bg = nt.nodes.new("ShaderNodeBackground")
    bg.inputs["Color"].default_value = (*color, 1)
    bg.inputs["Strength"].default_value = strength
    mx = nt.nodes.new("ShaderNodeMixShader")
    nt.links.new(lp.outputs["Is Camera Ray"], mx.inputs[0])
    nt.links.new(src, mx.inputs[1])
    nt.links.new(bg.outputs[0], mx.inputs[2])
    nt.links.new(mx.outputs[0], out.inputs["Surface"])


def aim_area(loc, target, size, energy, color, size_y=None, name="Fill"):
    """Area light at loc (Blender coords) pointing at target."""
    from mathutils import Vector as V
    ob = vox.area(loc, (0, 0, 0), size, energy, color, size_y=size_y, name=name)
    d = V(target) - V(loc)
    ob.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()
    return ob
