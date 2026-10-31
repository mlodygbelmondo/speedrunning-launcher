"""Second batch of procedural 16x16 textures (all original drawings, no Mojang assets).

uv run textures2.py  -> /tmp/srl/art/out/textures/
"""
import numpy as np
from PIL import Image
import textures as T
from textures import R, hexc, pal, vnoise, white, quant, paint, save, save_mask, S

OUT = T.OUT


def noise_tex(seed, p, props=None, w=0.55, cell=4, cx=None, cy=None):
    r = R(seed)
    v = w * white(r) + (1 - w) * vnoise(r, cx or cell, cy or cell)
    return paint(v, p, props)


def specks(img, seed, cols, n=6, shape=None, mask=None):
    r = R(seed)
    shapes = [[(0, 0), (1, 0), (0, 1)], [(0, 0), (1, 0), (1, 1)], [(0, 0), (0, 1), (1, 1), (1, 0)],
              [(0, 0), (1, 0)], [(0, 0), (1, 1), (2, 1)], [(0, 0), (0, 1), (-1, 1)]]
    pts = []
    for _ in range(n):
        x, y = int(r.integers(0, S)), int(r.integers(0, S))
        sh = shape or shapes[int(r.integers(0, len(shapes)))]
        for i, (dx, dy) in enumerate(sh):
            c = cols[min(len(cols) - 1, i if i < 2 else int(r.integers(0, len(cols))))]
            img[(y + dy) % S, (x + dx) % S] = hexc(c) if isinstance(c, str) else c
            if mask is not None:
                mask[(y + dy) % S, (x + dx) % S] = 1
            pts.append(((x + dx) % S, (y + dy) % S))
    return pts


def ore(base, seed, cols, n=5):
    img = base.copy()
    specks(img, seed, cols, n)
    return img


def frame(img, light, dark):
    img[0, :] = img[:, 0] = hexc(light)
    img[S - 1, :] = img[:, S - 1] = hexc(dark)
    return img


def log_side(seed, p):
    r = R(seed)
    col = np.repeat(r.random(S)[None, :], S, 0)
    v = 0.5 * col + 0.3 * vnoise(r, 2, 8) + 0.2 * white(r)
    img = paint(v, p, [1, 2, 3, 3, 2, 1][:len(p)])
    for gx in r.choice(S, 3, replace=False):
        y0 = int(r.integers(0, S))
        for i in range(int(r.integers(4, 9))):
            img[(y0 + i) % S, gx] = p[0]
    return img


def log_top(seed, rings, bark):
    r = R(seed)
    y, x = np.mgrid[0:S, 0:S]
    d = np.maximum(abs(x - 7.5), abs(y - 7.5)) + 0.6 * (white(r) - 0.5)
    rp = pal(*rings)
    img = rp[(np.floor(d / 1.6).astype(int)) % len(rings)].astype(float)
    img *= (0.94 + 0.12 * white(r))[..., None]
    edge = np.maximum(abs(x - 7.5), abs(y - 7.5)) > 6.6
    bp = pal(*bark)
    img[edge] = bp[quant(white(r), [1] * len(bark))[edge]]
    return img


def planks(seed, p):
    r = R(seed)
    img = np.zeros((S, S, 3))
    pp = pal(*p)
    for b in range(4):
        base = r.random()
        y0 = b * 4
        seam_x = int(r.integers(2, 14))
        for yy in range(y0, y0 + 4):
            for xx in range(S):
                v = 0.55 * base + 0.25 * r.random() + 0.2 * (0.5 + 0.5 * np.sin(xx * 0.9 + yy * 3 + b))
                c = pp[1 + min(len(pp) - 2, int(v * (len(pp) - 1)))]
                if yy == y0 + 3:
                    c = pp[0]
                if xx == seam_x and b % 2 == 0:
                    c = pp[0]
                if xx == (seam_x + 8) % S and b % 2 == 1:
                    c = pp[0]
                img[yy, xx] = c
    return img


def cross_blades(seed, cols, n=9, hmin=6, hmax=14, lean=True):
    r = R(seed)
    img = np.zeros((S, S, 3))
    a = np.zeros((S, S))
    p = [hexc(c) for c in cols]
    xs = sorted(r.choice(np.arange(1, 15), n, replace=False))
    for x in xs:
        h = int(r.integers(hmin, hmax + 1))
        ln = r.choice([-1, 0, 0, 1]) if lean else 0
        for i in range(h):
            yy = S - 1 - i
            xx = int(np.clip(x + (ln if i > h * 0.6 else 0), 0, S - 1))
            img[yy, xx] = p[min(len(p) - 1, i * len(p) // h)]
            a[yy, xx] = 1
    return img, a


def sprite(rows, cmap):
    """Hand-drawn sprite from a list of 16 strings; '.' = transparent."""
    img = np.zeros((S, S, 3))
    a = np.zeros((S, S))
    for y, row in enumerate(rows):
        for x, ch in enumerate(row.ljust(S, ".")[:S]):
            if ch != ".":
                img[y, x] = hexc(cmap[ch])
                a[y, x] = 1
    return img, a


def flower(seed, head, center=None, stem="#3f7a26", tall=7):
    img = np.zeros((S, S, 3))
    a = np.zeros((S, S))
    for y in range(S - tall, S):
        img[y, 7] = hexc(stem)
        a[y, 7] = 1
    for (x, y) in [(6, 12), (8, 14), (5, 11), (9, 13)]:
        img[y, x] = hexc("#4d8f2f")
        a[y, x] = 1
    cy = S - tall - 2
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1), (0, 0), (-2, 0), (2, 0), (0, -2)]:
        c = head[(abs(dx) + abs(dy)) % len(head)]
        img[cy + dy, 7 + dx] = hexc(c)
        a[cy + dy, 7 + dx] = 1
    if center:
        img[cy, 7] = hexc(center)
    return img, a


def metal_block(seed, base, light, dark, inner=None):
    r = R(seed)
    img = noise_tex(seed, pal(*base), [1, 2, 2, 1][:len(base)], w=0.7)
    frame(img, light, dark)
    img[1, 1:S - 1] = hexc(light)
    img[1:S - 1, 1] = hexc(light)
    img[S - 2, 1:S - 1] = hexc(dark)
    img[1:S - 1, S - 2] = hexc(dark)
    if inner:
        inner(img, r)
    return img


def main():
    n0 = len(T.SAVED)
    # ---------------------------------------------------------------- nether stone family
    bs = T.blackstone(17)
    pb = noise_tex(101, pal("#2a2630", "#2f2a36", "#342f3c", "#3a3443"), [1, 3, 3, 1], w=0.75)
    frame(pb, "#4a4454", "#1f1b24")
    save("polished_blackstone", pb)
    ch = pb.copy()
    for i in range(3, 13):
        for j in (3, 12):
            ch[j, i] = ch[i, j] = hexc("#1a171e")
    for i in range(5, 11):
        for j in (5, 10):
            ch[j, i] = ch[i, j] = hexc("#4f4859")
    ch[7:9, 7:9] = hexc("#1a171e")
    save("chiseled_polished_blackstone", ch)
    cr = T.pbb(19)
    r = R(102)
    for (x, y, ln) in [(3, 1, 7), (11, 6, 6), (6, 9, 6)]:
        for k in range(ln):
            cr[(y + k) % S, (x + (k // 2) * (1 if k % 3 else -1)) % S] = hexc("#0d0b10")
    save("cracked_polished_blackstone_bricks", cr)
    save("polished_basalt_side", noise_tex(103, pal("#3d3d44", "#47474f", "#52525a", "#5d5d66", "#6a6a73"), w=0.3, cx=1, cy=16))
    pbt = noise_tex(104, pal("#4a4a52", "#55555e", "#60606a"), w=0.7)
    frame(pbt, "#6e6e78", "#36363c")
    pbt[4:12, 4] = pbt[4:12, 11] = pbt[4, 4:12] = pbt[11, 4:12] = hexc("#3c3c43")
    save("polished_basalt_top", pbt)

    # nylium + wart + stems
    nr = T.netherrack(15)
    cn = noise_tex(105, pal("#6e0f12", "#831519", "#951b1f", "#a82428", "#bd3434"), [1, 2, 3, 2, 1], w=0.6)
    save("crimson_nylium", cn)
    wn = noise_tex(106, pal("#145a52", "#1a6c62", "#207e71", "#2b917f", "#3aa58f"), [1, 2, 3, 2, 1], w=0.6)
    save("warped_nylium", wn)
    for name, top in (("crimson_nylium_side", cn), ("warped_nylium_side", wn)):
        img = nr.copy()
        r = R(len(name))
        for x in range(S):
            d = int(np.clip(round(2.5 + 1.2 * np.sin(x * 1.3) + r.normal(0, 0.7)), 1, 5))
            img[:d, x] = top[:d, x]
        save(name, img)
    save("nether_wart_block", noise_tex(107, pal("#5e0a0c", "#730d10", "#860f13", "#9a1519", "#ad2026"), w=0.5))
    save("warped_wart_block", noise_tex(108, pal("#0c5a5a", "#127070", "#178585", "#1f9a96", "#2eada6"), w=0.5))
    save("crimson_stem", log_side(109, pal("#3e1424", "#4c1a2c", "#5a1f33", "#6a273b", "#7a2f45", "#8c3a51")))
    save("crimson_stem_top", log_top(110, ("#7a2b3c", "#8f3747", "#6c2534"), ("#3e1424", "#4c1a2c")))
    save("warped_stem", log_side(111, pal("#143c3c", "#1a4a48", "#215a55", "#2a6b62", "#3a7d6e", "#4e9480")))
    save("warped_stem_top", log_top(112, ("#3a7f6c", "#4b927b", "#2f6c5c"), ("#143c3c", "#1a4a48")))
    sl = noise_tex(113, pal("#d86a1c", "#ec8a2a", "#f7a83c", "#fdc65a", "#ffe08a"), [1, 2, 3, 2, 1], w=0.5)
    save("shroomlight", sl)
    # nether ores, debris, gravel, soul soil, bone, quartz
    save("nether_quartz_ore", ore(nr, 114, ["#f4efe6", "#d9d0c4", "#bfb4a6"], 6))
    save("nether_gold_ore", ore(nr, 115, ["#ffe16a", "#f2b52c", "#c98a18"], 7))
    ad = noise_tex(116, pal("#3a2a25", "#46332c", "#523d34", "#5e463c", "#6c5045"), w=0.4, cx=2, cy=8)
    for y in (3, 8, 12):
        ad[y, :] = ad[y, :] * 0.7
    save("ancient_debris_side", ad)
    adt = noise_tex(117, pal("#4a3a33", "#5a463d", "#6a5247"), w=0.5)
    for k in range(0, 16, 4):
        adt[k:k + 3, k:k + 3] = adt[k:k + 3, k:k + 3] * 0.75
    save("ancient_debris_top", adt)
    save("gravel", noise_tex(118, pal("#5f5a57", "#736d69", "#86807b", "#9a938e", "#7d6f66", "#a8a19c"), [2, 3, 3, 2, 1, 1], w=0.85))
    ss = noise_tex(119, pal("#3e2f26", "#48372c", "#523e32", "#5d4739", "#6a5243"), w=0.6)
    save("soul_soil", ss)
    bside = noise_tex(120, pal("#d7d1b8", "#e1dcc4", "#e9e5cf", "#cfc8ac"), w=0.6, cx=16, cy=2)
    save("bone_block_side", bside)
    bt = noise_tex(121, pal("#ddd6bd", "#e8e3cd"), w=0.7)
    frame(bt, "#efeadb", "#bdb597")
    bt[5:11, 5:11] = hexc("#cbc3a5")
    bt[6:10, 6:10] = hexc("#b8ae8c")
    save("bone_block_top", bt)
    q = noise_tex(122, pal("#e9e4dc", "#efebe4", "#f4f1eb", "#e1dbd2"), w=0.75)
    save("quartz_block", frame(q.copy(), "#fbf9f5", "#d4cdc2"))
    save("smooth_quartz", q)

    # ---------------------------------------------------------------- props
    img, a = sprite([
        "......",
        "..22..", ".2..2.", ".2..2.", "..22..", "......",
        "..11..", ".1..1.", ".1..1.", "..11..", "......",
        "..22..", ".2..2.", ".2..2.", "..22..", "......"],
        {"1": "#3c4150", "2": "#5b6276"})
    img2, a2 = sprite([r[3:] + r[:3] if len(r) else r for r in [
        "......", "..11..", ".1..1.", ".1..1.", "..11..", "......",
        "..22..", ".2..2.", ".2..2.", "..22..", "......",
        "..11..", ".1..1.", ".1..1.", "..11..", "......"]], {"1": "#3c4150", "2": "#5b6276"})
    img[:, 3:6], a[:, 3:6] = img2[:, 0:3], a2[:, 0:3]
    save("chain", img, a)
    # lantern (UV layout: body sides 0..6 x 2..9, top 0..6 x 9..15 -> we map regions in Blender)
    lan = np.zeros((S, S, 3)) + hexc("#2a2d33")
    la = np.ones((S, S))
    body = sprite([
        "dddddd", "dyyyyd", "dywwyd", "dywwyd", "dyyoyd", "dyyyyd", "dddddd"],
        {"d": "#30343c", "y": "#ffcf62", "w": "#fff2c0", "o": "#ffb03a"})[0]
    lan[0:7, 0:6] = body[0:7, 0:6]
    lan[0:7, 6:12] = body[0:7, 0:6]
    lan[8:12, 0:4] = hexc("#3a3f49")
    lan[8:12, 4:8] = hexc("#24272d")
    save("lantern", lan)
    e = np.zeros((S, S))
    e[1:6, 1:5] = 1
    e[1:6, 7:11] = 1
    save_mask("lantern_e", e)
    sbody = sprite([
        "dddddd", "dccccd", "dcwwcd", "dcwwcd", "dccbcd", "dccccd", "dddddd"],
        {"d": "#30343c", "c": "#6ff2ff", "w": "#d8ffff", "b": "#3ac8e0"})[0]
    slan = lan.copy()
    slan[0:7, 0:6] = sbody[0:7, 0:6]
    slan[0:7, 6:12] = sbody[0:7, 0:6]
    save("soul_lantern", slan)
    # chest
    wood = pal("#6b4423", "#7d5229", "#8f5f31", "#a06c38", "#b07a40")
    ct = noise_tex(123, wood, w=0.6, cx=8, cy=2)
    frame(ct, "#c08a4c", "#4a2e16")
    save("chest_top", ct)
    cs = ct.copy()
    cs[9:11, :] = hexc("#3b2410")
    save("chest_side", cs)
    cf = cs.copy()
    cf[7:12, 7:9] = hexc("#cfd3d8")
    cf[8:11, 7:9] = hexc("#9aa0a8")
    cf[9, 7:9] = hexc("#3a3f46")
    save("chest_front", cf)
    # spawner cage
    sp = np.zeros((S, S, 3)) + hexc("#3a414c")
    sa = np.zeros((S, S))
    for i in range(S):
        for j in (0, 5, 10, 15):
            sa[i, j] = sa[j, i] = 1
            sp[i, j] = sp[j, i] = hexc("#55606e") if (i + j) % 3 else hexc("#2a3038")
    save("spawner", sp, sa)
    # mushrooms
    save("brown_mushroom", *sprite(["", "", "", "", "", "", "", "", "....bbbbb", "...bBBBBBb", "...bbbbbbb", ".......w", ".......w", ".......w", "......ww", ""],
                                   {"b": "#8a5a3c", "B": "#a8724e", "w": "#d6cdb9"}))
    save("red_mushroom", *sprite(["", "", "", "", "", "", "", "....rrrrr", "...rRwrRRr", "...rrrrwrr", "....rrrrr", ".......w", ".......w", ".......w", "......ww", ""],
                                 {"r": "#b8202a", "R": "#d8323a", "w": "#ece4d6"}))
    # nether plants
    save("crimson_roots", *cross_blades(124, ["#6e0f1a", "#8c1424", "#a81c2c", "#c42a38"], n=8, hmin=5, hmax=12))
    save("warped_roots", *cross_blades(125, ["#0f5a52", "#147064", "#1a8676", "#27a08a"], n=8, hmin=5, hmax=12))
    save("nether_sprouts", *cross_blades(126, ["#127066", "#1a8a7a", "#2aa08c"], n=10, hmin=2, hmax=6))
    save("crimson_fungus", *sprite(["", "", "", "", "", "", "....rrrrrr", "...rRRyRRRr", "...rrrrrrrr", ".....rr.rr", ".......o", ".......o", ".......o", ".......o", "......oo", ""],
                                   {"r": "#8e1420", "R": "#b0222c", "y": "#ffb24a", "o": "#d07040"}))
    save("warped_fungus", *sprite(["", "", "", "", "", "", "....tttttt", "...tTToTTTt", "...tttttttt", ".....tt.tt", ".......o", ".......o", ".......o", ".......o", "......oo", ""],
                                  {"t": "#167a6c", "T": "#22a08c", "o": "#e07a30"}))
    vines = ["#6e0f1a", "#8c1424", "#a81c2c"]
    img = np.zeros((S, S, 3)); a = np.zeros((S, S))
    rr = R(127)
    for x in (4, 8, 11):
        for y in range(S):
            xx = x + (1 if (y // 3 + x) % 2 else 0)
            img[y, xx] = hexc(vines[(y + x) % 3]); a[y, xx] = 1
            if rr.random() < 0.25:
                img[y, xx + 1] = hexc(vines[0]); a[y, xx + 1] = 1
    save("weeping_vines_plant", img, a)
    img2 = img.copy(); a2 = a.copy(); a2[10:, :] = 0
    img2[9, 7:10] = hexc("#c42a38"); a2[9, 7:10] = 1
    save("weeping_vines", img2, a2)
    tv = ["#0f6a5e", "#159080", "#20ad96"]
    img = np.zeros((S, S, 3)); a = np.zeros((S, S))
    for x in (4, 8, 11):
        for y in range(S):
            xx = x + (1 if (y // 3 + x) % 2 else 0)
            img[y, xx] = hexc(tv[(y + x) % 3]); a[y, xx] = 1
    save("twisting_vines_plant", img, a)
    a3 = a.copy(); a3[:6, :] = 0
    save("twisting_vines", img, a3)

    # ---------------------------------------------------------------- overworld stones / ores
    st = T.stone(4)
    save("andesite", noise_tex(128, pal("#6e6e6e", "#7b7b7b", "#888888", "#959595", "#a3a3a3"), w=0.75))
    save("diorite", noise_tex(129, pal("#9e9e9e", "#b4b4b4", "#c8c8c8", "#dcdcdc", "#ececec"), [1, 2, 3, 2, 1], w=0.8))
    save("granite", noise_tex(130, pal("#7c5444", "#8c6150", "#9a6d5a", "#a87a66", "#b88a74"), w=0.8))
    save("coal_ore", ore(st, 131, ["#1c1c1c", "#2c2c2c", "#3a3a3a"], 6))
    save("iron_ore", ore(st, 132, ["#d8af93", "#c49a80", "#ad846a"], 6))
    save("gold_ore", ore(st, 133, ["#fce35a", "#f0c03a", "#c89420"], 6))
    save("lapis_ore", ore(st, 134, ["#2a5ad0", "#1d44a8", "#163a8c"], 6))
    save("diamond_ore", ore(st, 135, ["#8ef4f0", "#4ad8d2", "#2ab0aa"], 6))
    save("redstone_ore", ore(st, 136, ["#ff2a2a", "#c41616", "#8e0c0c"], 6))
    save("emerald_ore", ore(st, 137, ["#5af08a", "#2ac860", "#1a9444"], 4))
    cob = np.zeros((S, S, 3))
    r = R(138)
    cp = pal("#4e4e4e", "#5e5e5e", "#6e6e6e", "#7e7e7e", "#8e8e8e", "#9e9e9e")
    cells = r.random((5, 5))
    y, x = np.mgrid[0:S, 0:S]
    pts = r.random((14, 2)) * 16
    d = np.full((S, S), 99.0); d2 = np.full((S, S), 99.0); own = np.zeros((S, S), int)
    for k, (px, py) in enumerate(pts):
        for ox in (-16, 0, 16):
            for oy in (-16, 0, 16):
                dd = np.hypot(x - px - ox, y - py - oy)
                upd = dd < d
                d2 = np.where(upd, d, np.minimum(d2, dd))
                own = np.where(upd, k, own)
                d = np.where(upd, dd, d)
    lv = r.random(14)
    v = np.clip(lv[own] * 0.6 + 0.4 * (1 - d / 6), 0, 1)
    cob = cp[np.clip((v * 5).astype(int) + 1, 0, 5)]
    cob[(d2 - d) < 0.9] = cp[0]
    save("cobblestone", cob)
    mc = cob.copy()
    mm = vnoise(R(139), 4, 4) > 0.6
    mc[mm] = pal("#4a6a2c", "#5a7d34", "#3f5a26")[quant(white(R(140)), [1, 1, 1])[mm]]
    save("mossy_cobblestone", mc)
    sm = noise_tex(141, pal("#9a9a9a", "#a2a2a2", "#aaaaaa"), w=0.7)
    frame(sm, "#b6b6b6", "#7c7c7c")
    save("smooth_stone", sm)
    save("smooth_stone_slab_side", np.concatenate([sm[:8], sm[:8]], 0))
    # sandstone, clay, ocean plants
    save("sandstone_top", noise_tex(142, pal("#d9c88c", "#e0d095", "#e6d89f", "#d2bf80"), w=0.8))
    sst = noise_tex(143, pal("#cfbb7a", "#d8c586", "#e0cf92", "#e7d89e"), w=0.4, cx=16, cy=2)
    sst[0:3, :] = hexc("#e2d296"); sst[3, :] = hexc("#bfa96a"); sst[12, :] = hexc("#c4ae70")
    save("sandstone", sst)
    save("sandstone_bottom", noise_tex(144, pal("#cdb877", "#d6c383", "#dfcd8f"), w=0.8))
    save("clay", noise_tex(145, pal("#8f96a6", "#9aa1b1", "#a5acbb", "#b0b7c5"), w=0.8))
    save("seagrass", *cross_blades(146, ["#1e6a2a", "#2a8034", "#3a9640", "#4cab4c"], n=8, hmin=8, hmax=15))
    save("tall_seagrass_bottom", *cross_blades(147, ["#1e6a2a", "#2a8034", "#3a9640"], n=7, hmin=16, hmax=16, lean=False))
    save("tall_seagrass_top", *cross_blades(148, ["#2a8034", "#3a9640", "#4cab4c"], n=7, hmin=6, hmax=14))
    img = np.zeros((S, S, 3)); a = np.zeros((S, S))
    for y in range(S):
        for x in (6, 7, 8, 9):
            if x in (6, 9) and (y // 2) % 2:
                continue
            img[y, x] = hexc("#3a7a2a" if x in (7, 8) else "#5a9a3a"); a[y, x] = 1
        side = 5 if (y // 4) % 2 else 10
        img[y, side] = hexc("#4c8c30"); a[y, side] = 1
    save("kelp_plant", img, a)
    a2 = a.copy(); a2[:4, :] = 0
    save("kelp", img, a2)
    save("lily_pad", *sprite(["", "", "", "....gggggg", "...gGGGGGGg", "..gGGgGGGGGg", "..gGGGgGGGGg", "..gGGGGg...", "..gGGGGGgGGg",
                              "..gGGGGGGGGg", "...gGGGGGGg", "....gggggg"], {"g": "#1f5a1a", "G": "#2e7a24"}))
    save("sugar_cane", *sprite(["...c.....c...c", "...C.l...C...C", "...c..l..c...c", "...c.....c..lc", "...C.....C...C",
                                "...c.....c...c", "..lc.....c...c", "...C.....Cl..C", "...c.....c...c", "...c.....c...c",
                                "...C.....C...C", "...c..l..c...c", "...c.....c...c", "...C.....C...C", "...c.....c...c",
                                "...c.....c...c"], {"c": "#86c25a", "C": "#a6d878", "l": "#5a9a3a"}))
    save("vine", *cross_blades(149, ["#2a5a1a", "#346a22", "#3e7a2a"], n=10, hmin=10, hmax=16))
    # ---------------------------------------------------------------- woods
    save("oak_planks", planks(150, ("#5e4527", "#8a6638", "#987140", "#a57c47", "#b2884f")))
    save("spruce_planks", planks(151, ("#3e2a17", "#5e4226", "#6a4a2b", "#755331", "#805c37")))
    save("birch_planks", planks(152, ("#9a8a5c", "#c4b07a", "#cdb983", "#d6c28c", "#dfcb95")))
    save("dark_oak_planks", planks(153, ("#2a1a0c", "#3e2912", "#463016", "#4f371a", "#583e1f")))
    save("jungle_planks", planks(154, ("#6a4428", "#a0704a", "#ab7a52", "#b6845a", "#c08e62")))
    save("cherry_planks", planks(155, ("#a46a6e", "#d8a8a2", "#e0b4ad", "#e8c0b8", "#efcac2")))
    save("spruce_log", log_side(156, pal("#2a1d10", "#352516", "#3f2c1b", "#4a3420", "#553c25", "#604429")))
    save("spruce_log_top", log_top(157, ("#7a5634", "#6a4a2c", "#86603b"), ("#2a1d10", "#352516")))
    bl = noise_tex(158, pal("#d6d6ce", "#e2e2da", "#ececE6".lower(), "#c8c8c0"), w=0.8)
    r = R(159)
    for _ in range(9):
        x, y = int(r.integers(0, S)), int(r.integers(0, S))
        for k in range(int(r.integers(2, 5))):
            bl[y, (x + k) % S] = hexc("#2c2c28")
    save("birch_log", bl)
    save("birch_log_top", log_top(160, ("#c8b47e", "#b8a46e", "#d4c08a"), ("#e2e2da", "#c8c8c0")))
    save("dark_oak_log", log_side(161, pal("#2a1f12", "#33261a", "#3c2d1f", "#463524", "#4f3c29")))
    save("dark_oak_log_top", log_top(162, ("#4f3a22", "#5c4429", "#463420"), ("#2a1f12", "#33261a")))
    save("jungle_log", log_side(163, pal("#3e2f12", "#4c3a18", "#5a451e", "#685024", "#765b2a")))
    save("jungle_log_top", log_top(164, ("#a07a3a", "#8e6a30", "#b08844"), ("#3e2f12", "#4c3a18")))
    save("cherry_log", log_side(165, pal("#2a1418", "#33181e", "#3d1d24", "#47222a", "#522831", "#5c2e38")))
    save("cherry_log_top", log_top(166, ("#d8a0a4", "#c88c92", "#e4b0b2"), ("#33181e", "#3d1d24")))
    for nm, p, sd in (("birch_leaves", pal("#40652a", "#4a7232", "#567f3a", "#628c42", "#70994c", "#7ea657"), 167),
                      ("spruce_leaves", pal("#1c3a24", "#22442a", "#2a4e31", "#325838", "#3b6240", "#466c4a"), 168),
                      ("dark_oak_leaves", pal("#1e4012", "#244a16", "#2c561c", "#346222", "#3c6e28", "#467a30"), 169),
                      ("jungle_leaves", pal("#2a6a18", "#32781c", "#3a8622", "#449428", "#50a230", "#5cae38"), 170),
                      ("acacia_leaves", pal("#3e6a1c", "#487822", "#528628", "#5c922e", "#689e36", "#74aa3e"), 171),
                      ("cherry_leaves", pal("#d86a9c", "#e47ea8", "#ec92b6", "#f2a6c4", "#f6bad2", "#fbd0e0"), 172)):
        r = R(sd)
        v = 0.6 * white(r) + 0.4 * vnoise(r, 4, 4)
        img = paint(v, p, [1, 2, 3, 3, 2, 1])
        a = np.ones((S, S))
        a[(white(r) < 0.14) & (v < np.quantile(v, 0.6))] = 0
        save(nm, img, a)
    # cherry petals (flat, alpha)
    img = np.zeros((S, S, 3)); a = np.zeros((S, S))
    r = R(173)
    pc = ["#f4a6c6", "#f8bcd4", "#ec8eb6", "#fcd2e2"]
    for (cx, cy) in [(3, 3), (11, 4), (4, 11), (12, 12), (8, 8), (7, 2), (2, 7), (13, 8)]:
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)]:
            if r.random() < 0.85:
                img[(cy + dy) % S, (cx + dx) % S] = hexc(pc[int(r.integers(0, 4))]); a[(cy + dy) % S, (cx + dx) % S] = 1
        img[cy, cx] = hexc("#f6e07a")
    save("pink_petals", img, a)
    # ---------------------------------------------------------------- plants & flowers
    save("fern", *cross_blades(174, ["#2f5a22", "#3a6a28", "#467a30", "#528a38"], n=8, hmin=5, hmax=12))
    save("tall_grass_bottom", *cross_blades(175, ["#3f6e24", "#4b7f2b", "#5a9134", "#6aa33e"], n=10, hmin=16, hmax=16))
    save("tall_grass_top", *cross_blades(176, ["#5a9134", "#6aa33e", "#7cb44a"], n=9, hmin=5, hmax=13))
    save("large_fern_bottom", *cross_blades(177, ["#2f5a22", "#3a6a28", "#467a30"], n=9, hmin=16, hmax=16))
    save("large_fern_top", *cross_blades(178, ["#3a6a28", "#467a30", "#528a38"], n=8, hmin=6, hmax=13))
    save("dandelion", *flower(179, ["#f7d23a", "#e8b820", "#fff07a"], "#f0a020"))
    save("poppy", *flower(180, ["#d0242c", "#b01820", "#e8383e"], "#2a1410"))
    save("azure_bluet", *flower(181, ["#e8eef8", "#d4dcea", "#ffffff"], "#f0d040", tall=6))
    save("oxeye_daisy", *flower(182, ["#f4f4ee", "#e4e4dc", "#ffffff"], "#f0c020", tall=8))
    save("cornflower", *flower(183, ["#4a6ae0", "#3a54c0", "#6a88f0"], "#2a3a90"))
    save("lily_of_the_valley", *flower(184, ["#f4f4f4", "#e8e8e8", "#ffffff"], None, tall=9))
    save("allium", *flower(185, ["#b46ae0", "#9a50c8", "#c88af0"], "#e0b0ff"))
    save("blue_orchid", *flower(186, ["#2ab0e8", "#1a90c8", "#5ac8f4"], "#1a5a90"))
    save("red_tulip", *flower(187, ["#d8302a", "#b8201a"], None))
    save("orange_tulip", *flower(188, ["#f07a20", "#d86010"], None))
    save("white_tulip", *flower(189, ["#f0f0ea", "#d8d8d0"], None))
    save("pink_tulip", *flower(190, ["#f0a0c0", "#d880a8"], None))
    save("dead_bush", *cross_blades(191, ["#6a4a2a", "#7e5a34", "#8e683c"], n=7, hmin=4, hmax=10))
    save("sweet_berry_bush", *cross_blades(192, ["#1f4a22", "#2a5a2a", "#356a32"], n=11, hmin=6, hmax=13))
    sap, sa_ = cross_blades(193, ["#3a6a24", "#467a2c", "#528a34"], n=5, hmin=6, hmax=12)
    save("oak_sapling", sap, sa_)
    # ---------------------------------------------------------------- village / misc
    gp = noise_tex(194, pal("#8a6a3a", "#967442", "#a27e4a", "#ae8a54"), w=0.8)
    save("grass_path_top", gp)
    gps = T.dirt(2).copy(); gps[:2, :] = gp[:2, :]
    save("grass_path_side", gps)
    fm = noise_tex(195, pal("#4a3020", "#5a3a26", "#6a462e"), w=0.6)
    for y in (1, 5, 9, 13):
        fm[y, :] = fm[y, :] * 0.65
    save("farmland", fm)
    save("wheat", *cross_blades(196, ["#5a7a28", "#8a9a30", "#c8b040", "#dcc458"], n=9, hmin=8, hmax=14, lean=False))
    save("carrots", *cross_blades(197, ["#2a7a2a", "#3a8a30", "#4a9a38"], n=7, hmin=4, hmax=8))
    save("potatoes", *cross_blades(198, ["#2a6a2a", "#3a7a30", "#4a8a38"], n=7, hmin=4, hmax=8))
    save("beetroots", *cross_blades(199, ["#2a6a2a", "#6a2a3a", "#8a2a3a"], n=7, hmin=4, hmax=8))
    hb = noise_tex(200, pal("#a88a1a", "#c0a024", "#d2b02e", "#e0c03a"), w=0.4, cx=16, cy=1)
    hb[3, :] = hb[12, :] = hexc("#7a3a12")
    save("hay_block_side", hb)
    save("hay_block_top", noise_tex(201, pal("#b8961e", "#ccaa2a", "#dcba36"), w=0.8))
    gl = np.zeros((S, S, 3)) + hexc("#cfe8ee"); ga = np.zeros((S, S))
    ga[0, :] = ga[S - 1, :] = ga[:, 0] = ga[:, S - 1] = 1
    for k in range(3, 7):
        ga[k, k + 2] = 0.6
    gl[0, :] = gl[:, 0] = hexc("#ffffff")
    save("glass", gl, ga)
    for nm, c in (("white_wool", ("#d8d8d8", "#e4e4e4", "#eeeeee")), ("red_wool", ("#8a1e1e", "#a02424", "#b42c2c")),
                  ("yellow_wool", ("#d8b020", "#e8c02a", "#f4cc34")), ("white_terracotta", ("#c4a898", "#ccb0a0", "#d4b8a8")),
                  ("terracotta", ("#8e5a40", "#985f44", "#a26648"))):
        save(nm, noise_tex(len(nm) * 7, pal(*c), w=0.7))
    save("crafting_table_top", planks(202, ("#5e4527", "#8a6638", "#987140", "#a57c47", "#b2884f")))
    save("bookshelf", planks(203, ("#5e4527", "#8a6638", "#987140", "#a57c47", "#b2884f")))
    bsf = T.SAVED  # noqa
    bk = Image.open(f"{OUT}/bookshelf.png").convert("RGB")
    bka = np.array(bk).astype(float)
    r = R(204)
    for row in (1, 9):
        for x in range(1, 15):
            c = ["#7a1e1e", "#1e3a7a", "#2a6a2a", "#7a5a1e", "#4a2a6a"][int(r.integers(0, 5))]
            bka[row:row + 6, x] = hexc(c) * (0.8 + 0.2 * r.random())
    save("bookshelf", bka)
    save("barrel_side", planks(205, ("#4a3018", "#6a4a28", "#76522c", "#805a30", "#8a6234")))
    save("composter", planks(206, ("#4a3018", "#7a5a30", "#866236", "#906a3a", "#9a723e")))
    # doors / torch / misc
    dr = planks(207, ("#5e4527", "#8a6638", "#987140", "#a57c47", "#b2884f"))
    frame(dr, "#b2884f", "#4a3420")
    save("oak_door", dr)
    save("iron_door", frame(noise_tex(208, pal("#b8b8b8", "#c4c4c4", "#d0d0d0"), w=0.6), "#e0e0e0", "#7a7a7a"))
    tor = np.zeros((S, S, 3)) + hexc("#6a4a2a")
    tor[:, 7:9] = hexc("#7e5a34")
    tor[6:8, 7:9] = hexc("#ffe07a")
    tor[5:6, 7:9] = hexc("#fff4c0")
    save("torch", tor)
    save("cobweb", *sprite(["w.......w.......", ".w......w......w", "..w.....w.....w.", "...w....w....w..", "....w...w...w...",
                            ".....w..w..w....", "......wwwww.....", "wwwwwwwwwwwwwwww", "......wwwww.....", ".....w..w..w....",
                            "....w...w...w...", "...w....w....w..", "..w.....w.....w.", ".w......w......w", "w.......w.......", ""],
                           {"w": "#e8e8e8"}))
    save("snow", noise_tex(209, pal("#eef2f6", "#f6f8fb", "#ffffff"), w=0.8))
    save("ice", noise_tex(210, pal("#8ab0ec", "#98bcf0", "#a6c8f4"), w=0.6))
    save("podzol_top", noise_tex(211, pal("#4a3018", "#5a3c20", "#6a4828", "#7a5432"), w=0.6))
    save("coarse_dirt", noise_tex(212, pal("#4e3524", "#5e412c", "#6e4d34", "#7e5a3e"), w=0.9))
    save("mycelium_top", noise_tex(213, pal("#6a5a6a", "#7a6878", "#887686"), w=0.7))
    save("moss_block", noise_tex(214, pal("#4a6a24", "#56782a", "#628632", "#6e943a"), w=0.6))
    save("bee_nest_side", planks(215, ("#8a5a20", "#c89a40", "#d4a64a", "#deb054", "#e8ba5e")))
    save("bee_nest_top", noise_tex(216, pal("#c89a40", "#d4a64a", "#deb054"), w=0.6))
    save("prismarine", noise_tex(217, pal("#3a8a7a", "#4a9a8a", "#5aaa9a", "#6abaa8"), w=0.6))
    save("magma_block", Image.open(f"{OUT}/magma.png").convert("RGB"))

    # ---------------------------------------------------------------- ranked tier blocks
    def coal_inner(img, r):
        for (x, y) in [(4, 4), (9, 6), (6, 10), (11, 11), (3, 12)]:
            img[y:y + 2, x:x + 2] = hexc("#0a0a0c")
            img[y, x] = hexc("#3a3a40")
    save("coal_block", metal_block(220, ("#161618", "#1c1c1f", "#222226", "#28282c"), "#3a3a40", "#08080a", coal_inner))

    def iron_inner(img, r):
        for y in (5, 10):
            img[y, 2:14] = hexc("#a8a8a8")
        for x in (5, 10):
            img[2:14, x] = hexc("#bcbcbc")
        img[3, 3] = img[12, 12] = hexc("#ffffff")
    save("iron_block", metal_block(221, ("#d2d2d2", "#dadada", "#e2e2e2", "#cacaca"), "#f4f4f4", "#8e8e8e", iron_inner))

    def diamond_inner(img, r):
        for k in range(3, 13):
            img[k, k] = hexc("#e8ffff")
            img[k, 15 - k] = hexc("#5ad8e0")
        img[3:5, 7:9] = hexc("#ffffff")
    save("diamond_block", metal_block(222, ("#5ad8d2", "#6ae4de", "#7aeeea", "#4ac8c4"), "#c8ffff", "#2a9a98", diamond_inner))

    def emerald_inner(img, r):
        for y in range(3, 13):
            for x in range(3, 13):
                if (x + y) % 4 == 0:
                    img[y, x] = hexc("#9affb8")
                elif (x - y) % 4 == 0:
                    img[y, x] = hexc("#1a8a44")
        img[4, 4:7] = hexc("#e0ffe8")
    save("emerald_block", metal_block(223, ("#2ac860", "#38d46c", "#46e078", "#22b456"), "#a6ffc4", "#127a38", emerald_inner))

    def netherite_inner(img, r):
        for y in (4, 8, 12):
            img[y, 2:14] = hexc("#2a2628")
            img[y - 1, 2:14] = hexc("#5a5054")
        for x in (6, 11):
            img[2:14, x] = img[2:14, x] * 0.8
    save("netherite_block", metal_block(224, ("#3a3436", "#423b3e", "#4a4246", "#342e30"), "#6a6064", "#1a1618", netherite_inner))
    print(len(T.SAVED) - n0, "new textures")


if __name__ == "__main__":
    main()
