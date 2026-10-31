"""Cartography grade for the SSG map: warm parchment tint, paper grain, soft vignette and a thin frame.

uv run --with numpy --with pillow python grade_ssg.py [raw.png] [out.png]
"""
import sys
import numpy as np
from PIL import Image, ImageFilter

src = sys.argv[1] if len(sys.argv) > 1 else "/tmp/srl/art/work/mode-ssg_raw.png"
dst = sys.argv[2] if len(sys.argv) > 2 else "/tmp/srl/art/out/mode-ssg.png"
im = np.asarray(Image.open(src).convert("RGB")).astype(np.float32) / 255.0
H, W, _ = im.shape
paper = np.array([0.93, 0.86, 0.70], np.float32)
# soften saturation a little and pull towards parchment
lum = (im * np.array([0.3, 0.59, 0.11], np.float32)).sum(2, keepdims=True)
im = lum + (im - lum) * 0.82
im = im * 0.84 + paper * 0.16 * (0.6 + 0.4 * lum)
# paper grain (low-frequency blotches + fine fibre noise), deterministic
rng = np.random.default_rng(2026)
blot = Image.fromarray((rng.random((H // 24 + 1, W // 24 + 1)) * 255).astype(np.uint8)).resize((W, H), Image.BICUBIC)
blot = np.asarray(blot.filter(ImageFilter.GaussianBlur(18))).astype(np.float32)[..., None] / 255.0
fine = rng.normal(0, 1, (H, W, 1)).astype(np.float32)
im = im * (0.95 + 0.08 * blot) + fine * 0.012
# vignette, slightly warm at the edges
yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
d = np.sqrt(((xx - W / 2) / (W / 2)) ** 2 + ((yy - H / 2) / (H / 2)) ** 2) / 1.414
v = 1 - 0.28 * np.clip((d - 0.45) / 0.55, 0, 1) ** 1.6
im = im * v[..., None] + (1 - v[..., None]) * np.array([0.30, 0.22, 0.12], np.float32) * 0.25
# thin inked frame
f = 6
ink = np.array([0.24, 0.18, 0.11], np.float32)
im[f:f + 2, f:W - f] = ink
im[H - f - 2:H - f, f:W - f] = ink
im[f:H - f, f:f + 2] = ink
im[f:H - f, W - f - 2:W - f] = ink
Image.fromarray((np.clip(im, 0, 1) * 255 + 0.5).astype(np.uint8)).save(dst)
print("graded", dst)
