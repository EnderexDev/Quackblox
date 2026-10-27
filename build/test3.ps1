param([string]$T1, [string]$T2, [switch]$WithUnlock)
function Players { @(Get-Process RobloxPlayerBeta -ErrorAction SilentlyContinue) }
function Join($ticket) {
  $ms = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
  $tracker = Get-Random -Minimum 100000000 -Maximum 999999999
  $pl = [uri]::EscapeDataString("https://www.roblox.com/Game/PlaceLauncher.ashx?request=RequestGame&browserTrackerId=$tracker&placeId=1818&isPlayTogetherGame=false&joinAttemptId=$([guid]::NewGuid())&joinAttemptOrigin=PlayButton")
  Start-Process "roblox-player:1+launchmode:play+gameinfo:$ticket+launchtime:$ms+placelauncherurl:$pl+browsertrackerid:$tracker+robloxLocale:pt_br+gameLocale:pt_br+channel:+LaunchExp:InApp"
}
$hold = $null
if ($WithUnlock) {
  $hold = Start-Process "$PSScriptRoot\RobloxMultiTest.exe" -ArgumentList '--test','150' -PassThru -WindowStyle Hidden -RedirectStandardOutput "$PSScriptRoot\hold.txt"
  Start-Sleep 2
  "unlock: " + (Get-Content "$PSScriptRoot\hold.txt")
}
"antes: " + (Players).Count
Join $T1
Start-Sleep 35
"apos 1o: " + ((Players | % { "$($_.Id) '$($_.MainWindowTitle)'" }) -join ', ')
Join $T2
Start-Sleep 40
"apos 2o: " + ((Players | % { "$($_.Id) '$($_.MainWindowTitle)'" }) -join ', ')
Players | Stop-Process -Force
if ($hold -and -not $hold.HasExited) { Stop-Process -Id $hold.Id -Force }
