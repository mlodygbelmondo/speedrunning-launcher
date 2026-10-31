<#
.SYNOPSIS
  Speedrunluncher bake-off: Windows harness (Windows PowerShell 5.1 compatible).

.DESCRIPTION
  Runs the three test apps (tauri, electron, gpui) through the scripted scenario described in
  bakeoff/SPEC.md, samples each app's whole process tree every 250 ms (private bytes, working set,
  CPU, context switches), captures presentation truth with PresentMon (needs admin), and writes
  results-windows-<computer>-<timestamp>.json + .md next to this script.

  Easiest: double-click run-bakeoff.cmd, accept the admin prompt, keep hands off for ~20 minutes,
  then send back the results file it names at the end.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File run-bakeoff.ps1
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File run-bakeoff.ps1 -Apps tauri -Runs 1 -PhaseMs 5000 -NoPresentMon
#>
[CmdletBinding()]
param(
    [string]$Apps = 'tauri,electron,gpui',
    [int]$Runs = 3,
    [int]$PhaseMs = 20000,
    [string]$RefreshHz = 'auto',
    [switch]$NoPresentMon,
    [int]$SampleMs = 250,
    [int]$SettleMs = 1000,
    [int]$StartupTimeoutSec = 60,
    [double]$PauseSec = 3,
    [string]$OutDir = '',
    [string]$TauriExe = '',
    [string]$ElectronExe = '',
    [string]$GpuiExe = '',
    [ValidateSet('auto', 'native', 'cim')]
    [string]$Sampler = 'auto',
    [switch]$NoSizeCheck,
    [switch]$NoPause,
    [switch]$NoElevate,
    [switch]$SelfTest,
    [string]$SelfTestJsonl = ''
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # Invoke-WebRequest is 10x slower with the progress bar in 5.1
$ScriptRoot = $PSScriptRoot
if (-not $ScriptRoot) { $ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path }
if (-not $OutDir) { $OutDir = $ScriptRoot }
$Inv = [System.Globalization.CultureInfo]::InvariantCulture

# PresentMon pin (console app, official GameTechDev release). Verified 2026-10-24.
$PM_VERSION = '2.6.0'
$PM_URL = 'https://github.com/GameTechDev/PresentMon/releases/download/v2.6.0/PresentMon-2.6.0-x64.exe'
$PM_SIZE = 980320
$PM_SHA256 = 'B2A706BC6AD475749E3B7E3409263AA1E6906D45BDCF993F6DBC0F660188F1AF'
$PM_SESSION = 'BakeoffPM'

$SCENARIO_PHASES = @('idle', 'transitions', 'scroll', 'minimized', 'destroyed')
$FRAME_PHASE_NAMES = @('transitions', 'scroll')

# ---------------------------------------------------------------------------------------------
# Native helpers (C# 5 so the stock csc of .NET Framework 4.x can compile it)
# ---------------------------------------------------------------------------------------------
$CSharp = @'
using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

namespace Bakeoff
{
    public class ProcInfo
    {
        public int Pid;
        public int ParentPid;
        public long CreateTime;      // FILETIME (100 ns)
        public long CpuTime100ns;    // user + kernel
        public long WorkingSet;
        public long PrivateBytes;
        public int Threads;
        public string Name;
        public long[] ThreadIds;      // null = unknown (CIM fallback)
        public long[] ThreadSwitches;
    }

    public class Sample
    {
        public double T;
        public double Dt;
        public int NProc;
        public long PrivateBytes;
        public long WorkingSet;
        public long MainPrivateBytes;
        public long MainWorkingSet;
        public double CpuPct = -1;
        public double MainCpuPct = -1;
        public long CtxSw = -1;
        public long MainCtxSw = -1;
        public int[] Pids;
    }

    public class TreeSampler
    {
        int root;
        long rootCreate = -1;
        Dictionary<int, long> knownCreate = new Dictionary<int, long>();
        Dictionary<string, string> knownNames = new Dictionary<string, string>();
        Dictionary<string, long> prevCpu = new Dictionary<string, long>();
        Dictionary<string, long> prevCtx = new Dictionary<string, long>();
        double prevT = -1;

        public TreeSampler(int rootPid) { root = rootPid; }

        public int Root { get { return root; } }

        public List<ProcInfo> Members(List<ProcInfo> snap)
        {
            Dictionary<int, ProcInfo> alive = new Dictionary<int, ProcInfo>();
            foreach (ProcInfo p in snap) { if (p.Pid > 4 && !alive.ContainsKey(p.Pid)) alive[p.Pid] = p; }
            Dictionary<int, ProcInfo> members = new Dictionary<int, ProcInfo>();
            ProcInfo r;
            if (alive.TryGetValue(root, out r))
            {
                if (rootCreate < 0) rootCreate = r.CreateTime;
                if (r.CreateTime == rootCreate) members[root] = r;
            }
            foreach (ProcInfo p in alive.Values)
            {
                long c;
                if (knownCreate.TryGetValue(p.Pid, out c) && c == p.CreateTime) members[p.Pid] = p;
            }
            bool changed = true;
            while (changed)
            {
                changed = false;
                foreach (ProcInfo p in alive.Values)
                {
                    if (members.ContainsKey(p.Pid)) continue;
                    ProcInfo parent;
                    bool isChild = false;
                    if (members.TryGetValue(p.ParentPid, out parent))
                    {
                        isChild = parent.CreateTime <= p.CreateTime;
                    }
                    else
                    {
                        long pc;
                        ProcInfo other;
                        // parent already exited (orphan): trust our record unless the pid was reused
                        if (knownCreate.TryGetValue(p.ParentPid, out pc) && pc <= p.CreateTime &&
                            !(alive.TryGetValue(p.ParentPid, out other) && other.CreateTime != pc))
                            isChild = true;
                        if (p.ParentPid == root && rootCreate >= 0 && rootCreate <= p.CreateTime &&
                            !(alive.TryGetValue(root, out other) && other.CreateTime != rootCreate))
                            isChild = true;
                    }
                    if (isChild) { members[p.Pid] = p; changed = true; }
                }
            }
            return new List<ProcInfo>(members.Values);
        }

        public Sample Tick(List<ProcInfo> snap, double tMs)
        {
            List<ProcInfo> mem = Members(snap);
            bool first = prevT < 0;
            double dt = first ? 0 : tMs - prevT;
            Sample s = new Sample();
            s.T = Math.Round(tMs, 3);
            s.Dt = Math.Round(dt, 3);
            long totCpu = 0, mainCpu = 0, totCtx = 0, mainCtx = 0;
            bool ctxKnown = true;
            List<int> pids = new List<int>();
            foreach (ProcInfo p in mem)
            {
                string key = p.Pid.ToString(CultureInfo.InvariantCulture) + ":" + p.CreateTime.ToString(CultureInfo.InvariantCulture);
                knownCreate[p.Pid] = p.CreateTime;
                knownNames[key] = p.Name;
                long prev;
                long dCpu;
                if (prevCpu.TryGetValue(key, out prev)) dCpu = Math.Max(0, p.CpuTime100ns - prev);
                else dCpu = first ? 0 : p.CpuTime100ns;
                prevCpu[key] = p.CpuTime100ns;
                long dCtx = 0;
                if (p.ThreadIds != null && p.ThreadSwitches != null)
                {
                    for (int i = 0; i < p.ThreadIds.Length; i++)
                    {
                        string tk = key + ":" + p.ThreadIds[i].ToString(CultureInfo.InvariantCulture);
                        long pv;
                        if (prevCtx.TryGetValue(tk, out pv)) dCtx += Math.Max(0, p.ThreadSwitches[i] - pv);
                        else if (!first) dCtx += p.ThreadSwitches[i];
                        prevCtx[tk] = p.ThreadSwitches[i];
                    }
                }
                else ctxKnown = false;
                s.PrivateBytes += p.PrivateBytes;
                s.WorkingSet += p.WorkingSet;
                totCpu += dCpu;
                totCtx += dCtx;
                if (p.Pid == root)
                {
                    s.MainPrivateBytes = p.PrivateBytes;
                    s.MainWorkingSet = p.WorkingSet;
                    mainCpu = dCpu;
                    mainCtx = dCtx;
                }
                pids.Add(p.Pid);
            }
            s.NProc = mem.Count;
            s.Pids = pids.ToArray();
            if (!first && dt > 0)
            {
                // 100 ns units -> ms, relative to wall time, as % of one core
                s.CpuPct = Math.Round(totCpu / 10000.0 / dt * 100.0, 3);
                s.MainCpuPct = Math.Round(mainCpu / 10000.0 / dt * 100.0, 3);
                if (ctxKnown) { s.CtxSw = totCtx; s.MainCtxSw = mainCtx; }
            }
            prevT = tMs;
            return s;
        }

        public int[] KnownPids()
        {
            return new List<int>(knownCreate.Keys).ToArray();
        }

        public string[] KnownNames()
        {
            List<string> l = new List<string>();
            foreach (KeyValuePair<string, string> kv in knownNames) l.Add(kv.Key.Split(':')[0] + ":" + kv.Value);
            return l.ToArray();
        }
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct DEVMODE
    {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
        public short dmSpecVersion; public short dmDriverVersion; public short dmSize; public short dmDriverExtra;
        public int dmFields; public int dmPositionX; public int dmPositionY; public int dmDisplayOrientation;
        public int dmDisplayFixedOutput; public short dmColor; public short dmDuplex; public short dmYResolution;
        public short dmTTOption; public short dmCollate;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
        public short dmLogPixels; public int dmBitsPerPel; public int dmPelsWidth; public int dmPelsHeight;
        public int dmDisplayFlags; public int dmDisplayFrequency; public int dmICMMethod; public int dmICMIntent;
        public int dmMediaType; public int dmDitherType; public int dmReserved1; public int dmReserved2;
        public int dmPanningWidth; public int dmPanningHeight;
    }

    public static class Native
    {
        [DllImport("ntdll.dll")]
        static extern int NtQuerySystemInformation(int infoClass, IntPtr buffer, int length, out int returnLength);

        [DllImport("kernel32.dll")]
        static extern void GetSystemTimePreciseAsFileTime(out long fileTime);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        static extern bool EnumDisplaySettings(string deviceName, int modeNum, ref DEVMODE devMode);

        static bool preciseOk = true;

        public static double NowEpochMs()
        {
            if (preciseOk)
            {
                try
                {
                    long ft;
                    GetSystemTimePreciseAsFileTime(out ft);
                    return (ft - 116444736000000000L) / 10000.0;
                }
                catch (Exception) { preciseOk = false; }
            }
            return (DateTime.UtcNow - new DateTime(1970, 1, 1, 0, 0, 0, DateTimeKind.Utc)).TotalMilliseconds;
        }

        // [qpc, epochMs]: the tightest of several back-to-back reads
        public static double[] QpcAnchor()
        {
            double bestGap = double.MaxValue; long bestQ = 0; double bestE = 0;
            for (int i = 0; i < 25; i++)
            {
                long q1 = System.Diagnostics.Stopwatch.GetTimestamp();
                double e = NowEpochMs();
                long q2 = System.Diagnostics.Stopwatch.GetTimestamp();
                if (q2 - q1 < bestGap) { bestGap = q2 - q1; bestQ = q1 + (q2 - q1) / 2; bestE = e; }
            }
            return new double[] { bestQ, bestE, System.Diagnostics.Stopwatch.Frequency };
        }

        public static int PrimaryRefreshHz()
        {
            DEVMODE dm = new DEVMODE();
            dm.dmSize = (short)Marshal.SizeOf(typeof(DEVMODE));
            if (EnumDisplaySettings(null, -1, ref dm)) return dm.dmDisplayFrequency;
            return 0;
        }

        // One NtQuerySystemInformation(SystemProcessInformation) call -> every process with parent pid,
        // create time, CPU times, working set, private bytes and per-thread context switches. x64 layout.
        public static List<ProcInfo> Snapshot()
        {
            if (IntPtr.Size != 8) throw new PlatformNotSupportedException("native sampler needs 64-bit PowerShell");
            int size = 1 << 20;
            for (int attempt = 0; attempt < 8; attempt++)
            {
                IntPtr buf = Marshal.AllocHGlobal(size);
                try
                {
                    int ret;
                    int st = NtQuerySystemInformation(5, buf, size, out ret);
                    if (st == unchecked((int)0xC0000004)) { size = Math.Max(size * 2, ret + 65536); continue; }
                    if (st != 0) throw new Exception("NtQuerySystemInformation failed: 0x" + st.ToString("X8"));
                    List<ProcInfo> list = new List<ProcInfo>(512);
                    long off = 0;
                    while (true)
                    {
                        IntPtr p = new IntPtr(buf.ToInt64() + off);
                        int next = Marshal.ReadInt32(p, 0);
                        int nthreads = Marshal.ReadInt32(p, 4);
                        ProcInfo pi = new ProcInfo();
                        pi.CreateTime = Marshal.ReadInt64(p, 32);
                        pi.CpuTime100ns = Marshal.ReadInt64(p, 40) + Marshal.ReadInt64(p, 48);
                        int nameLen = (ushort)Marshal.ReadInt16(p, 56);
                        IntPtr namePtr = Marshal.ReadIntPtr(p, 64);
                        pi.Name = namePtr == IntPtr.Zero ? "" : Marshal.PtrToStringUni(namePtr, nameLen / 2);
                        pi.Pid = (int)Marshal.ReadInt64(p, 80);
                        pi.ParentPid = (int)Marshal.ReadInt64(p, 88);
                        pi.Threads = nthreads;
                        pi.WorkingSet = Marshal.ReadInt64(p, 144);
                        pi.PrivateBytes = Marshal.ReadInt64(p, 200);
                        pi.ThreadIds = new long[nthreads];
                        pi.ThreadSwitches = new long[nthreads];
                        for (int t = 0; t < nthreads; t++)
                        {
                            int to = 256 + t * 80;
                            pi.ThreadIds[t] = Marshal.ReadInt64(p, to + 48);
                            pi.ThreadSwitches[t] = (uint)Marshal.ReadInt32(p, to + 64);
                        }
                        list.Add(pi);
                        if (next == 0) break;
                        off += next;
                    }
                    return list;
                }
                finally { Marshal.FreeHGlobal(buf); }
            }
            throw new Exception("NtQuerySystemInformation: buffer kept growing");
        }
    }

    public class PmRow
    {
        public int Pid;
        public string App;
        public string SwapChain;
        public string Mode;
        public long Qpc;
        public double MsBetweenPresents;
        public double MsUntilDisplayed;   // NaN = not displayed (dropped)
        public double MsGPUBusy;          // NaN = not available
    }

    public class PmFile
    {
        public string Header;
        public long TotalRows;
        public List<PmRow> Rows = new List<PmRow>();
        public double DwmMedianMsBetweenPresents = double.NaN;
        public string Error;
    }

    public class PmPhase
    {
        public int Presents;
        public int SwapChains;
        public string MainApp;
        public int MainPid;
        public string MainSwapChain;
        public string MainMode;
        public int Displayed;
        public int Dropped;
        public double[] IntervalsMs = new double[0];
        public double[] GpuBusyMs = new double[0];
    }

    public static class PresentMon
    {
        static double Num(string s)
        {
            if (s == null || s.Length == 0 || s == "NA") return double.NaN;
            double v;
            if (double.TryParse(s, NumberStyles.Float, CultureInfo.InvariantCulture, out v)) return v;
            return double.NaN;
        }

        // Reads a PresentMon 2.x CSV (default metrics + --qpc_time) and keeps rows of the given pids.
        public static PmFile Read(string path, int[] pids)
        {
            PmFile res = new PmFile();
            Dictionary<int, bool> want = new Dictionary<int, bool>();   // no HashSet: System.Core isn't referenced by default in PS 5.1
            foreach (int pp in pids) want[pp] = true;
            List<double> dwm = new List<double>();
            using (FileStream fs = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete))
            using (StreamReader sr = new StreamReader(fs))
            {
                string header = sr.ReadLine();
                res.Header = header;
                if (header == null) { res.Error = "empty CSV"; return res; }
                string[] cols = header.Split(',');
                Dictionary<string, int> ix = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
                for (int i = 0; i < cols.Length; i++) ix[cols[i].Trim()] = i;
                int cApp, cPid, cSc, cMode, cQpc, cBetween, cUntil, cGpu;
                if (!ix.TryGetValue("ProcessID", out cPid) || !ix.TryGetValue("TimeInQPC", out cQpc))
                {
                    res.Error = "unexpected PresentMon CSV header (need ProcessID and TimeInQPC): " + header;
                    return res;
                }
                if (!ix.TryGetValue("Application", out cApp)) cApp = -1;
                if (!ix.TryGetValue("SwapChainAddress", out cSc)) cSc = -1;
                if (!ix.TryGetValue("PresentMode", out cMode)) cMode = -1;
                if (!ix.TryGetValue("MsBetweenPresents", out cBetween)) cBetween = -1;
                if (!ix.TryGetValue("MsUntilDisplayed", out cUntil)) cUntil = -1;
                if (!ix.TryGetValue("MsGPUBusy", out cGpu)) cGpu = -1;
                if (cUntil < 0) res.Error = "CSV has no MsUntilDisplayed column (display tracking off?)";
                string line;
                while ((line = sr.ReadLine()) != null)
                {
                    if (line.Length == 0) continue;
                    string[] f = line.Split(',');
                    if (f.Length <= cQpc || f.Length <= cPid) continue;
                    res.TotalRows++;
                    int pid;
                    if (!int.TryParse(f[cPid], NumberStyles.Integer, CultureInfo.InvariantCulture, out pid)) continue;
                    if (cApp >= 0 && cBetween >= 0 && f.Length > cBetween &&
                        string.Equals(f[cApp], "dwm.exe", StringComparison.OrdinalIgnoreCase))
                    {
                        double b = Num(f[cBetween]);
                        if (!double.IsNaN(b) && b > 0 && dwm.Count < 200000) dwm.Add(b);
                    }
                    if (!want.ContainsKey(pid)) continue;
                    PmRow r = new PmRow();
                    r.Pid = pid;
                    r.App = cApp >= 0 ? f[cApp] : "";
                    r.SwapChain = cSc >= 0 && f.Length > cSc ? f[cSc] : "";
                    r.Mode = cMode >= 0 && f.Length > cMode ? f[cMode] : "";
                    long q;
                    if (!long.TryParse(f[cQpc], NumberStyles.Integer, CultureInfo.InvariantCulture, out q)) continue;
                    r.Qpc = q;
                    r.MsBetweenPresents = cBetween >= 0 && f.Length > cBetween ? Num(f[cBetween]) : double.NaN;
                    r.MsUntilDisplayed = cUntil >= 0 && f.Length > cUntil ? Num(f[cUntil]) : double.NaN;
                    r.MsGPUBusy = cGpu >= 0 && f.Length > cGpu ? Num(f[cGpu]) : double.NaN;
                    res.Rows.Add(r);
                }
            }
            if (dwm.Count > 0)
            {
                dwm.Sort();
                res.DwmMedianMsBetweenPresents = dwm[dwm.Count / 2];
            }
            return res;
        }

        // Stats for one phase window [qa, qb) in QPC ticks. The "main" swap chain is the one with the
        // most frames displayed inside the window (WebView2/Electron GPU process, or the GPUI exe).
        public static PmPhase Phase(List<PmRow> rows, long qa, long qb, double freq)
        {
            Dictionary<string, List<PmRow>> groups = new Dictionary<string, List<PmRow>>();
            PmPhase res = new PmPhase();
            foreach (PmRow r in rows)
            {
                bool inPresent = r.Qpc >= qa && r.Qpc < qb;
                double screen = double.IsNaN(r.MsUntilDisplayed) ? double.NaN : r.Qpc + r.MsUntilDisplayed * freq / 1000.0;
                bool inScreen = !double.IsNaN(screen) && screen >= qa && screen < qb;
                if (!inPresent && !inScreen) continue;
                if (inPresent) res.Presents++;
                string k = r.Pid.ToString(CultureInfo.InvariantCulture) + "|" + r.SwapChain;
                List<PmRow> g;
                if (!groups.TryGetValue(k, out g)) { g = new List<PmRow>(); groups[k] = g; }
                g.Add(r);
            }
            res.SwapChains = groups.Count;
            List<PmRow> best = null; int bestDisp = -1; int bestPres = -1;
            foreach (List<PmRow> g in groups.Values)
            {
                int disp = 0, pres = 0;
                foreach (PmRow r in g)
                {
                    if (r.Qpc >= qa && r.Qpc < qb) pres++;
                    if (!double.IsNaN(r.MsUntilDisplayed))
                    {
                        double sc = r.Qpc + r.MsUntilDisplayed * freq / 1000.0;
                        if (sc >= qa && sc < qb) disp++;
                    }
                }
                if (disp > bestDisp || (disp == bestDisp && pres > bestPres)) { best = g; bestDisp = disp; bestPres = pres; }
            }
            if (best == null) return res;
            res.MainApp = best[0].App;
            res.MainPid = best[0].Pid;
            res.MainSwapChain = best[0].SwapChain;
            res.MainMode = best[0].Mode;
            List<double> screens = new List<double>();
            List<double> gpu = new List<double>();
            foreach (PmRow r in best)
            {
                bool inPresent = r.Qpc >= qa && r.Qpc < qb;
                if (inPresent && double.IsNaN(r.MsUntilDisplayed)) res.Dropped++;
                if (inPresent && !double.IsNaN(r.MsGPUBusy)) gpu.Add(r.MsGPUBusy);
                if (!double.IsNaN(r.MsUntilDisplayed))
                {
                    double sc = r.Qpc + r.MsUntilDisplayed * freq / 1000.0;
                    if (sc >= qa && sc < qb) screens.Add(sc);
                }
            }
            screens.Sort();
            res.Displayed = screens.Count;
            List<double> iv = new List<double>();
            for (int i = 1; i < screens.Count; i++)
            {
                double d = (screens[i] - screens[i - 1]) * 1000.0 / freq;
                if (d > 0) iv.Add(Math.Round(d, 3));
            }
            res.IntervalsMs = iv.ToArray();
            res.GpuBusyMs = gpu.ToArray();
            return res;
        }
    }

    // Minimal JSON writer for hashtables / arrays / primitives / PSObjects (fast and PS-version independent).
    public static class Json
    {
        public static string Write(object o)
        {
            StringBuilder sb = new StringBuilder(1 << 16);
            W(sb, o, 0);
            return sb.ToString();
        }

        static void Str(StringBuilder sb, string s)
        {
            sb.Append('"');
            foreach (char c in s)
            {
                switch (c)
                {
                    case '"': sb.Append("\\\""); break;
                    case '\\': sb.Append("\\\\"); break;
                    case '\n': sb.Append("\\n"); break;
                    case '\r': sb.Append("\\r"); break;
                    case '\t': sb.Append("\\t"); break;
                    default:
                        if (c < 0x20) sb.Append("\\u").Append(((int)c).ToString("x4"));
                        else sb.Append(c);
                        break;
                }
            }
            sb.Append('"');
        }

        static void W(StringBuilder sb, object o, int depth)
        {
            if (depth > 40) { sb.Append("null"); return; }
            System.Management.Automation.PSObject pso = o as System.Management.Automation.PSObject;
            if (pso != null)
            {
                object b = pso.BaseObject;
                if (b is System.Management.Automation.PSCustomObject)
                {
                    sb.Append('{');
                    bool firstP = true;
                    foreach (System.Management.Automation.PSPropertyInfo p in pso.Properties)
                    {
                        if (!firstP) sb.Append(',');
                        firstP = false;
                        Str(sb, p.Name);
                        sb.Append(':');
                        object v = null;
                        try { v = p.Value; } catch (Exception) { }
                        W(sb, v, depth + 1);
                    }
                    sb.Append('}');
                    return;
                }
                o = b;
            }
            if (o == null) { sb.Append("null"); return; }
            if (o is string) { Str(sb, (string)o); return; }
            if (o is char) { Str(sb, o.ToString()); return; }
            if (o is bool) { sb.Append((bool)o ? "true" : "false"); return; }
            if (o is double || o is float)
            {
                double d = Convert.ToDouble(o, CultureInfo.InvariantCulture);
                if (double.IsNaN(d) || double.IsInfinity(d)) sb.Append("null");
                else sb.Append(d.ToString("R", CultureInfo.InvariantCulture));
                return;
            }
            if (o is decimal) { sb.Append(((decimal)o).ToString(CultureInfo.InvariantCulture)); return; }
            if (o is int || o is long || o is short || o is byte || o is uint || o is ulong || o is ushort || o is sbyte)
            {
                sb.Append(Convert.ToString(o, CultureInfo.InvariantCulture));
                return;
            }
            if (o is DateTime) { Str(sb, ((DateTime)o).ToString("o", CultureInfo.InvariantCulture)); return; }
            if (o is IDictionary)
            {
                sb.Append('{');
                bool firstD = true;
                foreach (DictionaryEntry e in (IDictionary)o)
                {
                    if (!firstD) sb.Append(',');
                    firstD = false;
                    Str(sb, Convert.ToString(e.Key, CultureInfo.InvariantCulture));
                    sb.Append(':');
                    W(sb, e.Value, depth + 1);
                }
                sb.Append('}');
                return;
            }
            if (o is IEnumerable)
            {
                sb.Append('[');
                bool firstA = true;
                foreach (object x in (IEnumerable)o)
                {
                    if (!firstA) sb.Append(',');
                    firstA = false;
                    W(sb, x, depth + 1);
                }
                sb.Append(']');
                return;
            }
            Str(sb, o.ToString());
        }
    }
}
'@

if (-not ('Bakeoff.TreeSampler' -as [type])) {
    Add-Type -TypeDefinition $CSharp -Language CSharp
}

# ---------------------------------------------------------------------------------------------
# Small helpers
# ---------------------------------------------------------------------------------------------
function Write-Info([string]$msg) { Write-Host $msg }
function Write-Warn([string]$msg) { Write-Host ('WARNING: ' + $msg) -ForegroundColor Yellow }
function Write-Err([string]$msg) { Write-Host ('ERROR: ' + $msg) -ForegroundColor Red }

function Write-Box([string[]]$lines, [string]$color = 'Yellow') {
    $w = 0
    foreach ($l in $lines) { if ($l.Length -gt $w) { $w = $l.Length } }
    $bar = '!' * ($w + 4)
    Write-Host $bar -ForegroundColor $color
    foreach ($l in $lines) { Write-Host ('! ' + $l.PadRight($w) + ' !') -ForegroundColor $color }
    Write-Host $bar -ForegroundColor $color
}

function Get-EpochMs { return [Bakeoff.Native]::NowEpochMs() }

function Test-Admin {
    try {
        $id = [Security.Principal.WindowsIdentity]::GetCurrent()
        return (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    } catch { return $false }
}

function Test-Interactive {
    if ($NoPause) { return $false }
    try { return [Environment]::UserInteractive -and -not [Console]::IsInputRedirected } catch { return $false }
}

function Wait-Exit([int]$code) {
    if (Test-Interactive) {
        Write-Host ''
        Read-Host 'Press Enter to close this window' | Out-Null
    }
    exit $code
}

function Format-Num($v, [int]$digits = 2) {
    if ($null -eq $v) { return '-' }
    if ($v -is [string]) { return $v }
    $d = [double]$v
    if ([double]::IsNaN($d)) { return '-' }
    if ([Math]::Abs($d) -ge 100) { return $d.ToString('0', $Inv) }
    return $d.ToString('0.' + ('#' * $digits), $Inv)
}

function Round-OrNull($v, [int]$digits = 1) {
    if ($null -eq $v) { return $null }
    $d = [double]$v
    if ([double]::IsNaN($d) -or [double]::IsInfinity($d)) { return $null }
    return [Math]::Round($d, $digits)
}

function Get-Percentile([double[]]$sorted, [double]$p) {
    # $sorted must be sorted ascending; linear interpolation like numpy's default
    if ($null -eq $sorted -or $sorted.Length -eq 0) { return $null }
    if ($sorted.Length -eq 1) { return $sorted[0] }
    $k = ($sorted.Length - 1) * $p / 100.0
    $lo = [Math]::Floor($k)
    $hi = [Math]::Min($lo + 1, $sorted.Length - 1)
    return $sorted[$lo] + ($sorted[$hi] - $sorted[$lo]) * ($k - $lo)
}

function Get-Sorted($vals) {
    $list = New-Object 'System.Collections.Generic.List[double]'
    foreach ($v in $vals) { if ($null -ne $v) { $d = [double]$v; if (-not [double]::IsNaN($d)) { $list.Add($d) } } }
    $arr = $list.ToArray()
    [Array]::Sort($arr)
    return ,$arr
}

function Get-Median($vals) {
    $s = Get-Sorted $vals
    if ($s.Length -eq 0) { return $null }
    return (Get-Percentile $s 50)
}

function Get-Mean($vals) {
    $s = Get-Sorted $vals
    if ($s.Length -eq 0) { return $null }
    $sum = 0.0
    foreach ($x in $s) { $sum += $x }
    return $sum / $s.Length
}

function Get-Min($vals) {
    $s = Get-Sorted $vals
    if ($s.Length -eq 0) { return $null }
    return $s[0]
}

function Get-Max($vals) {
    $s = Get-Sorted $vals
    if ($s.Length -eq 0) { return $null }
    return $s[$s.Length - 1]
}

# ---------------------------------------------------------------------------------------------
# JSONL protocol
# ---------------------------------------------------------------------------------------------
function New-EventReader([string]$path) {
    return @{ Path = $path; Offset = [long]0; Pending = ''; Lines = (New-Object 'System.Collections.Generic.List[string]');
              Events = (New-Object 'System.Collections.Generic.List[object]') }
}

function Update-EventReader($r) {
    # Reads new complete lines from the app's report file without locking it.
    if (-not (Test-Path -LiteralPath $r.Path)) { return }
    $fs = $null
    try {
        $fs = New-Object System.IO.FileStream($r.Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read,
            ([System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete))
        if ($fs.Length -le $r.Offset) { return }
        [void]$fs.Seek($r.Offset, [System.IO.SeekOrigin]::Begin)
        $n = [int]($fs.Length - $r.Offset)
        $buf = New-Object byte[] $n
        $read = 0
        while ($read -lt $n) {
            $got = $fs.Read($buf, $read, $n - $read)
            if ($got -le 0) { break }
            $read += $got
        }
        $r.Offset += $read
        $text = $r.Pending + [System.Text.Encoding]::UTF8.GetString($buf, 0, $read)
        $parts = $text.Split("`n")
        $r.Pending = $parts[$parts.Length - 1]
        for ($i = 0; $i -lt $parts.Length - 1; $i++) {
            $line = $parts[$i].Trim()
            if ($line.Length -eq 0) { continue }
            $r.Lines.Add($line)
            try { $r.Events.Add(($line | ConvertFrom-Json)) }
            catch { $r.Events.Add([pscustomobject]@{ ev = '_bad_line'; raw = $line.Substring(0, [Math]::Min(200, $line.Length)) }) }
        }
    } catch {
        # file busy for a moment: try again next tick
    } finally {
        if ($fs) { $fs.Dispose() }
    }
}

function Get-EventsFromLines([string[]]$lines) {
    $evs = New-Object 'System.Collections.Generic.List[object]'
    foreach ($l in $lines) {
        $t = $l.Trim()
        if ($t.Length -eq 0) { continue }
        try { $evs.Add(($t | ConvertFrom-Json)) } catch { $evs.Add([pscustomobject]@{ ev = '_bad_line' }) }
    }
    return ,$evs
}

function Get-Prop($obj, [string]$name) {
    if ($null -eq $obj) { return $null }
    $p = $obj.PSObject.Properties[$name]
    if ($null -eq $p) { return $null }
    return $p.Value
}

function Get-PhaseWindows($events, [double]$spawnMs) {
    $marks = @()
    $done = $null
    foreach ($e in $events) {
        $ev = Get-Prop $e 'ev'
        if ($ev -eq 'phase' -and $null -ne (Get-Prop $e 't')) { $marks += , @([string](Get-Prop $e 'name'), [double](Get-Prop $e 't')) }
        if ($ev -eq 'done' -and $null -eq $done -and $null -ne (Get-Prop $e 't')) { $done = [double](Get-Prop $e 't') }
    }
    $wins = @()
    $firstT = $done
    if ($marks.Count -gt 0) { $firstT = $marks[0][1] }
    if ($null -ne $firstT) { $wins += , @('startup', $spawnMs, $firstT) }
    for ($i = 0; $i -lt $marks.Count; $i++) {
        if ($i + 1 -lt $marks.Count) { $end = $marks[$i + 1][1] } else { $end = $done }
        if ($null -eq $end) { continue }
        $wins += , @($marks[$i][0], $marks[$i][1], $end)
    }
    return , $wins
}

function Test-RunEvents($events) {
    $names = @{}
    $phases = @{}
    $frames = @{}
    $bad = $false
    foreach ($e in $events) {
        $ev = Get-Prop $e 'ev'
        $names[[string]$ev] = $true
        if ($ev -eq 'phase') { $phases[[string](Get-Prop $e 'name')] = $true }
        if ($ev -eq 'frames') { $frames[[string](Get-Prop $e 'phase')] = $true }
        if ($ev -eq '_bad_line') { $bad = $true }
    }
    $missing = @()
    foreach ($r in @('start', 'first_frame', 'done')) { if (-not $names.ContainsKey($r)) { $missing += $r } }
    foreach ($p in $SCENARIO_PHASES) { if (-not $phases.ContainsKey($p)) { $missing += ('phase:' + $p) } }
    foreach ($p in $FRAME_PHASE_NAMES) { if (-not $frames.ContainsKey($p)) { $missing += ('frames:' + $p) } }
    if ($bad) { $missing += 'unparseable JSONL lines' }
    return , $missing
}

# ---------------------------------------------------------------------------------------------
# Statistics (shared by producer frames and PresentMon)
# ---------------------------------------------------------------------------------------------
function Get-IntervalStats([double[]]$intervals, [double]$hz) {
    # intervals in ms -> p50/p95/p99/max/jank (share > 1.5x refresh interval)
    $s = Get-Sorted $intervals
    $refreshMs = 1000.0 / $hz
    $res = [ordered]@{ intervals = $s.Length; p50_ms = $null; p95_ms = $null; p99_ms = $null; max_ms = $null; jank_pct = $null }
    if ($s.Length -eq 0) { return $res }
    $jank = 0
    foreach ($x in $s) { if ($x -gt 1.5 * $refreshMs) { $jank++ } }
    $res.p50_ms = Round-OrNull (Get-Percentile $s 50) 2
    $res.p95_ms = Round-OrNull (Get-Percentile $s 95) 2
    $res.p99_ms = Round-OrNull (Get-Percentile $s 99) 2
    $res.max_ms = Round-OrNull $s[$s.Length - 1] 2
    $res.jank_pct = Round-OrNull (100.0 * $jank / $s.Length) 2
    return $res
}

function Get-FrameStats($ts, [double]$hz, $windowMs) {
    # producer timestamps (ms relative to phase start) -> frame stats
    $s = Get-Sorted $ts
    $n = $s.Length
    $iv = New-Object 'System.Collections.Generic.List[double]'
    for ($i = 1; $i -lt $n; $i++) { $iv.Add($s[$i] - $s[$i - 1]) }
    $st = Get-IntervalStats $iv.ToArray() $hz
    $res = [ordered]@{ frames = $n; fps = $null }
    if ($n -ge 2) {
        $span = $s[$n - 1] - $s[0]
        if ($span -gt 0) { $res.fps = Round-OrNull (1000.0 * ($n - 1) / $span) 2 }
    }
    foreach ($k in @('p50_ms', 'p95_ms', 'p99_ms', 'max_ms', 'jank_pct')) { $res[$k] = $st[$k] }
    $res.refresh_coverage_pct = $null
    if ($windowMs -and $windowMs -gt 0) {
        $res.refresh_coverage_pct = Round-OrNull ([Math]::Min(100.0, 100.0 * $n / ($windowMs * $hz / 1000.0))) 2
    }
    return @{ stats = $res; intervals = $iv.ToArray() }
}

function Get-RunSummary($run, [double]$hz) {
    $events = $run.events_parsed
    $spawn = [double]$run.spawn_ms
    $sum = [ordered]@{}
    $ff = $null; $st = $null
    foreach ($e in $events) {
        $ev = Get-Prop $e 'ev'
        if ($ev -eq 'first_frame' -and $null -eq $ff) { $ff = $e }
        if ($ev -eq 'start' -and $null -eq $st) { $st = $e }
    }
    $sum.first_frame_ms = $null
    if ($ff) { $sum.first_frame_ms = Round-OrNull ([double](Get-Prop $ff 't') - $spawn) 1 }
    $sum.start_event_ms = $null
    if ($st) { $sum.start_event_ms = Round-OrNull ([double](Get-Prop $st 't') - $spawn) 1 }
    $wins = Get-PhaseWindows $events $spawn
    $phases = [ordered]@{}
    $cols = $run.sample_columns
    $iT = [Array]::IndexOf($cols, 't'); $iDt = [Array]::IndexOf($cols, 'dt'); $iN = [Array]::IndexOf($cols, 'nproc')
    $iPriv = [Array]::IndexOf($cols, 'private_bytes'); $iWs = [Array]::IndexOf($cols, 'working_set')
    $iMPriv = [Array]::IndexOf($cols, 'main_private_bytes'); $iCpu = [Array]::IndexOf($cols, 'cpu_pct')
    $iMCpu = [Array]::IndexOf($cols, 'main_cpu_pct'); $iCtx = [Array]::IndexOf($cols, 'ctxsw'); $iMCtx = [Array]::IndexOf($cols, 'main_ctxsw')
    foreach ($w in $wins) {
        $name = $w[0]; $a = [double]$w[1]; $b = [double]$w[2]
        $settle = 0.0
        if ($name -ne 'startup') { $settle = [Math]::Min([double]$SettleMs, 0.1 * ($b - $a)) }
        $priv = @(); $ws = @(); $mpriv = @(); $cpu = @(); $mcpu = @(); $nprocMax = 0
        $durMs = 0.0; $ctx = 0.0; $mctx = 0.0; $ctxKnown = $true; $n = 0
        foreach ($s in $run.samples) {
            $dt = [double]$s[$iDt]
            if ($dt -le 0) { continue }
            $mid = [double]$s[$iT] - $dt / 2.0
            if ($mid -lt ($a + $settle) -or $mid -ge $b) { continue }
            $n++
            $priv += [double]$s[$iPriv] / 1MB
            $ws += [double]$s[$iWs] / 1MB
            $mpriv += [double]$s[$iMPriv] / 1MB
            if ($null -ne $s[$iCpu]) { $cpu += [double]$s[$iCpu] }
            if ($null -ne $s[$iMCpu]) { $mcpu += [double]$s[$iMCpu] }
            if ($null -eq $s[$iCtx]) { $ctxKnown = $false } else { $ctx += [double]$s[$iCtx]; $mctx += [double]$s[$iMCtx] }
            if ([int]$s[$iN] -gt $nprocMax) { $nprocMax = [int]$s[$iN] }
            $durMs += $dt
        }
        $ctxps = $null; $mctxps = $null
        if ($ctxKnown -and $durMs -gt 0) { $ctxps = Round-OrNull ($ctx / ($durMs / 1000.0)) 1; $mctxps = Round-OrNull ($mctx / ($durMs / 1000.0)) 1 }
        $phases[$name] = [ordered]@{
            window_ms = Round-OrNull ($b - $a) 1; samples = $n
            private_median_mb = Round-OrNull (Get-Median $priv) 1; private_max_mb = Round-OrNull (Get-Max $priv) 1
            ws_median_mb = Round-OrNull (Get-Median $ws) 1; main_private_median_mb = Round-OrNull (Get-Median $mpriv) 1
            cpu_mean_pct = Round-OrNull (Get-Mean $cpu) 2; main_cpu_mean_pct = Round-OrNull (Get-Mean $mcpu) 2
            ctxsw_per_s = $ctxps; main_ctxsw_per_s = $mctxps; nproc_max = $nprocMax
        }
    }
    $sum.phases = $phases
    $frames = [ordered]@{}
    $intervals = @{}
    foreach ($e in $events) {
        if ((Get-Prop $e 'ev') -ne 'frames') { continue }
        $ph = [string](Get-Prop $e 'phase')
        $wms = $null
        foreach ($w in $wins) { if ($w[0] -eq $ph) { $wms = [double]$w[2] - [double]$w[1] } }
        $fs = Get-FrameStats (Get-Prop $e 'ts') $hz $wms
        $frames[$ph] = $fs.stats
        $intervals[$ph] = $fs.intervals
    }
    $sum.frames = $frames
    $info = @()
    foreach ($e in $events) { if ((Get-Prop $e 'ev') -eq 'info') { $info += $e } }
    $sum.info = $info
    return @{ summary = $sum; intervals = $intervals }
}

function Get-PresentMonRunStats($run, [double]$hz) {
    # per-phase presentation stats for one run from the PresentMon rows of the app's process tree
    if (-not $run.pm_rows) { return $null }
    $anchor = $run.qpc_anchor
    $q0 = [double]$anchor[0]; $e0 = [double]$anchor[1]; $freq = [double]$anchor[2]
    $wins = Get-PhaseWindows $run.events_parsed ([double]$run.spawn_ms)
    $out = [ordered]@{}
    $ivs = @{}
    foreach ($w in $wins) {
        $name = $w[0]
        if ($name -eq 'startup') { continue }
        $qa = [long]($q0 + ([double]$w[1] - $e0) * $freq / 1000.0)
        $qb = [long]($q0 + ([double]$w[2] - $e0) * $freq / 1000.0)
        $ph = [Bakeoff.PresentMon]::Phase($run.pm_rows, $qa, $qb, $freq)
        $wms = [double]$w[2] - [double]$w[1]
        $st = Get-IntervalStats $ph.IntervalsMs $hz
        $fps = $null
        if ($wms -gt 0) { $fps = Round-OrNull ($ph.Displayed * 1000.0 / $wms) 2 }
        $cov = $null
        if ($wms -gt 0) { $cov = Round-OrNull ([Math]::Min(100.0, 100.0 * $ph.Displayed / ($wms * $hz / 1000.0))) 2 }
        $dropPct = $null
        if (($ph.Displayed + $ph.Dropped) -gt 0) { $dropPct = Round-OrNull (100.0 * $ph.Dropped / ($ph.Displayed + $ph.Dropped)) 2 }
        $out[$name] = [ordered]@{
            presents = $ph.Presents; swapchains = $ph.SwapChains; main_app = $ph.MainApp; main_pid = $ph.MainPid
            present_mode = $ph.MainMode; displayed = $ph.Displayed; displayed_fps = $fps; dropped = $ph.Dropped
            dropped_pct = $dropPct; p50_ms = $st.p50_ms; p95_ms = $st.p95_ms; p99_ms = $st.p99_ms; max_ms = $st.max_ms
            jank_pct = $st.jank_pct; refresh_coverage_pct = $cov
            gpu_busy_mean_ms = Round-OrNull (Get-Mean $ph.GpuBusyMs) 3
            gpu_busy_p95_ms = Round-OrNull (Get-Percentile (Get-Sorted $ph.GpuBusyMs) 95) 3
            display_intervals_ms = $ph.IntervalsMs
        }
        $ivs[$name] = $ph.IntervalsMs
    }
    return @{ phases = $out; intervals = $ivs }
}

function Get-AppAggregate($runs, [double]$hz) {
    $okRuns = @($runs | Where-Object { $_.ok })
    $perRun = @()
    $pooled = @{}
    $pmPooled = @{}
    $pmPerRun = @()
    foreach ($r in $okRuns) {
        $rs = Get-RunSummary $r $hz
        $perRun += , $rs.summary
        foreach ($k in $rs.intervals.Keys) {
            if (-not $pooled.ContainsKey($k)) { $pooled[$k] = New-Object 'System.Collections.Generic.List[double]' }
            $pooled[$k].AddRange([double[]]$rs.intervals[$k])
        }
        $pm = Get-PresentMonRunStats $r $hz
        if ($pm) {
            $r.presentmon_phases = $pm.phases
            $pmPerRun += , $pm.phases
            foreach ($k in $pm.intervals.Keys) {
                if (-not $pmPooled.ContainsKey($k)) { $pmPooled[$k] = New-Object 'System.Collections.Generic.List[double]' }
                $pmPooled[$k].AddRange([double[]]$pm.intervals[$k])
            }
        }
    }
    $agg = [ordered]@{ runs_ok = $okRuns.Count; runs_total = @($runs).Count }
    $starts = @(); $startEv = @()
    foreach ($s in $perRun) { $starts += $s.first_frame_ms; $startEv += $s.start_event_ms }
    $warm = $null
    if ($starts.Count -gt 1) { $warm = Round-OrNull (Get-Median ($starts[1..($starts.Count - 1)])) 1 }
    $first = $null
    if ($starts.Count -gt 0) { $first = $starts[0] }
    $agg.start = [ordered]@{ first_run_ms = $first; warm_median_ms = $warm; median_ms = Round-OrNull (Get-Median $starts) 1
        min_ms = Round-OrNull (Get-Min $starts) 1; max_ms = Round-OrNull (Get-Max $starts) 1
        start_event_median_ms = Round-OrNull (Get-Median $startEv) 1; all_ms = $starts }
    $phases = [ordered]@{}
    foreach ($n in (@('startup') + $SCENARIO_PHASES)) {
        $vals = @()
        foreach ($s in $perRun) { if ($s.phases.Contains($n)) { $vals += , $s.phases[$n] } }
        if ($vals.Count -eq 0) { continue }
        $pick = { param($k) $o = @(); foreach ($v in $vals) { $o += $v[$k] }; return , $o }
        $phases[$n] = [ordered]@{
            private_median_mb = Round-OrNull (Get-Median (& $pick 'private_median_mb')) 1
            private_max_mb = Round-OrNull (Get-Max (& $pick 'private_max_mb')) 1
            ws_median_mb = Round-OrNull (Get-Median (& $pick 'ws_median_mb')) 1
            main_private_median_mb = Round-OrNull (Get-Median (& $pick 'main_private_median_mb')) 1
            cpu_mean_pct = Round-OrNull (Get-Median (& $pick 'cpu_mean_pct')) 2
            main_cpu_mean_pct = Round-OrNull (Get-Median (& $pick 'main_cpu_mean_pct')) 2
            ctxsw_per_s = Round-OrNull (Get-Median (& $pick 'ctxsw_per_s')) 1
            main_ctxsw_per_s = Round-OrNull (Get-Median (& $pick 'main_ctxsw_per_s')) 1
            nproc_max = Get-Max (& $pick 'nproc_max')
        }
    }
    $agg.phases = $phases
    $frames = [ordered]@{}
    foreach ($ph in $SCENARIO_PHASES) {
        if (-not $pooled.ContainsKey($ph)) { continue }
        $vals = @(); foreach ($s in $perRun) { if ($s.frames.Contains($ph)) { $vals += , $s.frames[$ph] } }
        $st = Get-IntervalStats $pooled[$ph].ToArray() $hz
        $fr = @(); $fps = @(); $cov = @()
        foreach ($v in $vals) { $fr += $v.frames; $fps += $v.fps; $cov += $v.refresh_coverage_pct }
        $frames[$ph] = [ordered]@{ frames_median = Get-Median $fr; fps_median = Round-OrNull (Get-Median $fps) 2
            p50_ms = $st.p50_ms; p95_ms = $st.p95_ms; p99_ms = $st.p99_ms; max_ms = $st.max_ms; jank_pct = $st.jank_pct
            refresh_coverage_pct = Round-OrNull (Get-Median $cov) 2 }
    }
    $agg.frames = $frames
    $pmAgg = [ordered]@{}
    foreach ($ph in $SCENARIO_PHASES) {
        $vals = @(); foreach ($p in $pmPerRun) { if ($p.Contains($ph)) { $vals += , $p[$ph] } }
        if ($vals.Count -eq 0) { continue }
        $iv = @()
        if ($pmPooled.ContainsKey($ph)) { $iv = $pmPooled[$ph].ToArray() }
        $st = Get-IntervalStats $iv $hz
        $g = { param($k) $o = @(); foreach ($v in $vals) { $o += $v[$k] }; return , $o }
        $apps = @(); foreach ($v in $vals) { if ($v.main_app -and ($apps -notcontains $v.main_app)) { $apps += $v.main_app } }
        $modes = @(); foreach ($v in $vals) { if ($v.present_mode -and ($modes -notcontains $v.present_mode)) { $modes += $v.present_mode } }
        $pmAgg[$ph] = [ordered]@{ displayed_median = Get-Median (& $g 'displayed'); displayed_fps_median = Round-OrNull (Get-Median (& $g 'displayed_fps')) 2
            dropped_median = Get-Median (& $g 'dropped'); dropped_pct_median = Round-OrNull (Get-Median (& $g 'dropped_pct')) 2
            p50_ms = $st.p50_ms; p95_ms = $st.p95_ms; p99_ms = $st.p99_ms; max_ms = $st.max_ms; jank_pct = $st.jank_pct
            refresh_coverage_pct = Round-OrNull (Get-Median (& $g 'refresh_coverage_pct')) 2
            gpu_busy_mean_ms = Round-OrNull (Get-Median (& $g 'gpu_busy_mean_ms')) 3
            presenting_process = ($apps -join ','); present_mode = ($modes -join ',') }
    }
    $agg.presentmon = $pmAgg
    $agg.per_run = $perRun
    $agg.targets = Get-Targets $agg $hz
    return $agg
}

function Get-Targets($agg, [double]$hz) {
    $t = [ordered]@{}
    $refreshMs = 1000.0 / $hz
    foreach ($ph in $FRAME_PHASE_NAMES) {
        $src = 'producer'
        $f = $null
        if ($agg.presentmon.Contains($ph)) { $f = $agg.presentmon[$ph]; $src = 'PresentMon' }
        elseif ($agg.frames.Contains($ph)) { $f = $agg.frames[$ph] }
        $cov = $null; $p99 = $null
        if ($f) { $cov = $f.refresh_coverage_pct; $p99 = $f.p99_ms }
        $t[('{0} refreshes presented >= 99% ({1})' -f $ph, $src)] = [ordered]@{ value = $cov; pass = $(if ($null -eq $cov) { $null } else { $cov -ge 99.0 }) }
        $lim = 1.5 * $refreshMs
        $t[('{0} p99 <= 1.5x refresh ({1} ms, {2})' -f $ph, $lim.ToString('0.00', $Inv), $src)] = [ordered]@{ value = $p99; pass = $(if ($null -eq $p99) { $null } else { $p99 -le $lim }) }
    }
    $st = $agg.start
    $t['cold start (first run) <= 1000 ms'] = [ordered]@{ value = $st.first_run_ms; pass = $(if ($null -eq $st.first_run_ms) { $null } else { $st.first_run_ms -le 1000 }) }
    $t['warm start (median runs 2..N) <= 400 ms'] = [ordered]@{ value = $st.warm_median_ms; pass = $(if ($null -eq $st.warm_median_ms) { $null } else { $st.warm_median_ms -le 400 }) }
    $idle = $null; if ($agg.phases.Contains('idle')) { $idle = $agg.phases['idle'].cpu_mean_pct }
    $t['idle CPU <= 0.5% (tree)'] = [ordered]@{ value = $idle; pass = $(if ($null -eq $idle) { $null } else { $idle -le 0.5 }) }
    $dc = $null; $dm = $null
    if ($agg.phases.Contains('destroyed')) { $dc = $agg.phases['destroyed'].main_cpu_mean_pct; $dm = $agg.phases['destroyed'].main_private_median_mb }
    $t['destroyed CPU ~0 (main <= 0.2%)'] = [ordered]@{ value = $dc; pass = $(if ($null -eq $dc) { $null } else { $dc -le 0.2 }) }
    $t['destroyed memory <= 20 MB (main, private)'] = [ordered]@{ value = $dm; pass = $(if ($null -eq $dm) { $null } else { $dm -le 20 }) }
    return $t
}

# ---------------------------------------------------------------------------------------------
# Markdown
# ---------------------------------------------------------------------------------------------
function Format-Table-Md([string[]]$header, $rows) {
    $sb = New-Object System.Text.StringBuilder
    [void]$sb.AppendLine('| ' + ($header -join ' | ') + ' |')
    [void]$sb.AppendLine('|' + ((@('---') * $header.Count) -join '|') + '|')
    foreach ($r in $rows) {
        $cells = @()
        foreach ($c in $r) { $cells += (Format-Num $c) }
        [void]$sb.AppendLine('| ' + ($cells -join ' | ') + ' |')
    }
    return $sb.ToString()
}

function Format-Summary($result) {
    $envi = $result.environment
    $cfg = $result.config
    $o = New-Object System.Text.StringBuilder
    [void]$o.AppendLine('# Bake-off results: Windows (' + $envi.computer + ')')
    [void]$o.AppendLine('')
    [void]$o.AppendLine(('- Refresh: {0} Hz ({1}); phase {2} ms; {3} run(s) per app; sampling every {4} ms ({5} sampler)' -f (Format-Num $envi.refresh_hz), $envi.refresh_source, $cfg.phase_ms, $cfg.runs, $cfg.sample_ms, $envi.sampler))
    [void]$o.AppendLine(('- {0} (build {1}); CPU {2}; RAM {3} GB' -f $envi.os, $envi.os_build, $envi.cpu, $envi.ram_gb))
    $gl = @(); foreach ($g in $envi.gpus) { $gl += ('{0} (driver {1}, {2} Hz)' -f $g.name, $g.driver, $g.refresh_hz) }
    [void]$o.AppendLine('- GPU: ' + ($gl -join '; '))
    $pmLine = 'off'
    if ($envi.presentmon.enabled) { $pmLine = 'PresentMon ' + $envi.presentmon.version }
    [void]$o.AppendLine('- Presentation capture: ' + $pmLine)
    foreach ($w in $result.warnings) { [void]$o.AppendLine('- Warning: ' + $w) }
    [void]$o.AppendLine('')
    $appNames = @($result.summary.Keys)

    [void]$o.AppendLine('## Startup (spawn -> first_frame)')
    [void]$o.AppendLine('')
    $rows = @()
    foreach ($a in $appNames) {
        $s = $result.summary[$a]
        $rows += , @($a, ('{0}/{1}' -f $s.runs_ok, $s.runs_total), $s.start.first_run_ms, $s.start.warm_median_ms, $s.start.median_ms, $s.start.start_event_median_ms)
    }
    [void]$o.AppendLine((Format-Table-Md @('app', 'runs ok', 'first run ms', 'warm median ms', 'median ms', 'host start ev ms') $rows))

    [void]$o.AppendLine('## Memory and CPU per phase (medians across runs)')
    [void]$o.AppendLine('')
    [void]$o.AppendLine('private/WS = private bytes / working set summed over the process tree; main = host process only; CPU = % of one core.')
    [void]$o.AppendLine('')
    $rows = @()
    foreach ($a in $appNames) {
        $ph = $result.summary[$a].phases
        foreach ($n in (@('startup') + $SCENARIO_PHASES)) {
            if (-not $ph.Contains($n)) { continue }
            $p = $ph[$n]
            $rows += , @($a, $n, $p.private_median_mb, $p.private_max_mb, $p.ws_median_mb, $p.main_private_median_mb, $p.cpu_mean_pct, $p.main_cpu_mean_pct, $p.ctxsw_per_s, $p.nproc_max)
        }
    }
    [void]$o.AppendLine((Format-Table-Md @('app', 'phase', 'private med MB', 'private max MB', 'WS med MB', 'main private MB', 'CPU %', 'main CPU %', 'ctxsw/s', 'procs') $rows))

    $rows = @()
    foreach ($a in $appNames) {
        $pm = $result.summary[$a].presentmon
        foreach ($n in $SCENARIO_PHASES) {
            if (-not $pm.Contains($n)) { continue }
            $p = $pm[$n]
            $rows += , @($a, $n, $p.displayed_median, $p.displayed_fps_median, $p.p50_ms, $p.p95_ms, $p.p99_ms, $p.max_ms, $p.jank_pct, $p.refresh_coverage_pct, $p.dropped_median, $p.gpu_busy_mean_ms, $p.presenting_process)
        }
    }
    if ($rows.Count -gt 0) {
        [void]$o.AppendLine('## Presented frames (PresentMon, displayed-frame intervals pooled across runs)')
        [void]$o.AppendLine('')
        [void]$o.AppendLine((Format-Table-Md @('app', 'phase', 'displayed', 'fps', 'p50 ms', 'p95 ms', 'p99 ms', 'max ms', 'jank %', 'refreshes hit %', 'dropped', 'GPU busy ms', 'presenter') $rows))
    }

    [void]$o.AppendLine('## Producer frames (app rAF/render timestamps, pooled across runs)')
    [void]$o.AppendLine('')
    $rows = @()
    foreach ($a in $appNames) {
        $fr = $result.summary[$a].frames
        foreach ($n in $SCENARIO_PHASES) {
            if (-not $fr.Contains($n)) { continue }
            $f = $fr[$n]
            $rows += , @($a, $n, $f.frames_median, $f.fps_median, $f.p50_ms, $f.p95_ms, $f.p99_ms, $f.max_ms, $f.jank_pct, $f.refresh_coverage_pct)
        }
    }
    [void]$o.AppendLine((Format-Table-Md @('app', 'phase', 'frames', 'fps', 'p50 ms', 'p95 ms', 'p99 ms', 'max ms', 'jank %', 'refreshes hit %') $rows))
    [void]$o.AppendLine(('jank % = share of frame intervals > 1.5x the refresh interval ({0} ms).' -f (1500.0 / $envi.refresh_hz).ToString('0.00', $Inv)))
    [void]$o.AppendLine('')

    [void]$o.AppendLine('## Targets')
    [void]$o.AppendLine('')
    $tnames = @()
    foreach ($a in $appNames) { foreach ($k in $result.summary[$a].targets.Keys) { if ($tnames -notcontains $k) { $tnames += $k } } }
    $rows = @()
    foreach ($k in $tnames) {
        $row = @($k)
        foreach ($a in $appNames) {
            $v = $null
            if ($result.summary[$a].targets.Contains($k)) { $v = $result.summary[$a].targets[$k] }
            if ($null -eq $v -or $null -eq $v.pass) { $row += '-' }
            elseif ($v.pass) { $row += ('PASS ' + (Format-Num $v.value)) }
            else { $row += ('FAIL ' + (Format-Num $v.value)) }
        }
        $rows += , $row
    }
    if ($result.sizes) {
        $row = @('download (zipped app folder) <= 25 MB')
        foreach ($a in $appNames) {
            if ($result.sizes.Contains($a) -and $null -ne $result.sizes[$a].zip_mb) {
                $z = $result.sizes[$a].zip_mb
                if ($z -le 25) { $row += ('PASS ' + (Format-Num $z)) } else { $row += ('FAIL ' + (Format-Num $z)) }
            } else { $row += '-' }
        }
        $rows += , $row
    }
    [void]$o.AppendLine((Format-Table-Md (@('target') + $appNames) $rows))

    $infoLines = @()
    foreach ($a in $appNames) {
        foreach ($s in $result.summary[$a].per_run) {
            foreach ($e in $s.info) {
                $parts = @()
                foreach ($p in $e.PSObject.Properties) {
                    if ($p.Name -eq 'ev' -or $p.Name -eq 't') { continue }
                    $v = $p.Value
                    if ($null -eq $v) { continue }
                    if ($v -is [string] -and $v.Length -gt 160) { continue }
                    if ($v -is [string] -or $v -is [ValueType]) { $parts += ('{0}={1}' -f $p.Name, $v) }
                }
                if ($parts.Count -gt 0) { $infoLines += ('- ' + $a + ': ' + ($parts -join ', ')) }
            }
            break
        }
    }
    if ($infoLines.Count -gt 0) {
        [void]$o.AppendLine('## App info events (first ok run)')
        [void]$o.AppendLine('')
        foreach ($l in $infoLines) { [void]$o.AppendLine($l) }
        [void]$o.AppendLine('')
    }

    $failed = @()
    foreach ($a in @($result.runs.Keys)) {
        $i = 0
        foreach ($r in $result.runs[$a]) {
            $i++
            if (-not $r.ok) {
                $why = $r.error
                if (-not $why) { $why = 'missing ' + ($r.missing -join ', ') }
                $failed += ('- {0} run {1}: {2}' -f $a, $i, $why)
            }
        }
    }
    if ($failed.Count -gt 0) {
        [void]$o.AppendLine('## Failed runs')
        [void]$o.AppendLine('')
        foreach ($l in $failed) { [void]$o.AppendLine($l) }
    }
    return $o.ToString()
}

# ---------------------------------------------------------------------------------------------
# Environment
# ---------------------------------------------------------------------------------------------
function Get-RegValue([string]$path, [string]$name) {
    try { return (Get-ItemProperty -Path $path -Name $name -ErrorAction Stop).$name } catch { return $null }
}

function Get-EnvInfo {
    $info = [ordered]@{ platform = 'windows'; computer = $env:COMPUTERNAME; harness = 'bakeoff/harness/windows/run-bakeoff.ps1'
        powershell = $PSVersionTable.PSVersion.ToString(); is_admin = (Test-Admin) }
    try {
        $os = Get-CimInstance Win32_OperatingSystem
        $info.os = $os.Caption; $info.os_version = $os.Version; $info.os_build = $os.BuildNumber
        $info.ram_gb = [Math]::Round([double]$os.TotalVisibleMemorySize / 1MB, 1)
    } catch { $info.os = [Environment]::OSVersion.VersionString }
    try {
        $ubr = Get-RegValue 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' 'UBR'
        $disp = Get-RegValue 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' 'DisplayVersion'
        $info.os_display_version = $disp; $info.os_ubr = $ubr
    } catch { }
    try { $info.cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name.Trim(); $info.cpu_threads = [Environment]::ProcessorCount } catch { }
    $gpus = @()
    try {
        foreach ($g in (Get-CimInstance Win32_VideoController)) {
            $gpus += [ordered]@{ name = $g.Name; driver = $g.DriverVersion; refresh_hz = $g.CurrentRefreshRate
                resolution = ('{0}x{1}' -f $g.CurrentHorizontalResolution, $g.CurrentVerticalResolution) }
        }
    } catch { }
    $info.gpus = $gpus
    try {
        $bat = @(Get-CimInstance Win32_Battery)
        if ($bat.Count -gt 0) { $info.battery_present = $true; $info.on_ac = (@($bat | Where-Object { $_.BatteryStatus -eq 2 -or $_.BatteryStatus -ge 6 }).Count -gt 0) }
        else { $info.battery_present = $false }
    } catch { }
    try { $info.power_plan = ((powercfg /getactivescheme) | Out-String).Trim() } catch { }
    $info.hags = Get-RegValue 'HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers' 'HwSchMode'   # 2 = on
    $info.directx_user_global = Get-RegValue 'HKCU:\Software\Microsoft\DirectX\UserGpuPreferences' 'DirectXUserGlobalSettings'
    $info.game_mode = Get-RegValue 'HKCU:\Software\Microsoft\GameBar' 'AutoGameModeEnabled'
    $wv = Get-RegValue 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' 'pv'
    if (-not $wv) { $wv = Get-RegValue 'HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' 'pv' }
    $info.webview2_runtime = $wv
    return $info
}

function Get-RefreshInfo {
    if ($RefreshHz -ne 'auto') { return @{ hz = [double]::Parse($RefreshHz, $Inv); source = '-RefreshHz' } }
    try {
        $hz = [Bakeoff.Native]::PrimaryRefreshHz()
        if ($hz -gt 1) { return @{ hz = [double]$hz; source = 'EnumDisplaySettings (primary monitor)' } }
    } catch { }
    try {
        $max = 0
        foreach ($g in (Get-CimInstance Win32_VideoController)) { if ($g.CurrentRefreshRate -gt $max) { $max = $g.CurrentRefreshRate } }
        if ($max -gt 1) { return @{ hz = [double]$max; source = 'Win32_VideoController.CurrentRefreshRate' } }
    } catch { }
    return @{ hz = 60.0; source = 'fallback' }
}

# ---------------------------------------------------------------------------------------------
# Process sampling
# ---------------------------------------------------------------------------------------------
function Get-SnapshotCim {
    $list = New-Object 'System.Collections.Generic.List[Bakeoff.ProcInfo]'
    foreach ($p in (Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, CreationDate, KernelModeTime, UserModeTime, WorkingSetSize, PrivatePageCount, Name, ThreadCount)) {
        $pi = New-Object Bakeoff.ProcInfo
        $pi.Pid = [int]$p.ProcessId
        $pi.ParentPid = [int]$p.ParentProcessId
        if ($p.CreationDate) { $pi.CreateTime = $p.CreationDate.ToFileTimeUtc() }
        $pi.CpuTime100ns = [long]$p.KernelModeTime + [long]$p.UserModeTime
        $pi.WorkingSet = [long]$p.WorkingSetSize
        $pi.PrivateBytes = [long]$p.PrivatePageCount
        $pi.Threads = [int]$p.ThreadCount
        $pi.Name = $p.Name
        $list.Add($pi)
    }
    return , $list
}

function Select-Sampler {
    if ($Sampler -eq 'cim') { return 'cim' }
    try {
        $snap = [Bakeoff.Native]::Snapshot()
        $me = $null
        foreach ($p in $snap) { if ($p.Pid -eq $PID) { $me = $p } }
        if ($null -eq $me) { throw 'own process not found in snapshot' }
        $gp = Get-Process -Id $PID
        $cim = Get-CimInstance Win32_Process -Filter ('ProcessId={0}' -f $PID)
        if ($me.ParentPid -ne [int]$cim.ParentProcessId) { throw ('parent pid mismatch {0} vs {1}' -f $me.ParentPid, $cim.ParentProcessId) }
        $ratio = [double]$me.WorkingSet / [double]$gp.WorkingSet64
        if ($ratio -lt 0.5 -or $ratio -gt 2.0) { throw ('working set mismatch {0} vs {1}' -f $me.WorkingSet, $gp.WorkingSet64) }
        if ($me.Name -notmatch 'powershell|pwsh') { throw ('unexpected image name ' + $me.Name) }
        return 'native'
    } catch {
        if ($Sampler -eq 'native') { throw }
        Write-Warn ('native sampler self-check failed ({0}); falling back to Win32_Process (CIM), which costs more CPU' -f $_)
        return 'cim'
    }
}

function Get-Snapshot([string]$mode) {
    if ($mode -eq 'native') { return , ([Bakeoff.Native]::Snapshot()) }
    return , (Get-SnapshotCim)
}

function Convert-Sample($s) {
    # compact row matching $SampleColumns
    $cpu = $null; $mcpu = $null; $ctx = $null; $mctx = $null
    if ($s.CpuPct -ge 0) { $cpu = $s.CpuPct; $mcpu = $s.MainCpuPct }
    if ($s.CtxSw -ge 0) { $ctx = $s.CtxSw; $mctx = $s.MainCtxSw }
    return , @($s.T, $s.Dt, $s.NProc, $s.PrivateBytes, $s.WorkingSet, $s.MainPrivateBytes, $s.MainWorkingSet, $cpu, $mcpu, $ctx, $mctx)
}
$SampleColumns = @('t', 'dt', 'nproc', 'private_bytes', 'working_set', 'main_private_bytes', 'main_working_set', 'cpu_pct', 'main_cpu_pct', 'ctxsw', 'main_ctxsw')

function Stop-Tree($sampler, [string]$mode) {
    $killed = @()
    for ($round = 0; $round -lt 4; $round++) {
        $alive = $sampler.Members((Get-Snapshot $mode))
        if ($alive.Count -eq 0) { break }
        foreach ($p in $alive) {
            if ($round -eq 0) { $killed += ('{0}:{1}' -f $p.Pid, $p.Name) }
            try { Stop-Process -Id $p.Pid -Force -ErrorAction Stop } catch { }
        }
        Start-Sleep -Milliseconds 500
    }
    return , $killed
}

# ---------------------------------------------------------------------------------------------
# PresentMon
# ---------------------------------------------------------------------------------------------
function Get-PresentMonExe {
    $dir = Join-Path $ScriptRoot 'tools'
    $exe = Join-Path $dir ('PresentMon-{0}-x64.exe' -f $PM_VERSION)
    if (Test-Path -LiteralPath $exe) {
        $h = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
        if ($h -eq $PM_SHA256) { return $exe }
        Write-Warn 'cached PresentMon has the wrong hash; downloading again'
        Remove-Item -LiteralPath $exe -Force
    }
    if (-not (Test-Path -LiteralPath $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
    Write-Info ('Downloading PresentMon {0} from GitHub (~1 MB)...' -f $PM_VERSION)
    try { [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12 } catch { }
    $tmp = $exe + '.download'
    Invoke-WebRequest -Uri $PM_URL -OutFile $tmp -UseBasicParsing
    $len = (Get-Item -LiteralPath $tmp).Length
    $h = (Get-FileHash -LiteralPath $tmp -Algorithm SHA256).Hash
    if ($len -ne $PM_SIZE -or $h -ne $PM_SHA256) {
        Remove-Item -LiteralPath $tmp -Force
        throw ('PresentMon download failed verification (size {0}, sha256 {1}); expected {2} / {3}' -f $len, $h, $PM_SIZE, $PM_SHA256)
    }
    Move-Item -LiteralPath $tmp -Destination $exe -Force
    Write-Info ('PresentMon verified: sha256 {0}' -f $h)
    return $exe
}

function Start-PresentMon([string]$exe, [string]$csv, [int]$maxSec) {
    $outLog = $csv + '.stdout.txt'
    $errLog = $csv + '.stderr.txt'
    $pmArgs = @('--output_file', ('"{0}"' -f $csv), '--qpc_time', '--no_console_stats', '--session_name', $PM_SESSION,
        '--stop_existing_session', '--timed', [string]$maxSec, '--terminate_after_timed')
    $p = Start-Process -FilePath $exe -ArgumentList ($pmArgs -join ' ') -NoNewWindow -PassThru -RedirectStandardOutput $outLog -RedirectStandardError $errLog
    # wait until the ETW session is up (CSV created) or PresentMon dies
    $deadline = (Get-Date).AddSeconds(8)
    while ((Get-Date) -lt $deadline) {
        if ($p.HasExited) { break }
        if (Test-Path -LiteralPath $csv) { break }
        Start-Sleep -Milliseconds 100
    }
    if ($p.HasExited) {
        $msg = ''
        try { $msg = ((Get-Content -LiteralPath $errLog -ErrorAction SilentlyContinue) + (Get-Content -LiteralPath $outLog -ErrorAction SilentlyContinue)) -join ' ' } catch { }
        throw ('PresentMon exited immediately (code {0}): {1}' -f $p.ExitCode, $msg)
    }
    Start-Sleep -Milliseconds 1000   # let the trace session settle before the app starts
    return $p
}

function Stop-PresentMon([string]$exe, $p) {
    if ($null -eq $p) { return }
    try { & $exe --terminate_existing_session --session_name $PM_SESSION 2>&1 | Out-Null } catch { }
    if (-not $p.WaitForExit(15000)) {
        Write-Warn 'PresentMon did not exit after stopping its session; killing it'
        try { $p.Kill() } catch { }
        [void]$p.WaitForExit(5000)
    }
}

# ---------------------------------------------------------------------------------------------
# One app run
# ---------------------------------------------------------------------------------------------
function Invoke-AppRun([string]$app, [string]$exe, [string[]]$exeArgs, [int]$runIdx, [int]$total, [string]$mode, [string]$pmExe, [string]$workDir) {
    $tag = '[{0} {1}/{2}]' -f $app, ($runIdx + 1), $total
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ('bakeoff-{0}-{1}' -f $app, [Guid]::NewGuid().ToString('N').Substring(0, 8))
    New-Item -ItemType Directory -Path $tmp | Out-Null
    $report = Join-Path $tmp 'report.jsonl'
    [IO.File]::WriteAllText($report, '')
    $run = [ordered]@{ app = $app; status = 'running'; ok = $false; sample_columns = $SampleColumns }
    $pm = $null
    $timeoutSec = $StartupTimeoutSec + 5 * $PhaseMs / 1000.0 + 30
    if ($pmExe) {
        $csv = Join-Path $tmp 'presentmon.csv'
        try { $pm = Start-PresentMon $pmExe $csv ([int]($timeoutSec + 60)) }
        catch { Write-Warn ('{0} PresentMon failed to start: {1}' -f $tag, $_); $run.presentmon_error = [string]$_; $pm = $null }
    }
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.Arguments = ($exeArgs -join ' ')
    $psi.UseShellExecute = $false
    $psi.WorkingDirectory = $workDir
    $envDict = $null
    try { $envDict = $psi.Environment } catch { $envDict = $null }
    if ($null -eq $envDict) { $envDict = $psi.EnvironmentVariables }
    $drop = @()
    foreach ($k in @($envDict.Keys)) { if ([string]$k -like 'ELECTRON_*') { $drop += $k } }
    foreach ($k in $drop) { [void]$envDict.Remove($k) }   # ELECTRON_RUN_AS_NODE etc. would turn Electron into plain Node
    $envDict['BAKEOFF_OUT'] = $report
    $envDict['BAKEOFF_PHASE_MS'] = [string]$PhaseMs
    $anchor = [Bakeoff.Native]::QpcAnchor()
    $run.qpc_anchor = $anchor
    Write-Info ('{0} launching {1}' -f $tag, $exe)
    $spawn = Get-EpochMs
    try {
        $proc = [System.Diagnostics.Process]::Start($psi)
    } catch {
        $run.status = 'spawn failed: ' + $_
        $run.error = $run.status
        Stop-PresentMon $pmExe $pm
        Write-Err ('{0} {1}' -f $tag, $run.status)
        $run.spawn_ms = $spawn; $run.events_parsed = @(); $run.samples = @(); $run.raw_jsonl = @(); $run.missing = @()
        return $run
    }
    $run.spawn_ms = [Math]::Round($spawn, 3)
    $run.pid = $proc.Id
    $sampler = New-Object Bakeoff.TreeSampler($proc.Id)
    $reader = New-EventReader $report
    $samples = New-Object 'System.Collections.Generic.List[object]'
    $seen = @{}
    $doneAt = $null
    $t0 = Get-Date
    $tick = [double]$SampleMs
    $next = Get-EpochMs
    while ($true) {
        try { $s = $sampler.Tick((Get-Snapshot $mode), (Get-EpochMs)); $samples.Add((Convert-Sample $s)) }
        catch { Write-Warn ('{0} sampling error: {1}' -f $tag, $_) }
        Update-EventReader $reader
        foreach ($e in $reader.Events) {
            $ev = Get-Prop $e 'ev'
            $key = [string]$ev + ':' + [string](Get-Prop $e 'name') + [string](Get-Prop $e 'phase')
            if ($seen.ContainsKey($key)) { continue }
            $seen[$key] = $true
            if ($ev -eq 'first_frame') { Write-Info ('{0} first_frame (+{1} ms)' -f $tag, ([double](Get-Prop $e 't') - $spawn).ToString('0', $Inv)) }
            elseif ($ev -eq 'phase') { Write-Info ('{0} phase {1}' -f $tag, (Get-Prop $e 'name')) }
            elseif ($ev -eq 'done') { Write-Info ('{0} done' -f $tag); $doneAt = Get-Date }
        }
        $now = Get-Date
        $exited = $proc.HasExited
        if ($null -ne $doneAt -and ($exited -or ($now - $doneAt).TotalSeconds -gt 5)) { $run.status = 'done'; break }
        if ($exited -and $null -eq $doneAt) {
            Start-Sleep -Milliseconds 300
            Update-EventReader $reader
            $hasDone = $false
            foreach ($e in $reader.Events) { if ((Get-Prop $e 'ev') -eq 'done') { $hasDone = $true } }
            if ($hasDone) { $run.status = 'done' } else { $run.status = ('exited early (code {0})' -f $proc.ExitCode) }
            break
        }
        $hasFF = $false
        foreach ($e in $reader.Events) { if ((Get-Prop $e 'ev') -eq 'first_frame') { $hasFF = $true; break } }
        if (-not $hasFF -and ($now - $t0).TotalSeconds -gt $StartupTimeoutSec) { $run.status = ('no first_frame within {0}s' -f $StartupTimeoutSec); break }
        if (($now - $t0).TotalSeconds -gt $timeoutSec) { $run.status = ('timeout after {0:0}s' -f $timeoutSec); break }
        $next += $tick
        $delay = $next - (Get-EpochMs)
        if ($delay -gt 0) { Start-Sleep -Milliseconds ([int]$delay) } else { $next = Get-EpochMs }
    }
    $exitCode = $null
    if ($proc.HasExited) { $exitCode = $proc.ExitCode }
    $run.exit_code = $exitCode
    $run.leftovers_killed = Stop-Tree $sampler $mode
    if ($run.leftovers_killed.Count -gt 0) { Write-Info ('{0} killed leftovers: {1}' -f $tag, ($run.leftovers_killed -join ', ')) }
    Update-EventReader $reader
    $run.raw_jsonl = $reader.Lines.ToArray()
    $run.events_parsed = $reader.Events.ToArray()
    $run.samples = $samples.ToArray()
    $run.tree_processes = $sampler.KnownNames()
    if ($pm) {
        Stop-PresentMon $pmExe $pm
        try {
            $f = [Bakeoff.PresentMon]::Read($csv, $sampler.KnownPids())
            $run.presentmon = [ordered]@{ total_rows = $f.TotalRows; app_rows = $f.Rows.Count; error = $f.Error; header = $f.Header
                dwm_median_ms_between_presents = $(if ([double]::IsNaN($f.DwmMedianMsBetweenPresents)) { $null } else { [Math]::Round($f.DwmMedianMsBetweenPresents, 3) }) }
            if ($f.Error) { Write-Warn ('{0} PresentMon: {1}' -f $tag, $f.Error) }
            $run.pm_rows = $f.Rows
            Write-Info ('{0} PresentMon: {1} rows total, {2} from the app tree' -f $tag, $f.TotalRows, $f.Rows.Count)
        } catch {
            Write-Warn ('{0} could not read PresentMon CSV: {1}' -f $tag, $_)
            $run.presentmon_error = [string]$_
            # keep PresentMon's own output so a one-shot run on someone else's PC is diagnosable
            $pmOut = ''
            foreach ($lp in @(($csv + '.stdout.txt'), ($csv + '.stderr.txt'))) {
                try { $pmOut += ((Get-Content -LiteralPath $lp -ErrorAction SilentlyContinue | Select-Object -First 40) -join "`n") + "`n" } catch { }
            }
            $run.presentmon_output = $pmOut.Trim()
            try { $run.presentmon_exit_code = $pm.ExitCode } catch { }
        }
    }
    $run.missing = Test-RunEvents $run.events_parsed
    $run.ok = ($run.status -eq 'done' -and $run.missing.Count -eq 0)
    if ($run.status -ne 'done') { $run.error = $run.status }
    if ($run.ok) { Write-Info ('{0} ok' -f $tag) }
    else { Write-Err ('{0} FAILED: {1} {2}' -f $tag, $run.status, ($(if ($run.missing.Count -gt 0) { 'missing ' + ($run.missing -join ', ') } else { '' }))) }
    try { Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue } catch { }
    return $run
}

function Get-AppSizes($appDefs) {
    $sizes = [ordered]@{}
    try { Add-Type -AssemblyName System.IO.Compression.FileSystem } catch { return $sizes }
    foreach ($a in $appDefs.Keys) {
        $dir = $appDefs[$a].dir
        if (-not (Test-Path -LiteralPath $dir)) { continue }
        $bytes = 0
        foreach ($f in (Get-ChildItem -LiteralPath $dir -Recurse -File -Force)) { $bytes += $f.Length }
        $zip = Join-Path ([IO.Path]::GetTempPath()) ('bakeoff-size-{0}.zip' -f $a)
        $zipMb = $null
        try {
            if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
            [IO.Compression.ZipFile]::CreateFromDirectory($dir, $zip, [IO.Compression.CompressionLevel]::Optimal, $false)
            $zipMb = [Math]::Round((Get-Item -LiteralPath $zip).Length / 1MB, 1)
            Remove-Item -LiteralPath $zip -Force
        } catch { }
        $sizes[$a] = [ordered]@{ folder_mb = [Math]::Round($bytes / 1MB, 1); zip_mb = $zipMb }
    }
    return $sizes
}

function Write-Results($result, [string]$stem) {
    $json = [Bakeoff.Json]::Write($result)
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($stem + '.json', $json, $utf8)
    $md = Format-Summary $result
    [IO.File]::WriteAllText($stem + '.md', $md, $utf8)
    return $md
}

function Remove-InternalFields($result) {
    # drop non-serialisable / bulky helper fields before writing
    foreach ($a in @($result.runs.Keys)) {
        foreach ($r in $result.runs[$a]) {
            if ($r.Contains('pm_rows')) { $r.Remove('pm_rows') }
            if ($r.Contains('events_parsed')) { $r.Remove('events_parsed') }
        }
    }
}

# ---------------------------------------------------------------------------------------------
# Self-test (runs anywhere PowerShell runs, including pwsh on Linux): synthetic samples, a JSONL
# report (from mock_app.py or generated) and a synthetic PresentMon 2.6 CSV go through the same
# summary code as a real run.
# ---------------------------------------------------------------------------------------------
function Invoke-SelfTest {
    $hz = 144.0
    Write-Info 'Self-test: building synthetic run data'
    $lines = @()
    if ($SelfTestJsonl -and (Test-Path -LiteralPath $SelfTestJsonl)) {
        $lines = [IO.File]::ReadAllLines($SelfTestJsonl)
    } else {
        $t = 1729780000000.0
        $lines += ('{{"ev":"start","t":{0},"stack":"selftest","pid":1}}' -f ($t + 20).ToString('R', $Inv))
        $lines += ('{{"ev":"first_frame","t":{0}}}' -f ($t + 250).ToString('R', $Inv))
        $pt = $t + 260
        foreach ($ph in $SCENARIO_PHASES) {
            $lines += ('{{"ev":"phase","name":"{0}","t":{1}}}' -f $ph, $pt.ToString('R', $Inv))
            if ($FRAME_PHASE_NAMES -contains $ph) {
                $ts = @(); for ($i = 0; $i -lt 280; $i++) { $ts += ($i * 6.944).ToString('0.###', $Inv) }
                $lines += ('{{"ev":"frames","phase":"{0}","ts":[{1}]}}' -f $ph, ($ts -join ','))
            }
            if ($ph -eq 'minimized') { $lines += '{"ev":"info","phase":"minimized","raf_count":0,"visibility":"hidden"}' }
            $pt += 2000
        }
        $lines += ('{{"ev":"done","t":{0}}}' -f $pt.ToString('R', $Inv))
    }
    $events = Get-EventsFromLines $lines
    $startEv = $null; $doneEv = $null
    foreach ($e in $events) { if ((Get-Prop $e 'ev') -eq 'start') { $startEv = $e }; if ((Get-Prop $e 'ev') -eq 'done') { $doneEv = $e } }
    $spawn = [double](Get-Prop $startEv 't') - 15.0
    $end = [double](Get-Prop $doneEv 't') + 500.0

    # synthetic process tree: root 1000 -> child 1001 (+ grandchild 1002 appearing later, orphaned at the end)
    $sampler = New-Object Bakeoff.TreeSampler(1000)
    $samples = New-Object 'System.Collections.Generic.List[object]'
    $cpu = @{ 1000 = [long]0; 1001 = [long]0; 1002 = [long]0 }
    $t = $spawn
    $k = 0
    while ($t -lt $end) {
        $snap = New-Object 'System.Collections.Generic.List[Bakeoff.ProcInfo]'
        $mk = {
            param($procId, $ppid, $create, $ws, $priv, $name)
            $p = New-Object Bakeoff.ProcInfo
            $p.Pid = $procId; $p.ParentPid = $ppid; $p.CreateTime = $create; $p.WorkingSet = $ws; $p.PrivateBytes = $priv; $p.Name = $name
            $p.CpuTime100ns = $cpu[$procId]; $p.ThreadIds = [long[]]@(1, 2); $p.ThreadSwitches = [long[]]@(($k * 3), ($k * 2))
            return $p
        }
        $cpu[1000] += 25000; $cpu[1001] += 5000; $cpu[1002] += 1000   # 2.5 ms + 0.5 ms + 0.1 ms per 250 ms tick
        $snap.Add((& $mk 999 1 50 1MB 1MB 'unrelated.exe'))
        if ($k -lt ($end - $spawn - 1500) / 250) { $snap.Add((& $mk 1000 999 100 (80MB) (60MB) 'app.exe')) }
        $snap.Add((& $mk 1001 1000 200 (120MB) (90MB) 'msedgewebview2.exe'))
        if ($k -gt 10) { $snap.Add((& $mk 1002 1001 300 (30MB) (20MB) 'msedgewebview2.exe')) }
        $snap.Add((& $mk 1003 999 400 1MB 1MB 'not-a-child.exe'))
        $s = $sampler.Tick($snap, $t)
        $samples.Add((Convert-Sample $s))
        $t += 250
        $k++
    }
    $treePids = $sampler.KnownPids()
    Write-Info ('Self-test: tree pids discovered = {0} (expect 1000,1001,1002)' -f (($treePids | Sort-Object) -join ','))

    # synthetic PresentMon 2.6 CSV (default metrics + --qpc_time header, verified against the 2.6.0 source)
    $anchor = [Bakeoff.Native]::QpcAnchor()
    $q0 = [double]$anchor[0]; $e0 = [double]$anchor[1]; $freq = [double]$anchor[2]
    $shift = $e0 - $spawn   # pretend the run happened "now"
    $csvPath = Join-Path ([IO.Path]::GetTempPath()) 'bakeoff-selftest-presentmon.csv'
    $sb = New-Object System.Text.StringBuilder
    [void]$sb.AppendLine('Application,ProcessID,SwapChainAddress,PresentRuntime,SyncInterval,PresentFlags,AllowsTearing,PresentMode,TimeInQPC,MsBetweenSimulationStart,MsBetweenPresents,MsBetweenDisplayChange,MsInPresentAPI,MsRenderPresentLatency,MsUntilDisplayed,CPUStartQPC,MsBetweenAppStart,MsCPUBusy,MsCPUWait,MsGPULatency,MsGPUTime,MsGPUBusy,MsGPUWait,MsAnimationError,AnimationTime,MsFlipDelay,MsAllInputToPhotonLatency,MsClickToPhotonLatency')
    $period = 1000.0 / $hz
    $tt = $spawn + 300
    $i = 0
    while ($tt -lt $end) {
        $q = [long]($q0 + ($tt - $spawn) * $freq / 1000.0)
        $until = '4.5000'
        if ($i % 120 -eq 7) { $until = 'NA' }   # dropped frame
        [void]$sb.AppendLine(('msedgewebview2.exe,1002,0x1A2B,Other,1,0,0,Composed: Flip,{0},NA,{1},{1},0.1,1.0,{2},{0},{1},1.0,5.9,0.2,1.5,1.2,0.3,NA,NA,NA,NA,NA' -f $q, $period.ToString('0.0000', $Inv), $until))
        [void]$sb.AppendLine(('dwm.exe,77,0x9,DXGI,1,0,0,Composed: Flip,{0},NA,{1},{1},0.1,1.0,1.0,{0},{1},1.0,5.9,0.2,1.5,1.2,0.3,NA,NA,NA,NA,NA' -f ($q + 10), $period.ToString('0.0000', $Inv)))
        $tt += $period
        if ($i % 97 -eq 50) { $tt += 2 * $period }   # a stutter
        $i++
    }
    [IO.File]::WriteAllText($csvPath, $sb.ToString())
    $f = [Bakeoff.PresentMon]::Read($csvPath, $treePids)
    if ($f.Error) { throw ('PresentMon CSV parse error: ' + $f.Error) }
    Write-Info ('Self-test: PresentMon rows total={0} app={1} dwm median={2} ms' -f $f.TotalRows, $f.Rows.Count, $f.DwmMedianMsBetweenPresents.ToString('0.000', $Inv))
    # PresentMon QPC was generated relative to $spawn at q0, so the anchor must map q0 -> spawn
    $run = [ordered]@{ app = 'selftest'; status = 'done'; ok = $true; sample_columns = $SampleColumns; spawn_ms = $spawn
        raw_jsonl = $lines; events_parsed = $events; samples = $samples.ToArray(); qpc_anchor = @($q0, $spawn, $freq); pm_rows = $f.Rows
        presentmon = [ordered]@{ total_rows = $f.TotalRows; app_rows = $f.Rows.Count } }
    $run.missing = Test-RunEvents $events
    $run.ok = ($run.missing.Count -eq 0)
    if (-not $run.ok) { Write-Warn ('self-test run missing events: ' + ($run.missing -join ', ')) }
    $runs = @($run)
    $result = [ordered]@{
        config = [ordered]@{ apps = @('selftest'); runs = 1; phase_ms = $PhaseMs; sample_ms = 250; settle_ms = $SettleMs; self_test = $true }
        environment = [ordered]@{ platform = 'selftest'; computer = 'selftest'; refresh_hz = $hz; refresh_source = 'self-test'; sampler = 'synthetic'
            os = [Environment]::OSVersion.VersionString; os_build = '-'; cpu = '-'; ram_gb = '-'; gpus = @(); presentmon = [ordered]@{ enabled = $true; version = $PM_VERSION } }
        warnings = @('self-test with synthetic data')
        runs = [ordered]@{ selftest = $runs }
        summary = [ordered]@{}
        sizes = [ordered]@{ selftest = [ordered]@{ folder_mb = 30.5; zip_mb = 12.3 } }
    }
    $result.summary['selftest'] = Get-AppAggregate $runs $hz
    Remove-InternalFields $result
    $stem = Join-Path ([IO.Path]::GetTempPath()) 'bakeoff-selftest'
    $md = Write-Results $result $stem
    Write-Host ''
    Write-Host $md
    # sanity checks
    $sum = $result.summary['selftest']
    $errs = @()
    if (-not $sum.presentmon.Contains('transitions')) { $errs += 'no PresentMon stats for transitions' }
    elseif ($sum.presentmon['transitions'].displayed_median -lt 10) { $errs += 'PresentMon transitions: too few displayed frames' }
    if (-not $sum.frames.Contains('scroll')) { $errs += 'no producer frame stats for scroll' }
    if ($null -eq $sum.phases['idle'].cpu_mean_pct) { $errs += 'no idle CPU' }
    if ($null -eq $sum.phases['idle'].ctxsw_per_s) { $errs += 'no ctxsw rate' }
    if (($treePids | Sort-Object) -join ',' -ne '1000,1001,1002') { $errs += 'tree discovery wrong' }
    $parsed = (Get-Content -LiteralPath ($stem + '.json') -Raw | ConvertFrom-Json)
    if ($null -eq $parsed.summary.selftest.targets) { $errs += 'JSON round-trip failed' }
    if ($errs.Count -gt 0) { Write-Err ('SELF-TEST FAILED: ' + ($errs -join '; ')); exit 1 }
    Write-Host ('SELF-TEST OK: wrote {0}.json and {0}.md' -f $stem) -ForegroundColor Green
    exit 0
}

# ---------------------------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------------------------
if ($SelfTest) { Invoke-SelfTest }

Write-Host ''
Write-Host '=== Speedrunluncher bake-off: Windows harness ===' -ForegroundColor Cyan

# Elevation for PresentMon (ETW needs admin)
if (-not $NoPresentMon -and -not (Test-Admin)) {
    if ($NoElevate) {
        Write-Warn 'not running as administrator, so PresentMon (presentation timing) is disabled. Re-run as admin for full results.'
        $NoPresentMon = $true
    } else {
        Write-Info 'PresentMon needs administrator rights to read GPU presentation events (ETW).'
        Write-Info 'Opening an elevated window; please accept the Windows prompt...'
        $fwd = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', ('"{0}"' -f $PSCommandPath))
        foreach ($kv in $PSBoundParameters.GetEnumerator()) {
            if ($kv.Value -is [System.Management.Automation.SwitchParameter]) {
                if ($kv.Value.IsPresent) { $fwd += ('-' + $kv.Key) }
            } else { $fwd += ('-' + $kv.Key); $fwd += ('"{0}"' -f $kv.Value) }
        }
        try {
            Start-Process -FilePath 'powershell.exe' -Verb RunAs -ArgumentList ($fwd -join ' ') -WorkingDirectory $ScriptRoot | Out-Null
            Write-Info 'The bake-off continues in the new (administrator) window. You can close this one.'
            exit 0
        } catch {
            Write-Warn 'elevation was cancelled or failed.'
            if (Test-Interactive) {
                $ans = Read-Host 'Continue WITHOUT PresentMon (less useful results)? [y/N]'
                if ($ans -notmatch '^[yY]') { Write-Info 'Aborted. Right-click run-bakeoff.cmd -> Run as administrator, or accept the prompt next time.'; Wait-Exit 1 }
            }
            $NoPresentMon = $true
        }
    }
}

$warnings = New-Object 'System.Collections.Generic.List[string]'
$appList = @()
foreach ($a in ($Apps -split ',')) { $a = $a.Trim().ToLower(); if ($a) { $appList += $a } }
$defs = [ordered]@{}
foreach ($a in $appList) {
    switch ($a) {
        'tauri' { $exe = $(if ($TauriExe) { $TauriExe } else { Join-Path $ScriptRoot 'tauri\bakeoff-tauri.exe' }); $defs[$a] = @{ exe = $exe; args = @(); dir = (Split-Path -Parent $exe) } }
        'electron' { $exe = $(if ($ElectronExe) { $ElectronExe } else { Join-Path $ScriptRoot 'electron\bakeoff-electron.exe' }); $defs[$a] = @{ exe = $exe; args = @(); dir = (Split-Path -Parent $exe) } }
        'gpui' { $exe = $(if ($GpuiExe) { $GpuiExe } else { Join-Path $ScriptRoot 'gpui\bakeoff-gpui.exe' }); $defs[$a] = @{ exe = $exe; args = @(); dir = (Split-Path -Parent $exe) } }
        'mock' {
            $mock = Join-Path $ScriptRoot 'mock_app.py'
            if (-not (Test-Path -LiteralPath $mock)) { $mock = Join-Path $ScriptRoot '..\mock_app.py' }
            $py = Get-Command python -ErrorAction SilentlyContinue
            if (-not $py) { $py = Get-Command py -ErrorAction SilentlyContinue }
            if (-not $py) { Write-Err 'mock needs Python on PATH'; Wait-Exit 1 }
            $defs[$a] = @{ exe = $py.Source; args = @(('"{0}"' -f (Resolve-Path $mock).Path)); dir = $ScriptRoot }
        }
        default { Write-Err ("unknown app '{0}' (use tauri, electron, gpui)" -f $a); Wait-Exit 1 }
    }
}
$missingExe = @()
foreach ($a in $defs.Keys) { if (-not (Test-Path -LiteralPath $defs[$a].exe)) { $missingExe += ('{0}: {1}' -f $a, $defs[$a].exe) } }
if ($missingExe.Count -gt 0) {
    Write-Err 'app executables not found. Run this script from the extracted bake-off zip (it needs the tauri\, electron\ and gpui\ folders next to it):'
    foreach ($m in $missingExe) { Write-Host ('  ' + $m) }
    Wait-Exit 1
}

# Files extracted from a downloaded zip carry the 'mark of the web'; clear it so Windows doesn't
# block or prompt for the test executables.
try { Get-ChildItem -LiteralPath $ScriptRoot -Recurse -File -ErrorAction SilentlyContinue | Unblock-File -ErrorAction SilentlyContinue } catch { }

$refresh = Get-RefreshInfo
$envInfo = Get-EnvInfo
$envInfo.refresh_hz = $refresh.hz
$envInfo.refresh_source = $refresh.source
$mode = Select-Sampler
$envInfo.sampler = $mode
$envInfo.presentmon = [ordered]@{ enabled = $false; version = $PM_VERSION; sha256 = $PM_SHA256 }

$pmExe = $null
if (-not $NoPresentMon) {
    try { $pmExe = Get-PresentMonExe; $envInfo.presentmon.enabled = $true }
    catch {
        Write-Warn ('PresentMon unavailable: {0}' -f $_)
        $warnings.Add('PresentMon unavailable: ' + [string]$_)
    }
} else { $warnings.Add('PresentMon disabled: presented-frame metrics missing, producer timestamps only') }

if ($envInfo.battery_present -and -not $envInfo.on_ac) { $warnings.Add('laptop on battery: plug it in for comparable numbers'); Write-Warn 'laptop is on battery; plug it in for comparable numbers' }
$busy = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -match '^(chrome|msedge|firefox|javaw|obs64|steam|Discord|Spotify)$' } | Select-Object -ExpandProperty ProcessName -Unique)
if ($busy.Count -gt 0) { $warnings.Add('other heavy apps running: ' + ($busy -join ', ')); Write-Warn ('please close: ' + ($busy -join ', ') + ' (they skew the numbers)') }

$perRunSec = 5 * $PhaseMs / 1000.0 + 8 + $PauseSec
if ($pmExe) { $perRunSec += 4 }
$estMin = [Math]::Ceiling($perRunSec * $Runs * $defs.Count / 60.0)
Write-Host ''
Write-Host ('Apps: {0} | runs: {1} | phase: {2} ms | refresh: {3} Hz ({4}) | sampler: {5} | PresentMon: {6}' -f ($appList -join ','), $Runs, $PhaseMs, $refresh.hz, $refresh.source, $mode, $(if ($pmExe) { 'on' } else { 'off' }))
Write-Box @(('This takes about {0} minute(s). Keep your hands off the PC:' -f $estMin),
    'no mouse/keyboard, no other windows on top, keep the screen on.',
    'Windows will open and close by themselves; that is expected.') 'Cyan'
if (Test-Interactive) { Start-Sleep -Seconds 5 }

$result = [ordered]@{
    config = [ordered]@{ apps = $appList; runs = $Runs; phase_ms = $PhaseMs; sample_ms = $SampleMs; settle_ms = $SettleMs
        startup_timeout_s = $StartupTimeoutSec; exes = $defs }
    environment = $envInfo
    warnings = $warnings
    runs = [ordered]@{}
    summary = [ordered]@{}
    sizes = $null
}
$stamp = (Get-Date).ToString('yyyyMMdd-HHmmss', $Inv)
$computer = ($env:COMPUTERNAME -replace '[^A-Za-z0-9_.-]', '_')
$stem = Join-Path $OutDir ('results-windows-{0}-{1}' -f $computer, $stamp)
$exitCode = 0
try {
    foreach ($a in $defs.Keys) {
        $result.runs[$a] = @()
        for ($i = 0; $i -lt $Runs; $i++) {
            $r = Invoke-AppRun $a $defs[$a].exe $defs[$a].args $i $Runs $mode $pmExe $defs[$a].dir
            $result.runs[$a] += , $r
            if ($PauseSec -gt 0) { Start-Sleep -Milliseconds ([int]($PauseSec * 1000)) }
        }
        $dwm = @()
        foreach ($r in $result.runs[$a]) { if ($r.presentmon -and $r.presentmon.dwm_median_ms_between_presents) { $dwm += 1000.0 / $r.presentmon.dwm_median_ms_between_presents } }
        if ($dwm.Count -gt 0) { $envInfo.dwm_measured_hz = Round-OrNull (Get-Median $dwm) 2 }
        $result.summary[$a] = Get-AppAggregate $result.runs[$a] $refresh.hz
    }
    if (-not $NoSizeCheck) {
        Write-Info 'Measuring app sizes (zipping each app folder)...'
        $result.sizes = Get-AppSizes $defs
    }
} catch {
    Write-Err ('harness error: {0}' -f $_)
    Write-Host $_.ScriptStackTrace
    $warnings.Add('harness error: ' + [string]$_)
    foreach ($a in @($result.runs.Keys)) { if (-not $result.summary.Contains($a)) { try { $result.summary[$a] = Get-AppAggregate $result.runs[$a] $refresh.hz } catch { } } }
    $exitCode = 3
}

Remove-InternalFields $result
if (-not (Test-Path -LiteralPath $OutDir)) { New-Item -ItemType Directory -Path $OutDir | Out-Null }
$md = Write-Results $result $stem
Write-Host ''
Write-Host $md
foreach ($a in @($result.runs.Keys)) { foreach ($r in $result.runs[$a]) { if (-not $r.ok -and $exitCode -eq 0) { $exitCode = 2 } } }
Write-Host ''
Write-Host ('Summary: {0}.md' -f $stem)
Write-Host ''
Write-Host ('Send this file back: {0}.json' -f $stem) -ForegroundColor Green
Wait-Exit $exitCode
