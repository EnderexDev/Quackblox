param([switch]$WithUnlock)
$ErrorActionPreference = 'Stop'
$cmd = (Get-ItemProperty 'Registry::HKEY_CLASSES_ROOT\roblox-player\shell\open\command').'(default)'
$player = ($cmd -split '"')[1]
"player: $player"
"antes: " + @(Get-Process RobloxPlayerBeta -ErrorAction SilentlyContinue).Count

$hold = $null
if ($WithUnlock) {
  $hold = Start-Process "$PSScriptRoot\QuackbloxTest.exe" -ArgumentList '--test','90' -PassThru -WindowStyle Hidden -RedirectStandardOutput "$PSScriptRoot\hold.txt"
  Start-Sleep 2
  "unlock: " + (Get-Content "$PSScriptRoot\hold.txt")
}

$a = Start-Process $player -WorkingDirectory (Split-Path $player) -PassThru
Start-Sleep 15
"A vivo apos 15s: " + (-not $a.HasExited) + " pid " + $a.Id
$b = Start-Process $player -WorkingDirectory (Split-Path $player) -PassThru
Start-Sleep 20
"A vivo apos abrir B: " + (-not $a.HasExited)
"B vivo: " + (-not $b.HasExited) + " pid " + $b.Id
Get-Process RobloxPlayerBeta -ErrorAction SilentlyContinue | Select Id, MainWindowTitle, StartTime | Format-Table | Out-String

foreach ($p in @($a, $b)) { if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force } }
if ($hold -and -not $hold.HasExited) { Stop-Process -Id $hold.Id -Force }
Start-Sleep 2
"depois: " + @(Get-Process RobloxPlayerBeta -ErrorAction SilentlyContinue).Count
