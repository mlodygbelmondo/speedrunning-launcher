import sys, time
sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcdrive import Server
seed = sys.argv[1]
s = Server("/tmp/srl/mcworld/1.16.1/w_" + seed, "/tmp/srl/mcworld/1.16.1/server.jar", seed=seed)
t = time.time()
s.ready()
print("ready in", time.time() - t)
for what, dim in [("bastion_remnant", "minecraft:the_nether"), ("fortress", "minecraft:the_nether"),
                  ("stronghold", "minecraft:overworld"), ("buried_treasure", "minecraft:overworld"),
                  ("ruined_portal", "minecraft:overworld"), ("village", "minecraft:overworld"),
                  ("shipwreck", "minecraft:overworld")]:
    pos, line = s.locate(what, dim)
    print(what, pos, line[-90:])
s.stop()
