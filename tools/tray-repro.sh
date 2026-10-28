#!/usr/bin/env bash
# Test only: waits for the Dev app, runs tools/eco-close-test.sh (a client with Eco closed by X,
# leaving its tray process), then plays the same account again and says whether that client lives.
# Usage: tools/tray-repro.sh <account id>
cd "$(dirname "$0")/.."
acc=$1
until node tools/cdp.mjs eval "window.__TAURI_INTERNALS__.invoke('clients_get').then(()=>1)" 2>/dev/null | grep -q 1; do sleep 2; done
node tools/cdp.mjs eval "window.__TAURI_INTERNALS__.invoke('app_account').then(u=>u.name)" >/dev/null
bash tools/eco-close-test.sh "$acc"
echo "--- second play, with the tray process there:"
node tools/cdp.mjs eval "window.__TAURI_INTERNALS__.invoke('play',{account:'$acc',game:'1818'}).then(JSON.stringify,String)"
sleep 14
echo "clients: $(node tools/cdp.mjs eval "window.__TAURI_INTERNALS__.invoke('clients_get').then(s=>JSON.stringify(s.clients.map(c=>[c.pid,c.window])))")"
powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name='RobloxPlayerBeta.exe'\" | % { '{0} parent {1} {2}' -f \$_.ProcessId, \$_.ParentProcessId, (Get-Process -Id \$_.ProcessId).PriorityClass }"
sed 's/\x1b\[[0-9;]*m//g' /tmp/qb-dev.log | grep DEBUG-ECO
