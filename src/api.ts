// Every call to the Rust side, typed. The login of an account never comes this way.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type User = { id: number; name: string; displayName: string };
export type Account = User & { lastPlayed: number | null; expired: boolean };
/** The window sends them all each time (settings.rs). `discordWebhook` is his own channel's key. */
export type Settings = {
  autoClose: boolean;
  language: "pt" | "en";
  hotkeys: boolean;
  alertWindows: boolean;
  alertDiscord: boolean;
  discordWebhook: string;
  loginCheck: boolean;
};
/** Where a client is, from its log. "dropped": out of the server, Roblox's error dialog on screen. */
export type ClientState = "starting" | "inGame" | "dropped" | "joinFailed";
/** A Roblox client Quackblox opened. `account` is "app" or a saved account's id, as in `play`. */
export type Client = { pid: number; account: string; cpu: number; ram: number; window: boolean; state: ClientState; reason: number | null };
/** `others`: Roblox windows open that Quackblox did not open. `rejoin`: accounts that go back in by themselves. */
export type Clients = { clients: Client[]; eco: string[]; rejoin: string[]; others: number };
/** `reason`: why it left its last server, from its log (Roblox's error code; 285 is leaving on purpose). */
export type ClientExited = {
  pid: number;
  account: string;
  code: number;
  hadWindow: boolean;
  reason: number | null;
  kind: "closed" | "dropped" | "crashed" | "joinFailed" | "ended";
};
/** A client that dropped, crashed or never got in. `rejoinIn`: seconds until it goes back in;
 * `noRejoin`: why it will not, when rejoin is on. */
export type ClientFell = { account: string; name: string; reason: number | null; kind: "dropped" | "crashed" | "joinFailed"; rejoinIn: number | null; noRejoin: string | null };
/** `error`: null when the client opened again. */
export type ClientRejoin = { account: string; name: string; error: string | null };
/** Seconds in game by user id, then by place id; counted since `since` (Unix seconds). */
export type Stats = { since: number; users: Record<string, Record<string, number>> };
/** `jobId`: a server picked; `linkCode`: a private server. */
export type Game = { placeId: number; jobId: string | null; linkCode?: string | null; name: string; icon: string | null };
export type Presence = { userId: number; kind: "offline" | "site" | "game" | "studio"; location: string; placeId: number | null; jobId: string | null };
/** `ping`: the average of the players in the server, not this PC's. */
export type Server = { id: string; playing: number; maxPlayers: number; fps: number | null; ping: number | null };
export type Friend = { id: number; name: string; displayName: string; location: string; placeId: number; jobId: string | null };

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
  /** Where each account is, by account key ("app" or a saved account's id). */
  presence: () => invoke<Record<string, Presence>>("presence"),
  servers: (placeId: number, fullestFirst: boolean, skipFull: boolean, cursor: string | null) =>
    invoke<{ servers: Server[]; next: string | null }>("servers", { placeId, fullestFirst, skipFull, cursor }),
  /** `found` false: the server is not in the list's first pages. */
  serverSeat: (placeId: number, jobId: string) => invoke<{ found: boolean; playing: number; maxPlayers: number }>("server_seat", { placeId, jobId }),
  friendsInGame: (account: string) => invoke<Friend[]>("friends_in_game", { account }),
  /** The server the account's newest client joined, read from its log; null until it joined. */
  clientServer: (account: string) => invoke<{ placeId: number; jobId: string } | null>("client_server", { account }),
  clients: () => invoke<Clients>("clients_get"),
  setEco: (account: string, on: boolean) => invoke<Clients>("clients_set_eco", { account, on }),
  setRejoin: (account: string, on: boolean) => invoke<Clients>("clients_set_rejoin", { account, on }),
  testDiscord: (webhook: string) => invoke<void>("vigia_test_discord", { webhook }),
  testWindows: () => invoke<void>("vigia_test_windows"),
  /** The names of the logins that turned out expired. */
  checkLogins: () => invoke<string[]>("vigia_check_logins"),
  stats: () => invoke<Stats>("stats_get"),
  resetStats: () => invoke<Stats>("stats_reset"),
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
  clientFell: (f: (c: ClientFell) => void) => listen<ClientFell>("client-fell", (e) => f(e.payload)),
  clientRejoin: (f: (c: ClientRejoin) => void) => listen<ClientRejoin>("client-rejoin", (e) => f(e.payload)),
  /** A saved account changed on the Rust side (a login found expired). */
  accountsChanged: (f: () => void) => listen("accounts-changed", () => f()),
  /** The hotkeys another program already holds, as "Ctrl+Shift+1, Ctrl+Shift+2". */
  hotkeysFailed: (f: (keys: string) => void) => listen<string>("hotkeys-failed", (e) => f(e.payload)),
};
