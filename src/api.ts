// Every call to the Rust side, typed. The login of an account never comes this way.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type User = { id: number; name: string; displayName: string };
export type Account = User & { lastPlayed: number | null; expired: boolean };
export type Settings = { autoClose: boolean; language: "pt" | "en"; hotkeys: boolean };
/** A Roblox client Quackblox opened. `account` is "app" or a saved account's id, as in `play`. */
export type Client = { pid: number; account: string; cpu: number; ram: number; window: boolean };
/** `others`: Roblox windows open that Quackblox did not open. */
export type Clients = { clients: Client[]; eco: string[]; others: number };
export type ClientExited = { pid: number; account: string; code: number; hadWindow: boolean };
export type Game = { placeId: number; jobId: string | null; name: string; icon: string | null };

export const api = {
  multiStatus: () => invoke<{ active: boolean; stuck: boolean; closed: number }>("multi_status"),
  closeRobloxNow: () => invoke<void>("multi_close_now"),
  settings: () => invoke<Settings>("settings_get"),
  setSettings: (settings: Settings) => invoke<Settings>("settings_set", { settings }),
  accounts: () => invoke<Account[]>("accounts_list"),
  addAccount: () => invoke<void>("accounts_add"),
  removeAccount: (id: number) => invoke<void>("accounts_remove", { id }),
  moveAccount: (id: number, to: number) => invoke<void>("accounts_move", { id, to }),
  appAccount: () => invoke<User>("app_account"),
  avatars: (ids: number[]) => invoke<[number, string][]>("avatars", { ids }),
  game: (input: string) => invoke<Game>("game_info", { input }),
  /** `account` is "app" or a saved account's id. */
  play: (account: string, game: string) => invoke<{ multi: boolean }>("play", { account, game }),
  clients: () => invoke<Clients>("clients_get"),
  setEco: (account: string, on: boolean) => invoke<Clients>("clients_set_eco", { account, on }),
  focusClient: (account: string) => invoke<void>("clients_focus", { account }),
  /** Says how many windows it placed. */
  grid: () => invoke<number>("clients_grid"),
  minimizeOthers: () => invoke<number>("clients_minimize_others"),
  startup: () => invoke<boolean>("startup_get"),
  setStartup: (on: boolean) => invoke<boolean>("startup_set", { on }),
};

export const on = {
  multi: (f: (active: boolean) => void) => listen<boolean>("multi", (e) => f(e.payload)),
  multiStuck: (f: () => void) => listen<boolean>("multi-stuck", () => f()),
  robloxClosed: (f: (count: number) => void) => listen<number>("roblox-closed", (e) => f(e.payload)),
  startup: (f: (on: boolean) => void) => listen<boolean>("startup", (e) => f(e.payload)),
  accountAdded: (f: (u: User) => void) => listen<User>("account-added", (e) => f(e.payload)),
  accountError: (f: (msg: string) => void) => listen<string>("account-error", (e) => f(e.payload)),
  /** Once a second, from the thread that watches the clients. */
  clients: (f: (c: Clients) => void) => listen<Clients>("clients", (e) => f(e.payload)),
  clientExited: (f: (c: ClientExited) => void) => listen<ClientExited>("client-exited", (e) => f(e.payload)),
  /** The hotkeys another program already holds, as "Ctrl+Shift+1, Ctrl+Shift+2". */
  hotkeysFailed: (f: (keys: string) => void) => listen<string>("hotkeys-failed", (e) => f(e.payload)),
};
