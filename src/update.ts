// Updates without running an installer by hand, as in Quackify (its state/update.ts): the app
// asks GitHub's latest release (its latest.json, see scripts/release.mjs) a moment after it
// starts and every 6 hours, and the footer can ask any time. Updating hands over to the update
// window (setup.rs), which downloads the version, checks its signature against the key in
// tauri.conf.json, installs it and opens the new Quackblox. A dev build never asks.
import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { check } from "@tauri-apps/plugin-updater";
import { notify, toast } from "./toast";

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "latest" }
  | { kind: "available"; version: string }
  | { kind: "starting"; version: string }
  | { kind: "error"; message: string };

const FIRST_CHECK_MS = 10_000;
const EVERY_MS = 6 * 60 * 60 * 1000;

let state: UpdateState = { kind: "idle" };
/** The newest version heard of, kept when a later check fails. */
let found: string | null = null;
/** The version a notice was already shown for, so the 6-hour check does not repeat it. */
let told: string | null = null;
const listeners = new Set<() => void>();

const set = (s: UpdateState) => {
  state = s;
  listeners.forEach((f) => f());
};

export function useUpdate(): UpdateState {
  return useSyncExternalStore(
    (f) => {
      listeners.add(f);
      return () => listeners.delete(f);
    },
    () => state,
  );
}

/** Asks for a new version. `quiet`: the timer's check, which says nothing unless one is found. */
export async function checkForUpdate(quiet = false) {
  if (import.meta.env.DEV) return;
  if (state.kind === "checking" || state.kind === "starting") return;
  set({ kind: "checking" });
  try {
    const update = await check();
    // Only its version is used here: the update window downloads it again on its own side.
    update?.close().catch(() => {});
    found = update?.version ?? null;
    if (!update) {
      set({ kind: "latest" });
      if (!quiet) notify("O Quackblox está na versão mais nova", "info");
      return;
    }
    set({ kind: "available", version: update.version });
    if (told !== update.version) {
      told = update.version;
      notify(`Saiu o Quackblox ${update.version}`, "info", { label: "Atualizar", run: installUpdate });
    }
  } catch (e) {
    // A version found before is still there to install.
    set(found ? { kind: "available", version: found } : { kind: "error", message: String(e) });
    if (!quiet) toast(`Não consegui procurar atualização: ${String(e)}`);
  }
}

/** Closes Quackblox and opens the update window, which installs the version found. */
export async function installUpdate() {
  if (!found || state.kind !== "available") return;
  const version = found;
  set({ kind: "starting", version });
  try {
    await invoke("update_in_window");
  } catch (e) {
    set({ kind: "available", version });
    toast(`Não consegui atualizar: ${String(e)}`);
  }
}

/** The first check a moment after start, then the timer. Called once by the main window. */
export function startUpdateChecks() {
  setTimeout(() => checkForUpdate(true), FIRST_CHECK_MS);
  setInterval(() => checkForUpdate(true), EVERY_MS);
}
