param([switch]$WithUnlock, [string]$Url = 'roblox://experiences/start?placeId=1818')
function Players { @(Get-Process RobloxPlayerBeta -ErrorAction SilentlyContinue) }
"antes: " + (Players).Count
$hold = $null
if ($WithUnlock) {
  $hold = Start-Process "$PSScriptRoot\RobloxMultiTest.exe" -ArgumentList '--test','120' -PassThru -WindowStyle Hidden -RedirectStandardOutput "$PSScriptRoot\hold.txt"
  Start-Sleep 2
  "unlock: " + (Get-Content "$PSScriptRoot\hold.txt")
}
Start-Process $Url
Start-Sleep 30
"apos 1o jogo: " + ((Players | % { "$($_.Id) '$($_.MainWindowTitle)'" }) -join ', ')
Start-Process $Url
Start-Sleep 30
"apos 2o jogo: " + ((Players | % { "$($_.Id) '$($_.MainWindowTitle)'" }) -join ', ')
Get-CimInstance Win32_Process -Filter "Name='RobloxPlayerBeta.exe'" | % { "$($_.ProcessId) <- $($_.ParentProcessId): " + $_.CommandLine.Substring(0, [Math]::Min(160, $_.CommandLine.Length)) }
Players | Stop-Process -Force
if ($hold -and -not $hold.HasExited) { Stop-Process -Id $hold.Id -Force }
