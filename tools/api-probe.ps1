# Test only: calls a Roblox web API as one of the Dev's saved accounts (or the Roblox app's login)
# and prints the status and the body. The login is decrypted in memory and never printed.
#   tools/api-probe.ps1 -As Enderexx_1 -Url https://presence.roblox.com/v1/presence/users -Body '{"userIds":[1]}'
#   tools/api-probe.ps1 -As app -Url https://games.roblox.com/v1/games/1818/servers/Public?limit=10
#   -As none: no login at all.
param([string]$As = "app", [string]$Url, [string]$Body, [string]$Method)
Add-Type -AssemblyName System.Security
Add-Type -AssemblyName System.Net.Http
$cookie = $null
if ($As -eq "app") {
  $json = Get-Content "$env:LOCALAPPDATA\Roblox\LocalStorage\RobloxCookies.dat" -Raw | ConvertFrom-Json
  $plain = [Text.Encoding]::UTF8.GetString([Security.Cryptography.ProtectedData]::Unprotect([Convert]::FromBase64String($json.CookiesData), $null, 'CurrentUser'))
  $cookie = [regex]::Match($plain, '\.ROBLOSECURITY\t([^;\s]+)').Groups[1].Value
} elseif ($As -ne "none") {
  $list = Get-Content "$env:APPDATA\dev.enderex.quackblox.dev\accounts.json" -Raw | ConvertFrom-Json
  $acc = $list | ? { $_.name -eq $As } | select -First 1
  if (!$acc) { throw "no saved account named $As" }
  $cookie = [Text.Encoding]::UTF8.GetString([Security.Cryptography.ProtectedData]::Unprotect([Convert]::FromBase64String($acc.login), $null, 'CurrentUser'))
}
$h = New-Object System.Net.Http.HttpClientHandler; $h.UseCookies = $false
$c = New-Object System.Net.Http.HttpClient($h)
if (!$Method) { $Method = if ($Body) { 'POST' } else { 'GET' } }
function Send($csrf) {
  $req = New-Object System.Net.Http.HttpRequestMessage($Method, $Url)
  if ($cookie) { $req.Headers.TryAddWithoutValidation('Cookie', ".ROBLOSECURITY=$cookie") | Out-Null }
  if ($csrf) { $req.Headers.TryAddWithoutValidation('x-csrf-token', $csrf) | Out-Null }
  if ($Body) { $req.Content = New-Object System.Net.Http.StringContent($Body, [Text.Encoding]::UTF8, 'application/json') }
  $c.SendAsync($req).Result
}
$res = Send $null
if ($res.StatusCode -eq 403 -and $res.Headers.Contains('x-csrf-token')) { $res = Send ($res.Headers.GetValues('x-csrf-token') | select -First 1) }
"HTTP $([int]$res.StatusCode)"
$res.Content.ReadAsStringAsync().Result
