#!/usr/bin/env node
// Test only: puts a .ROBLOSECURITY (from the env var QB_TEST_COOKIE, never printed) into the open
// login window through DevTools, as if someone had logged in there.
const port = process.env.CDP_PORT ?? 9333;
const cookie = process.env.QB_TEST_COOKIE;
if (!cookie) throw new Error("QB_TEST_COOKIE is empty");
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const login = targets.find((t) => t.url.includes("roblox.com"));
if (!login) throw new Error("no login window open");
const ws = new WebSocket(login.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
const reply = new Promise((r) => ws.addEventListener("message", (e) => r(JSON.parse(e.data)), { once: true }));
ws.send(
  JSON.stringify({
    id: 1,
    method: "Network.setCookie",
    params: { name: ".ROBLOSECURITY", value: cookie, domain: ".roblox.com", path: "/", secure: true, httpOnly: true },
  }),
);
console.log("setCookie success:", (await reply).result?.success);
ws.close();
