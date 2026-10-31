"""Make 960px JPEGs (q82) of each render and contact sheets: sheet.jpg (round 1, 6 heroes) and
sheet2.jpg (all 11 images, 3 columns, <=1600px, q80)."""
import os
import sys
from PIL import Image, ImageDraw, ImageFont

OUT = "/tmp/srl/art/out"
HEROES = ["overworld", "nether-portal", "bastion", "fortress", "stronghold", "the-end"]
MODES = ["mode-rsg", "mode-fsg", "mode-ssg", "mode-ranked", "mode-draftout"]
ALL = HEROES + MODES
os.makedirs(f"{OUT}/small", exist_ok=True)

for n in ALL:
    im = Image.open(f"{OUT}/{n}.png").convert("RGB")
    assert im.size == (1920, 1080), (n, im.size)
    im.resize((960, 540), Image.LANCZOS).save(f"{OUT}/small/{n}.jpg", quality=82, optimize=True)

try:
    font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 16)
except OSError:
    font = ImageFont.load_default()


def sheet(names, cols, path, W=1600, GAP=10, LABEL=26):
    cw = (W - (cols + 1) * GAP) // cols
    ch = cw * 9 // 16
    rows = (len(names) + cols - 1) // cols
    H = GAP + rows * (ch + LABEL + GAP)
    s = Image.new("RGB", (W, H), (18, 18, 22))
    d = ImageDraw.Draw(s)
    for i, n in enumerate(names):
        im = Image.open(f"{OUT}/{n}.png").convert("RGB").resize((cw, ch), Image.LANCZOS)
        x = GAP + (i % cols) * (cw + GAP)
        y = GAP + (i // cols) * (ch + LABEL + GAP)
        s.paste(im, (x, y))
        d.text((x + 2, y + ch + 5), f"{n}.png", fill=(220, 220, 228), font=font)
    s.save(path, quality=80, optimize=True)
    return s.size


if "--sheet1" in sys.argv:
    print("sheet", sheet(HEROES, 2, f"{OUT}/sheet.jpg"))
print("sheet2", sheet(ALL, 3, f"{OUT}/sheet2.jpg"), "smalls", len(ALL))
