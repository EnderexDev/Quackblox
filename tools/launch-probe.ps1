# Test only: opens a Roblox client as a saved account of the Dev with a hand-built PlaceLauncher
# request, the way accounts.rs does, to try a way of joining before the app has it. Prints the
# client's pid; the ticket and the login are never printed.
#   tools/launch-probe.ps1 -As EnderexDev -Request "RequestFollowUser&userId=123"
#   tools/launch-probe.ps1 -As Enderexx_1 -Request "RequestPrivateGame&placeId=1&linkCode=abc"
# The Roblox processes left in the tray are ended first, as play does.
param([string]$As, [string]$Request)
Add-Type -AssemblyName System.Security
Add-Type -AssemblyName System.Net.Http
$list = Get-Content "$env:APPDATA\dev.enderex.quackblox.dev\accounts.json" -Raw | ConvertFrom-Json
$acc = $list | ? { $_.name -eq $As } | select -First 1
if (!$acc) { throw "no saved account named $As" }
$cookie = [Text.Encoding]::UTF8.GetString([Security.Cryptography.ProtectedData]::Unprotect([Convert]::FromBase64String($acc.login), $null, 'CurrentUser'))
$h = New-Object System.Net.Http.HttpClientHandler; $h.UseCookies = $false
$c = New-Object System.Net.Http.HttpClient($h)
function Send($csrf) {
  $req = New-Object System.Net.Http.HttpRequestMessage('POST', 'https://auth.roblox.com/v1/authentication-ticket')
  $req.Headers.TryAddWithoutValidation('Cookie', ".ROBLOSECURITY=$cookie") | Out-Null
  $req.Headers.TryAddWithoutValidation('Referer', 'https://www.roblox.com/') | Out-Null
  $req.Headers.TryAddWithoutValidation('Origin', 'https://www.roblox.com') | Out-Null
  if ($csrf) { $req.Headers.TryAddWithoutValidation('x-csrf-token', $csrf) | Out-Null }
  $req.Content = New-Object System.Net.Http.StringContent('{}', [Text.Encoding]::UTF8, 'application/json')
  $c.SendAsync($req).Result
}
$res = Send $null
if ($res.StatusCode -eq 403) { $res = Send ($res.Headers.GetValues('x-csrf-token') | select -First 1) }
if (!$res.Headers.Contains('rbx-authentication-ticket')) { throw "no ticket (HTTP $([int]$res.StatusCode))" }
$ticket = $res.Headers.GetValues('rbx-authentication-ticket') | select -First 1

# Tray players: RobloxPlayerBeta with no window.
Get-Process RobloxPlayerBeta -ErrorAction SilentlyContinue | ? { $_.MainWindowHandle -eq 0 } | % { "ending tray player $($_.Id)"; $_.Kill() }

$now = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$tracker = ($now % 900000000) + 100000000
$launcher = "https://www.roblox.com/Game/PlaceLauncher.ashx?request=$Request&browserTrackerId=$tracker&isPlayTogetherGame=false&joinAttemptId=$([guid]::NewGuid())&joinAttemptOrigin=PlayButton"
$link = "roblox-player:1+launchmode:play+gameinfo:$ticket+launchtime:$now+placelauncherurl:$([Uri]::EscapeDataString($launcher))+browsertrackerid:$tracker+robloxLocale:pt_br+gameLocale:pt_br+channel:+LaunchExp:InApp"
$before = @(Get-Process RobloxPlayerBeta -ErrorAction SilentlyContinue | % Id)
Start-Process $link
Start-Sleep 4
$new = Get-Process RobloxPlayerBeta -ErrorAction SilentlyContinue | ? { $before -notcontains $_.Id }
"opened: $(($new | % Id) -join ',')"
