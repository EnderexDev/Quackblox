#!/usr/bin/env bash
# Test only: a second Dev beside one already open (the single-instance lock and the ports belong
# to the Dev's identifier). Its own identifier and data folder, Vite on 1430, DevTools on 9334
# (CDP_PORT=9334 for cdp.mjs), the Dev's saved accounts copied in. NO_LAUNCH=1 makes play refuse
# before any client opens. Delete %APPDATA%/dev.enderex.quackblox.dev2 after: it holds a copy of
# the logins.
#   NO_LAUNCH=1 bash tools/dev2.sh
set -e
cd "$(dirname "$0")/.."
D="$APPDATA/dev.enderex.quackblox.dev2"
mkdir -p "$D"
cp "$APPDATA/dev.enderex.quackblox.dev/accounts.json" "$APPDATA/dev.enderex.quackblox.dev/settings.json" "$D/"
CONF='{"identifier":"dev.enderex.quackblox.dev2","productName":"Quackblox Dev 2","build":{"beforeDevCommand":"npm run dev -- --port 1430","devUrl":"http://localhost:1430"}}'
[ -n "$NO_LAUNCH" ] && export QUACKBLOX_NO_LAUNCH=1
WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9334 npx tauri dev --config src-tauri/tauri.dev.conf.json --config "$CONF"
