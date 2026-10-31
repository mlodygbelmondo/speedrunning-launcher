#!/usr/bin/env python3
"""Speedrunluncher bake-off: Linux harness (Python 3, stdlib only).

Launches each test app in scenario mode (see bakeoff/SPEC.md), samples its whole process
tree every 200 ms (PSS, CPU, context switches), reads the JSONL report the app writes to
$BAKEOFF_OUT, and summarises everything per phase.

    python3 bakeoff/harness/linux/run.py                       # all three apps, 3 runs, 20 s phases
    python3 bakeoff/harness/linux/run.py --apps tauri --runs 1 --phase-ms 5000
    python3 bakeoff/harness/linux/run.py --nested              # headless weston (locked session / CI)
    python3 bakeoff/harness/linux/run.py --apps mock           # harness self-test, no real apps needed

Results: bakeoff/results/linux-<host>-<timestamp>.json (+ .md). Exit code 0 if every run
completed with all protocol events, 2 otherwise.
"""
import argparse
import glob
import json
import math
import os
import platform
import re
import shutil
import signal
import socket
import statistics
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
BAKEOFF = os.path.abspath(os.path.join(HERE, "..", ".."))

DEFAULT_BINS = {
    "tauri": os.path.join(BAKEOFF, "tauri", "src-tauri", "target", "release", "bakeoff-tauri"),
    "electron": os.path.join(BAKEOFF, "electron", "out", "bakeoff-electron-linux-x64", "bakeoff-electron"),
    "gpui": os.path.join(BAKEOFF, "gpui", "target", "release", "bakeoff-gpui"),
    "mock": os.path.join(BAKEOFF, "harness", "mock_app.py"),
}

PHASES = ["idle", "transitions", "scroll", "minimized", "destroyed"]
FRAME_PHASES = ["transitions", "scroll"]
REQUIRED_EVENTS = ["start", "first_frame", "done"]

CLK_TCK = os.sysconf("SC_CLK_TCK")
PAGE_KB = os.sysconf("SC_PAGE_SIZE") // 1024

# Targets from bakeoff/README.md
TARGETS = {
    "refresh_coverage_pct_min": 99.0,
    "p99_vs_refresh_max": 1.5,
    "warm_start_ms_max": 400.0,
    "cold_start_ms_max": 1000.0,
    "idle_cpu_pct_max": 0.5,
    "destroyed_main_cpu_pct_max": 0.2,  # "CPU ~ 0"
    "destroyed_main_mb_max": 20.0,
}

_started_procs = []  # (Popen, label) for cleanup on exit/signal (apps first, weston last)


def log(msg=""):
    print(msg, flush=True)


def warn_box(lines):
    width = max(len(l) for l in lines) + 4
    log("!" * width)
    for l in lines:
        log("! " + l.ljust(width - 4) + " !")
    log("!" * width)


def run_cmd(cmd, timeout=5, env=None):
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, env=env)
        return p.returncode, p.stdout, p.stderr
    except (OSError, subprocess.SubprocessError) as e:
        return -1, "", str(e)


def read_file(path, default=None):
    try:
        with open(path, "r", errors="replace") as f:
            return f.read()
    except OSError:
        return default


# --------------------------------------------------------------------------------------
# Environment detection
# --------------------------------------------------------------------------------------

def detect_refresh_gnome():
    """Current refresh rate(s) via org.gnome.Mutter.DisplayConfig.GetCurrentState."""
    rc, out, _ = run_cmd(["gdbus", "call", "--session", "--dest", "org.gnome.Mutter.DisplayConfig",
                          "--object-path", "/org/gnome/Mutter/DisplayConfig",
                          "--method", "org.gnome.Mutter.DisplayConfig.GetCurrentState"])
    if rc != 0 or not out:
        return None
    rates = []
    # mode tuple: ('1920x1080@144.028', 1920, 1080, 144.0279..., 1.0, [scales], {props})
    for m in re.finditer(r"\('([^']+)', (\d+), (\d+), ([0-9.]+), [^{}]*?\{([^{}]*)\}\)", out):
        if "'is-current': <true>" in m.group(5):
            rates.append({"mode": m.group(1), "hz": float(m.group(4)),
                          "vrr": "+vrr" in m.group(1)})
    if not rates:
        return None
    return {"source": "gnome-displayconfig", "hz": max(r["hz"] for r in rates), "modes": rates}


def detect_refresh_xrandr():
    if not os.environ.get("DISPLAY"):
        return None
    rc, out, _ = run_cmd(["xrandr", "--current"])
    if rc != 0:
        return None
    rates = [float(x) for x in re.findall(r"([0-9]+\.[0-9]+)\*", out)]
    if not rates:
        return None
    return {"source": "xrandr", "hz": max(rates), "modes": rates}


def detect_refresh(arg):
    if arg and arg != "auto":
        return {"source": "--refresh-hz", "hz": float(arg)}
    return detect_refresh_gnome() or detect_refresh_xrandr() or {"source": "fallback", "hz": 60.0}


def graphical_sessions():
    """Return [{id, type, locked, active}] for the current user's graphical logind sessions."""
    res = []
    rc, out, _ = run_cmd(["loginctl", "list-sessions", "--no-legend"])
    if rc != 0:
        return res
    user = os.environ.get("USER") or ""
    uid = str(os.getuid())
    for line in out.splitlines():
        parts = line.split()
        if len(parts) < 3 or (parts[1] != uid and parts[2] != user):
            continue
        sid = parts[0]
        rc, props, _ = run_cmd(["loginctl", "show-session", sid, "-p", "Type", "-p", "LockedHint",
                                "-p", "Active", "-p", "Class"])
        d = dict(l.split("=", 1) for l in props.splitlines() if "=" in l)
        if d.get("Type") in ("wayland", "x11"):
            res.append({"id": sid, "type": d.get("Type"), "locked": d.get("LockedHint") == "yes",
                        "active": d.get("Active") == "yes"})
    return res


def screensaver_active():
    rc, out, _ = run_cmd(["gdbus", "call", "--session", "--dest", "org.gnome.ScreenSaver",
                          "--object-path", "/org/gnome/ScreenSaver",
                          "--method", "org.gnome.ScreenSaver.GetActive"])
    if rc != 0:
        return None
    return "true" in out


def detect_gpus():
    gpus = []
    rc, out, _ = run_cmd(["lspci", "-nnk"])
    if rc == 0:
        cur = None
        for line in out.splitlines():
            if not line.startswith(("\t", " ")):
                cur = None
                if re.search(r"VGA compatible|3D controller|Display controller", line):
                    cur = {"device": line.split(" ", 1)[1].strip(), "slot": line.split(" ", 1)[0]}
                    gpus.append(cur)
            elif cur is not None and "Kernel driver in use:" in line:
                cur["driver"] = line.split(":", 1)[1].strip()
    nv = read_file("/proc/driver/nvidia/version")
    info = {"lspci": gpus}
    if nv:
        m = re.search(r"Kernel Module(?: for x86_64)?\s+([0-9.]+)", nv)
        info["nvidia_driver"] = m.group(1) if m else nv.splitlines()[0]
        info["nvidia_drm_modeset"] = (read_file("/sys/module/nvidia_drm/parameters/modeset", "") or "").strip()
    rc, out, _ = run_cmd(["nvidia-smi", "--query-gpu=name,driver_version,pstate", "--format=csv,noheader"])
    if rc == 0 and out.strip():
        info["nvidia_smi"] = out.strip()
    # render nodes -> driver/PCI slot (used to explain which GPU each app opened)
    nodes = {}
    for node in sorted(glob.glob("/sys/class/drm/renderD*")):
        dev = os.path.realpath(os.path.join(node, "device"))
        drv = os.path.basename(os.path.realpath(os.path.join(dev, "driver"))) if os.path.exists(os.path.join(dev, "driver")) else "?"
        nodes[os.path.basename(node)] = {"driver": drv, "pci": os.path.basename(dev)}
    info["render_nodes"] = nodes
    for tool in (["glxinfo", "-B"], ["eglinfo", "-B"]):
        if shutil.which(tool[0]):
            rc, out, _ = run_cmd(tool)
            if rc == 0:
                keep = [l.strip() for l in out.splitlines()
                        if re.search(r"OpenGL renderer|OpenGL core profile version|OpenGL version string|EGL vendor|EGL driver name", l)]
                info[tool[0]] = keep[:8]
                break
    return info


def egl_hints():
    """Static hints for 'does WebKitGTK use the NVIDIA EGL path'. The runtime answer comes from
    the /proc/<pid>/maps + fd scan done during each run (see gpu_usage in the results)."""
    vendors = sorted(os.path.basename(p) for p in glob.glob("/usr/share/glvnd/egl_vendor.d/*.json")
                     + glob.glob("/etc/glvnd/egl_vendor.d/*.json"))
    keys = ["__EGL_VENDOR_LIBRARY_FILENAMES", "__EGL_VENDOR_LIBRARY_DIRS", "__NV_PRIME_RENDER_OFFLOAD",
            "__GLX_VENDOR_LIBRARY_NAME", "__VK_LAYER_NV_optimus", "DRI_PRIME", "MESA_VK_DEVICE_SELECT",
            "WEBKIT_DISABLE_DMABUF_RENDERER", "WEBKIT_DISABLE_COMPOSITING_MODE", "WEBKIT_FORCE_SANDBOX",
            "GDK_BACKEND", "LIBGL_ALWAYS_SOFTWARE", "ELECTRON_OZONE_PLATFORM_HINT"]
    env = {k: os.environ[k] for k in keys if k in os.environ}
    return {"egl_vendor_files": vendors, "nvidia_egl_vendor_installed": any("nvidia" in v for v in vendors),
            "env": env}


def power_hints():
    h = {}
    for ac in glob.glob("/sys/class/power_supply/*/online"):
        typ = (read_file(os.path.join(os.path.dirname(ac), "type"), "") or "").strip()
        if typ in ("Mains", "USB", "USB_C", "USB_PD"):
            h.setdefault("ac_online", []).append((read_file(ac, "") or "").strip() == "1")
    if "ac_online" in h:
        h["on_ac"] = any(h.pop("ac_online"))
    prof = read_file("/sys/firmware/acpi/platform_profile")
    if prof:
        h["platform_profile"] = prof.strip()
    gov = read_file("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
    if gov:
        h["cpu_governor"] = gov.strip()
    epp = read_file("/sys/devices/system/cpu/cpu0/cpufreq/energy_performance_preference")
    if epp:
        h["cpu_epp"] = epp.strip()
    return h


def cpu_model():
    txt = read_file("/proc/cpuinfo", "")
    m = re.search(r"model name\s*:\s*(.+)", txt)
    return m.group(1).strip() if m else platform.processor()


def os_release():
    txt = read_file("/etc/os-release", "")
    m = re.search(r'PRETTY_NAME="?([^"\n]+)', txt)
    return m.group(1) if m else platform.platform()


def mem_total_mb():
    m = re.search(r"MemTotal:\s+(\d+)", read_file("/proc/meminfo", "") or "")
    return round(int(m.group(1)) / 1024) if m else None


def electron_sandbox_problem(binary):
    """Electron's SUID sandbox helper must be root:root 4755, otherwise Electron aborts at startup
    (Ubuntu 24.04+ also blocks the user-namespace fallback via AppArmor). Returns the fix command
    or None when everything is fine."""
    sandbox = os.path.join(os.path.dirname(binary), "chrome-sandbox")
    try:
        st = os.stat(sandbox)
    except OSError:
        return None  # no helper shipped: Electron uses namespaces or the app disables it itself
    if st.st_uid == 0 and st.st_gid == 0 and (st.st_mode & 0o7777) == 0o4755:
        return None
    return "sudo chown root:root '%s' && sudo chmod 4755 '%s'" % (sandbox, sandbox)


# --------------------------------------------------------------------------------------
# Nested weston
# --------------------------------------------------------------------------------------

class NestedWeston:
    def __init__(self, hz, width=1920, height=1080):
        self.hz = hz
        self.width = width
        self.height = height
        self.proc = None
        self.socket = None
        self.renderer = None
        self.xdisplay = None
        self.logpath = None
        self.runtime_dir = None
        self._own_runtime = False

    def start(self):
        if not shutil.which("weston"):
            raise SystemExit("--nested needs weston (sudo apt install weston xwayland)")
        rt = os.environ.get("XDG_RUNTIME_DIR")
        if not rt or not os.path.isdir(rt):
            rt = tempfile.mkdtemp(prefix="bakeoff-xdg-")
            os.chmod(rt, 0o700)
            os.environ["XDG_RUNTIME_DIR"] = rt
            self._own_runtime = True
        self.runtime_dir = rt
        name = "bakeoff-h"
        if not self._socket_free(rt, name):
            name = "bakeoff-h-%d" % os.getpid()
        self.socket = name
        rc, out, err = run_cmd(["weston", "--version"])
        log("[nested] %s" % (out or err).strip())
        attempts = [["--renderer=gl"], ["--renderer=pixman"], ["--use-pixman"]]
        # --refresh-rate for the headless backend only exists in weston >= 14 (Ubuntu 24.04 ships 13)
        rc_h, help_out, help_err = run_cmd(["weston", "--help"])
        self.refresh_supported = "--refresh-rate" in (help_out or "") + (help_err or "")
        if not self.refresh_supported:
            log("[nested] this weston has no --refresh-rate; output runs at its default (60 Hz)")
        for extra in attempts:
            self.logpath = os.path.join(tempfile.gettempdir(), "bakeoff-weston-%d.log" % os.getpid())
            cmd = ["weston", "--backend=headless"] + extra + [
                "--width=%d" % self.width, "--height=%d" % self.height,
                "--socket=%s" % name] + (
                ["--refresh-rate=%d" % int(round(self.hz * 1000))] if self.refresh_supported else []) + [
                "--xwayland", "--idle-time=0",  # default 300 s idle blanks the output (~1 fps after)
                "--log=%s" % self.logpath]
            env = dict(os.environ)
            env.pop("WAYLAND_DISPLAY", None)
            env.pop("WAYLAND_SOCKET", None)
            env.pop("DISPLAY", None)
            log("[nested] starting: %s" % " ".join(cmd))
            self.proc = subprocess.Popen(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                         start_new_session=True)
            _started_procs.append((self.proc, "weston"))
            sock = os.path.join(rt, name)
            deadline = time.time() + 15
            ok = False
            while time.time() < deadline:
                if self.proc.poll() is not None:
                    break
                txt = read_file(self.logpath, "") or ""
                if os.path.exists(sock) and ("enabled with head" in txt or "Output 'headless'" in txt
                                             or time.time() > deadline - 10):
                    ok = True
                    break
                time.sleep(0.1)
            if ok and self.proc.poll() is None:
                self.renderer = extra[0].split("=")[-1].replace("--use-", "")
                time.sleep(1.0)  # let xwayland/desktop-shell settle
                txt = read_file(self.logpath, "") or ""
                m = re.search(r"xserver listening on display (:\d+)", txt)
                self.xdisplay = m.group(1) if m else None
                log("[nested] weston up: socket=%s renderer=%s xwayland=%s" % (name, self.renderer, self.xdisplay))
                return
            log("[nested] weston %s failed; log tail:\n%s" % (extra[0], "\n".join((read_file(self.logpath, "") or "").splitlines()[-8:])))
            self.stop()
        raise SystemExit("could not start headless weston with gl or pixman renderer")

    @staticmethod
    def _socket_free(rt, name):
        """True if no compositor holds <name>.lock; stale socket/lock files are removed."""
        import fcntl
        lock = os.path.join(rt, name + ".lock")
        if not os.path.exists(lock):
            if os.path.exists(os.path.join(rt, name)):
                try:
                    os.unlink(os.path.join(rt, name))
                except OSError:
                    return False
            return True
        try:
            fd = os.open(lock, os.O_RDWR)
        except OSError:
            return False
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            os.close(fd)
            return False  # a live compositor owns it
        os.close(fd)
        for f in (lock, os.path.join(rt, name)):
            try:
                os.unlink(f)
            except OSError:
                pass
        return True

    def env(self, base):
        env = dict(base)
        env["WAYLAND_DISPLAY"] = self.socket
        env.pop("WAYLAND_SOCKET", None)
        env["XDG_SESSION_TYPE"] = "wayland"
        env["GDK_BACKEND"] = "wayland"
        env["ELECTRON_OZONE_PLATFORM_HINT"] = "wayland"
        if self.xdisplay:
            env["DISPLAY"] = self.xdisplay
        else:
            env.pop("DISPLAY", None)
        return env

    def log_tail(self, n=40):
        return (read_file(self.logpath, "") or "").splitlines()[-n:]

    def stop(self):
        if self.proc and self.proc.poll() is None:
            try:
                os.killpg(self.proc.pid, signal.SIGTERM)
            except OSError:
                pass
            try:
                self.proc.wait(5)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(self.proc.pid, signal.SIGKILL)
                except OSError:
                    pass
                self.proc.wait(5)
        if self._own_runtime and self.runtime_dir:
            shutil.rmtree(self.runtime_dir, ignore_errors=True)


# --------------------------------------------------------------------------------------
# /proc sampling
# --------------------------------------------------------------------------------------

def read_stat(pid):
    """-> dict(ppid, pgrp, sid, utime, stime, start, rss_kb, comm) or None."""
    try:
        with open("/proc/%d/stat" % pid, "rb") as f:
            data = f.read().decode("utf-8", "replace")
    except OSError:
        return None
    lp, rp = data.find("("), data.rfind(")")
    comm = data[lp + 1:rp]
    f = data[rp + 2:].split()
    try:
        return {"ppid": int(f[1]), "pgrp": int(f[2]), "sid": int(f[3]), "utime": int(f[11]),
                "stime": int(f[12]), "start": int(f[19]), "rss_kb": int(f[21]) * PAGE_KB, "comm": comm,
                "state": f[0]}
    except (IndexError, ValueError):
        return None


def read_pss_kb(pid):
    try:
        with open("/proc/%d/smaps_rollup" % pid, "rb") as f:
            for line in f:
                if line.startswith(b"Pss:"):
                    return int(line.split()[1]), "pss"
    except (OSError, ValueError, IndexError):
        pass
    return None, None


def read_ctxsw(pid):
    """Per-thread voluntary+nonvoluntary context switches: {tid: count}."""
    res = {}
    try:
        tids = os.listdir("/proc/%d/task" % pid)
    except OSError:
        return res
    for tid in tids:
        try:
            with open("/proc/%d/task/%s/status" % (pid, tid), "rb") as f:
                total = 0
                for line in f:
                    if line.startswith(b"voluntary_ctxt_switches") or line.startswith(b"nonvoluntary_ctxt_switches"):
                        total += int(line.split()[1])
                res[int(tid)] = total
        except (OSError, ValueError, IndexError):
            continue
    return res


def all_pids():
    out = []
    for name in os.listdir("/proc"):
        if name.isdigit():
            out.append(int(name))
    return out


class TreeSampler:
    """Samples a process tree rooted at `root` (spawned with start_new_session=True, so its
    session id == root pid). Membership: descendants via ppid + anything in the root's session
    + anything we've seen before that is still alive (pid,starttime match), so orphans that got
    re-parented to a subreaper stay counted."""

    def __init__(self, root, ctxsw=True):
        self.root = root
        self.ctxsw = ctxsw
        self.known = {}       # (pid,start) -> comm
        self.prev_cpu = {}    # (pid,start) -> ticks
        self.prev_ctx = {}    # (pid,start,tid) -> count
        self.prev_t = None
        self.root_key = None
        self.pss_source = "pss"

    def members(self):
        stats = {}
        for pid in all_pids():
            s = read_stat(pid)
            if s:
                stats[pid] = s
        members = {}
        if self.root in stats:
            self.root_key = (self.root, stats[self.root]["start"])
        # pass 1: session + known
        for pid, s in stats.items():
            key = (pid, s["start"])
            if pid == self.root or s["sid"] == self.root or s["pgrp"] == self.root or key in self.known:
                members[pid] = s
        # pass 2: descendants (fixed point)
        changed = True
        while changed:
            changed = False
            for pid, s in stats.items():
                if pid not in members and s["ppid"] in members:
                    members[pid] = s
                    changed = True
        return members

    def sample(self):
        t = time.time_ns() / 1e6
        mem = self.members()
        first = self.prev_t is None
        dt = (t - self.prev_t) if not first else 0.0
        tot_pss = main_pss = 0
        tot_cpu_ticks = main_cpu_ticks = 0
        tot_ctx = main_ctx = 0
        procs = []
        for pid, s in mem.items():
            if s["state"] in ("Z", "X"):
                continue
            key = (pid, s["start"])
            self.known[key] = s["comm"]
            pss, src = read_pss_kb(pid)
            if pss is None:
                pss = s["rss_kb"]
                self.pss_source = "rss-fallback"
            ticks = s["utime"] + s["stime"]
            prev = self.prev_cpu.get(key)
            if prev is None:
                d_ticks = 0 if first else ticks  # a process born since the last tick
            else:
                d_ticks = max(0, ticks - prev)
            self.prev_cpu[key] = ticks
            d_ctx = 0
            if self.ctxsw:
                for tid, c in read_ctxsw(pid).items():
                    k = (pid, s["start"], tid)
                    p = self.prev_ctx.get(k)
                    if p is None:
                        d_ctx += 0 if first else c  # thread born since the last tick
                    else:
                        d_ctx += max(0, c - p)
                    self.prev_ctx[k] = c
            is_main = pid == self.root
            tot_pss += pss
            tot_cpu_ticks += d_ticks
            tot_ctx += d_ctx
            if is_main:
                main_pss, main_cpu_ticks, main_ctx = pss, d_ticks, d_ctx
            procs.append((pid, s["comm"], pss))
        self.prev_t = t
        if first or dt <= 0:
            cpu = main_cpu = None
        else:
            cpu = tot_cpu_ticks / CLK_TCK / (dt / 1000.0) * 100.0
            main_cpu = main_cpu_ticks / CLK_TCK / (dt / 1000.0) * 100.0
        return {"t": round(t, 3), "dt": round(dt, 3), "nproc": len(procs),
                "pss_kb": tot_pss, "main_pss_kb": main_pss,
                "cpu_pct": None if cpu is None else round(cpu, 3),
                "main_cpu_pct": None if main_cpu is None else round(main_cpu, 3),
                "ctxsw": tot_ctx if not first else None, "main_ctxsw": main_ctx if not first else None,
                "procs": [[p, c, k] for p, c, k in procs]}

    def alive_members(self):
        res = []
        for pid in all_pids():
            s = read_stat(pid)
            if not s or s["state"] in ("Z", "X"):
                continue
            if (pid, s["start"]) in self.known or s["sid"] == self.root or s["pgrp"] == self.root:
                res.append((pid, s["comm"]))
        return res

    def gpu_usage(self):
        """Which GPU paths does the tree use right now? Looks at open DRM/NVIDIA device nodes and
        mapped GL/EGL/Vulkan driver libraries in each member."""
        res = {}
        lib_pat = re.compile(r"(libEGL_nvidia|libGLX_nvidia|libnvidia-eglcore|libnvidia-glcore|libnvidia-gpucomp|"
                             r"libEGL_mesa|libGLX_mesa|libgallium[^/\s]*|iris_dri|crocus_dri|radeonsi_dri|swrast_dri|"
                             r"kms_swrast_dri|libvulkan_intel|libvulkan_radeon|libvulkan_lvp|libvulkan\.so|"
                             r"libwebkit2gtk[^/\s]*|libwebkitgtk[^/\s]*)")
        for pid, s in self.members().items():
            libs, devs = set(), set()
            maps = read_file("/proc/%d/maps" % pid, "") or ""
            for m in lib_pat.finditer(maps):
                libs.add(m.group(1))
            try:
                for fd in os.listdir("/proc/%d/fd" % pid):
                    try:
                        target = os.readlink("/proc/%d/fd/%s" % (pid, fd))
                    except OSError:
                        continue
                    if target.startswith("/dev/dri/") or target.startswith("/dev/nvidia"):
                        devs.add(target)
            except OSError:
                pass
            if libs or devs:
                res["%d:%s" % (pid, s["comm"])] = {"libs": sorted(libs), "devices": sorted(devs)}
        return res


# --------------------------------------------------------------------------------------
# Statistics
# --------------------------------------------------------------------------------------

def percentile(vals, p):
    if not vals:
        return None
    s = sorted(vals)
    if len(s) == 1:
        return s[0]
    k = (len(s) - 1) * p / 100.0
    lo = math.floor(k)
    hi = min(lo + 1, len(s) - 1)
    return s[lo] + (s[hi] - s[lo]) * (k - lo)


def mean(vals):
    vals = [v for v in vals if v is not None]
    return sum(vals) / len(vals) if vals else None


def median(vals):
    vals = [v for v in vals if v is not None]
    return statistics.median(vals) if vals else None


def rnd(v, n=1):
    return None if v is None else round(v, n)


def parse_events(path):
    """Complete JSONL lines only (a half-written last line is ignored)."""
    evs = []
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            data = f.read()
    except OSError:
        return evs
    lines = data.split("\n")
    for line in lines[:-1] if not data.endswith("\n") else lines:
        line = line.strip()
        if not line:
            continue
        try:
            obj = json.loads(line)
            if isinstance(obj, dict):
                evs.append(obj)
        except ValueError:
            evs.append({"ev": "_bad_line", "raw": line[:200]})
    return evs


def phase_windows(events, spawn_ms):
    """[(name, start_ms, end_ms)] including a synthetic 'startup' window."""
    marks = [(e["name"], float(e["t"])) for e in events if e.get("ev") == "phase" and "t" in e and "name" in e]
    done = next((float(e["t"]) for e in events if e.get("ev") == "done" and "t" in e), None)
    wins = []
    first_t = marks[0][1] if marks else done
    if first_t is not None:
        wins.append(("startup", spawn_ms, first_t))
    for i, (name, t) in enumerate(marks):
        end = marks[i + 1][1] if i + 1 < len(marks) else done
        if end is None:
            continue
        wins.append((name, t, end))
    return wins


def frame_stats(ts, refresh_hz, window_ms=None):
    """Producer-side frame statistics from rAF/render timestamps (ms, relative to phase start)."""
    ts = sorted(float(x) for x in ts)
    n = len(ts)
    res = {"frames": n}
    if n < 2:
        res.update({"fps": None, "p50_ms": None, "p95_ms": None, "p99_ms": None, "max_ms": None,
                    "jank_pct": None, "refresh_coverage_pct": 0.0 if window_ms else None})
        return res, []
    iv = [b - a for a, b in zip(ts, ts[1:])]
    span = ts[-1] - ts[0]
    refresh_ms = 1000.0 / refresh_hz
    jank = sum(1 for x in iv if x > 1.5 * refresh_ms)
    res.update({
        "fps": rnd(1000.0 * (n - 1) / span, 2) if span > 0 else None,
        "p50_ms": rnd(percentile(iv, 50), 2), "p95_ms": rnd(percentile(iv, 95), 2),
        "p99_ms": rnd(percentile(iv, 99), 2), "max_ms": rnd(max(iv), 2),
        "jank_pct": rnd(100.0 * jank / len(iv), 2),
        # share of the display's refreshes in the phase that got a new frame (capped at 100)
        "refresh_coverage_pct": rnd(min(100.0, 100.0 * n / (window_ms / refresh_ms)), 2) if window_ms else None,
    })
    return res, iv


def summarize_run(run, refresh_hz, settle_ms):
    ev = run["events"]
    spawn = run["spawn_ms"]
    s = {}
    ff = next((e for e in ev if e.get("ev") == "first_frame"), None)
    st = next((e for e in ev if e.get("ev") == "start"), None)
    s["first_frame_ms"] = rnd(float(ff["t"]) - spawn, 1) if ff and "t" in ff else None
    s["start_event_ms"] = rnd(float(st["t"]) - spawn, 1) if st and "t" in st else None
    wins = phase_windows(ev, spawn)
    phases = {}
    for name, a, b in wins:
        settle = min(settle_ms, 0.1 * (b - a)) if name != "startup" else 0.0
        samp = [x for x in run["samples"] if x.get("dt") and a + settle <= x["t"] - x["dt"] / 2.0 < b]
        mem = [x["pss_kb"] / 1024.0 for x in samp]
        main_mem = [x["main_pss_kb"] / 1024.0 for x in samp]
        cpu = [x["cpu_pct"] for x in samp if x.get("cpu_pct") is not None]
        main_cpu = [x["main_cpu_pct"] for x in samp if x.get("main_cpu_pct") is not None]
        dur_s = sum(x["dt"] for x in samp) / 1000.0
        ctx = sum(x["ctxsw"] or 0 for x in samp)
        main_ctx = sum(x["main_ctxsw"] or 0 for x in samp)
        phases[name] = {
            "window_ms": rnd(b - a, 1), "samples": len(samp),
            "mem_median_mb": rnd(median(mem)), "mem_max_mb": rnd(max(mem) if mem else None),
            "main_mem_median_mb": rnd(median(main_mem)),
            "cpu_mean_pct": rnd(mean(cpu), 2), "main_cpu_mean_pct": rnd(mean(main_cpu), 2),
            "ctxsw_per_s": rnd(ctx / dur_s, 1) if dur_s > 0 else None,
            "main_ctxsw_per_s": rnd(main_ctx / dur_s, 1) if dur_s > 0 else None,
            "nproc_max": max((x["nproc"] for x in samp), default=None),
        }
    frames = {}
    intervals = {}
    for e in ev:
        if e.get("ev") == "frames" and isinstance(e.get("ts"), list):
            w = next(((b - a) for n, a, b in wins if n == e.get("phase")), None)
            fs, iv = frame_stats(e["ts"], refresh_hz, w)
            frames[e.get("phase")] = fs
            intervals[e.get("phase")] = iv
    s["phases"] = phases
    s["frames"] = frames
    # app-specific extras (e.g. raf_count/visibility at the end of `minimized`, UA, GPU status);
    # unknown event types are otherwise ignored
    s["info"] = [e for e in ev if e.get("ev") == "info"]
    return s, intervals


def check_run(run):
    names = [e.get("ev") for e in run["events"]]
    missing = [r for r in REQUIRED_EVENTS if r not in names]
    seen_phases = [e.get("name") for e in run["events"] if e.get("ev") == "phase"]
    missing += ["phase:" + p for p in PHASES if p not in seen_phases]
    seen_frames = [e.get("phase") for e in run["events"] if e.get("ev") == "frames"]
    missing += ["frames:" + p for p in FRAME_PHASES if p not in seen_frames]
    if "_bad_line" in names:
        missing.append("unparseable JSONL lines")
    return missing


def aggregate_app(runs, refresh_hz, settle_ms):
    ok_runs = [r for r in runs if r.get("ok")]
    per_run = []
    pooled = {}
    for r in ok_runs:
        s, iv = summarize_run(r, refresh_hz, settle_ms)
        per_run.append(s)
        for ph, vals in iv.items():
            pooled.setdefault(ph, []).extend(vals)
    agg = {"runs_ok": len(ok_runs), "runs_total": len(runs)}
    starts = [s["first_frame_ms"] for s in per_run]
    agg["start"] = {
        "first_run_ms": starts[0] if starts else None,
        "median_ms": rnd(median(starts)),
        "warm_median_ms": rnd(median(starts[1:])) if len(starts) > 1 else None,
        "min_ms": rnd(min(starts)) if starts else None, "max_ms": rnd(max(starts)) if starts else None,
        "start_event_median_ms": rnd(median([s["start_event_ms"] for s in per_run])),
        "all_ms": starts,
    }
    phases = {}
    names = []
    for s in per_run:
        for n in s["phases"]:
            if n not in names:
                names.append(n)
    for n in names:
        vals = [s["phases"][n] for s in per_run if n in s["phases"]]
        phases[n] = {
            "mem_median_mb": rnd(median([v["mem_median_mb"] for v in vals])),
            "mem_max_mb": rnd(max([v["mem_max_mb"] for v in vals if v["mem_max_mb"] is not None], default=None)),
            "main_mem_median_mb": rnd(median([v["main_mem_median_mb"] for v in vals])),
            "cpu_mean_pct": rnd(median([v["cpu_mean_pct"] for v in vals]), 2),
            "main_cpu_mean_pct": rnd(median([v["main_cpu_mean_pct"] for v in vals]), 2),
            "ctxsw_per_s": rnd(median([v["ctxsw_per_s"] for v in vals]), 1),
            "main_ctxsw_per_s": rnd(median([v["main_ctxsw_per_s"] for v in vals]), 1),
            "nproc_max": max([v["nproc_max"] for v in vals if v["nproc_max"] is not None], default=None),
        }
    agg["phases"] = phases
    frames = {}
    refresh_ms = 1000.0 / refresh_hz
    for ph, iv in pooled.items():
        vals = [s["frames"][ph] for s in per_run if ph in s["frames"]]
        jank = sum(1 for x in iv if x > 1.5 * refresh_ms)
        frames[ph] = {
            "frames_median": median([v["frames"] for v in vals]),
            "fps_median": rnd(median([v["fps"] for v in vals]), 2),
            "p50_ms": rnd(percentile(iv, 50), 2), "p95_ms": rnd(percentile(iv, 95), 2),
            "p99_ms": rnd(percentile(iv, 99), 2), "max_ms": rnd(max(iv), 2) if iv else None,
            "jank_pct": rnd(100.0 * jank / len(iv), 2) if iv else None,
            "refresh_coverage_pct": rnd(median([v["refresh_coverage_pct"] for v in vals]), 2),
        }
    agg["frames"] = frames
    agg["per_run"] = per_run
    agg["targets"] = evaluate_targets(agg, refresh_hz)
    return agg


def evaluate_targets(agg, refresh_hz):
    t = {}
    refresh_ms = 1000.0 / refresh_hz

    def verdict(val, ok):
        return {"value": val, "pass": None if val is None else bool(ok)}

    for ph in FRAME_PHASES:
        f = agg["frames"].get(ph, {})
        cov = f.get("refresh_coverage_pct")
        t["%s refreshes presented >= 99%%" % ph] = verdict(cov, cov is not None and cov >= TARGETS["refresh_coverage_pct_min"])
        p99 = f.get("p99_ms")
        t["%s p99 <= 1.5x refresh (%.2f ms)" % (ph, 1.5 * refresh_ms)] = verdict(
            p99, p99 is not None and p99 <= TARGETS["p99_vs_refresh_max"] * refresh_ms)
    st = agg["start"]
    t["cold start (first run) <= 1000 ms"] = verdict(st["first_run_ms"], st["first_run_ms"] is not None and st["first_run_ms"] <= TARGETS["cold_start_ms_max"])
    t["warm start (median runs 2..N) <= 400 ms"] = verdict(st["warm_median_ms"], st["warm_median_ms"] is not None and st["warm_median_ms"] <= TARGETS["warm_start_ms_max"])
    idle = agg["phases"].get("idle", {})
    t["idle CPU <= 0.5% (tree)"] = verdict(idle.get("cpu_mean_pct"), (idle.get("cpu_mean_pct") or 0) <= TARGETS["idle_cpu_pct_max"])
    d = agg["phases"].get("destroyed", {})
    t["destroyed CPU ~0 (main <= 0.2%)"] = verdict(d.get("main_cpu_mean_pct"), (d.get("main_cpu_mean_pct") or 0) <= TARGETS["destroyed_main_cpu_pct_max"])
    t["destroyed memory <= 20 MB (main)"] = verdict(d.get("main_mem_median_mb"), (d.get("main_mem_median_mb") or 0) <= TARGETS["destroyed_main_mb_max"])
    return t


# --------------------------------------------------------------------------------------
# Markdown
# --------------------------------------------------------------------------------------

def fmt(v, suffix=""):
    if v is None:
        return "–"
    if isinstance(v, float):
        return ("%.2f" % v).rstrip("0").rstrip(".") + suffix if abs(v) < 100 else "%.0f%s" % (v, suffix)
    return "%s%s" % (v, suffix)


def md_table(header, rows):
    out = ["| " + " | ".join(header) + " |", "|" + "|".join("---" for _ in header) + "|"]
    for r in rows:
        out.append("| " + " | ".join(fmt(c) if not isinstance(c, str) else c for c in r) + " |")
    return "\n".join(out)


def markdown_summary(result):
    env = result["environment"]
    lines = ["# Bake-off results: Linux (%s)" % env["hostname"], ""]
    mode = "nested headless weston (not real presentation)" if env["nested"] else "real session"
    lines.append("- Mode: **%s**" % mode)
    lines.append("- Refresh: %.2f Hz (%s); phase %d ms; %d run(s) per app; sampling every %d ms" % (
        env["refresh"]["hz"], env["refresh"]["source"], result["config"]["phase_ms"], result["config"]["runs"],
        result["config"]["sample_ms"]))
    lines.append("- %s, kernel %s, %s, %s" % (env["os"], env["kernel"], env["session_type"], env["cpu"]))
    gpus = "; ".join("%s [%s]" % (g["device"], g.get("driver", "?")) for g in env["gpu"].get("lspci", []))
    lines.append("- GPU: %s%s" % (gpus or "?", (" ; NVIDIA driver " + env["gpu"]["nvidia_driver"]) if env["gpu"].get("nvidia_driver") else ""))
    if env.get("session_locked") and not env["nested"]:
        lines.append("- **WARNING: the graphical session was LOCKED; real-session frame numbers are not representative.**")
    for w in result.get("warnings", []):
        lines.append("- Warning: %s" % w)
    lines.append("")
    apps = list(result["summary"].keys())

    lines.append("## Startup (spawn -> first_frame)")
    lines.append("")
    rows = []
    for a in apps:
        s = result["summary"][a]
        st = s.get("start", {})
        rows.append([a, "%d/%d" % (s["runs_ok"], s["runs_total"]), st.get("first_run_ms"), st.get("warm_median_ms"),
                     st.get("median_ms"), st.get("start_event_median_ms")])
    lines.append(md_table(["app", "runs ok", "first run ms", "warm median ms", "median ms", "host start ev ms"], rows))
    lines.append("")

    lines.append("## Memory and CPU per phase (medians across runs)")
    lines.append("")
    lines.append("mem = PSS summed over the process tree; main = host process only; CPU = % of one core.")
    lines.append("")
    rows = []
    for a in apps:
        ph = result["summary"][a].get("phases", {})
        for n in ["startup"] + PHASES:
            if n in ph:
                p = ph[n]
                rows.append([a, n, p["mem_median_mb"], p["mem_max_mb"], p["main_mem_median_mb"], p["cpu_mean_pct"],
                             p["main_cpu_mean_pct"], p["ctxsw_per_s"], p["nproc_max"]])
    lines.append(md_table(["app", "phase", "mem med MB", "mem max MB", "main MB", "CPU %", "main CPU %",
                           "ctxsw/s", "procs"], rows))
    lines.append("")

    lines.append("## Frames (producer timestamps, pooled across runs)")
    lines.append("")
    rows = []
    for a in apps:
        fr = result["summary"][a].get("frames", {})
        for n in PHASES:
            if n in fr:
                f = fr[n]
                rows.append([a, n, f["frames_median"], f["fps_median"], f["p50_ms"], f["p95_ms"], f["p99_ms"],
                             f["max_ms"], f["jank_pct"], f["refresh_coverage_pct"]])
    lines.append(md_table(["app", "phase", "frames", "fps", "p50 ms", "p95 ms", "p99 ms", "max ms", "jank %",
                           "refreshes hit %"], rows))
    lines.append("")
    lines.append("jank %% = share of frame intervals > 1.5x the refresh interval (%.2f ms)." % (1.5 * 1000.0 / env["refresh"]["hz"]))
    lines.append("")

    lines.append("## Targets")
    lines.append("")
    tnames = []
    for a in apps:
        for k in result["summary"][a].get("targets", {}):
            if k not in tnames:
                tnames.append(k)
    rows = []
    for k in tnames:
        row = [k]
        for a in apps:
            v = result["summary"][a].get("targets", {}).get(k)
            if not v or v["pass"] is None:
                row.append("–")
            else:
                row.append("%s %s" % ("PASS" if v["pass"] else "FAIL", fmt(v["value"])))
        rows.append(row)
    lines.append(md_table(["target"] + apps, rows))
    lines.append("")

    extra = []
    for a in apps:
        for s in result["summary"][a].get("per_run", []):
            for e in s.get("info", []):
                keep = {k: v for k, v in e.items() if k not in ("ev", "t") and isinstance(v, (int, float, str, bool))
                        and (not isinstance(v, str) or len(v) < 160)}
                if keep:
                    extra.append("- %s: %s" % (a, ", ".join("%s=%s" % kv for kv in keep.items())))
            break
    if extra:
        lines.append("## App info events (first ok run)")
        lines.append("")
        lines.extend(extra)
        lines.append("")

    gpu_lines = []
    for a in apps:
        for i, r in enumerate(result["runs"].get(a, [])):
            if r.get("gpu_usage"):
                gv = r.get("gpu_verdict") or {}
                head = "drivers opened: **%s**" % (", ".join(gv.get("gpu_drivers_opened", [])) or "none")
                if gv.get("webkit_nvidia_egl") is not None:
                    head += "; WebKitGTK NVIDIA EGL path: **%s**" % ("yes" if gv["webkit_nvidia_egl"] else "no")
                gpu_lines.append("- %s (run %d): %s" % (a, i + 1, head))
                for k, v in r["gpu_usage"].items():
                    gpu_lines.append("  - `%s`: %s" % (k, ", ".join(v["devices"] + v["libs"])))
                break
    if gpu_lines:
        lines.append("## GPU path per app (first run, sampled during transitions)")
        lines.append("")
        lines.extend(gpu_lines)
        lines.append("")

    failed = []
    for a in apps:
        for i, r in enumerate(result["runs"].get(a, [])):
            if not r.get("ok"):
                failed.append("- %s run %d: %s" % (a, i + 1, r.get("error") or ", ".join(r.get("missing", []))))
    if failed:
        lines.append("## Failed runs")
        lines.append("")
        lines.extend(failed)
        lines.append("")
    return "\n".join(lines)


# --------------------------------------------------------------------------------------
# Running one app
# --------------------------------------------------------------------------------------

def gpu_verdict(usage, render_nodes):
    """Condense gpu_usage into: which GPU drivers the tree opened, and (for WebKitGTK) whether the
    web process went down the NVIDIA EGL path."""
    if not usage:
        return None
    drivers = set()
    nvidia_libs = False
    webkit = None
    for key, v in usage.items():
        devs = v.get("devices", [])
        libs = v.get("libs", [])
        for d in devs:
            if d.startswith("/dev/nvidia"):
                drivers.add("nvidia")
            elif d.startswith("/dev/dri/renderD"):
                drivers.add(render_nodes.get(os.path.basename(d), {}).get("driver", os.path.basename(d)))
        nv = any(l.startswith(("libEGL_nvidia", "libnvidia-eglcore", "libnvidia-glcore", "libGLX_nvidia")) for l in libs)
        nvidia_libs = nvidia_libs or nv
        if "WebKitWebProces" in key:
            webkit = bool(nv and any(d.startswith("/dev/nvidia") for d in devs))
    return {"gpu_drivers_opened": sorted(drivers), "nvidia_gl_libs_loaded": nvidia_libs,
            "webkit_nvidia_egl": webkit}


def kill_tree(sampler, root_pid, grace_s=2.0):
    leftovers = sampler.alive_members()
    if not leftovers:
        return []
    for sig in (signal.SIGTERM, signal.SIGKILL):
        try:
            os.killpg(root_pid, sig)
        except OSError:
            pass
        for pid, _ in sampler.alive_members():
            try:
                os.kill(pid, sig)
            except OSError:
                pass
        deadline = time.time() + grace_s
        while time.time() < deadline and sampler.alive_members():
            time.sleep(0.1)
        if not sampler.alive_members():
            break
    return ["%d:%s" % x for x in leftovers]


def run_once(app, cmd, env, args, run_idx, total_runs):
    tmpdir = tempfile.mkdtemp(prefix="bakeoff-%s-" % app)
    out_path = os.path.join(tmpdir, "report.jsonl")
    open(out_path, "w").close()
    stdio_path = os.path.join(tmpdir, "stdio.log")
    env = dict(env)
    env["BAKEOFF_OUT"] = out_path
    env["BAKEOFF_PHASE_MS"] = str(args.phase_ms)
    timeout_s = args.startup_timeout + 5 * args.phase_ms / 1000.0 + 30.0
    tag = "[%s %d/%d]" % (app, run_idx + 1, total_runs)
    log("%s launching: %s" % (tag, " ".join(cmd)))
    stdio = open(stdio_path, "wb")
    spawn_ms = time.time_ns() / 1e6
    try:
        proc = subprocess.Popen(cmd, env=env, stdout=stdio, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
                                start_new_session=True)
    except OSError as e:
        stdio.close()
        return {"ok": False, "error": "spawn failed: %s" % e, "spawn_ms": spawn_ms, "events": [], "samples": []}
    _started_procs.append((proc, app))
    sampler = TreeSampler(proc.pid, ctxsw=not args.no_ctxsw)
    samples = []
    events = []
    seen = set()
    status = "running"
    done_at = None
    gpu_usage = None
    tick = args.sample_ms / 1000.0
    next_t = time.time()
    t_start = time.time()
    while True:
        samples.append(sampler.sample())
        events = parse_events(out_path)
        for e in events:
            key = (e.get("ev"), e.get("name") or e.get("phase"))
            if key not in seen:
                seen.add(key)
                if e.get("ev") in ("first_frame", "phase", "done"):
                    extra = ""
                    if e.get("ev") == "first_frame":
                        extra = " (+%.0f ms)" % (float(e.get("t", 0)) - spawn_ms)
                    log("%s %s%s%s" % (tag, e.get("ev"), (" " + e["name"]) if e.get("name") else "", extra))
                if e.get("ev") == "done":
                    done_at = time.time()
        # GPU path probe ~1 s into transitions
        if gpu_usage is None:
            tr = next((e for e in events if e.get("ev") == "phase" and e.get("name") == "transitions"), None)
            if tr and time.time() * 1000.0 - float(tr["t"]) > min(1000.0, args.phase_ms / 2.0):
                gpu_usage = sampler.gpu_usage()
                gv = gpu_verdict(gpu_usage, args.render_nodes)
                if gv:
                    log("%s GPU: drivers opened=%s%s" % (tag, ",".join(gv["gpu_drivers_opened"]) or "none", "" if gv["webkit_nvidia_egl"] is None else "; WebKitGTK NVIDIA EGL path: %s" % ("YES" if gv["webkit_nvidia_egl"] else "no")))
        rc = proc.poll()
        now = time.time()
        if done_at is not None and (rc is not None or now - done_at > 5.0):
            status = "done"
            break
        if rc is not None and done_at is None:
            # host died; give a moment for a final flush then stop
            time.sleep(0.3)
            events = parse_events(out_path)
            if any(e.get("ev") == "done" for e in events):
                status = "done"
            else:
                status = "exited early (code %s)" % rc
            break
        if not any(e.get("ev") == "first_frame" for e in events) and now - t_start > args.startup_timeout:
            status = "no first_frame within %ds" % args.startup_timeout
            break
        if now - t_start > timeout_s:
            status = "timeout after %.0fs" % timeout_s
            break
        next_t += tick
        delay = next_t - time.time()
        if delay > 0:
            time.sleep(delay)
        else:
            next_t = time.time()
    exit_code = proc.poll()
    leftovers = kill_tree(sampler, proc.pid)
    try:
        proc.wait(5)
    except subprocess.TimeoutExpired:
        pass
    stdio.close()
    events = parse_events(out_path)
    stdio_tail = (read_file(stdio_path, "") or "").splitlines()[-30:]
    run = {"spawn_ms": round(spawn_ms, 3), "events": events, "samples": samples, "status": status,
           "exit_code": exit_code, "leftovers_killed": leftovers, "pss_source": sampler.pss_source,
           "gpu_usage": gpu_usage, "gpu_verdict": gpu_verdict(gpu_usage, args.render_nodes),
           "stdio_tail": stdio_tail, "cmd": cmd}
    missing = check_run(run)
    run["missing"] = missing
    run["ok"] = status == "done" and not missing
    if status != "done":
        run["error"] = status
    if leftovers:
        log("%s killed leftovers: %s" % (tag, ", ".join(leftovers)))
    if not run["ok"]:
        log("%s FAILED: %s %s" % (tag, status, ("missing " + ", ".join(missing)) if missing else ""))
        if stdio_tail:
            log("%s last output:\n    %s" % (tag, "\n    ".join(stdio_tail[-10:])))
    else:
        log("%s ok" % tag)
    shutil.rmtree(tmpdir, ignore_errors=True)
    return run


def cleanup_all(include_weston=True):
    for p, label in reversed(_started_procs):
        if label == "weston" and not include_weston:
            continue
        if p.poll() is None:
            try:
                os.killpg(p.pid, signal.SIGKILL)
            except OSError:
                pass


def main():
    ap = argparse.ArgumentParser(description="Speedrunluncher bake-off Linux harness (see bakeoff/README.md)")
    ap.add_argument("--apps", default="tauri,electron,gpui", help="comma list: tauri,electron,gpui,mock")
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--phase-ms", type=int, default=20000)
    ap.add_argument("--refresh-hz", default="auto", help="auto (GNOME DisplayConfig -> xrandr -> 60) or a number")
    ap.add_argument("--nested", action="store_true", help="run inside a headless weston (socket bakeoff-h)")
    ap.add_argument("--sample-ms", type=int, default=200)
    ap.add_argument("--settle-ms", type=int, default=1000,
                    help="skip this much (max 10%% of the phase) at each phase start for mem/CPU stats")
    ap.add_argument("--startup-timeout", type=int, default=60, help="seconds to wait for first_frame")
    ap.add_argument("--no-ctxsw", action="store_true", help="skip per-thread context-switch sampling")
    ap.add_argument("--out-dir", default=os.path.join(BAKEOFF, "results"))
    ap.add_argument("--pause-s", type=float, default=3.0, help="idle pause between runs")
    ap.add_argument("--electron-no-sandbox", action="store_true",
                    help="pass --no-sandbox to Electron (only if you can't fix chrome-sandbox permissions)")
    ap.add_argument("--env", action="append", default=[], metavar="KEY=VAL", help="extra env for the apps (repeatable)")
    for a in ("tauri", "electron", "gpui"):
        ap.add_argument("--%s-bin" % a, default=DEFAULT_BINS[a])
    args = ap.parse_args()

    apps = [a.strip() for a in args.apps.split(",") if a.strip()]
    for a in apps:
        if a not in DEFAULT_BINS:
            ap.error("unknown app %r" % a)
    bins = {a: (getattr(args, "%s_bin" % a) if a != "mock" else DEFAULT_BINS["mock"]) for a in apps}
    missing_bins = [a for a in apps if not os.path.isfile(bins[a])]
    if missing_bins:
        for a in missing_bins:
            log("ERROR: %s binary not found at %s (build it first, see bakeoff/README.md, or pass --%s-bin)" % (a, bins[a], a))
        return 1

    signal.signal(signal.SIGTERM, lambda *_: (cleanup_all(), sys.exit(130)))

    warnings = []
    # make sure apps can find the real session when launched from a plain terminal/ssh
    base_env = dict(os.environ)
    # Inherited from Electron-based terminals/IDEs (VS Code, T3 Code...): would turn the Electron
    # app into plain Node and make it exit immediately.
    for k in [k for k in base_env if k.startswith("ELECTRON_")]:
        base_env.pop(k)
    if not args.nested and not base_env.get("WAYLAND_DISPLAY") and not base_env.get("DISPLAY"):
        rt = base_env.get("XDG_RUNTIME_DIR", "/run/user/%d" % os.getuid())
        if os.path.exists(os.path.join(rt, "wayland-0")):
            base_env["WAYLAND_DISPLAY"] = "wayland-0"
            base_env["XDG_RUNTIME_DIR"] = rt
            warnings.append("no WAYLAND_DISPLAY/DISPLAY in the environment; using %s/wayland-0" % rt)
        else:
            warnings.append("no display environment found; apps will probably fail (try --nested)")
    for kv in args.env:
        k, _, v = kv.partition("=")
        base_env[k] = v

    real_refresh = detect_refresh("auto")
    if args.nested:
        hz = float(args.refresh_hz) if args.refresh_hz != "auto" else (real_refresh["hz"] if real_refresh["source"] != "fallback" else 60.0)
        refresh = {"source": "nested weston --refresh-rate", "hz": hz, "real_display": real_refresh}
    else:
        refresh = detect_refresh(args.refresh_hz)
    sessions = graphical_sessions()
    ss = screensaver_active()
    locked = any(s["locked"] for s in sessions) or bool(ss)

    env_info = {
        "platform": "linux", "hostname": socket.gethostname(), "os": os_release(), "kernel": platform.release(),
        "cpu": cpu_model(), "cpu_count": os.cpu_count(), "mem_total_mb": mem_total_mb(),
        "python": platform.python_version(),
        "session_type": os.environ.get("XDG_SESSION_TYPE", "?"), "desktop": os.environ.get("XDG_CURRENT_DESKTOP", "?"),
        "wayland_display": base_env.get("WAYLAND_DISPLAY"), "x_display": base_env.get("DISPLAY"),
        "sessions": sessions, "screensaver_active": ss, "session_locked": locked,
        "refresh": refresh, "nested": args.nested,
        "presentation": "nested (not real presentation)" if args.nested else "real session",
        "gpu": detect_gpus(), "egl": egl_hints(), "power": power_hints(),
        "harness": "bakeoff/harness/linux/run.py",
    }

    log("Speedrunluncher bake-off (Linux) | apps=%s runs=%d phase=%d ms | refresh %.2f Hz (%s) | %s" % (
        ",".join(apps), args.runs, args.phase_ms, refresh["hz"], refresh["source"], env_info["presentation"]))
    if env_info["egl"]["nvidia_egl_vendor_installed"]:
        log("EGL vendors: %s (runtime GPU path per app is recorded under gpu_usage)" % ", ".join(env_info["egl"]["egl_vendor_files"]))
    if env_info["power"].get("on_ac") is False:
        warnings.append("running on battery; plug in the laptop for comparable numbers")
    if locked and not args.nested:
        warn_box(["THE GRAPHICAL SESSION IS LOCKED (LockedHint=yes / screensaver active).",
                  "A locked GNOME session does not present app windows, so frame numbers",
                  "and GPU work will be garbage. Unlock the screen and keep it awake, or",
                  "re-run with --nested (headless weston, not real presentation)."])
        warnings.append("session was locked during a real-session run: frame data not representative")
    elif locked:
        log("note: the real session is locked; that's fine with --nested.")

    args.render_nodes = env_info["gpu"].get("render_nodes", {})
    weston = None
    result = {"config": {"apps": apps, "runs": args.runs, "phase_ms": args.phase_ms, "sample_ms": args.sample_ms,
                         "settle_ms": args.settle_ms, "bins": bins, "extra_env": args.env},
              "environment": env_info, "warnings": warnings, "runs": {}, "summary": {}}
    try:
        if args.nested:
            weston = NestedWeston(refresh["hz"])
            weston.start()
            env_info["nested_weston"] = {"socket": weston.socket, "renderer": weston.renderer,
                                         "xwayland_display": weston.xdisplay,
                                         "refresh_rate_flag": weston.refresh_supported}
            if not weston.refresh_supported:
                refresh["hz"] = 60.0
                refresh["source"] = "nested weston default (no --refresh-rate support)"
                warnings.append("nested weston ignores the requested refresh rate; jank is judged against 60 Hz")
            base_env = weston.env(base_env)
        for a in apps:
            cmd = [bins[a]]
            if a == "mock":
                cmd = [sys.executable, bins[a]]
            if a == "electron":
                fix = electron_sandbox_problem(bins[a])
                if fix and args.electron_no_sandbox:
                    cmd.append("--no-sandbox")
                    warnings.append("electron launched with --no-sandbox (requested via --electron-no-sandbox)")
                elif fix:
                    warn_box(["Electron's chrome-sandbox is not root:root with mode 4755, so Electron",
                              "would abort at startup. Fix it once (needs sudo):",
                              "  " + fix,
                              "or re-run with --electron-no-sandbox (not representative). Skipping electron."])
                    warnings.append("electron skipped: chrome-sandbox not root:root 4755 (fix: %s)" % fix)
                    result["runs"][a] = [{"ok": False, "error": "chrome-sandbox not root:root 4755; fix: " + fix,
                                          "events": [], "samples": [], "spawn_ms": 0}]
                    result["summary"][a] = aggregate_app(result["runs"][a], refresh["hz"], args.settle_ms)
                    continue
            app_env = base_env
            if weston and a == "gpui":
                # GPUI's Wayland client unwraps a wl_seat, and headless weston has none: use its Xwayland.
                app_env = dict(base_env)
                app_env.pop("WAYLAND_DISPLAY", None)
                app_env["XDG_SESSION_TYPE"] = "x11"
                if weston.xdisplay:
                    app_env["DISPLAY"] = weston.xdisplay
                warnings.append("gpui ran through nested Xwayland %s (GPUI's Wayland backend needs a wl_seat, "
                                "which headless weston lacks)" % weston.xdisplay)
            result["runs"][a] = []
            for i in range(args.runs):
                run = run_once(a, cmd, app_env, args, i, args.runs)
                result["runs"][a].append(run)
                if args.pause_s > 0:
                    time.sleep(args.pause_s)
            result["summary"][a] = aggregate_app(result["runs"][a], refresh["hz"], args.settle_ms)
    except KeyboardInterrupt:
        log("\ninterrupted; writing partial results")
        result["warnings"].append("interrupted by user")
        for a in result["runs"]:
            if a not in result["summary"]:
                result["summary"][a] = aggregate_app(result["runs"][a], refresh["hz"], args.settle_ms)
    finally:
        cleanup_all(include_weston=False)
        if weston:
            env_info.setdefault("nested_weston", {})["log_tail"] = weston.log_tail()
            weston.stop()

    os.makedirs(args.out_dir, exist_ok=True)
    stamp = time.strftime("%Y%m%d-%H%M%S")
    host = re.sub(r"[^A-Za-z0-9_.-]", "_", socket.gethostname())
    stem = os.path.join(args.out_dir, "linux-%s-%s%s" % (host, "nested-" if args.nested else "", stamp))
    md = markdown_summary(result)
    with open(stem + ".json", "w", encoding="utf-8") as f:
        json.dump(result, f, indent=1)
    with open(stem + ".md", "w", encoding="utf-8") as f:
        f.write(md + "\n")
    log("")
    log(md)
    log("")
    log("Results: %s.json" % stem)
    log("Summary: %s.md" % stem)
    all_ok = all(r.get("ok") for runs in result["runs"].values() for r in runs) and all(
        len(result["runs"].get(a, [])) == args.runs for a in apps)
    return 0 if all_ok else 2


if __name__ == "__main__":
    try:
        sys.exit(main())
    finally:
        cleanup_all()
