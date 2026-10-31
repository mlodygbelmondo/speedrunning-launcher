import sys, json
sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcdrive import Server
ver, seed = sys.argv[1], sys.argv[2]
qs = json.loads(sys.argv[3])  # [[what, dim, x, z], ...]
s = Server(f"/tmp/srl/mcworld/{ver}/w_{seed}", f"/tmp/srl/mcworld/{ver}/server.jar", seed=seed, java=(sys.argv[4] if len(sys.argv) > 4 else "java"))
s.ready()
for what, dim, x, z in qs:
    pos, line = s.locate(what, dim, at=(x, z))
    print("LOC", what, x, z, "->", pos, line[-60:], flush=True)
s.stop()
