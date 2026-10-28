# Test only: says whether Ctrl+Shift+<digit> is free (another program, like Quackblox, does not
# hold it) by registering it for a moment. Usage: powershell -File tools/hotkey-free.ps1 <digit>
param([int]$Digit = 0)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class H {
  [DllImport("user32.dll")] public static extern bool RegisterHotKey(IntPtr h, int id, uint mods, uint vk);
  [DllImport("user32.dll")] public static extern bool UnregisterHotKey(IntPtr h, int id);
}
"@
$free = [H]::RegisterHotKey([IntPtr]::Zero, 77, 0x2 -bor 0x4, [uint32](0x30 + $Digit)) # Ctrl | Shift
if ($free) { [H]::UnregisterHotKey([IntPtr]::Zero, 77) | Out-Null }
"Ctrl+Shift+$Digit free: $free"
