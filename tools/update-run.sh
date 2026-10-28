#!/usr/bin/env bash
# Test only: restarts the installed Quackblox with a DevTools port, asks for an update from the
# footer, installs it and follows until the app comes back on a newer version.
cd "$(dirname "$0")/.."
export CDP_PORT=${PORT:-9343}
powershell -NoProfile -Command "Stop-Process -Name quackblox -Force -EA SilentlyContinue"
sleep 2
(WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=$CDP_PORT "$LOCALAPPDATA/Quackblox/quackblox.exe" >/dev/null 2>&1 &)
sleep 8
node tools/cdp.mjs eval "document.querySelector('.footer .update-link')?.click(); 'asked'" >/dev/null
for i in $(seq 1 10); do
  sleep 2
  f=$(node tools/cdp.mjs eval "document.querySelector('.footer').innerText.split('\n').pop()" 2>/dev/null)
  case "$f" in *"Atualizar pro"*) break;; esac
done
echo "before: $f"
echo "click: $(node tools/cdp.mjs eval "(()=>{const b=document.querySelector('.update-link.available');if(!b)return 'no button';b.click();return 'clicked '+b.disabled})()")"
for i in $(seq 1 25); do
  sleep 3
  t=$(node tools/cdp.mjs eval "(document.querySelector('.setup h1')?.innerText||'')+(document.querySelector('.footer')?.innerText.split('\n').pop()||'')" 2>/dev/null)
  case "$t" in *"Procurar atualização"*) break;; esac
done
echo "after: $t"
