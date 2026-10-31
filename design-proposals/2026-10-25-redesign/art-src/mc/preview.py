"""Quick top-down preview of a world box (for picking spots). Writes to /tmp/srl/mcworld/previews/.

usage: preview.py <worlddir> <dim n|o> x0 z0 x1 z1 y0 y1 name [scale]
"""
import os
import sys
import zlib
import numpy as np
from PIL import Image

sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcregion import Dim, extract_box

KEY = [("water", (40, 80, 190)), ("lava", (255, 120, 20)), ("magma", (180, 60, 20)), ("grass_block", (90, 150, 60)),
       ("cherry_leaves", (240, 170, 200)), ("leaves", (40, 100, 35)), ("log", (100, 70, 40)), ("planks", (170, 130, 80)), ("sand", (220, 205, 150)),
       ("gravel", (130, 125, 120)), ("netherrack", (110, 35, 35)), ("nether_brick", (60, 25, 30)),
       ("blackstone", (40, 36, 44)), ("basalt", (85, 85, 90)), ("gold", (240, 200, 60)), ("soul", (80, 60, 45)),
       ("nylium", (150, 30, 40)), ("warped", (30, 120, 110)), ("crimson", (140, 30, 40)), ("obsidian", (30, 20, 50)),
       ("stone_brick", (120, 120, 120)), ("cobblestone", (110, 110, 110)), ("stone", (125, 125, 125)), ("dirt", (120, 85, 55)),
       ("snow", (240, 240, 245)), ("ice", (160, 190, 240)), ("glowstone", (255, 230, 140)), ("path", (150, 120, 70)),
       ("farmland", (100, 70, 40)), ("wheat", (200, 190, 80)), ("hay", (210, 180, 60)), ("terracotta", (160, 90, 60)),
       ("clay", (160, 165, 180)), ("sandstone", (215, 200, 140)), ("kelp", (50, 110, 40)), ("seagrass", (50, 120, 50)),
       ("cherry_leaves", (240, 170, 200)), ("cherry", (90, 40, 50)), ("pink_petals", (240, 160, 200)), ("bedrock", (30, 30, 30))]


def color(name):
    name = name.split("[")[0]
    for k, c in KEY:
        if k in name:
            return c
    h = zlib.crc32(name.encode())
    return (h & 255, (h >> 8) & 255, (h >> 16) & 255)


def main():
    wd, dk = sys.argv[1], sys.argv[2]
    x0, z0, x1, z1, y0, y1 = map(int, sys.argv[3:9])
    name = sys.argv[9]
    sc = int(sys.argv[10]) if len(sys.argv) > 10 else 3
    sub = {"n": "DIM-1/region", "o": "region", "e": "DIM1/region"}[dk]
    pal, arr = extract_box(Dim(f"{wd}/world/{sub}"), x0, y0, z0, x1, y1, z1)
    skip = set(i for i, p in enumerate(pal) if p.split("[")[0] in ("minecraft:air", "minecraft:cave_air", "minecraft:void_air"))
    solid = ~np.isin(arr, list(skip))
    nx, ny, nz = arr.shape
    top = np.where(solid.any(1), ny - 1 - np.argmax(solid[:, ::-1, :], axis=1), -1)  # (nx, nz)
    cols = np.array([color(p) for p in pal], float)
    ii = np.clip(top, 0, ny - 1)
    blk = np.take_along_axis(arr, ii[:, None, :], 1)[:, 0, :]
    img = cols[blk]
    shade = 0.55 + 0.45 * (top - top.min()) / max(1, top.max() - top.min())
    gx = np.gradient(top.astype(float), axis=0)
    img = img * shade[..., None] * np.clip(1 - 0.06 * gx, 0.6, 1.3)[..., None]
    img[top < 0] = (0, 0, 0)
    im = Image.fromarray(np.clip(img.transpose(1, 0, 2), 0, 255).astype(np.uint8))  # rows = z, cols = x
    im = im.resize((nx * sc, nz * sc), Image.NEAREST)
    os.makedirs("/tmp/srl/mcworld/previews", exist_ok=True)
    im.save(f"/tmp/srl/mcworld/previews/{name}.png")
    names = {}
    for i, p in enumerate(pal):
        names[p.split("[")[0]] = names.get(p.split("[")[0], 0) + int((arr == i).sum())
    print(name, arr.shape, "top", top.min(), top.max())
    print(sorted(names.items(), key=lambda kv: -kv[1])[:40])


if __name__ == "__main__":
    main()
