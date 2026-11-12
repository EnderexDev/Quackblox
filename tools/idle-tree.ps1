# What the whole process tree of a Quackblox costs while idle: average CPU over -CpuSeconds (the
# exe and its WebView2 apart) and memory (private working set, the Task Manager's "Memory", and
# private commit) of the exe and every process below it. A test build goes through its life
# cycle: started with --minimized (as Windows starts it), then opened (a second launch, as the
# tray does) and left without focus, then closed by its X.
#
# A test build beside the installed app (only one at a time):
#   powershell -File tools\idle-tree.ps1 -Exe C:\bench\step1.exe
# It runs with its own WEBVIEW2_USER_DATA_FOLDER (otherwise its WebView2 joins the installed
# app's processes) and is ended by pid at the end. Build it with another identifier, so it has
# its own data and single-instance lock, and write settings.json in that data folder first with
# autoClose, hotkeys and alertWindows off: with two Roblox open, it must not close one.
#
# The installed app, read only (nothing is opened, closed or clicked):
#   powershell -File tools\idle-tree.ps1 -Id <pid>
param([string]$Exe = "", [int]$Id = 0, [int]$SettleSeconds = 15, [int]$CpuSeconds = 30, [string]$Data = "")
$ErrorActionPreference = "Stop"

Add-Type @"
using System; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class QB {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public static IntPtr MainWindow(uint pid) {
    IntPtr best = IntPtr.Zero; long area = 0;
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid && IsWindowVisible(h)) { RECT r; GetWindowRect(h, out r); long a = (long)(r.R - r.L) * (r.B - r.T); if (a > area) { area = a; best = h; } }
      return true; }, IntPtr.Zero);
    return area >= 10000 ? best : IntPtr.Zero;
  }
}
"@

function Get-Tree([int]$Root) {
    $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name
    $ids = [System.Collections.Generic.List[int]]::new(); $ids.Add($Root)
    for ($i = 0; $i -lt $ids.Count; $i++) {
        # Only WebView2 below the exe: the Roblox clients it opened are its children too.
        foreach ($p in $all) { if ($p.ParentProcessId -eq $ids[$i] -and $p.Name -eq "msedgewebview2.exe" -and -not $ids.Contains([int]$p.ProcessId)) { $ids.Add([int]$p.ProcessId) } }
    }
    $all | Where-Object { $ids.Contains([int]$_.ProcessId) }
}

# CPU over the window, memory at its end. The tree is read again at the end: a WebView2 process
# that ended in the middle still counts its CPU up to then (it is missing from the end only).
function Measure-Tree([int]$Root, [string]$State) {
    $t0 = @{}
    foreach ($p in Get-Tree $Root) { $g = Get-Process -Id $p.ProcessId -ErrorAction SilentlyContinue; if ($g) { $t0[[int]$p.ProcessId] = $g.TotalProcessorTime.TotalSeconds } }
    $sw = [Diagnostics.Stopwatch]::StartNew()
    Start-Sleep -Seconds $CpuSeconds
    $tree = @(Get-Tree $Root)
    $exe = 0.0; $web = 0.0; $commit = 0; $ids = @()
    foreach ($p in $tree) {
        $g = Get-Process -Id $p.ProcessId -ErrorAction SilentlyContinue
        if (-not $g) { continue }
        $ids += [int]$p.ProcessId
        $was = if ($t0.ContainsKey([int]$p.ProcessId)) { $t0[[int]$p.ProcessId] } else { 0 }
        $used = $g.TotalProcessorTime.TotalSeconds - $was
        if ([int]$p.ProcessId -eq $Root) { $exe += $used } else { $web += $used }
        $commit += $g.PrivateMemorySize64
    }
    $elapsed = $sw.Elapsed.TotalSeconds
    $ws = (Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | Where-Object { $ids -contains [int]$_.IDProcess } | Measure-Object WorkingSetPrivate -Sum).Sum
    $fg = [QB]::GetForegroundWindow(); $fgPid = 0; [void][QB]::GetWindowThreadProcessId($fg, [ref]$fgPid)
    [pscustomobject]@{
        state = $State
        cpuCores = [math]::Round(($exe + $web) / $elapsed, 3)
        exeCores = [math]::Round($exe / $elapsed, 3)
        webviewCores = [math]::Round($web / $elapsed, 3)
        privateWsMB = [math]::Round($ws / 1MB, 1)
        commitMB = [math]::Round($commit / 1MB, 1)
        webview2 = @($tree | Where-Object Name -eq "msedgewebview2.exe").Count
        window = ([QB]::MainWindow([uint32]$Root) -ne [IntPtr]::Zero)
        focused = ($fgPid -eq $Root)
        seconds = [math]::Round($elapsed, 1)
    }
}

if ($Id) {
    $rows = @(Measure-Tree $Id "installed, as it is")
    $rows | Format-Table -AutoSize | Out-String -Width 220
    return
}

if (-not $Exe) { throw "-Exe or -Id" }
$exePath = (Resolve-Path $Exe).Path
if (-not $Data) { $Data = Join-Path $env:TEMP ("qb-bench-webview-" + [IO.Path]::GetFileNameWithoutExtension($exePath)) }
function Start-Exe([string]$Arguments) {
    $psi = New-Object Diagnostics.ProcessStartInfo $exePath
    $psi.UseShellExecute = $false
    $psi.Arguments = $Arguments
    $psi.Environment["WEBVIEW2_USER_DATA_FOLDER"] = $Data
    [Diagnostics.Process]::Start($psi)
}
$before = [QB]::GetForegroundWindow()
$p = Start-Exe "--minimized"
try {
    Start-Sleep -Seconds $SettleSeconds
    $rows = @(Measure-Tree $p.Id "started hidden")
    # A second launch only shows the first one's window (single instance), as the tray click does.
    $sw = [Diagnostics.Stopwatch]::StartNew()
    [void](Start-Exe "")
    $h = [IntPtr]::Zero
    while ($sw.ElapsedMilliseconds -lt 20000 -and $h -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 50; $h = [QB]::MainWindow([uint32]$p.Id) }
    if ($h -eq [IntPtr]::Zero) { throw "no window in 20 s" }
    [Console]::Error.WriteLine("window after $($sw.ElapsedMilliseconds) ms")
    # Open without focus: the focus goes back to the window that had it.
    Start-Sleep -Seconds 2
    [void][QB]::SetForegroundWindow($before)
    Start-Sleep -Seconds $SettleSeconds
    $rows += Measure-Tree $p.Id "open, unfocused"
    # Closed: the window's own close (WM_CLOSE), as the X does.
    [void][QB]::PostMessage($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
    Start-Sleep -Seconds $SettleSeconds
    $rows += Measure-Tree $p.Id "closed"
    $rows | Format-Table -AutoSize | Out-String -Width 220
} finally {
    foreach ($q in @(Get-Tree $p.Id)) { Stop-Process -Id $q.ProcessId -Force -ErrorAction SilentlyContinue }
}
