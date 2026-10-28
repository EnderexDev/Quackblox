# Builds dist\Quackblox.exe (tray app) and build\QuackbloxTest.exe (console, for --test).
$csc = "$env:WINDIR\Microsoft.NET\Framework64\v4.0.30319\csc.exe"
$refs = '-r:System.Windows.Forms.dll', '-r:System.Drawing.dll'
New-Item -ItemType Directory -Force "$PSScriptRoot\dist", "$PSScriptRoot\build" | Out-Null
& $csc -nologo -target:winexe -optimize "-out:$PSScriptRoot\dist\Quackblox.exe" @refs "$PSScriptRoot\src\Quackblox.cs"
& $csc -nologo -target:exe "-out:$PSScriptRoot\build\QuackbloxTest.exe" @refs "$PSScriptRoot\src\Quackblox.cs"
