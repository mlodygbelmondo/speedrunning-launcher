"""cropcheck.py <png> <fx> <out.jpg>: preview the app crops of a 1920x1080 master (same maths as make-art.sh).

Hero: scale to 800 high, crop 1280 wide at x = round((w-1280)*fx); the left 45% is darkened where the UI text sits.
Card: 640x360 centre crop. Both are stacked into one JPEG.
"""
import sys
from PIL import Image, ImageDraw

src, fx, out = sys.argv[1], float(sys.argv[2]), sys.argv[3]
im = Image.open(src).convert("RGB")
w = round(im.width * 800 / im.height)
s = im.resize((w, 800), Image.LANCZOS)
x = round((w - 1280) * fx)
hero = s.crop((x, 0, x + 1280, 800))
ov = Image.new("RGB", (576, 800), (10, 10, 14))
hero.paste(Image.blend(hero.crop((0, 0, 576, 800)), ov, 0.55), (0, 0))
d = ImageDraw.Draw(hero)
for f in (0.55, 0.80):
    d.line([(int(1280 * f), 0), (int(1280 * f), 800)], fill=(255, 255, 0), width=2)
card = im.resize((640, 360), Image.LANCZOS)
dc = ImageDraw.Draw(card)
for f in (1 / 3, 2 / 3):
    dc.line([(int(640 * f), 0), (int(640 * f), 360)], fill=(255, 255, 0), width=1)
sheet = Image.new("RGB", (1280, 800 + 370), (20, 20, 24))
sheet.paste(hero, (0, 0))
sheet.paste(card, (0, 810))
sheet.resize((960, 878), Image.LANCZOS).save(out, quality=82)
print(out, "scaled w", w, "crop x", x)
