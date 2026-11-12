import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { Setup } from "./components/Setup";
import { startUpdateChecks } from "./update";
import { setLang } from "./i18n";
import { IS_DEV } from "./components/Logo";
import type { Settings } from "./api";
import "./styles/index.css";

// No browser menu on a right click, except in text fields (cut, copy, paste).
window.addEventListener("contextmenu", (e) => {
  const el = e.target as HTMLElement | null;
  if (!el?.closest("input, textarea")) e.preventDefault();
});

// Quackblox Dev's colours are green (tokens.css).
if (IS_DEV) document.documentElement.classList.add("dev");

/** Quackblox-Setup.exe opens this page in its "setup" window (setup.rs), in place of the app. */
const isSetup = getCurrentWindow().label === "setup";

// body.idle while the window has no focus (minimizing takes it too): endless animations pause
// (app.css) and the polls wait (App.tsx). A window covered by another stays "visible" to
// visibilitychange, so focus is what tells the app nobody is looking at it.
document.body.classList.toggle("idle", !document.hasFocus());
getCurrentWindow()
  .onFocusChanged(({ payload: focused }) => {
    document.body.classList.toggle("idle", !focused);
    window.dispatchEvent(new Event(focused ? "app-focus" : "app-blur"));
  })
  .catch(() => {});
// The main window asks for updates; the installer does not.
if (!isSetup) startUpdateChecks();

// The language first, so the first frame and the first notices are already in it.
invoke<Settings>("settings_get")
  .then((s) => setLang(s.language))
  .catch(() => {})
  .finally(() =>
    ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
      <React.StrictMode>{isSetup ? <Setup /> : <App />}</React.StrictMode>,
    ),
  );
