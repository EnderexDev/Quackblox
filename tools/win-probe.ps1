# Test only: on a Roblox player's main window, sets a title and Eco (idle priority plus
# EcoQoS power throttling), then reads them back for <seconds> to see whether Roblox undoes them.
# Usage: powershell -File tools/win-probe.ps1 <pid> <title> <seconds> [-ReadOnly]
# With -ReadOnly it changes nothing and only reads, once.
param([int]$ProcessId, [string]$Title = "Quackblox teste", [int]$Seconds = 20, [switch]$ReadOnly)
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class W {
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool SetWindowText(IntPtr h, string s);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [StructLayout(LayoutKind.Sequential)] public struct PPT { public uint Version, ControlMask, StateMask; }
  [DllImport("kernel32.dll")] public static extern IntPtr OpenProcess(uint a, bool i, int pid);
  [DllImport("kernel32.dll")] public static extern bool SetProcessInformation(IntPtr h, int c, ref PPT p, int n);
  [DllImport("kernel32.dll")] public static extern bool GetProcessInformation(IntPtr h, int c, ref PPT p, int n);
  [DllImport("kernel32.dll")] public static extern uint GetPriorityClass(IntPtr h);
  [DllImport("kernel32.dll")] public static extern bool SetPriorityClass(IntPtr h, uint c);
}
"@
$p = Get-Process -Id $ProcessId
$h = $p.MainWindowHandle
"window 0x{0:X}" -f [int64]$h
$ph = [W]::OpenProcess(0x0200 -bor 0x1000, $false, $ProcessId) # SET_INFORMATION | QUERY_LIMITED
if ($ReadOnly) { $Seconds = 1 } else {
  [W]::SetWindowText($h, $Title) | Out-Null
  $ppt = New-Object W+PPT; $ppt.Version = 1; $ppt.ControlMask = 1; $ppt.StateMask = 1
  "eco set: " + [W]::SetProcessInformation($ph, 4, [ref]$ppt, 12) + " idle: " + [W]::SetPriorityClass($ph, 0x40)
}
for ($i = 0; $i -lt $Seconds; $i++) {
  $sb = New-Object System.Text.StringBuilder 256
  [W]::GetWindowText($h, $sb, 256) | Out-Null
  $q = New-Object W+PPT; $q.Version = 1
  [W]::GetProcessInformation($ph, 4, [ref]$q, 12) | Out-Null
  "{0,2}s title '{1}' priority 0x{2:X} throttle {3}/{4}" -f $i, $sb, [W]::GetPriorityClass($ph), $q.ControlMask, $q.StateMask
  Start-Sleep 1
}
