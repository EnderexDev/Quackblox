#!/usr/bin/env node
// Test only: a plain click (press and release, no move) on the name of saved card <n>, through
// WebView2's DevTools port. Usage: node tools/click-card.mjs <n>
const port = process.env.CDP_PORT ?? 9333;
const n = Number(process.argv[2]);
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const ws = new WebSocket((targets.find((t) => t.type === "page") ?? targets[0]).webSocketDebuggerUrl);
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
    const k = ++id;
    pending.set(k, r);
    ws.send(JSON.stringify({ id: k, method, params }));
  });
const spot = (await send("Runtime.evaluate", {
  expression: `(() => { const r = document.querySelectorAll('[data-account]')[${n}].querySelector('.account-name').getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; })()`,
  returnByValue: true,
})).result.result.value;
await send("Input.dispatchMouseEvent", { type: "mousePressed", x: spot[0], y: spot[1], button: "left", buttons: 1, clickCount: 1 });
await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: spot[0], y: spot[1], button: "left", buttons: 0, clickCount: 1 });
await new Promise((r) => setTimeout(r, 600));
console.log("clicked", n);
ws.close();
