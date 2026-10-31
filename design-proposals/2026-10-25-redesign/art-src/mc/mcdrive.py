"""Drive an official Minecraft dedicated server through stdin (no rcon, no mods).

Used only to generate vanilla worlds locally; nothing from the server leaves /tmp/srl/mcworld.
"""
import os
import queue
import re
import subprocess
import threading
import time


class Server:
    def __init__(self, workdir, jar, java="java", xmx="8G", seed=None, props=None, log="server.log", jvm=()):
        self.dir = workdir
        os.makedirs(workdir, exist_ok=True)
        with open(f"{workdir}/eula.txt", "w") as f:
            f.write("eula=true\n")
        p = {
            "online-mode": "false", "server-port": "25601", "view-distance": "4", "max-tick-time": "-1",
            "spawn-protection": "0", "generate-structures": "true", "level-name": "world",
            "sync-chunk-writes": "false", "enable-rcon": "false", "enable-query": "false",
        }
        if seed is not None:
            p["level-seed"] = str(seed)
        p.update(props or {})
        with open(f"{workdir}/server.properties", "w") as f:
            for k, v in p.items():
                f.write(f"{k}={v}\n")
        self.q = queue.Queue()
        self.logf = open(f"{workdir}/{log}", "a")
        self.p = subprocess.Popen([java, f"-Xmx{xmx}", f"-Xms{xmx}", *jvm, "-jar", jar, "nogui"], cwd=workdir,
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                  text=True, bufsize=1)
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self):
        for line in self.p.stdout:
            self.logf.write(line)
            self.logf.flush()
            self.q.put(line.rstrip("\n"))
        self.q.put(None)

    def drain(self):
        out = []
        while True:
            try:
                out.append(self.q.get_nowait())
            except queue.Empty:
                return out

    def wait_for(self, pattern, timeout=600):
        rx = re.compile(pattern)
        t0 = time.time()
        while time.time() - t0 < timeout:
            try:
                line = self.q.get(timeout=1)
            except queue.Empty:
                continue
            if line is None:
                raise RuntimeError("server exited")
            m = rx.search(line)
            if m:
                return m, line
        raise TimeoutError(pattern)

    def send(self, cmd):
        self.p.stdin.write(cmd + "\n")
        self.p.stdin.flush()

    def cmd(self, cmd, pattern=None, timeout=120):
        self.drain()
        self.send(cmd)
        if pattern:
            return self.wait_for(pattern, timeout)
        return None

    def ready(self, timeout=900):
        return self.wait_for(r"Done \(", timeout)

    # ---------------------------------------------------------------- helpers
    def locate(self, what, dim="minecraft:overworld", at=None, timeout=300, kind="structure"):
        pos = f"positioned {at[0]} 64 {at[1]} " if at else ""
        base = f"locate {what}" if kind == "structure" else f"locate biome {what}"
        self.drain()
        self.send(f"execute in {dim} {pos}run {base}")
        m, line = self.wait_for(r"(nearest .* is at \[(-?\d+), (~|-?\d+), (-?\d+)\]|Could not find|There is no|Unable to find|nearest)", timeout)
        mm = re.search(r"is at \[(-?\d+), (~|-?\d+), (-?\d+)\]", line)
        if not mm:
            return None, line
        y = None if mm.group(2) == "~" else int(mm.group(2))
        return (int(mm.group(1)), y, int(mm.group(3))), line

    def forceload_area(self, dim, x0, z0, x1, z1):
        """Force-load (=> fully generate) every chunk in the block box, in <=256 chunk batches."""
        cx0, cz0, cx1, cz1 = x0 >> 4, z0 >> 4, x1 >> 4, z1 >> 4
        step = 16
        n = 0
        for a in range(cx0, cx1 + 1, step):
            for b in range(cz0, cz1 + 1, step):
                ax1, bz1 = min(a + step - 1, cx1), min(b + step - 1, cz1)
                self.send(f"execute in {dim} run forceload add {a * 16} {b * 16} {ax1 * 16} {bz1 * 16}")
                n += (ax1 - a + 1) * (bz1 - b + 1)
        return n

    def wait_generated(self, dim, x0, z0, x1, z1, timeout=3600, every=4):
        """Poll chunk centres with `execute if block` until none reports 'not loaded'."""
        t0 = time.time()
        pts = [(cx * 16 + 8, cz * 16 + 8) for cx in range(x0 >> 4, (x1 >> 4) + 1, every)
               for cz in range(z0 >> 4, (z1 >> 4) + 1, every)]
        pts.append((x1, z1))
        while time.time() - t0 < timeout:
            self.drain()
            for (x, z) in pts:
                self.send(f"execute in {dim} if block {x} 0 {z} minecraft:bedrock")
            time.sleep(3)
            lines = self.drain()
            bad = sum(1 for l in lines if "not loaded" in l)
            ok = sum(1 for l in lines if "Test passed" in l or "Test failed" in l)
            print(f"[mc] gen poll: ok={ok} notloaded={bad} of {len(pts)} ({time.time() - t0:.0f}s)", flush=True)
            if bad == 0 and ok >= len(pts):
                return True
            time.sleep(5)
        raise TimeoutError("generation")

    def save(self):
        self.cmd("save-all flush", r"Saved the game|Saved the world", timeout=600)

    def stop(self):
        try:
            self.send("stop")
            self.p.wait(timeout=300)
        except Exception:
            self.p.kill()
        self.logf.close()
