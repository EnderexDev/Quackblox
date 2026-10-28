#!/usr/bin/env bash
# Test only: starts an install in Quackblox-Setup.exe and asks its window to close (as Alt+F4
# does) while NSIS runs; reports whether the window stayed and the install finished.
cd "$(dirname "$0")/.."
port=${PORT:-9338}
(WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=$port ./src-tauri/target/release/bundle/nsis/Quackblox-Setup.exe >/dev/null 2>&1 &)
sleep 7
export CDP_PORT=$port
node tools/cdp.mjs eval "document.querySelector('.setup-actions .primary').click()" >/dev/null
node tools/cdp.mjs eval "'at close: '+document.querySelector('.setup-line').innerText"
powershell -NoProfile -Command "Get-Process Quackblox-Setup | % { [void]\$_.CloseMainWindow() }; Start-Sleep 1; 'setup still running: ' + [bool](Get-Process Quackblox-Setup -EA SilentlyContinue)"
for i in $(seq 1 15); do
  sleep 3
  s=$(node tools/cdp.mjs eval "document.querySelector('.setup h1').innerText" 2>/dev/null)
  [ "$s" != "Quackblox" ] && break
done
echo "result: $s"
echo "temp copies left: $(ls "$TEMP" | grep -c Quackblox-Install)"
node tools/cdp.mjs eval "document.querySelector('.setup-close').click()" >/dev/null 2>&1
