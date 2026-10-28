import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { Setup } from "./components/Setup";
import { startUpdateChecks } from "./update";
import { setLang } from "./i18n";
import type { Settings } from "./api";
import "./styles/index.css";

// No browser menu on a right click, except in text fields (cut, copy, paste).
window.addEventListener("contextmenu", (e) => {
  const el = e.target as HTMLElement | null;
  if (!el?.closest("input, textarea")) e.preventDefault();
});

/** Quackblox-Setup.exe opens this page in its "setup" window (setup.rs), in place of the app. */
const isSetup = getCurrentWindow().label === "setup";
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
