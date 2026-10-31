#!/usr/bin/env python3
"""Refreshes the pinned data in data/catalogue.json: sha512, size, the world folders in the zip and
their level.dat DataVersion. Optionally reports newer upstream releases.

    python3 -I tools/refresh_catalogue.py [--write] [--latest] [--cache DIR] [--only ID ...]

- Without --write it only checks: it downloads every installable map (from the author's own URL,
  into --cache, default ~/.cache/srl-practice-refresh), recomputes sha512 and size, reads the zip's
  listing and each level.dat, and prints what differs from the catalogue. Exit code 1 if anything
  differs, so it can run in CI.
- With --write it stores the recomputed values in the catalogue (version and URL stay as they are:
  moving to a new release is a reviewed edit, not something a script does on its own).
- With --latest it also asks the GitHub API for each repo's newest release and prints the entries
  that are behind, with the matching asset. Uses `gh api` when gh is installed (authenticated, no
  rate limit worries), else plain HTTPS.

The zips are data: nothing in them is extracted to disk or run. Only level.dat is read, in memory.
Run it with `python3 -I` so nothing from the current directory is imported.
"""

import argparse
import gzip
import hashlib
import io
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import urllib.request
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
CATALOGUE = os.path.join(HERE, "..", "data", "catalogue.json")
USER_AGENT = "SpeedrunningLauncher-catalogue-refresh/1"
# Minecraft 1.16.1's world DataVersion. A world saved by a newer game asks before opening in 1.16.1.
MC_1_16_1 = 2567


def download(url, dest):
    tmp = dest + ".part"
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=60) as resp, open(tmp, "wb") as out:
        shutil.copyfileobj(resp, out, 1 << 20)
    os.replace(tmp, dest)


def sha512_of(path):
    h = hashlib.sha512()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def read_nbt_fields(raw):
    """Returns (DataVersion, Version.Name, LevelName) from an uncompressed level.dat."""
    buf = io.BytesIO(raw)

    def rd(n):
        b = buf.read(n)
        if len(b) != n:
            raise ValueError("truncated NBT")
        return b

    def string():
        (n,) = struct.unpack(">H", rd(2))
        return rd(n).decode("utf-8", "replace")

    def payload(tag, depth):
        if depth > 512:
            raise ValueError("NBT nested too deep")
        if tag == 1:
            return struct.unpack(">b", rd(1))[0]
        if tag == 2:
            return struct.unpack(">h", rd(2))[0]
        if tag == 3:
            return struct.unpack(">i", rd(4))[0]
        if tag == 4:
            return struct.unpack(">q", rd(8))[0]
        if tag == 5:
            return struct.unpack(">f", rd(4))[0]
        if tag == 6:
            return struct.unpack(">d", rd(8))[0]
        if tag in (7, 11, 12):
            (n,) = struct.unpack(">i", rd(4))
            rd(n * {7: 1, 11: 4, 12: 8}[tag])
            return None
        if tag == 8:
            return string()
        if tag == 9:
            inner = rd(1)[0]
            (n,) = struct.unpack(">i", rd(4))
            return [payload(inner, depth + 1) for _ in range(max(n, 0))]
        if tag == 10:
            out = {}
            while True:
                t = rd(1)[0]
                if t == 0:
                    return out
                name = string()
                out[name] = payload(t, depth + 1)
        raise ValueError(f"unknown NBT tag {tag}")

    if rd(1)[0] != 10:
        raise ValueError("level.dat root is not a compound")
    string()
    root = payload(10, 0)
    data = root.get("Data", {})
    version = data.get("Version") or {}
    return data.get("DataVersion"), version.get("Name"), data.get("LevelName")


def inspect_zip(path):
    """World folders (relative to the zip root, '' for the root itself) with their level.dat fields."""
    worlds = []
    with zipfile.ZipFile(path) as z:
        names = [n.replace("\\", "/") for n in z.namelist()]
        roots = sorted(
            {n[: -len("level.dat")].rstrip("/") for n in names if n == "level.dat" or n.endswith("/level.dat")},
            key=lambda r: (r.count("/"), r),
        )
        # A level.dat inside another world (backups, nested copies) belongs to that world.
        kept = []
        for r in roots:
            if r.startswith("__MACOSX") or any(r == k or k == "" or r.startswith(k + "/") for k in kept):
                continue
            kept.append(r)
        for r in kept:
            member = "level.dat" if r == "" else r + "/level.dat"
            original = next(n for n in z.namelist() if n.replace("\\", "/") == member)
            raw = gzip.decompress(z.read(original))
            dv, vname, level = read_nbt_fields(raw)
            worlds.append({"folder": r, "data_version": dv, "version_name": vname, "level_name": level})
    return worlds


def latest_release(repo):
    try:
        out = subprocess.run(
            ["gh", "api", f"repos/{repo}/releases?per_page=10"], capture_output=True, text=True, check=True
        ).stdout
    except (OSError, subprocess.CalledProcessError):
        req = urllib.request.Request(
            f"https://api.github.com/repos/{repo}/releases?per_page=10", headers={"User-Agent": USER_AGENT}
        )
        with urllib.request.urlopen(req, timeout=30) as resp:
            out = resp.read().decode()
    releases = [r for r in json.loads(out) if not r["draft"] and not r["prerelease"]]
    releases.sort(key=lambda r: r["published_at"], reverse=True)
    return releases[0] if releases else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--write", action="store_true")
    ap.add_argument("--latest", action="store_true")
    ap.add_argument("--cache", default=os.path.expanduser("~/.cache/srl-practice-refresh"))
    ap.add_argument("--only", nargs="*")
    ap.add_argument("--catalogue", default=CATALOGUE)
    args = ap.parse_args()

    with open(args.catalogue, encoding="utf-8") as f:
        cat = json.load(f)
    os.makedirs(args.cache, exist_ok=True)
    differs = False
    for m in cat["maps"]:
        d = m.get("download")
        if not d or (args.only and m["id"] not in args.only):
            continue
        name = re.sub(r"[^A-Za-z0-9._-]", "_", f"{m['id']}-{d['version']}.zip")
        dest = os.path.join(args.cache, name)
        if not os.path.exists(dest) or os.path.getsize(dest) != d.get("size"):
            print(f"downloading {d['url']}")
            download(d["url"], dest)
        sha = sha512_of(dest)
        size = os.path.getsize(dest)
        worlds = inspect_zip(dest)
        new = {
            "sha512": sha,
            "size": size,
            "worlds": [w["folder"] for w in worlds],
            "data_version": max((w["data_version"] or 0) for w in worlds) if worlds else None,
        }
        changed = {k: (d.get(k), v) for k, v in new.items() if d.get(k) != v}
        fits = "fits 1.16.1" if new["data_version"] == MC_1_16_1 else f"NOT 1.16.1 ({MC_1_16_1})"
        print(f"{m['id']} {d['version']}: {size} bytes, worlds {new['worlds']}, DataVersion {new['data_version']} {fits}")
        for w in worlds:
            print(f"    {w['folder']!r}: {w['version_name']} / LevelName {w['level_name']!r}")
        if changed:
            differs = True
            for k, (old, v) in changed.items():
                print(f"    {k}: catalogue {old!r} -> file {v!r}")
            if args.write:
                d.update(new)
        if args.latest and d.get("repo"):
            rel = latest_release(d["repo"])
            pattern = re.compile(d.get("asset_pattern", ".*"))
            assets = [a for a in (rel or {}).get("assets", []) if pattern.search(a["name"])]
            # Compare the asset, not the tag: some authors move one tag (`latest`) between versions.
            if rel and [a["browser_download_url"] for a in assets] != [d["url"]]:
                urls = [(a["browser_download_url"], a["size"]) for a in assets]
                print(f"    NEWER upstream: {rel['tag_name']} ({rel['published_at']}) {urls}")
    if args.write:
        with open(args.catalogue, "w", encoding="utf-8") as f:
            json.dump(cat, f, indent=2, ensure_ascii=False)
            f.write("\n")
    sys.exit(1 if differs and not args.write else 0)


if __name__ == "__main__":
    main()
