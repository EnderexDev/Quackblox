#!/usr/bin/env bash
# Test only: plays the app account and a saved account <gap> seconds apart, then counts the new
# clients. Usage: tools/gap-test.sh <gap seconds> <saved account id> <pids to keep, comma list>
cd "$(dirname "$0")/.."
gap=$1; alt=$2; keep=$3
before=$(powershell -NoProfile -Command "(@(Get-Process RobloxPlayerBeta -EA SilentlyContinue | % Id)) -join ','")
node tools/cdp.mjs eval "window.__TAURI_INTERNALS__.invoke('play',{account:'app',game:'1818'}).then(JSON.stringify,String)" >/dev/null
sleep "$gap"
node tools/cdp.mjs eval "window.__TAURI_INTERNALS__.invoke('play',{account:'$alt',game:'1818'}).then(JSON.stringify,String)" >/dev/null
sleep 12; sleep 12; sleep 10
after=$(powershell -NoProfile -Command "(@(Get-Process RobloxPlayerBeta -EA SilentlyContinue | % Id)) -join ','")
echo "gap ${gap}s: before [$before] after [$after]"
# Close the clients this test opened, never the ones in $keep.
powershell -NoProfile -Command "Get-Process RobloxPlayerBeta -EA SilentlyContinue | ? { ('$keep' -split ',') -notcontains \$_.Id.ToString() } | Stop-Process -Force"
