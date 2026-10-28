# Test only: each Roblox player window's visible frame (DWM bounds), state and title, plus the
# window in the foreground. Usage: powershell -File tools/win-rects.ps1
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class R {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, Rt, B; }
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out RECT r, int n);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
}
"@
$fg = [R]::GetForegroundWindow()
foreach ($p in Get-Process RobloxPlayerBeta -ErrorAction SilentlyContinue) {
  $h = $p.MainWindowHandle
  $r = New-Object R+RECT
  [R]::DwmGetWindowAttribute($h, 9, [ref]$r, 16) | Out-Null
  "pid {0} {1}x{2} at ({3},{4}) min={5} front={6}" -f $p.Id, ($r.Rt - $r.L), ($r.B - $r.T), $r.L, $r.T, [R]::IsIconic($h), ($h -eq $fg)
}
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.Screen]::AllScreens | % { "screen {0} work {1}" -f $_.DeviceName, $_.WorkingArea }
