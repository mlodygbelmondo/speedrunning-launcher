import sys, json
sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcregion import Dim, structure_starts, chunk_status
WD = sys.argv[1]
c = json.load(open(f"{WD}/candidates.json"))
dims = {"n": Dim(f"{WD}/world/DIM-1/region"), "o": Dim(f"{WD}/world/region")}
for what, pts in c.items():
    d = dims["n"] if what in ("bastion_remnant", "fortress") else dims["o"]
    for (x, z) in pts:
        ch = d.chunk(x >> 4, z >> 4)
        if ch is None:
            print(what, x, z, "NO CHUNK"); continue
        st = structure_starts(ch)
        keys = [k for k, v in st.items() if v.get("id") != "INVALID"]
        for k in keys:
            v = st[k]
            kids = v.get("Children", [])
            k0 = kids[0] if kids else {}
            info = {kk: vv for kk, vv in k0.items() if kk in ("id", "Template", "pool_element", "BiomeType", "VerticalPlacement", "Rot", "Mirror")}
            pe = k0.get("pool_element")
            if isinstance(pe, dict):
                info["pool_element"] = pe.get("location") or pe
            print(what, (x, z), "status", chunk_status(ch), "key", k, "BB", list(v.get("BB", [])), "n", len(kids), info)
