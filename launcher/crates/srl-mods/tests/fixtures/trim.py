"""Trims the live mcsr-meta files (and Modrinth version lists) into the test fixtures.

Usage, from crates/srl-mods, with the four live files downloaded into <live dir>
(mods.json, extra.json, legal-builds.csv, conditional-mods.json; URLs in src/meta.rs):

    python3 -I tests/fixtures/trim.py <live dir> tests/fixtures/meta

Keeps only builds for 1.16.1 and 26.3 (plus one 1.16.5 build, to test version matching),
the extras the modes use, the CSV rows of the kept builds plus older legal 1.16.1 builds,
and the three newest Modrinth versions of each extra (fetched live). Never any jar.
"""
import csv, io, json, os, sys, urllib.request

live, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
TARGETS = {"1.16.1", "26.3"}


def keep_versions(m, extra_targets=()):
    vs = [v for v in m["versions"] if set(v["target_version"]) & (TARGETS | set(extra_targets))]
    for v in vs:
        # Long target lists (half-heart-hardcore) are irrelevant beyond our targets.
        v["target_version"] = [t for t in v["target_version"] if t in TARGETS or t in extra_targets] or v["target_version"][:1]
    return vs


mods = json.load(open(os.path.join(live, "mods.json")))
kept = []
for m in mods["mods"]:
    vs = keep_versions(m, ("1.16.5",) if m["modid"] == "lithium" else ())
    if vs:
        m["versions"] = vs
        kept.append(m)
mods["mods"] = kept
json.dump(mods, open(os.path.join(out, "mods.json"), "w"), indent=1)

EXTRA = {"mcsrranked", "fsg-mod", "draftout", "nochatrestrictions", "no-telemetry", "commandqmod", "custom-map-resetter", "peepopractice"}
extra = json.load(open(os.path.join(live, "extra.json")))
extra["mods"] = [dict(m, versions=keep_versions(m)) for m in extra["mods"] if m["modid"] in EXTRA]
json.dump(extra, open(os.path.join(out, "extra.json"), "w"), indent=1)

hashes = {v["sha512"] for m in kept for v in m["versions"]}
ids = {m["modid"] for m in kept}
rows = list(csv.reader(open(os.path.join(live, "legal-builds.csv"))))
head, body = rows[0], rows[1:]
keep_rows = [r for r in body if r[4] in hashes or (r[0] in ids and "1.16.1" in r[1])]
buf = io.StringIO()
w = csv.writer(buf, lineterminator="\n")
w.writerow(head)
w.writerows(keep_rows)
open(os.path.join(out, "legal-builds.csv"), "w").write(buf.getvalue())

open(os.path.join(out, "conditional-mods.json"), "w").write(open(os.path.join(live, "conditional-mods.json")).read())

UA = {"User-Agent": "SpeedrunningLauncher-dev/0.1 (fixtures)"}
FIELDS = ["id", "project_id", "version_number", "version_type", "date_published", "game_versions", "loaders", "files"]
for modid, slug in [("mcsrranked", "mcsr-ranked"), ("fsg-mod", "fsg-mod"), ("draftout", "draftout")]:
    url = f"https://api.modrinth.com/v2/project/{slug}/version?loaders=%5B%22fabric%22%5D"
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA)) as r:
        vs = json.load(r)
    vs = vs[:3]
    for v in vs:
        for k in list(v):
            if k not in FIELDS:
                del v[k]
        for f in v["files"]:
            for k in list(f):
                if k not in ("url", "filename", "primary", "size", "hashes"):
                    del f[k]
    json.dump(vs, open(os.path.join(out, f"modrinth-{modid}.json"), "w"), indent=1)

for f in sorted(os.listdir(out)):
    print(f, os.path.getsize(os.path.join(out, f)))
