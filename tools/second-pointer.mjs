#!/usr/bin/env node
// Test only: starts dragging saved card 1 with the mouse, then presses and releases card 0 with a
// second pointer (pointerId 7, as a second finger would), then finishes the drag onto card 0.
// Prints whether the drag survived the second pointer and the order saved.
// Usage: node tools/second-pointer.mjs
const port = process.env.CDP_PORT ?? 9333;
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
const evalJs = async (expr) => (await send("Runtime.evaluate", { expression: expr, returnByValue: true, awaitPromise: true })).result.result.value;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const names = () => evalJs(`[...document.querySelectorAll('[data-account]')].map(c => c.querySelector('.account-name').textContent + (c.classList.contains('dragging') ? '*' : '')).join(' | ')`);

const spots = await evalJs(`[...document.querySelectorAll('[data-account]')].map(c => {
  const r = c.querySelector('.account-name').getBoundingClientRect();
  return [r.left + r.width / 2, r.top + r.height / 2];
})`);
const mouse = (type, x, y, buttons) => send("Input.dispatchMouseEvent", { type, x, y, button: "left", buttons, clickCount: type === "mouseMoved" ? 0 : 1 });
const [x0, y0] = spots[1];
await mouse("mousePressed", x0, y0, 1);
await mouse("mouseMoved", x0 - 20, y0, 1);
await sleep(100);
console.log("dragging:", await names());
// The second pointer, pressed and released on the other card.
await evalJs(`(() => {
  const card = document.querySelectorAll('[data-account]')[0];
  const r = card.querySelector('.account-name').getBoundingClientRect();
  const opts = { pointerId: 7, bubbles: true, button: 0, buttons: 1, clientX: r.left + 5, clientY: r.top + 5, isPrimary: false };
  card.dispatchEvent(new PointerEvent('pointerdown', opts));
  card.dispatchEvent(new PointerEvent('pointerup', { ...opts, buttons: 0 }));
  return true;
})()`);
await sleep(100);
console.log("after second pointer:", await names());
const [x1, y1] = spots[0];
for (let i = 1; i <= 10; i++) {
  await mouse("mouseMoved", x0 + ((x1 - x0) * i) / 10, y0, 1);
  await sleep(25);
}
await mouse("mouseReleased", x1, y1, 0);
await sleep(800);
console.log("saved:", await evalJs(`window.__TAURI_INTERNALS__.invoke('accounts_list').then(l => l.map(a => a.name).join(' | '))`));
ws.close();
