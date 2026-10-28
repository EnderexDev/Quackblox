# Test only: how many calls in a row a Roblox endpoint takes before 429, with a Dev account's login
# (never printed) or none. Prints one status per call and the gap between them.
#   tools/rate-probe.ps1 -As EnderexDev -Url "https://games.roblox.com/v1/games/1818/servers/Public?limit=100" -N 10 -GapMs 0
param([string]$As = "none", [string]$Url, [string]$Body, [int]$N = 10, [int]$GapMs = 0)
Add-Type -AssemblyName System.Security
Add-Type -AssemblyName System.Net.Http
$cookie = $null
if ($As -ne "none") {
  $list = Get-Content "$env:APPDATA\dev.enderex.quackblox.dev\accounts.json" -Raw | ConvertFrom-Json
  $acc = $list | ? { $_.name -eq $As } | select -First 1
  $cookie = [Text.Encoding]::UTF8.GetString([Security.Cryptography.ProtectedData]::Unprotect([Convert]::FromBase64String($acc.login), $null, 'CurrentUser'))
}
$h = New-Object System.Net.Http.HttpClientHandler; $h.UseCookies = $false
$c = New-Object System.Net.Http.HttpClient($h)
$out = @()
for ($i = 0; $i -lt $N; $i++) {
  $req = New-Object System.Net.Http.HttpRequestMessage($(if ($Body) { 'POST' } else { 'GET' }), $Url)
  if ($cookie) { $req.Headers.TryAddWithoutValidation('Cookie', ".ROBLOSECURITY=$cookie") | Out-Null }
  if ($Body) { $req.Content = New-Object System.Net.Http.StringContent($Body, [Text.Encoding]::UTF8, 'application/json') }
  $out += [int]$c.SendAsync($req).Result.StatusCode
  if ($GapMs) { Start-Sleep -Milliseconds $GapMs }
}
"$($out -join ' ')"
