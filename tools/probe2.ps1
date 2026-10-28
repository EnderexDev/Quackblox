Add-Type -AssemblyName System.Security
Add-Type -AssemblyName System.Net.Http
$json = Get-Content "$env:LOCALAPPDATA\Roblox\LocalStorage\RobloxCookies.dat" -Raw | ConvertFrom-Json
$plain = [Text.Encoding]::UTF8.GetString([Security.Cryptography.ProtectedData]::Unprotect([Convert]::FromBase64String($json.CookiesData), $null, 'CurrentUser'))
$v = [regex]::Match($plain, '\.ROBLOSECURITY\t([^;\s]+)').Groups[1].Value
"len=$($v.Length) startsWarn=$($v.StartsWith('_|WARNING')) hasPipeUnderscore=$($v.Contains('|_'))"
$h = New-Object System.Net.Http.HttpClientHandler; $h.UseCookies = $false
$c = New-Object System.Net.Http.HttpClient($h)
$req = New-Object System.Net.Http.HttpRequestMessage('GET', 'https://users.roblox.com/v1/users/authenticated')
$req.Headers.TryAddWithoutValidation('Cookie', ".ROBLOSECURITY=$v") | Out-Null
$res = $c.SendAsync($req).Result
"$([int]$res.StatusCode) $($res.Content.ReadAsStringAsync().Result)"
