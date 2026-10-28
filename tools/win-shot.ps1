# Test only: a picture of one process's main window, drawn by the window itself (PrintWindow),
# so it works behind other windows.  tools/win-shot.ps1 -ProcessId 123 -Out shot.png
param([int]$ProcessId, [string]$Out)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class S {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
}
"@
$h = (Get-Process -Id $ProcessId).MainWindowHandle
$r = New-Object S+RECT
[S]::GetWindowRect($h, [ref]$r) | Out-Null
$bmp = New-Object Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
$g = [Drawing.Graphics]::FromImage($bmp)
$dc = $g.GetHdc()
[S]::PrintWindow($h, $dc, 2) | Out-Null
$g.ReleaseHdc($dc)
$bmp.Save($Out)
"saved $Out"
