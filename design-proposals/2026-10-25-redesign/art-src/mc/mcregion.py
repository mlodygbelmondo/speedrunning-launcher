"""Minimal Anvil region / NBT reader (1.16 "Level/Sections/BlockStates" and 1.18+ "sections/block_states").

Extracted block layouts are kept in /tmp/srl/mcworld only (never shipped).
"""
import gzip
import math
import os
import struct
import zlib
import numpy as np

# ---------------------------------------------------------------- NBT


class R:
    __slots__ = ("b", "i")

    def __init__(self, b):
        self.b, self.i = b, 0

    def take(self, n):
        v = self.b[self.i:self.i + n]
        self.i += n
        return v


def _payload(r, t):
    if t == 1:
        return struct.unpack(">b", r.take(1))[0]
    if t == 2:
        return struct.unpack(">h", r.take(2))[0]
    if t == 3:
        return struct.unpack(">i", r.take(4))[0]
    if t == 4:
        return struct.unpack(">q", r.take(8))[0]
    if t == 5:
        return struct.unpack(">f", r.take(4))[0]
    if t == 6:
        return struct.unpack(">d", r.take(8))[0]
    if t == 7:
        n = struct.unpack(">i", r.take(4))[0]
        return np.frombuffer(r.take(n), dtype=">i1").copy()
    if t == 8:
        n = struct.unpack(">H", r.take(2))[0]
        return r.take(n).decode("utf-8", "replace")
    if t == 9:
        et = r.take(1)[0]
        n = struct.unpack(">i", r.take(4))[0]
        return [_payload(r, et) for _ in range(n)]
    if t == 10:
        d = {}
        while True:
            tt = r.take(1)[0]
            if tt == 0:
                return d
            nl = struct.unpack(">H", r.take(2))[0]
            name = r.take(nl).decode("utf-8", "replace")
            d[name] = _payload(r, tt)
    if t == 11:
        n = struct.unpack(">i", r.take(4))[0]
        return np.frombuffer(r.take(4 * n), dtype=">i4").copy()
    if t == 12:
        n = struct.unpack(">i", r.take(4))[0]
        return np.frombuffer(r.take(8 * n), dtype=">i8").copy()
    raise ValueError(f"bad tag {t}")


def nbt_load(b):
    r = R(b)
    t = r.take(1)[0]
    nl = struct.unpack(">H", r.take(2))[0]
    r.take(nl)
    return _payload(r, t)


# ---------------------------------------------------------------- region


class Region:
    def __init__(self, path):
        self.path = path
        with open(path, "rb") as f:
            self.data = f.read()

    def chunk(self, lx, lz):
        i = 4 * ((lx & 31) + (lz & 31) * 32)
        off = int.from_bytes(self.data[i:i + 3], "big")
        if off == 0:
            return None
        o = off * 4096
        ln = struct.unpack(">i", self.data[o:o + 4])[0]
        comp = self.data[o + 4]
        raw = self.data[o + 5:o + 4 + ln]
        if comp == 2:
            raw = zlib.decompress(raw)
        elif comp == 1:
            raw = gzip.decompress(raw)
        elif comp == 3:
            pass
        else:
            raise ValueError(f"compression {comp}")
        return nbt_load(raw)


class Dim:
    """A dimension folder ('region' dir); caches regions."""

    def __init__(self, region_dir):
        if not os.path.isdir(region_dir):  # 26.x layout: world/dimensions/minecraft/<dim>/region
            alt = {"/world/region": "/world/dimensions/minecraft/overworld/region",
                   "/world/DIM-1/region": "/world/dimensions/minecraft/the_nether/region",
                   "/world/DIM1/region": "/world/dimensions/minecraft/the_end/region"}
            for a, b in alt.items():
                if region_dir.endswith(a) and os.path.isdir(region_dir[:-len(a)] + b):
                    region_dir = region_dir[:-len(a)] + b
        self.dir = region_dir
        self.cache = {}

    def chunk(self, cx, cz):
        key = (cx >> 5, cz >> 5)
        if key not in self.cache:
            p = f"{self.dir}/r.{key[0]}.{key[1]}.mca"
            self.cache[key] = Region(p) if os.path.exists(p) else None
        reg = self.cache[key]
        return reg.chunk(cx & 31, cz & 31) if reg else None


def state_string(entry):
    # 1.21.5+ stores heterogeneous lists as compounds wrapping the value under the key ""
    if isinstance(entry, dict) and "" in entry and "Name" not in entry:
        entry = entry[""]
    if isinstance(entry, str):
        return entry
    if "Name" not in entry and "id" in entry:  # 26.x: {"id": ..., "properties": {...}}
        name, props = entry["id"], entry.get("properties")
    else:
        name, props = entry["Name"], entry.get("Properties")
    if props:
        name += "[" + ",".join(f"{k}={v}" for k, v in sorted(props.items())) + "]"
    return name


def unpack(longs, bits, count=4096):
    """Non-spanning packed indices (1.16+)."""
    per = 64 // bits
    a = np.asarray(longs, dtype=np.int64).view(np.uint64)
    shifts = (np.arange(per, dtype=np.uint64) * np.uint64(bits))
    mask = np.uint64((1 << bits) - 1)
    vals = ((a[:, None] >> shifts[None, :]) & mask).reshape(-1)
    return vals[:count].astype(np.int32)


def chunk_sections(ch):
    """Yield (sectionY, palette_strings, idx[4096] as (y,z,x)) for both formats."""
    if "Level" in ch:  # 1.16
        lv = ch["Level"]
        for s in lv.get("Sections", []):
            pal = s.get("Palette")
            if not pal or "BlockStates" not in s:
                continue
            ps = [state_string(e) for e in pal]
            bits = max(4, math.ceil(math.log2(len(ps))))
            idx = unpack(s["BlockStates"], bits)
            yield s["Y"], ps, idx.reshape(16, 16, 16)
    else:  # 1.18+
        for s in ch.get("sections", []):
            bs = s.get("block_states")
            if not bs:
                continue
            pal = bs["palette"]
            ps = [state_string(e) for e in pal]
            if "data" not in bs or len(ps) == 1:
                idx = np.zeros(4096, np.int32)
            else:
                bits = max(4, math.ceil(math.log2(len(ps))))
                idx = unpack(bs["data"], bits)
            yield s["Y"], ps, idx.reshape(16, 16, 16)


def chunk_status(ch):
    if "Level" in ch:
        return ch["Level"].get("Status")
    return ch.get("Status")


def structure_starts(ch):
    if "Level" in ch:
        return ch["Level"].get("Structures", {}).get("Starts", {})
    return ch.get("structures", {}).get("starts", {})


def extract_box(dim, x0, y0, z0, x1, y1, z1, air="minecraft:air"):
    """Return (palette list, uint16 array [nx, ny, nz]) for the inclusive block box."""
    pal = {air: 0}
    nx, ny, nz = x1 - x0 + 1, y1 - y0 + 1, z1 - z0 + 1
    out = np.zeros((nx, ny, nz), np.uint16)
    missing = 0
    for cx in range(x0 >> 4, (x1 >> 4) + 1):
        for cz in range(z0 >> 4, (z1 >> 4) + 1):
            ch = dim.chunk(cx, cz)
            if ch is None:
                missing += 1
                continue
            for sy, ps, idx in chunk_sections(ch):
                for p in ps:
                    if p not in pal:
                        pal[p] = len(pal)
                gmap = np.array([pal[p] for p in ps], np.int32)
                g = gmap[idx]  # (y,z,x)
                bx0, by0, bz0 = cx * 16, sy * 16, cz * 16
                # overlap
                ax0, ax1 = max(x0, bx0), min(x1, bx0 + 15)
                ay0, ay1 = max(y0, by0), min(y1, by0 + 15)
                az0, az1 = max(z0, bz0), min(z1, bz0 + 15)
                if ax0 > ax1 or ay0 > ay1 or az0 > az1:
                    continue
                sub = g[ay0 - by0:ay1 - by0 + 1, az0 - bz0:az1 - bz0 + 1, ax0 - bx0:ax1 - bx0 + 1]
                out[ax0 - x0:ax1 - x0 + 1, ay0 - y0:ay1 - y0 + 1, az0 - z0:az1 - z0 + 1] = sub.transpose(2, 0, 1)
    inv = [None] * len(pal)
    for k, v in pal.items():
        inv[v] = k
    if missing:
        print(f"[mcregion] {missing} chunks missing in box")
    return inv, out
