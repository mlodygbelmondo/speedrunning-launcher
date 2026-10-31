"""Procedural 16x16 pixel-art block textures (all original, no Mojang assets).

Run: uv run textures.py  -> writes PNGs to /tmp/srl/art/out/textures/
Emission / metal masks are written as <name>_e.png / <name>_m.png (greyscale).
"""
import os
import numpy as np
from PIL import Image

OUT = "/tmp/srl/art/out/textures"
WORK = "/tmp/srl/art/work"
S = 16
os.makedirs(OUT, exist_ok=True)
os.makedirs(WORK, exist_ok=True)
SAVED = []


# ---------------------------------------------------------------- helpers
def R(seed):
    return np.random.default_rng(seed)


def hexc(h):
    h = h.lstrip("#")
    return np.array([int(h[i:i + 2], 16) for i in (0, 2, 4)], float)


def pal(*hs):
    return np.array([hexc(h) for h in hs])


def vnoise(r, cx=4, cy=4):
    """Tileable value noise, cell size cx x cy pixels, smooth-step interpolation."""
    nx, ny = S // cx, S // cy
    g = r.random((ny, nx))
    y = np.arange(S) / cy
    x = np.arange(S) / cx
    y0 = np.floor(y).astype(int)
    x0 = np.floor(x).astype(int)
    fy, fx = y - y0, x - x0
    fy = fy * fy * (3 - 2 * fy)
    fx = fx * fx * (3 - 2 * fx)
    y1, x1 = (y0 + 1) % ny, (x0 + 1) % nx
    y0, x0 = y0 % ny, x0 % nx
    a = g[y0][:, x0]
    b = g[y0][:, x1]
    c = g[y1][:, x0]
    d = g[y1][:, x1]
    FX, FY = fx[None, :], fy[:, None]
    return (a * (1 - FX) + b * FX) * (1 - FY) + (c * (1 - FX) + d * FX) * FY


def white(r):
    return r.random((S, S))


def quant(v, props):
    """Rank-quantise v into len(props) buckets with the given proportions."""
    props = np.asarray(props, float)
    props = props / props.sum()
    cuts = np.quantile(v, np.cumsum(props)[:-1])
    return np.searchsorted(cuts, v, side="right")


def paint(v, p, props=None):
    if props is None:
        props = [1] * len(p)
    return p[quant(v, props)].copy()


def save(name, rgb, alpha=None):
    rgb = np.clip(rgb, 0, 255).astype(np.uint8)
    if alpha is not None:
        a = np.clip(alpha * 255 if alpha.max() <= 1 else alpha, 0, 255).astype(np.uint8)
        img = Image.fromarray(np.dstack([rgb, a]), "RGBA")
    else:
        img = Image.fromarray(rgb, "RGB")
    img.save(f"{OUT}/{name}.png")
    SAVED.append(name)


def save_mask(name, m):
    m = np.clip(m * 255, 0, 255).astype(np.uint8)
    Image.fromarray(m, "L").save(f"{OUT}/{name}.png")


def put(img, x, y, c):
    img[y % S, x % S] = hexc(c) if isinstance(c, str) else c


# ---------------------------------------------------------------- earth
GRASS = pal("#3d6b22", "#477a29", "#528a30", "#5d9a37", "#6aa83f", "#7ab64a")
DIRT = pal("#563b26", "#654630", "#735237", "#835e40", "#926a49")


def grass_top(seed=1):
    r = R(seed)
    v = 0.6 * white(r) + 0.4 * vnoise(r, 4, 4)
    img = paint(v, GRASS, [1, 2, 3, 3, 2, 1])
    return img


def dirt(seed=2):
    r = R(seed)
    v = 0.6 * white(r) + 0.4 * vnoise(r, 4, 4)
    img = paint(v, DIRT, [1, 2, 3, 2, 1])
    for _ in range(7):  # pebbles
        x, y = r.integers(0, S, 2)
        put(img, x, y, ["#8a7a68", "#9c8b76", "#4a3220"][r.integers(0, 3)])
    return img


def grass_side(seed=3):
    r = R(seed)
    img = dirt(seed + 100)
    gt = grass_top(seed + 200)
    depth = np.clip(np.round(2.6 + 1.2 * np.sin(np.arange(S) * 1.7 + r.random() * 6)
                             + r.normal(0, 0.8, S)), 2, 5).astype(int)
    for x in range(S):
        d = depth[x]
        for y in range(d):
            img[y, x] = gt[y, x]
        img[d - 1, x] = GRASS[1] if r.random() < 0.7 else GRASS[0]
        if r.random() < 0.18:  # drip
            img[d, x] = GRASS[0]
    return img


def stone(seed=4, p=None):
    r = R(seed)
    p = pal("#626262", "#6e6e6e", "#7a7a7a", "#858585", "#929292", "#a0a0a0") if p is None else p
    v = 0.5 * white(r) + 0.35 * vnoise(r, 8, 2) + 0.15 * vnoise(r, 4, 4)
    img = paint(v, p, [1, 2, 3, 3, 2, 1])
    for _ in range(3):  # small dark cracks
        x, y = r.integers(0, S, 2)
        for i in range(r.integers(2, 4)):
            img[y % S, (x + i) % S] = p[0] * 0.9
    return img


def deepslate(seed=5):
    r = R(seed)
    p = pal("#2f2f35", "#38383f", "#424249", "#4c4c54", "#56565e", "#62626a")
    rows = np.repeat(r.random(S)[:, None], S, 1)
    v = 0.35 * rows + 0.35 * vnoise(r, 8, 1) + 0.3 * white(r)
    return paint(v, p, [1, 2, 3, 3, 2, 1])


def sand(seed=6):
    r = R(seed)
    p = pal("#c7b277", "#d2bf86", "#dac991", "#e1d29c", "#e9dba8")
    v = 0.8 * white(r) + 0.2 * vnoise(r, 4, 4)
    return paint(v, p, [1, 2, 4, 2, 1])


def water(seed=7):
    r = R(seed)
    p = pal("#1f4a91", "#26559f", "#2e61ad", "#376dba", "#4580c9", "#6a9fdc")
    y, x = np.mgrid[0:S, 0:S]
    v = 0.5 + 0.5 * np.sin((x * 2 * np.pi / 16) * 1 + (y * 2 * np.pi / 16) * 2 + 3 * vnoise(r, 8, 8))
    v = 0.6 * v + 0.4 * white(r)
    return paint(v, p, [2, 3, 4, 3, 2, 0.6])


# ---------------------------------------------------------------- wood
BARK = pal("#3b2a19", "#4a3521", "#574029", "#644a30", "#715436", "#7d5e3c")


def log_side(seed=8):
    r = R(seed)
    col = np.repeat(r.random(S)[None, :], S, 0)
    v = 0.5 * col + 0.3 * vnoise(r, 2, 8) + 0.2 * white(r)
    img = paint(v, BARK, [1, 2, 3, 3, 2, 1])
    for gx in r.choice(S, 3, replace=False):  # deep grooves
        y0 = r.integers(0, S)
        for i in range(r.integers(4, 9)):
            img[(y0 + i) % S, gx] = BARK[0]
    return img


def log_top(seed=9):
    r = R(seed)
    y, x = np.mgrid[0:S, 0:S]
    d = np.maximum(abs(x - 7.5), abs(y - 7.5)) + 0.6 * (white(r) - 0.5)
    rings = pal("#a5814e", "#b58f5a", "#9a7646", "#b08a56")
    img = rings[(np.floor(d / 1.6).astype(int)) % 4].astype(float)
    img *= (0.94 + 0.12 * white(r))[..., None]
    img[(d < 1.2)] = hexc("#8c6a3e")
    edge = (np.maximum(abs(x - 7.5), abs(y - 7.5)) > 6.6)
    img[edge] = BARK[quant(white(r), [1, 2, 2])[edge] + 1]
    return img


def leaves(seed=10):
    r = R(seed)
    p = pal("#264b17", "#2e5a1d", "#376823", "#40772a", "#4b8531", "#5a943a")
    v = 0.6 * white(r) + 0.4 * vnoise(r, 4, 4)
    img = paint(v, p, [1, 2, 3, 3, 2, 1])
    a = np.ones((S, S))
    holes = (white(r) < 0.13) & (v < np.quantile(v, 0.6))
    a[holes] = 0
    return img, a


def tall_grass(seed=11):
    r = R(seed)
    img = np.zeros((S, S, 3))
    a = np.zeros((S, S))
    p = [hexc(c) for c in ("#3f6e24", "#4b7f2b", "#5a9134", "#6aa33e", "#7cb44a")]
    for x in [1, 3, 4, 6, 7, 9, 10, 12, 13, 14]:
        h = int(r.integers(6, 14))
        lean = r.choice([-1, 0, 0, 1])
        for i in range(h):
            yy = S - 1 - i
            xx = int(np.clip(x + (lean if i > h * 0.6 else 0), 0, S - 1))
            img[yy, xx] = p[min(4, i * 5 // h)]
            a[yy, xx] = 1
    return img, a


def flower(kind, seed=12):
    r = R(seed)
    img = np.zeros((S, S, 3))
    a = np.zeros((S, S))
    stem = [hexc("#3f6e24"), hexc("#4f8a2d")]
    for y in range(7, 16):
        img[y, 7] = stem[y % 2]
        a[y, 7] = 1
    for (x, y) in [(6, 11), (5, 10), (8, 13), (9, 12)]:
        img[y, x] = stem[1]
        a[y, x] = 1
    if kind == "red":
        pet = {(6, 4): "#a3141c", (7, 4): "#c41e26", (8, 4): "#a3141c",
               (5, 5): "#c41e26", (6, 5): "#e0333a", (7, 5): "#2a1410", (8, 5): "#e0333a", (9, 5): "#c41e26",
               (5, 6): "#a3141c", (6, 6): "#c41e26", (7, 6): "#e0333a", (8, 6): "#c41e26", (9, 6): "#a3141c",
               (6, 7): "#8a1016", (8, 7): "#8a1016", (7, 3): "#e0333a"}
    else:
        pet = {(6, 5): "#e8b820", (7, 5): "#f7d23a", (8, 5): "#e8b820",
               (6, 6): "#f7d23a", (7, 6): "#fff07a", (8, 6): "#f7d23a",
               (7, 4): "#f7d23a", (6, 7): "#c99a18", (7, 7): "#e8b820", (8, 7): "#c99a18"}
    for (x, y), c in pet.items():
        img[y, x] = hexc(c)
        a[y, x] = 1
    return img, a


# ---------------------------------------------------------------- obsidian
OBS = pal("#0c0914", "#120d1c", "#191226", "#221831", "#2e2142", "#423160")


def obsidian(seed=13):
    r = R(seed)
    y, x = np.mgrid[0:S, 0:S]
    streak = 0.5 + 0.5 * np.sin((x + y) * 2 * np.pi / 16 * 2 + 4 * vnoise(r, 4, 4))
    v = 0.45 * white(r) + 0.35 * vnoise(r, 4, 4) + 0.2 * streak
    return paint(v, OBS, [3, 4, 3, 2, 1.2, 0.5])


def crying_obsidian(seed=14):
    r = R(seed)
    img = obsidian(seed + 50)
    e = np.zeros((S, S))
    cols = [hexc(c) for c in ("#5a1aa0", "#8a33e0", "#b66cff", "#e2b8ff")]
    spots = [(3, 2), (11, 4), (6, 9), (13, 11), (2, 13), (9, 14)]
    for (sx, sy) in spots:
        ln = int(r.integers(1, 4))
        for i in range(ln + 1):  # tear trail downwards, brightest at the head
            lvl = 3 - min(3, i)
            img[(sy + i) % S, sx] = cols[lvl]
            e[(sy + i) % S, sx] = [0.25, 0.5, 0.8, 1.0][lvl]
        if r.random() < 0.6:
            img[sy % S, (sx + 1) % S] = cols[1]
            e[sy % S, (sx + 1) % S] = 0.5
    return img, e


# ---------------------------------------------------------------- nether
def netherrack(seed=15):
    r = R(seed)
    p = pal("#4a1212", "#5a1818", "#6a1f1f", "#7a2727", "#8b3230", "#a0423e")
    v = 0.4 * white(r) + 0.4 * vnoise(r, 4, 4) + 0.2 * vnoise(r, 2, 2)
    img = paint(v, p, [1, 2, 3, 3, 2, 0.7])
    return img


def bricks(seed, bp, mortar, hl, rows=4, bw=8, shadow=None, var=0.5):
    """Running-bond brick texture. rows = brick rows, bw = brick width."""
    r = R(seed)
    bh = S // rows
    img = np.zeros((S, S, 3))
    for row in range(rows):
        off = 0 if row % 2 == 0 else bw // 2
        y0 = row * bh
        nb = S // bw
        for b in range(nb):
            base = r.random()
            for yy in range(y0, y0 + bh):
                for k in range(bw):
                    xx = (b * bw + k + off) % S
                    if yy == y0 + bh - 1 or k == bw - 1:
                        img[yy, xx] = mortar
                        continue
                    v = var * base + (1 - var) * r.random()
                    c = bp[min(len(bp) - 1, int(v * len(bp)))]
                    if yy == y0 or k == 0:
                        c = hl if r.random() < 0.8 else c
                    elif shadow is not None and (yy == y0 + bh - 2 or k == bw - 2):
                        c = shadow if r.random() < 0.7 else c
                    img[yy, xx] = c
    return img


def nether_bricks(seed=16):
    return bricks(seed, pal("#3c171c", "#481c22", "#542229", "#5f2830"), hexc("#1a090c"),
                  hexc("#70343c"), rows=4, bw=8, shadow=hexc("#30131a"))


def blackstone(seed=17):
    r = R(seed)
    p = pal("#1e1b22", "#26232b", "#2e2a34", "#38333f", "#433d4b", "#524a5c")
    v = 0.4 * white(r) + 0.45 * vnoise(r, 4, 4) + 0.15 * vnoise(r, 8, 2)
    return paint(v, p, [1, 2, 3, 3, 2, 0.8])


def blackstone_top(seed=18):
    r = R(seed)
    y, x = np.mgrid[0:S, 0:S]
    d = np.maximum(abs(x - 7.5), abs(y - 7.5))
    img = blackstone(seed + 7)
    ring = (np.abs(d - 4.5) < 0.6)
    img[ring] = img[ring] * 0.75
    return img


def pbb(seed=19):
    return bricks(seed, pal("#342f3b", "#3b3543", "#433c4c", "#4a4354"), hexc("#141117"),
                  hexc("#5a5266"), rows=4, bw=8, shadow=hexc("#2a2530"))


def gilded(seed=20):
    r = R(seed)
    img = blackstone(seed + 3)
    m = np.zeros((S, S))
    cols = [hexc(c) for c in ("#8a5f12", "#c8921f", "#efc23c", "#ffe27a")]
    shapes = [[(0, 0), (1, 0), (0, 1)], [(0, 0), (1, 0), (1, 1), (2, 1)], [(0, 0), (0, 1), (1, 1)],
              [(0, 0), (1, 0)], [(0, 0), (1, 1), (1, 0), (2, 0), (1, -1)]]
    for (sx, sy) in [(2, 3), (10, 2), (6, 8), (12, 10), (3, 12), (9, 13), (14, 6)]:
        sh = shapes[r.integers(0, len(shapes))]
        for i, (dx, dy) in enumerate(sh):
            lvl = 3 if i == 0 else int(r.integers(1, 3))
            img[(sy + dy) % S, (sx + dx) % S] = cols[lvl]
            m[(sy + dy) % S, (sx + dx) % S] = 1
        img[(sy + 1) % S, (sx + 1) % S] = cols[0] if m[(sy + 1) % S, (sx + 1) % S] == 0 else img[(sy + 1) % S, (sx + 1) % S]
    return img, m


def gold_block(seed=21):
    r = R(seed)
    p = pal("#e5a92a", "#efb935", "#f5c740", "#f9d250")
    v = 0.5 * white(r) + 0.5 * vnoise(r, 4, 4)
    img = paint(v, p, [1, 3, 3, 1])
    img[0, :] = hexc("#fff0a0")
    img[:, 0] = hexc("#fde58a")
    img[S - 1, :] = hexc("#b5781a")
    img[:, S - 1] = hexc("#c58a1e")
    img[1, 1:S - 1] = hexc("#fbe070")
    img[S - 2, 1:S - 1] = hexc("#d19623")
    for i in range(3, 9):  # glint diagonals
        img[i, i + 2] = hexc("#fff3b0")
        img[i + 4, i + 1] = hexc("#fbe27a")
    for i in range(3, 13):
        if r.random() < 0.3:
            img[12, i] = hexc("#d9a02a")
    return img


def basalt_side(seed=22):
    r = R(seed)
    p = pal("#34343a", "#3e3e45", "#48484f", "#53535a", "#5e5e66", "#6c6c74")
    col = np.repeat(r.random(S)[None, :], S, 0)
    v = 0.55 * col + 0.3 * vnoise(r, 2, 8) + 0.15 * white(r)
    return paint(v, p, [1, 2, 3, 3, 2, 1])


def basalt_top(seed=23):
    r = R(seed)
    y, x = np.mgrid[0:S, 0:S]
    d = np.maximum(abs(x - 7.5), abs(y - 7.5)) + 0.7 * (white(r) - 0.5)
    p = pal("#3a3a40", "#4c4c53", "#5a5a62", "#44444b", "#62626a")
    img = p[(np.floor(d / 2).astype(int)) % 5].astype(float)
    img *= (0.92 + 0.14 * white(r))[..., None]
    edge = np.maximum(abs(x - 7.5), abs(y - 7.5)) > 6.6
    img[edge] = hexc("#2e2e34")
    return img


def soul_sand(seed=24):
    r = R(seed)
    p = pal("#3a291f", "#443126", "#4f3a2c", "#5a4333", "#664d3b")
    v = 0.6 * white(r) + 0.4 * vnoise(r, 4, 4)
    img = paint(v, p, [1, 2, 3, 2, 1])
    dark, mid, rim = hexc("#1f140e"), hexc("#2c1e15"), hexc("#735a46")
    for (fx, fy) in [(2, 2), (9, 7), (4, 11)]:
        for dx in range(-1, 5):  # lighter rim around the face
            img[(fy - 1) % S, (fx + dx) % S] = rim
        for (dx, dy) in [(0, 0), (2, 0)]:
            img[(fy + dy) % S, (fx + dx) % S] = dark
            img[(fy + dy + 1) % S, (fx + dx) % S] = mid
        for dx in range(0, 3):
            img[(fy + 3) % S, (fx + dx) % S] = dark if dx == 1 else mid
    return img


def lava(seed=25):
    r = R(seed)
    p = pal("#9a2406", "#bb340a", "#d8480e", "#ec6414", "#f8861e", "#fdab30", "#ffd04a")
    v = 0.55 * vnoise(r, 4, 4) + 0.3 * vnoise(r, 8, 4) + 0.15 * white(r)
    img = paint(v, p, [1.2, 2, 3, 3, 2, 1, 0.3])
    return img


def magma(seed=26):
    r = R(seed)
    p = pal("#2a0d06", "#361208", "#43170a", "#521d0c")
    v = 0.6 * white(r) + 0.4 * vnoise(r, 4, 4)
    img = paint(v, p, [1, 2, 2, 1])
    e = np.zeros((S, S))
    n = vnoise(r, 4, 4) + 0.5 * vnoise(r, 8, 8)
    n = (n - n.min()) / (n.max() - n.min())
    crack = np.abs(n - 0.5) < 0.07
    n2 = vnoise(r, 4, 4)
    crack |= np.abs(n2 - 0.5) < 0.05
    hot = [hexc("#c8380c"), hexc("#f2701a"), hexc("#ffb13a")]
    for yy, xx in zip(*np.nonzero(crack)):
        k = int(r.integers(0, 3))
        img[yy, xx] = hot[k]
        e[yy, xx] = [0.45, 0.75, 1.0][k]
    return img, e


def fire(seed=27, cols_hex=("#fff6c0", "#ffe066", "#ffb52e", "#ff8a1c", "#ef5a12", "#b8320a")):
    r = R(seed)
    img = np.zeros((S, S, 3))
    a = np.zeros((S, S))
    x = np.arange(S)
    h = np.zeros(S)
    for _ in range(4):  # flame tongues
        cx = r.uniform(1, 15)
        h += r.uniform(5, 10) * np.exp(-((x - cx) ** 2) / (2 * r.uniform(1.2, 2.2) ** 2))
    h = np.clip(h + 3 + r.normal(0, 0.6, S), 3, 15)
    cols = [hexc(c) for c in cols_hex]
    for xx in range(S):
        hh = int(h[xx])
        for i in range(hh):
            yy = S - 1 - i
            t = i / max(1, hh - 1)
            edge = abs(xx - 7.5) / 8
            k = int(np.clip(t * 4.2 + edge * 1.6 + r.normal(0, 0.35), 0, 5))
            img[yy, xx] = cols[k]
            a[yy, xx] = 1
    for _ in range(4):  # detached embers
        xx, yy = int(r.integers(2, 14)), int(r.integers(0, 5))
        if a[yy, xx] == 0:
            img[yy, xx] = cols[3]
            a[yy, xx] = 1
    return img, a


def glowstone(seed=28):
    r = R(seed)
    p = pal("#7a5424", "#a87632", "#d39d45", "#f0c25a", "#ffe08a", "#fff4c8")
    v = 0.5 * white(r) + 0.5 * vnoise(r, 4, 4)
    return paint(v, p, [1, 2, 3, 3, 2, 1])


def nether_wart(seed=29):
    r = R(seed)
    img = np.zeros((S, S, 3))
    a = np.zeros((S, S))
    stem, bulbs = hexc("#5a0d12"), [hexc("#7a1218"), hexc("#a31e26"), hexc("#c8343a")]
    for x0 in (3, 8, 12):
        top = int(r.integers(3, 7))
        for y in range(top, S):
            img[y, x0] = stem
            a[y, x0] = 1
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (-1, 1), (1, 1), (0, 1), (-2, 1), (2, 2)]:
            xx, yy = x0 + dx, top + dy
            if 0 <= xx < S and 0 <= yy < S:
                img[yy, xx] = bulbs[int(r.integers(0, 3))]
                a[yy, xx] = 1
    return img, a


def portal(seed=30):
    r = R(seed)
    p = pal("#2a0648", "#40096c", "#5c1094", "#7a1cb8", "#9a2ed6", "#b84cec", "#d890ff")
    y, x = np.mgrid[0:S, 0:S]
    sw = np.sin((x - 7.5) * 0.6 + np.sin(y * 2 * np.pi / 16) * 2.2 + 3 * vnoise(r, 8, 8))
    v = 0.5 * (0.5 + 0.5 * sw) + 0.3 * vnoise(r, 4, 4) + 0.2 * white(r)
    return paint(v, p, [1.5, 2, 3, 3, 2, 1, 0.4])


# ---------------------------------------------------------------- stronghold
STONEB = pal("#6c6c6c", "#767676", "#7f7f7f", "#888888", "#919191")


def stone_bricks(seed=31):
    # two rows of 16x8 bricks, offset by 8 (top row seam at tile edge)
    return bricks(seed, STONEB, hexc("#4a4a4a"), hexc("#9c9c9c"), rows=2, bw=16,
                  shadow=hexc("#616161"), var=0.35)


def mossy_stone_bricks(seed=32):
    r = R(seed)
    img = stone_bricks(seed + 11)
    moss = pal("#3a5422", "#46652a", "#557833", "#648a3c")
    m = vnoise(r, 4, 4) * 0.7 + 0.3 * white(r)
    y = np.mgrid[0:S, 0:S][0]
    m = m + 0.25 * ((y == 7) | (y == 15) | (y == 8) | (y == 0))
    mask = m > np.quantile(m, 0.62)
    idx = quant(white(r), [1, 2, 2, 1])
    img[mask] = moss[idx[mask]]
    return img


def cracked_stone_bricks(seed=33):
    r = R(seed)
    img = stone_bricks(seed + 5)
    x, y = 4, 1
    for _ in range(12):
        img[y % S, x % S] = hexc("#4a4a4a")
        x += int(r.integers(-1, 2))
        y += 1
    x, y = 12, 9
    for _ in range(6):
        img[y % S, x % S] = hexc("#4a4a4a")
        x += int(r.integers(0, 2))
        y += 1
    return img


FRAME = pal("#7f9478", "#8ea386", "#9bb193", "#a8bea0", "#b6caad")


def frame_top(seed=34):
    r = R(seed)
    v = 0.7 * white(r) + 0.3 * vnoise(r, 4, 4)
    img = paint(v, FRAME, [1, 2, 3, 2, 1])
    img[0, :] = img[:, 0] = hexc("#c4d6bb")
    img[S - 1, :] = img[:, S - 1] = hexc("#6a7d64")
    # eye socket
    for yy in range(4, 12):
        for xx in range(4, 12):
            edge = yy in (4, 11) or xx in (4, 11)
            img[yy, xx] = hexc("#2c5249") if edge else hexc("#14302a")
    for (xx, yy) in [(4, 4), (11, 4), (4, 11), (11, 11)]:
        img[yy, xx] = FRAME[2]
    # corner studs
    for (xx, yy) in [(1, 1), (14, 1), (1, 14), (14, 14)]:
        img[yy, xx] = hexc("#2f6a5c")
    return img


def frame_side(seed=35):
    r = R(seed)
    v = 0.7 * white(r) + 0.3 * vnoise(r, 4, 4)
    img = paint(v, FRAME, [1, 2, 3, 2, 1])
    # the block is 13px tall: rows 3..15 are visible. Decorative teal band + plinth
    img[3, :] = hexc("#c4d6bb")
    for xx in range(S):
        img[6, xx] = hexc("#2c5249")
        img[7, xx] = hexc("#3f6e62") if xx % 4 in (1, 2) else hexc("#14302a")
        img[8, xx] = hexc("#2c5249")
    img[14, :] = hexc("#6a7d64")
    img[15, :] = hexc("#55664f")
    return img


def eye(seed=36):
    img = np.zeros((S, S, 3)) + hexc("#0f4a40")
    p = {0: "#0b2c26", 1: "#1a7a68", 2: "#29a58b", 3: "#4fd6b8", 4: "#b8fff0"}
    pattern = [
        "..1111..",
        ".122221.",
        "12333321",
        "12300321",
        "12300321",
        "12333421",
        ".122221.",
        "..1111..",
    ]
    for j, row in enumerate(pattern):
        for i, ch in enumerate(row):
            if ch != ".":
                img[4 + j, 4 + i] = hexc(p[int(ch)])
    return img


def eye_side(seed=37):
    r = R(seed)
    p = pal("#0d3a32", "#145046", "#1a6658")
    return paint(white(r), p)


def end_stone(seed=38):
    r = R(seed)
    p = pal("#c3c17f", "#cfcd8b", "#d9d796", "#e2e1a2", "#ebeaae")
    v = 0.65 * white(r) + 0.35 * vnoise(r, 4, 4)
    img = paint(v, p, [1, 2, 3, 2, 1])
    for _ in range(6):  # pits
        x, y = r.integers(0, S, 2)
        img[y, x] = hexc("#a29f63")
        img[y, (x + 1) % S] = hexc("#b3b072")
        img[(y + 1) % S, x] = hexc("#e9e8b4")
    return img


def starfield(seed=39):
    r = R(seed)
    p = pal("#02080a", "#03100f", "#051614", "#081d1b")
    img = paint(white(r) * 0.5 + 0.5 * vnoise(r, 4, 4), p, [2, 3, 2, 1])
    e = np.zeros((S, S))
    stars = ["#5cf2d6", "#3dd6a0", "#c8fff2", "#6fb0ff", "#2ea88a", "#8affc8", "#a0d8ff"]
    pts = [(2, 1), (9, 3), (13, 7), (5, 6), (1, 10), (11, 12), (7, 14), (14, 15), (4, 13)]
    for i, (x, y) in enumerate(pts):
        img[y, x] = hexc(stars[i % len(stars)])
        e[y, x] = 1.0 if i % 3 else 0.6
    return img, e


def bedrock(seed=40):
    r = R(seed)
    p = pal("#1b1b1d", "#2f2f32", "#47474a", "#5e5e62", "#7a7a7e", "#959599")
    v = 0.55 * white(r) + 0.45 * vnoise(r, 2, 2)
    return paint(v, p, [2, 3, 3, 2, 1, 0.6])


def crystal(seed=41):
    r = R(seed)
    y, x = np.mgrid[0:S, 0:S]
    d = (np.abs(x - 7.5) + np.abs(y - 7.5))
    p = pal("#6a1fb0", "#9238d8", "#bb62f2", "#e09aff", "#f8d8ff")
    v = ((d / 3).astype(int) % 2) * 0.5 + 0.3 * white(r) + 0.2 * (1 - d / 15)
    img = paint(v, p, [1, 2, 3, 2, 0.8])
    img[0, :] = img[S - 1, :] = img[:, 0] = img[:, S - 1] = hexc("#f2c4ff")
    return img


def cage(seed=42):
    """Glass-like cage frame: bright border, transparent centre."""
    img = np.zeros((S, S, 3)) + hexc("#d8d0e8")
    a = np.zeros((S, S))
    for i in range(S):
        for j in (0, 1, S - 2, S - 1):
            a[i, j] = a[j, i] = 1
    img[1, :] = img[:, 1] = hexc("#a69cc0")
    a[7:9, 7:9] = 0
    return img, a


def iron_bars(seed=43):
    img = np.zeros((S, S, 3)) + hexc("#6e6e72")
    a = np.zeros((S, S))
    for x in (1, 5, 9, 13):
        a[:, x] = 1
        img[:, x] = hexc("#8a8a90")
        a[:, x + 1] = 1
        img[:, x + 1] = hexc("#55555a")
    for y in (1, 14):
        a[y, :] = 1
        img[y, :] = hexc("#7a7a80")
    return img, a


# ---------------------------------------------------------------- run
def main():
    save("grass_top", grass_top())
    save("grass_side", grass_side())
    save("dirt", dirt())
    save("stone", stone())
    save("deepslate", deepslate())
    save("sand", sand())
    save("water", water())
    save("oak_log", log_side())
    save("oak_log_top", log_top())
    img, a = leaves(); save("oak_leaves", img, a)
    img, a = tall_grass(); save("tall_grass", img, a)
    img, a = flower("red"); save("flower_red", img, a)
    img, a = flower("yellow", 13); save("flower_yellow", img, a)
    save("obsidian", obsidian())
    img, e = crying_obsidian(); save("crying_obsidian", img); save_mask("crying_obsidian_e", e)
    save("netherrack", netherrack())
    save("nether_bricks", nether_bricks())
    save("blackstone", blackstone())
    save("blackstone_top", blackstone_top())
    save("polished_blackstone_bricks", pbb())
    img, m = gilded(); save("gilded_blackstone", img); save_mask("gilded_blackstone_m", m)
    save("gold_block", gold_block())
    save("basalt_side", basalt_side())
    save("basalt_top", basalt_top())
    save("soul_sand", soul_sand())
    save("lava", lava())
    img, e = magma(); save("magma", img); save_mask("magma_e", e)
    img, a = fire(); save("fire", img, a)
    img, a = fire(77); save("fire_b", img, a)
    img, a = fire(91, ("#e8ffff", "#9ff6ff", "#4fe0f0", "#22b8d8", "#1888b8", "#0f5a8a")); save("soul_fire", img, a)
    save("glowstone", glowstone())
    img, a = nether_wart(); save("nether_wart", img, a)
    save("nether_portal", portal())
    save("stone_bricks", stone_bricks())
    save("mossy_stone_bricks", mossy_stone_bricks())
    save("cracked_stone_bricks", cracked_stone_bricks())
    save("end_portal_frame_top", frame_top())
    save("end_portal_frame_side", frame_side())
    save("eye_of_ender", eye())
    save("eye_of_ender_side", eye_side())
    save("end_stone", end_stone())
    img, e = starfield(); save("end_portal", img); save_mask("end_portal_e", e)
    save("bedrock", bedrock())
    save("end_crystal", crystal())
    img, a = cage(); save("crystal_cage", img, a)
    img, a = iron_bars(); save("iron_bars", img, a)
    rc = R(99); save("cloud", paint(white(rc), pal("#f4f4f6", "#fafafc", "#ffffff"), [1, 2, 3]))

    # preview sheet (work dir, not part of deliverables)
    n = len(SAVED)
    cols = 8
    rows = (n + cols - 1) // cols
    Z = 8
    sheet = Image.new("RGB", (cols * (S * Z + 8) + 8, rows * (S * Z + 8) + 8), (40, 40, 48))
    for i, name in enumerate(SAVED):
        im = Image.open(f"{OUT}/{name}.png").convert("RGBA").resize((S * Z, S * Z), Image.NEAREST)
        bg = Image.new("RGBA", im.size, (90, 140, 200, 255))
        bg.alpha_composite(im)
        sheet.paste(bg.convert("RGB"), (8 + (i % cols) * (S * Z + 8), 8 + (i // cols) * (S * Z + 8)))
    sheet.save(f"{WORK}/textures_preview.png")
    print(n, "textures")


if __name__ == "__main__":
    main()
