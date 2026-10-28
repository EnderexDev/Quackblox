#!/usr/bin/env bash
# Test only: opens a client of a saved account through the Dev app, turns Eco on, closes the
# window as a click on X would, and prints the priority of every player left (the tray one the
# client leaves behind should be back to Normal). Usage: tools/eco-close-test.sh <account id>
cd "$(dirname "$0")/.."
acc=$1
ev() { node tools/cdp.mjs eval "$1"; }
ev "window.__TAURI_INTERNALS__.invoke('play',{account:'$acc',game:'1818'}).then(JSON.stringify,String)"
until [ "$(ev "window.__TAURI_INTERNALS__.invoke('clients_get').then(s=>s.clients.filter(c=>c.account==='$acc'&&c.window).length)")" = "1" ]; do sleep 2; done
pid=$(ev "window.__TAURI_INTERNALS__.invoke('clients_set_eco',{account:'$acc',on:true}).then(s=>s.clients.find(c=>c.account==='$acc').pid)")
echo "client $pid: $(powershell -NoProfile -Command "(Get-Process -Id $pid).PriorityClass")"
# A window still loading ignores the close; it is asked again until the process ends.
until ! powershell -NoProfile -Command "if (Get-Process -Id $pid -EA SilentlyContinue) {exit 0} else {exit 1}"; do
  sleep 4
  powershell -NoProfile -Command "\$p = Get-Process -Id $pid -EA SilentlyContinue; if (\$p) { \$p.CloseMainWindow() | Out-Null }"
done
sleep 3
powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name='RobloxPlayerBeta.exe'\" | % { '{0} parent {1} {2}' -f \$_.ProcessId, \$_.ParentProcessId, (Get-Process -Id \$_.ProcessId).PriorityClass }"
ev "window.__TAURI_INTERNALS__.invoke('clients_set_eco',{account:'$acc',on:false}).then(s=>JSON.stringify(s.eco))"
