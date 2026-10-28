# Feasibility probe: can the Roblox app's own saved login be read (DPAPI, current user)?
# Prints only the account name it belongs to, never the cookie.
# RobloxCookies.dat = {CookiesVersion, CookiesData: base64(DPAPI(netscape-style lines joined by "; "))}
Add-Type -AssemblyName System.Security
$file = "$env:LOCALAPPDATA\Roblox\LocalStorage\RobloxCookies.dat"
$json = Get-Content $file -Raw | ConvertFrom-Json
$bytes = [Convert]::FromBase64String($json.CookiesData)
$plain = [Text.Encoding]::UTF8.GetString([Security.Cryptography.ProtectedData]::Unprotect($bytes, $null, 'CurrentUser'))
$m = [regex]::Match($plain, '\.ROBLOSECURITY\t([^;\s]+)')
if (-not $m.Success) { "no .ROBLOSECURITY in the app's login"; return }
$r = Invoke-WebRequest 'https://users.roblox.com/v1/users/authenticated' -Headers @{ Cookie = ".ROBLOSECURITY=$($m.Groups[1].Value)" } -UseBasicParsing
$u = $r.Content | ConvertFrom-Json
"app account: $($u.name) ($($u.id))"
