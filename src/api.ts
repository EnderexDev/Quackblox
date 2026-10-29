// Every call to the Rust side, typed. The login of an account never comes this way.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type User = { id: number; name: string; displayName: string };
/** `nickname`: his name for it, shown in place of the display name (empty for none); `color`: one of `COLORS`, or empty. */
export type Account = User & { lastPlayed: number | null; expired: boolean; nickname: string; color: string };
/** The colors an account can have, the same list as accounts.rs. */
export const COLORS = ["#ffcc33", "#fb923c", "#f87171", "#f472b6", "#a78bfa", "#38bdf8", "#4ade80", "#94a3b8"];
/** The window sends them all each time (settings.rs). `discordWebhook` is his own channel's key. */
export type Settings = {
  autoClose: boolean;
  language: "pt" | "en";
  hotkeys: boolean;
  alertWindows: boolean;
  alertDiscord: boolean;
  discordWebhook: string;
  loginCheck: boolean;
  /** The phone page on the home network (phone.rs). */
  phone: boolean;
  /** Discord's "Playing Quackblox" (discord.rs). */
  richPresence: boolean;
  /** A client over this many MB for a minute is told; 0 is off. */
  ramLimitMb: number;
  /** And opened again in the same game. */
  ramReopen: boolean;
};
/** The phone page: `links` (the home network's and Tailscale's) carry the pairing key after '#'. */
export type PhoneInfo = { on: boolean; listening: boolean; port: number; error: string | null; links: { kind: "lan" | "tailscale"; url: string; qr: string }[] };
/** Where a client is, from its log. "dropped": out of the server, Roblox's error dialog on screen. */
export type ClientState = "starting" | "inGame" | "dropped" | "joinFailed";
/** A Roblox client Quackblox opened. `account` is "app" or a saved account's id, as in `play`. */
export type Client = { pid: number; account: string; cpu: number; ram: number; window: boolean; state: ClientState; reason: number | null };
/** `others`: Roblox windows open that Quackblox did not open. `rejoin`: accounts that go back in by themselves.
 * `restart`: hours between restarts, by account. `places`: the accounts with a saved window place. */
export type Clients = { clients: Client[]; eco: string[]; rejoin: string[]; mute: string[]; restart: Record<string, number>; places: string[]; others: number };
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
/** Seconds in game by user id, then by place id; counted since `since` (Unix seconds). `days`: seconds by
 * local day (YYYY-MM-DD), then by user id. */
export type Stats = { since: number; users: Record<string, Record<string, number>>; days: Record<string, Record<string, number>> };
/** A drop the watch told, newest first. `at`: Unix seconds. */
export type DropEntry = { at: number; account: string; user: number | null; name: string; kind: "dropped" | "crashed" | "joinFailed"; reason: number | null; game: string };
/** A client closed and opened again: every N hours ("hours") or for its memory ("ram"). */
export type ClientRestart = { account: string; name: string; why: "hours" | "ram"; error: string | null };
/** What an accounts file brought in. */
export type Imported = { added: number; updated: number; groups: number; profiles: number; favorites: number; slots: number };
/** `jobId`: a server picked; `linkCode`: a private server. */
export type Game = { placeId: number; jobId: string | null; linkCode?: string | null; name: string; icon: string | null };
export type Presence = { userId: number; kind: "offline" | "site" | "game" | "studio"; location: string; placeId: number | null; jobId: string | null };
/** `ping`: the average of the players in the server, not this PC's. */
export type Server = { id: string; playing: number; maxPlayers: number; fps: number | null; ping: number | null };
export type Friend = { id: number; name: string; displayName: string; location: string; placeId: number; jobId: string | null };
/** Accounts opening one after another (launch.rs). `game`: what `play` takes. `eco`: the accounts put in
 * economy mode, the run's others taken out of it; left out, each stays as it is. */
export type Plan = { accounts: string[]; game: string; gameName: string; sameServer?: boolean; eco?: string[] | null; grid?: boolean };
export type Summary = { opened: number; failed: string[] };
/** A step of a run. "seat": waiting for room (`playing`/`max`); "stopped": he stopped that wait. */
export type LaunchStep = {
  account: string;
  name: string;
  kind: "seat" | "opening" | "opened" | "failed" | "stopped" | "seatOpened" | "firstServer" | "sameServer" | "sameServerTimeout" | "grid";
  game: string;
  playing: number;
  max: number;
  multi: boolean;
  error: string | null;
  count: number;
};
export type Group = { id: string; name: string; accounts: string[] };
export type Profile = { id: string; name: string; game: Game; accounts: string[]; eco: string[]; grid: boolean; sameServer: boolean };
/** `minute`: after midnight, the PC's time. `days`: 0 for Sunday; empty for every day. `last`: the day it last ran.
 * `action`: open the profile's accounts, or close them. */
export type Slot = { id: string; profile: string; minute: number; days: number[]; on: boolean; last: string; action: "open" | "close" };
/** A private server he keeps, with a name of his. */
export type Favorite = { id: string; name: string; game: Game };
export type Lists = { groups: Group[]; profiles: Profile[]; schedule: Slot[]; favorites: Favorite[] };

export const api = {
  multiStatus: () => invoke<{ active: boolean; stuck: boolean; closed: number }>("multi_status"),
  closeRobloxNow: () => invoke<void>("multi_close_now"),
  settings: () => invoke<Settings>("settings_get"),
  setSettings: (settings: Settings) => invoke<Settings>("settings_set", { settings }),
  accounts: () => invoke<Account[]>("accounts_list"),
  addAccount: () => invoke<void>("accounts_add"),
  removeAccount: (id: number) => invoke<void>("accounts_remove", { id }),
  moveAccount: (id: number, to: number) => invoke<void>("accounts_move", { id, to }),
  setLabel: (id: number, nickname: string, color: string) => invoke<void>("accounts_set_label", { id, nickname, color }),
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
  setMute: (account: string, on: boolean) => invoke<Clients>("clients_set_mute", { account, on }),
  /** 0 turns it off. */
  setRestart: (account: string, hours: number) => invoke<Clients>("clients_set_restart", { account, hours }),
  /** Keeps where the account's window is now as its place. */
  savePlace: (account: string) => invoke<Clients>("clients_save_place", { account }),
  forgetPlace: (account: string) => invoke<Clients>("clients_forget_place", { account }),
  /** Says how many windows it moved. */
  applyPlaces: () => invoke<number>("clients_apply_places"),
  /** Says how many clients it closed. */
  closeClients: (accounts: string[]) => invoke<number>("clients_close", { accounts }),
  history: () => invoke<DropEntry[]>("history_get"),
  clearHistory: () => invoke<void>("history_clear"),
  /** Says where it saved; null when he closed the dialog. */
  exportAccounts: (password: string) => invoke<string | null>("transfer_export", { password }),
  importAccounts: (password: string) => invoke<Imported | null>("transfer_import", { password }),
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
  launch: (plan: Plan) => invoke<Summary>("launch_run", { plan }),
  stopWait: (account: string) => invoke<void>("launch_stop", { account }),
  launchBusy: () => invoke<string[]>("launch_busy"),
  lists: () => invoke<Lists>("organize_get"),
  setLists: (lists: Lists) => invoke<Lists>("organize_set", { lists }),
  runProfile: (id: string) => invoke<Summary>("profile_run", { id }),
  /** The site logged in to a saved account, in a window of its own. */
  openSite: (account: string) => invoke<void>("site_open", { account }),
  phoneInfo: () => invoke<PhoneInfo>("phone_info"),
  /** Every phone paired with the old key stops working. */
  phoneNewKey: () => invoke<PhoneInfo>("phone_new_key"),
  /** The recent games, offered on the phone too. `game`: what `play` takes. */
  phoneGames: (games: { game: string; name: string }[]) => invoke<void>("phone_games", { games }),
  /** A live picture (JPEG) of the account's window. Fails with "minimized", "gone" or "none". */
  captureFrame: (account: string, width: number) => invoke<ArrayBuffer>("capture_frame", { account, width }),
  /** Saved to Pictures\Quackblox; one account, or all side by side. Says where. */
  capturePrint: (account: string | null) => invoke<string>("capture_print", { account }),
  /** Shows a print in its folder (only files in Pictures\Quackblox). */
  reveal: (path: string) => invoke<void>("capture_reveal", { path }),
  richPresenceAvailable: () => invoke<boolean>("rich_presence_available"),
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
  clientRestart: (f: (c: ClientRestart) => void) => listen<ClientRestart>("client-restart", (e) => f(e.payload)),
  /** A client's memory stayed over the limit for a minute. */
  clientRam: (f: (c: { account: string; name: string; mb: number; reopen: boolean }) => void) =>
    listen<{ account: string; name: string; mb: number; reopen: boolean }>("client-ram", (e) => f(e.payload)),
  launch: (f: (s: LaunchStep) => void) => listen<LaunchStep>("launch", (e) => f(e.payload)),
  /** A schedule slot just ran (its id): the window reads the lists again for its last day. */
  scheduleRan: (f: (id: string) => void) => listen<string>("schedule-ran", (e) => f(e.payload)),
  /** A saved account changed on the Rust side (a login found expired). */
  accountsChanged: (f: () => void) => listen("accounts-changed", () => f()),
  /** The hotkeys another program already holds, as "Ctrl+Shift+1, Ctrl+Shift+2". */
  hotkeysFailed: (f: (keys: string) => void) => listen<string>("hotkeys-failed", (e) => f(e.payload)),
};
