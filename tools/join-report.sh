#!/usr/bin/env bash
# Test only: what the newest Roblox client did, from its log. Prints the join lines (server and
# place), the disconnect reason, any error dialog, and whether its process still lives.
# Never prints the launch line (it holds the ticket).
#   tools/join-report.sh [seconds to wait first]
sleep "${1:-0}"
L="$LOCALAPPDATA/Roblox/logs"
f=$(ls -t "$L"/*_Player_*_last.log | grep -v CrashHandler | head -1)
echo "log: $(basename "$f")"
grep -E "Joining game|game_join_loadtime|Disconnected from server|Sending disconnect|ErrorPrompt|errorCode|GameJoinFailure|Teleport failed|join failed|status: ?[0-9]+|queuePosition|GameFull|Game is full|full" "$f" \
  | grep -v -i -E "gameinfo|ticket|placelauncherurl" \
  | sed -E 's/clienttime:[^,]+, //; s/^[^ ]+ //' | cut -c1-220 | head -20
powershell -NoProfile -Command "Get-Process RobloxPlayerBeta -ErrorAction SilentlyContinue | select Id,MainWindowTitle,@{n='win';e={\$_.MainWindowHandle -ne 0}} | ft -auto | out-string"
