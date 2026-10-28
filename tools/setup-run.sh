#!/usr/bin/env bash
# Test only: runs Quackblox-Setup.exe, sets the shortcut switch to <on|off>, installs and reports.
cd "$(dirname "$0")/.."
want=$1
(WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=${PORT:-9335} ./src-tauri/target/release/bundle/nsis/Quackblox-Setup.exe >/dev/null 2>&1 &)
sleep 7
export CDP_PORT=${PORT:-9335}
node tools/cdp.mjs eval "(()=>{const t=document.querySelector('.setup-option .toggle');const on=t.getAttribute('aria-checked')==='true';if(on!==('$want'==='on'))t.click();return 'button: '+document.querySelector('.setup-actions .primary').innerText})()"
node tools/cdp.mjs eval "document.querySelector('.setup-actions .primary').click()" >/dev/null
for i in $(seq 1 15); do
  sleep 3
  s=$(node tools/cdp.mjs eval "document.querySelector('.setup h1').innerText")
  [ "$s" != "Quackblox" ] && break
done
echo "result: $s / $(node tools/cdp.mjs eval "document.querySelector('.setup-line').innerText")"
node tools/cdp.mjs eval "document.querySelector('.setup-close').click()" >/dev/null 2>&1
echo "shortcut: $(ls "$USERPROFILE/Desktop" | grep -ic 'quackblox.lnk')"
