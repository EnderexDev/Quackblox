#!/usr/bin/env bash
# Test only: plays an account through the Dev app with tools/proc-watch.ps1 running, then prints
# the watch log and the client-exited events heard. Usage: tools/play-watch.sh <account> [seconds]
cd "$(dirname "$0")/.."
acc=$1; secs=${2:-40}
out="${TMPDIR:-/tmp}/play-watch.txt"
rm -f "$out"
powershell -NoProfile -ExecutionPolicy Bypass -File tools/proc-watch.ps1 "$secs" > "$out" 2>&1 &
watcher=$!
until grep -q ready "$out" 2>/dev/null; do sleep 0.3; done
node tools/cdp.mjs eval "window.__exits=[]; const I=window.__TAURI_INTERNALS__; I.invoke('plugin:event|listen',{event:'client-exited',target:{kind:'Any'},handler:I.transformCallback(e=>window.__exits.push(e.payload))}).then(()=>'listening')"
node tools/cdp.mjs eval "window.__TAURI_INTERNALS__.invoke('play',{account:'$acc',game:'1818'}).then(JSON.stringify,String)"
wait "$watcher"
grep -v "cmd: .*CrashHandler" "$out" | sed 's/placelauncherurl.*//'
echo "exits: $(node tools/cdp.mjs eval "JSON.stringify(window.__exits)")"
echo "clients: $(node tools/cdp.mjs eval "window.__TAURI_INTERNALS__.invoke('clients_get').then(s=>JSON.stringify([s.clients.map(c=>[c.pid,c.account,c.window]),s.others]))")"
