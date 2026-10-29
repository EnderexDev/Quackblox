#!/usr/bin/env node
// Drives the running Quackblox window through WebView2's DevTools port, for testing.
// Start the app with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333, then:
//   node tools/cdp.mjs eval "<js expression>"      prints the value (promises awaited)
//   node tools/cdp.mjs shot <file.png> [w h]       screenshot of the page (optionally resized)
import { writeFileSync } from "node:fs";

const port = process.env.CDP_PORT ?? 9333;
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
// CDP_MATCH picks another page by its address (the phone page in a headless Edge: CDP_MATCH=38251).
const match = process.env.CDP_MATCH ?? "tauri.localhost";
const page = targets.find((t) => t.type === "page" && t.url.includes(match)) ?? targets[0];
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
let id = 0;
const pending = new Map();
ws.addEventListener("message", (e) => {
  const m = JSON.parse(e.data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  }
});
const send = (method, params = {}) =>
  new Promise((r) => {
    const n = ++id;
    pending.set(n, r);
    ws.send(JSON.stringify({ id: n, method, params }));
  });

const [cmd, a, b, c] = process.argv.slice(2);
if (cmd === "eval") {
  const r = await send("Runtime.evaluate", { expression: a, awaitPromise: true, returnByValue: true });
  const v = r.result?.exceptionDetails ? r.result.exceptionDetails.exception?.description : r.result?.result?.value;
  console.log(typeof v === "string" ? v : JSON.stringify(v, null, 1));
} else if (cmd === "shot") {
  if (b && c) await send("Emulation.setDeviceMetricsOverride", { width: +b, height: +c, deviceScaleFactor: 1, mobile: false });
  const r = await send("Page.captureScreenshot", { format: "png" });
  if (b && c) await send("Emulation.clearDeviceMetricsOverride");
  writeFileSync(a, Buffer.from(r.result.data, "base64"));
  console.log("saved", a);
}
ws.close();
