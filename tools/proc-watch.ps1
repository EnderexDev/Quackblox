# Test only: logs every Roblox player process that appears for <seconds>, with its parent, its
# command line (the one-use ticket cut out) and when its window shows up and gets a title.
# Usage: powershell -File tools/proc-watch.ps1 <seconds>
param([int]$Seconds = 40)
$t0 = Get-Date
$ms = { [int]((Get-Date) - $t0).TotalMilliseconds }
$known = @{}
foreach ($p in Get-Process -ErrorAction SilentlyContinue) { $known[$p.Id] = $true }
$watch = @{}
# Roblox players already running (the tray's) are watched too: one may get the game.
foreach ($p in Get-Process RobloxPlayerBeta -ErrorAction SilentlyContinue) {
  $watch[$p.Id] = @{ win = ($p.MainWindowHandle -ne 0); title = $p.MainWindowTitle }
  "      already pid {0} window={1} title '{2}'" -f $p.Id, ($p.MainWindowHandle -ne 0), $p.MainWindowTitle
}
"ready"
while (((Get-Date) - $t0).TotalSeconds -lt $Seconds) {
  foreach ($p in Get-Process -ErrorAction SilentlyContinue) {
    if ($known.ContainsKey($p.Id)) { continue }
    $known[$p.Id] = $true
    if ($p.ProcessName -notmatch 'Roblox') { continue }
    $c = Get-CimInstance Win32_Process -Filter "ProcessId=$($p.Id)" -ErrorAction SilentlyContinue
    $parent = Get-CimInstance Win32_Process -Filter "ProcessId=$($c.ParentProcessId)" -ErrorAction SilentlyContinue
    $cmd = ($c.CommandLine -replace 'gameinfo:[^+]+', 'gameinfo:<ticket>')
    "{0,6} ms  NEW pid {1} {2}  parent {3} ({4})" -f (& $ms), $p.Id, $p.ProcessName, $c.ParentProcessId, $parent.Name
    "          cmd: $cmd"
    $watch[$p.Id] = @{ win = $false; title = '' }
  }
  foreach ($id in @($watch.Keys)) {
    $p = Get-Process -Id $id -ErrorAction SilentlyContinue
    $w = $watch[$id]
    if (-not $p) { if (-not $w.gone) { "{0,6} ms  pid {1} EXITED" -f (& $ms), $id; $w.gone = $true }; continue }
    if (-not $w.win -and $p.MainWindowHandle -ne 0) { $w.win = $true; "{0,6} ms  pid {1} window 0x{2:X}" -f (& $ms), $id, [int64]$p.MainWindowHandle }
    if ($p.MainWindowTitle -ne $w.title) { $w.title = $p.MainWindowTitle; "{0,6} ms  pid {1} title '{2}'" -f (& $ms), $id, $p.MainWindowTitle }
  }
  Start-Sleep -Milliseconds 100
}
