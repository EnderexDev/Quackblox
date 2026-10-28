#!/usr/bin/env node
// Test only: types a value into an input of the running Quackblox (React sees it as typed) and
// submits its form. CDP_PORT picks the window, as in cdp.mjs.
//   node tools/type-in.mjs "<css selector>" "<value>" [submit]
import { execFileSync } from "node:child_process";

const [selector, value, submit] = process.argv.slice(2);
const js = `(() => {
  const el = document.querySelector(${JSON.stringify(selector)});
  if (!el) return "no element";
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(el, ${JSON.stringify(value)});
  el.dispatchEvent(new Event("input", { bubbles: true }));
  ${submit ? "setTimeout(() => el.form?.requestSubmit(), 50);" : ""}
  return "ok";
})()`;
console.log(execFileSync("node", [new URL("./cdp.mjs", import.meta.url).pathname.replace(/^\/(\w:)/, "$1"), "eval", js], { encoding: "utf8" }).trim());
