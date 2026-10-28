# Test only: writes the Roblox app's .ROBLOSECURITY to stdout, for tools/inject-login.mjs. Never echo it.
Add-Type -AssemblyName System.Security
$json = Get-Content "$env:LOCALAPPDATA\Roblox\LocalStorage\RobloxCookies.dat" -Raw | ConvertFrom-Json
$plain = [Text.Encoding]::UTF8.GetString([Security.Cryptography.ProtectedData]::Unprotect([Convert]::FromBase64String($json.CookiesData), $null, 'CurrentUser'))
[Console]::Out.Write([regex]::Match($plain, '\.ROBLOSECURITY\t([^;\s]+)').Groups[1].Value)
