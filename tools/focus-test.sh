#!/usr/bin/env bash
# Test only: presses Ctrl+Shift+<key> for each key given and prints which process is in front
# after each one. Usage: tools/focus-test.sh 1 2 1 2
cd "$(dirname "$0")/.."
front() {
  powershell -NoProfile -Command "Add-Type -Name F -Namespace X -MemberDefinition '[DllImport(\"user32.dll\")] public static extern IntPtr GetForegroundWindow(); [DllImport(\"user32.dll\")] public static extern int GetWindowThreadProcessId(IntPtr h, out int p);'; \$p=0; [X.F]::GetWindowThreadProcessId([X.F]::GetForegroundWindow(), [ref]\$p) | Out-Null; (Get-Process -Id \$p).ProcessName + ' ' + \$p"
}
echo "start: $(front)"
for k in "$@"; do
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/keys.ps1 "$k"
  sleep 1
  echo "Ctrl+Shift+$k -> $(front)"
done
