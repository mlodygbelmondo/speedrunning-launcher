#!/usr/bin/env python3
"""Mock bake-off app: speaks the full SPEC.md JSONL protocol without drawing anything.

Use it to test the harnesses before the real apps exist:

    python3 bakeoff/harness/linux/run.py --apps mock --runs 1 --phase-ms 2000

Behaviour (all timing is wall-clock, like the real apps):
  * emits `start` immediately and `first_frame` after a fake 150-300 ms "startup";
  * allocates ~40 MB in the host process and spawns one helper child (~15 MB) during
    `transitions`, so the harness has to re-discover the process tree;
  * records fake frame timestamps at MOCK_FPS (default 144) during transitions and scroll,
    with an occasional injected long frame so the jank maths has something to find;
  * frees most memory in `destroyed`, emits `done` and exits 0;
  * by default leaves the helper child running as an orphan (MOCK_ORPHAN=0 disables it),
    which checks that the harness cleans up leftovers.

Env: BAKEOFF_OUT (required for scenario mode), BAKEOFF_PHASE_MS (default 20000),
     MOCK_FPS (default 144), MOCK_JANK_EVERY (default 97 frames, 0 = never), MOCK_ORPHAN (default 1).
"""
import json
import os
import random
import subprocess
import sys
import time

CHILD_CODE = r"""
import os, signal, sys, time
blob = bytearray(15 * 1024 * 1024)
for i in range(0, len(blob), 4096):
    blob[i] = 1
deadline = time.time() + float(sys.argv[1])
while time.time() < deadline:
    time.sleep(0.05)
"""


def now_ms():
    return time.time_ns() / 1e6


class Reporter:
    def __init__(self, path):
        self.f = open(path, "a", buffering=1, encoding="utf-8")

    def emit(self, obj):
        self.f.write(json.dumps(obj, separators=(",", ":")) + "\n")
        self.f.flush()


def touch(buf):
    for i in range(0, len(buf), 4096):
        buf[i] = 1


def frame_loop(duration_ms, fps, jank_every):
    """Fake rAF loop: returns timestamps in ms relative to the phase start."""
    period = 1.0 / fps
    t0 = time.perf_counter()
    end = t0 + duration_ms / 1000.0
    ts = []
    n = 0
    next_t = t0
    while True:
        now = time.perf_counter()
        if now >= end:
            break
        ts.append(round((now - t0) * 1000.0, 3))
        n += 1
        # a little fake work per frame
        s = 0
        for i in range(2000):
            s += i * i
        next_t += period
        if jank_every and n % jank_every == 0:
            next_t += period * 2  # skip two refreshes
        delay = next_t - time.perf_counter()
        if delay > 0:
            time.sleep(delay)
        else:
            next_t = time.perf_counter()
    return ts


def main():
    out = os.environ.get("BAKEOFF_OUT")
    if not out:
        print("mock_app: BAKEOFF_OUT not set; nothing to do (scenario mode only).")
        return 0
    phase_ms = float(os.environ.get("BAKEOFF_PHASE_MS", "20000"))
    fps = float(os.environ.get("MOCK_FPS", "144"))
    jank_every = int(os.environ.get("MOCK_JANK_EVERY", "97"))
    orphan = os.environ.get("MOCK_ORPHAN", "1") != "0"

    r = Reporter(out)
    r.emit({"ev": "start", "t": now_ms(), "stack": "mock", "pid": os.getpid(),
            "version": "mock 1.0 / python %s" % sys.version.split()[0]})

    heap = bytearray(40 * 1024 * 1024)
    touch(heap)
    time.sleep(random.uniform(0.15, 0.30))
    r.emit({"ev": "first_frame", "t": now_ms()})
    r.emit({"ev": "info", "note": "mock app, fake frames", "refresh_hz": fps})

    # idle
    r.emit({"ev": "phase", "name": "idle", "t": now_ms()})
    time.sleep(phase_ms / 1000.0)

    # transitions (spawn the helper child here so the harness must re-discover the tree)
    r.emit({"ev": "phase", "name": "transitions", "t": now_ms()})
    child_life = phase_ms * 6 / 1000.0 + 30
    child = subprocess.Popen([sys.executable, "-c", CHILD_CODE, str(child_life)])
    ts = frame_loop(phase_ms, fps, jank_every)
    r.emit({"ev": "frames", "phase": "transitions", "ts": ts})

    # scroll
    r.emit({"ev": "phase", "name": "scroll", "t": now_ms()})
    ts = frame_loop(phase_ms, fps, jank_every)
    r.emit({"ev": "frames", "phase": "scroll", "ts": ts})

    # minimized: a throttled toolkit would stop; mimic a 1 Hz timer counter
    r.emit({"ev": "phase", "name": "minimized", "t": now_ms()})
    t0 = time.perf_counter()
    mts = []
    while (time.perf_counter() - t0) * 1000.0 < phase_ms:
        mts.append(round((time.perf_counter() - t0) * 1000.0, 3))
        time.sleep(min(1.0, phase_ms / 1000.0 / 4))
    r.emit({"ev": "frames", "phase": "minimized", "ts": mts})

    # destroyed: drop the "UI" memory, stay alive
    r.emit({"ev": "phase", "name": "destroyed", "t": now_ms()})
    del heap
    time.sleep(phase_ms / 1000.0)

    r.emit({"ev": "done", "t": now_ms()})
    if not orphan:
        child.terminate()
        child.wait()
    return 0


if __name__ == "__main__":
    sys.exit(main())
