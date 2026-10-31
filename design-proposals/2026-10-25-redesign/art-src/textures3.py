"""Third batch of procedural 16x16 textures (original drawings, no Mojang assets).

A clearer chest: planked wood, dark outline on the visible 14x14 face, lid seam and a bright latch.
Huge-mushroom blocks (brown cap, red cap with white spots, stem) for the dark forest behind the village.
uv run --with numpy --with pillow python textures3.py  -> /tmp/srl/art/out/textures/
"""
import numpy as np
import textures as T
from textures import R, hexc, pal, vnoise, white, paint, save, S


def chest():
    r = R(321)
    wood = pal("#7a4a22", "#8f5a2a", "#a56b33", "#b77b3c", "#c88d48")
    v = 0.45 * white(r) + 0.55 * vnoise(r, 8, 2)
    base = paint(v, wood)
    # plank seams every 4 px (rows), slightly darker
    for row in (5, 9, 13):
        base[row, :] = base[row, :] * 0.72
    dark = hexc("#3a2210")
    # the model box spans pixels 1..15 horizontally and image rows 2..15 on the sides (14 px tall)
    top = base.copy()
    top[1, 1:15] = top[14, 1:15] = dark
    top[1:15, 1] = top[1:15, 14] = dark
    save("chest_top", top)
    side = base.copy()
    side[2, 1:15] = side[15, 1:15] = dark
    side[2:16, 1] = side[2:16, 14] = dark
    side[6:8, 1:15] = hexc("#2e1a0c")  # lid seam (lid = top 4 px of the 14 px box)
    side[5, 2:14] = side[5, 2:14] * 1.12
    save("chest_side", np.clip(side, 0, 255))
    front = side.copy()
    front[5:10, 7:9] = hexc("#d9dde2")
    front[6:9, 7:9] = hexc("#aab0b8")
    front[9, 7:9] = hexc("#4a4f56")
    front[5, 7] = hexc("#f4f6f8")
    save("chest_front", np.clip(front, 0, 255))


def mushrooms():
    from textures2 import noise_tex
    brown = noise_tex(331, pal("#6f5038", "#7c5a3f", "#886446", "#956f4e", "#a27a57"), [1, 2, 3, 2, 1], w=0.5)
    r = R(332)
    for _ in range(7):  # a few pale flecks
        x, y = int(r.integers(0, S)), int(r.integers(0, S))
        brown[y, x] = brown[y, x] * 0.5 + hexc("#c2a07c") * 0.5
    save("brown_mushroom_block", np.clip(brown, 0, 255))
    red = noise_tex(333, pal("#8e1a16", "#a2201b", "#b42a22", "#c4352b"), [1, 2, 3, 2], w=0.45)
    spot = hexc("#ece6da")
    for (x, y, w, h) in [(2, 2, 3, 2), (10, 1, 2, 2), (6, 7, 3, 3), (12, 9, 3, 2), (1, 11, 2, 3), (8, 13, 2, 2)]:
        red[y:y + h, x:x + w] = spot
        red[y + h - 1, x:x + w] = hexc("#cfc8ba")
    save("red_mushroom_block", np.clip(red, 0, 255))
    stem = noise_tex(334, pal("#c9bfa6", "#d4cab2", "#ddd4be", "#e6dec9"), w=0.35, cx=6, cy=1)
    save("mushroom_stem", np.clip(stem, 0, 255))


if __name__ == "__main__":
    chest()
    mushrooms()
    print("ok")
