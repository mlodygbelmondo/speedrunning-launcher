#!/usr/bin/env python3
"""Generate the shared, deterministic bake-off assets (images + data).

Run from anywhere: python3 bakeoff/tools/gen_assets.py
Needs ImageMagick (`convert`) for the hero and glow images.
"""
import json, random, struct, subprocess, zlib
from pathlib import Path

ASSETS = Path(__file__).resolve().parent.parent / "assets"
rng = random.Random(1337)

# --- 500 pixel-art avatars (8x8 blocks scaled to 64x64), pure-stdlib PNG writer ---
PALETTES = [
    [(0x6B, 0x7D, 0x4A), (0x59, 0x6B, 0x3C), (0x8A, 0x6A, 0x45), (0x70, 0x52, 0x34)],  # grass/dirt
    [(0x7F, 0x7F, 0x7F), (0x6A, 0x6A, 0x6A), (0x95, 0x95, 0x95), (0x55, 0x55, 0x55)],  # stone
    [(0x8B, 0x2E, 0x2E), (0x6E, 0x22, 0x22), (0xA8, 0x3C, 0x3C), (0x4A, 0x16, 0x16)],  # netherrack
    [(0x2B, 0x2B, 0x33), (0x3A, 0x34, 0x45), (0x1E, 0x1C, 0x24), (0x8B, 0x5C, 0xF6)],  # blackstone + amethyst
    [(0xD9, 0xC2, 0x6A), (0xC4, 0xA9, 0x4E), (0xE8, 0xD8, 0x8C), (0xA8, 0x8C, 0x3A)],  # gold
    [(0x3F, 0x76, 0xE4), (0x2F, 0x5D, 0xB8), (0x5A, 0x8F, 0xF0), (0x24, 0x48, 0x8F)],  # water
    [(0xE6, 0xE1, 0xC6), (0xD2, 0xCC, 0xAE), (0xF2, 0xEE, 0xD8), (0xB8, 0xB2, 0x92)],  # end stone
    [(0x3B, 0xB0, 0x6B), (0x2E, 0x8F, 0x56), (0x52, 0xC8, 0x82), (0x23, 0x6E, 0x42)],  # emerald
]

def png_bytes(w, h, rows):
    raw = b"".join(b"\x00" + bytes(r) for r in rows)
    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))

(ASSETS / "avatars").mkdir(parents=True, exist_ok=True)
for i in range(500):
    pal = rng.choice(PALETTES)
    grid = [[rng.choice(pal) for _ in range(8)] for _ in range(8)]
    rows = []
    for y in range(64):
        row = []
        for x in range(64):
            row.extend(grid[y // 8][x // 8])
        rows.append(row)
    (ASSETS / "avatars" / f"{i:03d}.png").write_bytes(png_bytes(64, 64, rows))

# --- hero banner + pre-rendered radial glow (GPUI has no radial gradients) ---
subprocess.run(["convert", "-seed", "7", "-size", "400x120", "plasma:#2E0754-#0A090E",
                "-blur", "0x1", "-scale", "1600x480!",
                "(", "-size", "1600x480", "gradient:rgba(10,9,14,0)-rgba(10,9,14,0.85)", "-rotate", "90", "-resize", "1600x480!", ")",
                "-composite", "-quality", "86", str(ASSETS / "hero.jpg")], check=True)
subprocess.run(["convert", "-size", "1200x1200",
                "radial-gradient:rgba(139,92,246,0.55)-rgba(139,92,246,0)",
                str(ASSETS / "glow.png")], check=True)

# --- 500 leaderboard rows + 8 live-pace rows + tile data ---
SYL = ["fein", "cour", "way", "ninja", "brain", "red", "lime", "doog", "ile", "spec", "nr", "zylenox",
       "aquacorde", "priffie", "silver", "rr", "mar", "in", "tow", "lyon", "ky", "bly", "nax", "dibe", "dy",
       "llama", "pag", "mescht", "knawk", "jojoe", "ryguy", "draco", "nix", "ether", "obby", "pearl"]
CATS = ["RSG", "Ranked", "SSG", "FSG", "AA"]
SPLITS = ["Nether", "Bastion", "Fortress", "Blind", "Stronghold", "End"]

def name():
    n = "".join(rng.choice(SYL) for _ in range(rng.randint(2, 3)))
    return n[:1].upper() + n[1:] + (str(rng.randint(1, 99)) if rng.random() < 0.3 else "")

def fmt_ms(ms):
    m, rem = divmod(ms, 60000)
    s, ms3 = divmod(rem, 1000)
    return f"{m}:{s:02d}.{ms3:03d}"

rows = []
for i in range(500):
    t = rng.randint(7 * 60000, 25 * 60000)
    d = rng.randint(-45000, 45000)
    rows.append({
        "rank": i + 1, "name": name(), "category": rng.choice(CATS), "time": fmt_ms(t),
        "delta": f"{'+' if d >= 0 else '-'}{abs(d) / 1000:.1f}", "ahead": d < 0,
        "elo": rng.randint(800, 2400), "avatar": f"avatars/{i:03d}.png",
    })
pace = []
for i in range(8):
    d = rng.randint(-20000, 20000)
    pace.append({"name": name(), "split": rng.choice(SPLITS), "time": fmt_ms(rng.randint(90000, 600000)),
                 "delta": f"{'+' if d >= 0 else '-'}{abs(d) / 1000:.1f}", "ahead": d < 0,
                 "avatar": f"avatars/{rng.randint(0, 499):03d}.png"})
tiles = {
    "pb": "14:32.481", "pb_category": "RSG 1.16.1",
    "today_runs": [rng.randint(3, 30) for _ in range(7)],
    "elo_history": [1500 + rng.randint(-120, 160) for _ in range(24)], "elo": 1642,
}
(ASSETS / "data.json").write_text(json.dumps({"rows": rows, "pace": pace, "tiles": tiles}, indent=1))
print("ok", len(rows), "rows")
