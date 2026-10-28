# Test only: where Roblox says these users are right now (0 offline, 1 site, 2 in game, 3 Studio).
# Asks with the Roblox app's own login, so friends' and own game details come back.
param([string]$Ids)
Add-Type -AssemblyName System.Security
Add-Type -AssemblyName System.Net.Http
$json = Get-Content "$env:LOCALAPPDATA\Roblox\LocalStorage\RobloxCookies.dat" -Raw | ConvertFrom-Json
$plain = [Text.Encoding]::UTF8.GetString([Security.Cryptography.ProtectedData]::Unprotect([Convert]::FromBase64String($json.CookiesData), $null, 'CurrentUser'))
$cookie = [regex]::Match($plain, '\.ROBLOSECURITY\t([^;\s]+)').Groups[1].Value
$h = New-Object System.Net.Http.HttpClientHandler; $h.UseCookies = $false
$c = New-Object System.Net.Http.HttpClient($h)
$body = '{"userIds":[' + $Ids + ']}'
$req = New-Object System.Net.Http.HttpRequestMessage('POST', 'https://presence.roblox.com/v1/presence/users')
$req.Headers.TryAddWithoutValidation('Cookie', ".ROBLOSECURITY=$cookie") | Out-Null
$req.Content = New-Object System.Net.Http.StringContent($body, [Text.Encoding]::UTF8, 'application/json')
$res = $c.SendAsync($req).Result
($res.Content.ReadAsStringAsync().Result | ConvertFrom-Json).userPresences | % { "$($_.userId): type=$($_.userPresenceType) place=$($_.placeId) loc=$($_.lastLocation)" }
