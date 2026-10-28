#!/usr/bin/env node
// Test only: drags account card <from> onto card <to> (places among the saved accounts, from 0)
// with real mouse events through WebView2's DevTools port, slowly, as a hand would.
// Usage: node tools/drag.mjs <from> <to>
const port = process.env.CDP_PORT ?? 9333;
const [from, to] = process.argv.slice(2).map(Number);
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = targets.find((t) => t.type === "page") ?? targets[0];
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
const evalJs = async (expr) => (await send("Runtime.evaluate", { expression: expr, returnByValue: true })).result.result.value;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// The middle of the name line of each saved card: a spot with no button on it.
const spots = await evalJs(`[...document.querySelectorAll('[data-account]')].map(c => {
  const r = c.querySelector('.account-name').getBoundingClientRect();
  return [r.left + r.width / 2, r.top + r.height / 2];
})`);
const [x0, y0] = spots[from];
const [x1, y1] = spots[to];
const mouse = (type, x, y, buttons) => send("Input.dispatchMouseEvent", { type, x, y, button: "left", buttons, clickCount: type === "mouseMoved" ? 0 : 1 });
await mouse("mousePressed", x0, y0, 1);
for (let i = 1; i <= 20; i++) {
  await mouse("mouseMoved", x0 + ((x1 - x0) * i) / 20, y0 + ((y1 - y0) * i) / 20, 1);
  await sleep(25);
}
console.log("while held:", await evalJs(`[...document.querySelectorAll('[data-account]')].map(c => c.querySelector('.account-name').textContent + (c.classList.contains('dragging') ? '*' : '')).join(' | ')`));
await mouse("mouseReleased", x1, y1, 0);
await sleep(800);
console.log("after drop:", await evalJs(`[...document.querySelectorAll('[data-account]')].map(c => c.querySelector('.account-name').textContent).join(' | ')`));
ws.close();
