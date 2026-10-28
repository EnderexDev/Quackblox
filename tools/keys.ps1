# Test only: presses Ctrl+Shift+<digit> through SendInput, as the keyboard would, so the global
# hotkeys fire. Usage: powershell -File tools/keys.ps1 <digit>
param([int]$Digit)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class K {
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
}
"@
$up = 2
[K]::keybd_event(0x11, 0, 0, [UIntPtr]::Zero)   # Ctrl
[K]::keybd_event(0x10, 0, 0, [UIntPtr]::Zero)   # Shift
[K]::keybd_event([byte](0x30 + $Digit), 0, 0, [UIntPtr]::Zero)
[K]::keybd_event([byte](0x30 + $Digit), 0, $up, [UIntPtr]::Zero)
[K]::keybd_event(0x10, 0, $up, [UIntPtr]::Zero)
[K]::keybd_event(0x11, 0, $up, [UIntPtr]::Zero)
