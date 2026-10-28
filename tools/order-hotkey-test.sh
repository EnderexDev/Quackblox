#!/usr/bin/env bash
# Test only: drags saved card 1 to the first place, opens a client for both saved accounts, then
# presses Ctrl+Shift+1 and Ctrl+Shift+2 and prints which account's window came forward each time.
# Usage: tools/order-hotkey-test.sh
cd "$(dirname "$0")/.."
ev() { node tools/cdp.mjs eval "$1"; }
node tools/drag.mjs 1 0 >/dev/null
order=$(ev "window.__TAURI_INTERNALS__.invoke('accounts_list').then(l=>l.map(a=>a.id).join(' '))")
echo "order: $(ev "window.__TAURI_INTERNALS__.invoke('accounts_list').then(l=>l.map(a=>a.name).join(' | '))")"
for acc in $order; do
  ev "window.__TAURI_INTERNALS__.invoke('play',{account:'$acc',game:'1818'}).then(JSON.stringify,String)" >/dev/null
  sleep 4
done
until [ "$(ev "window.__TAURI_INTERNALS__.invoke('clients_get').then(s=>s.clients.filter(c=>c.window).length)")" = "2" ]; do sleep 2; done
sleep 3
front_account() {
  pid=$(powershell -NoProfile -Command "Add-Type -Name F -Namespace X -MemberDefinition '[DllImport(\"user32.dll\")] public static extern IntPtr GetForegroundWindow(); [DllImport(\"user32.dll\")] public static extern int GetWindowThreadProcessId(IntPtr h, out int p);'; \$p=0; [X.F]::GetWindowThreadProcessId([X.F]::GetForegroundWindow(), [ref]\$p) | Out-Null; \$p")
  ev "window.__TAURI_INTERNALS__.invoke('clients_get').then(s=>(s.clients.find(c=>c.pid===$pid)||{account:'other $pid'}).account)"
}
for k in 1 2 1 2; do
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/keys.ps1 "$k"
  sleep 1
  echo "Ctrl+Shift+$k -> $(front_account)"
done
