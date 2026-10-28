#!/usr/bin/env bash
# Test only: leaves a tray player Quackblox never opened (Roblox started directly, closed by X),
# waits until it is over 30 s old, plays an account through the Dev app and says whether the
# client got its window and the tray player is gone. Usage: tools/tray-orphan.sh <account id>
cd "$(dirname "$0")/.."
acc=$1
ev() { node tools/cdp.mjs eval "$1"; }
players() { powershell -NoProfile -Command "(Get-Process RobloxPlayerBeta -EA SilentlyContinue | % { '{0}{1}' -f \$_.Id, \$(if (\$_.MainWindowHandle -ne 0) { '(window)' } else { '' }) }) -join ' '"; }
exe=$(powershell -NoProfile -Command "((Get-ItemProperty 'Registry::HKEY_CLASSES_ROOT\roblox-player\shell\open\command').'(default)' -split '\"')[1]")
cmd //c start "" "$exe"
until powershell -NoProfile -Command "if (Get-Process RobloxPlayerBeta -EA SilentlyContinue | ? MainWindowHandle -ne 0) {exit 0} else {exit 1}"; do sleep 2; done
sleep 5
powershell -NoProfile -Command "Get-Process RobloxPlayerBeta | ? MainWindowHandle -ne 0 | % { \$_.CloseMainWindow() | Out-Null }"
sleep 5
echo "after closing the app: $(players)"
sleep 35
ev "window.__TAURI_INTERNALS__.invoke('play',{account:'$acc',game:'1818'}).then(JSON.stringify,String)" >/dev/null
ok=no
for i in $(seq 1 15); do
  sleep 2
  [ "$(ev "window.__TAURI_INTERNALS__.invoke('clients_get').then(s=>s.clients.filter(c=>c.account==='$acc'&&c.window).length)")" = "1" ] && { ok=yes; break; }
done
echo "client window: $ok, players: $(players)"
