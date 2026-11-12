# What the app's WebView2 costs while idle: CPU per WebView2 process (by --type=) and the 3D
# GPU engine use of those processes, over a few seconds. Run it with the window open and
# without focus. Usage: powershell -File tools\idle-cost.ps1 [-Exe Quackblox] [-Seconds 10]
param([string]$Exe = "Quackblox", [int]$Seconds = 10)

Add-Type @"
using System; using System.Runtime.InteropServices;
public static class W {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
}
"@

$app = Get-Process -Name $Exe -ErrorAction Stop | Select-Object -First 1
$all = Get-CimInstance Win32_Process
# Every process below the app (WebView2's browser process, then its gpu and renderers).
$tree = @{}; $tree[[uint32]$app.Id] = $true
do {
  $grew = $false
  foreach ($p in $all) {
    if ($tree.ContainsKey([uint32]$p.ParentProcessId) -and -not $tree.ContainsKey([uint32]$p.ProcessId)) { $tree[[uint32]$p.ProcessId] = $true; $grew = $true }
  }
} while ($grew)
$web = $all | Where-Object { $_.Name -eq "msedgewebview2.exe" -and $tree.ContainsKey([uint32]$_.ProcessId) }
$kind = @{}
foreach ($p in $web) {
  $t = if ($p.CommandLine -match "--type=([\w-]+)") { $Matches[1] } else { "browser" }
  if ($p.CommandLine -match "--utility-sub-type=([\w.]+)") { $t = "utility:" + ($Matches[1] -split "\.")[-1] }
  $kind[[int]$p.ProcessId] = $t
}

$fg = [W]::GetForegroundWindow(); $fgPid = 0; [void][W]::GetWindowThreadProcessId($fg, [ref]$fgPid)
"app pid $($app.Id); minimized: $([W]::IsIconic($app.MainWindowHandle)); focused: $($fgPid -eq $app.Id)"

$ids = @($kind.Keys)
$before = @{}; foreach ($id in $ids) { $before[$id] = (Get-Process -Id $id).TotalProcessorTime.TotalSeconds }
$clock = [Diagnostics.Stopwatch]::StartNew()
# One sample per second of the 3D engine of every process, summed per pid.
$gpu = @{}
$samples = Get-Counter '\GPU Engine(*engtype_3D)\Utilization Percentage' -SampleInterval 1 -MaxSamples $Seconds
$elapsed = $clock.Elapsed.TotalSeconds
foreach ($s in $samples) {
  foreach ($c in $s.CounterSamples) {
    if ($c.InstanceName -match "^pid_(\d+)_") { $id = [int]$Matches[1]; if ($kind.ContainsKey($id)) { $gpu[$id] = $gpu[$id] + $c.CookedValue } }
  }
}
$rows = foreach ($id in $ids) {
  $cpu = ((Get-Process -Id $id).TotalProcessorTime.TotalSeconds - $before[$id]) / $elapsed
  [pscustomobject]@{ pid = $id; type = $kind[$id]; cores = [math]::Round($cpu, 3); gpu3d = [math]::Round(($gpu[$id] / $samples.Count), 1) }
}
$rows | Sort-Object cores -Descending | Format-Table -AutoSize | Out-String
"total: {0:N3} cores, {1:N1}% GPU 3D, over {2:N1} s" -f ($rows | Measure-Object cores -Sum).Sum, ($rows | Measure-Object gpu3d -Sum).Sum, $elapsed
