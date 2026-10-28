#!/usr/bin/env bash
# Test only: plays an account through the Dev app <rounds> times, closing each client by its X
# (which leaves a tray process behind) before the next play. Prints whether each client got its
# window, and the players left between rounds. Usage: tools/tray-cycle.sh <account id> [rounds]
cd "$(dirname "$0")/.."
acc=$1; rounds=${2:-2}
ev() { node tools/cdp.mjs eval "$1"; }
players() { powershell -NoProfile -Command "(Get-Process RobloxPlayerBeta -EA SilentlyContinue | % { '{0}{1}' -f \$_.Id, \$(if (\$_.MainWindowHandle -ne 0) { '(window)' } else { '' }) }) -join ' '"; }
until ev "window.__TAURI_INTERNALS__.invoke('clients_get').then(()=>1)" 2>/dev/null | grep -q 1; do sleep 2; done
for r in $(seq 1 "$rounds"); do
  echo "round $r, players before: $(players)"
  ev "window.__TAURI_INTERNALS__.invoke('play',{account:'$acc',game:'1818'}).then(JSON.stringify,String)" >/dev/null
  ok=no
  for i in $(seq 1 15); do
    sleep 2
    n=$(ev "window.__TAURI_INTERNALS__.invoke('clients_get').then(s=>s.clients.filter(c=>c.account==='$acc'&&c.window).length)")
    [ "$n" = "1" ] && { ok=yes; break; }
  done
  pid=$(ev "window.__TAURI_INTERNALS__.invoke('clients_get').then(s=>(s.clients.find(c=>c.account==='$acc')||{}).pid)")
  echo "round $r: window $ok, pid $pid"
  [ "$ok" = yes ] || continue
  sleep 10
  until ! powershell -NoProfile -Command "if (Get-Process -Id $pid -EA SilentlyContinue) {exit 0} else {exit 1}"; do
    powershell -NoProfile -Command "\$p = Get-Process -Id $pid -EA SilentlyContinue; if (\$p) { \$p.CloseMainWindow() | Out-Null }"
    sleep 4
  done
  sleep 2
done
echo "players at the end: $(players)"
