import { useCallback, useEffect, useRef, useState } from "react";
import { api, on, type Account, type Client, type ClientFell, type Clients, type Friend, type Game, type Group, type Lists, type Plan, type Presence, type Profile, type Server, type Settings, type Stats, type Summary, type User } from "./api";
import {
  ArrowUpIcon,
  CloseIcon,
  FollowIcon,
  GridIcon,
  LeafIcon,
  LockIcon,
  LoginIcon,
  MinimizeIcon,
  MoreIcon,
  PcIcon,
  PlayIcon,
  PlusIcon,
  RejoinIcon,
  SearchIcon,
  ServerIcon,
  TrashIcon,
  UsersIcon,
  GlobeIcon,
  CommandIcon,
  ScreenIcon,
} from "./components/icons";
import { getVersion } from "@tauri-apps/api/app";
import { Toggle } from "./components/Toggle";
import { Mascot, type Mood } from "./components/Mascot";
import { checkForUpdate, installUpdate, useUpdate } from "./update";
import { notify, toast, Toasts } from "./toast";
import { getLang, setLang, t as texts, useT, type Lang, type Texts } from "./i18n";
import { errorNames } from "./errorCodes";
import { OrganizeBlock, type Person } from "./components/Organize";
import { Palette, type Command } from "./components/Palette";
import { ExtrasBlock, print, ScreensSheet } from "./components/Screens";

const NO_CLIENTS: Clients = { clients: [], eco: [], rejoin: [], others: 0 };
type VigiaSettings = Pick<Settings, "alertWindows" | "alertDiscord" | "discordWebhook" | "loginCheck">;
/** Only the watch's own fields: the whole set also carries the language, which must not ride along. */
const vigiaOf = (s: Settings): VigiaSettings => ({ alertWindows: s.alertWindows, alertDiscord: s.alertDiscord, discordWebhook: s.discordWebhook, loginCheck: s.loginCheck });
const NO_VIGIA: VigiaSettings = { alertWindows: true, alertDiscord: false, discordWebhook: "", loginCheck: false };
/** The phone page and Discord's presence (phase 5): off until he turns them on. */
type ExtrasSettings = Pick<Settings, "phone" | "richPresence">;
const extrasOf = (s: Settings): ExtrasSettings => ({ phone: s.phone, richPresence: s.richPresence });
const NO_EXTRAS: ExtrasSettings = { phone: false, richPresence: false };
/** How long the duck panics after a drop, and cheers once every client is in. */
const PANIC_MS = 8000;
const CHEER_MS = 4000;

const RECENTS_KEY = "quackblox.recents";
const GAME_KEY = "quackblox.game";
const SAME_SERVER_KEY = "quackblox.sameServer";
/** How often the accounts' presence is asked while the window shows (15 in a row passed, 28/10/26). */
const PRESENCE_MS = 20000;
const JOB_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

const readJson = <T,>(key: string, fallback: T): T => {
  try {
    return JSON.parse(localStorage.getItem(key) ?? "") as T;
  } catch {
    return fallback;
  }
};

/** What `play` takes for a game: the id, or a link that also names the server or the private server. */
const gameArg = (g: Game) =>
  g.linkCode
    ? `https://www.roblox.com/games/${g.placeId}?privateServerLinkCode=${g.linkCode}`
    : g.jobId
      ? `https://www.roblox.com/games/start?placeId=${g.placeId}&gameInstanceId=${g.jobId}`
      : String(g.placeId);
const sameGame = (a: Game, b: Game) => a.placeId === b.placeId && a.jobId === b.jobId && (a.linkCode ?? null) === (b.linkCode ?? null);
/** One recent per game, and one per private server: a server picked inside a game replaces it. */
const sameRecent = (a: Game, b: Game) => a.placeId === b.placeId && (a.linkCode ?? null) === (b.linkCode ?? null);
/** What a Roblox error code means, or its official name when there is no text for it here. */
const errorText = (code: number, t: Texts) => t.errors[code] ?? t.errorUnknown(code, errorNames[code]);
/** The notice for a client that dropped, crashed or never got in, with what happens next. */
function fellText(e: ClientFell, t: Texts) {
  const what =
    e.kind === "crashed" ? t.clientCrashed(e.name) : e.kind === "joinFailed" ? t.clientJoinFailed(e.name) : t.clientLeft(e.name, e.reason ? errorText(e.reason, t) : "");
  const next = e.rejoinIn !== null ? t.rejoinIn(e.rejoinIn) : e.noRejoin ? t.noRejoin(e.noRejoin) : "";
  return [what.trim(), next].filter(Boolean).join(" ");
}

/** RAM as Task Manager rounds it: MB below 1 GB, GB with one decimal above. */
function memory(bytes: number) {
  const fmt = new Intl.NumberFormat(getLang() === "en" ? "en" : "pt-BR", { maximumFractionDigits: 1 });
  return bytes < 1024 ** 3 ? `${Math.round(bytes / 1024 ** 2)} MB` : `${fmt.format(bytes / 1024 ** 3)} GB`;
}

function ago(seconds: number | null, t: Texts) {
  if (!seconds) return t.neverPlayed;
  const s = Date.now() / 1000 - seconds;
  if (s < 60) return t.playedNow;
  if (s < 3600) return t.playedMinutes(Math.floor(s / 60));
  if (s < 86400) return t.playedHours(Math.floor(s / 3600));
  return t.playedDays(Math.floor(s / 86400));
}

export default function App() {
  const t = useT();
  const [multi, setMulti] = useState<boolean | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [appUser, setAppUser] = useState<User | null>(null);
  const [appError, setAppError] = useState<string | null>(null);
  const [avatars, setAvatars] = useState<Record<number, string>>({});
  const [game, setGame] = useState<Game | null>(() => readJson<Game | null>(GAME_KEY, null));
  // Lists saved before one recent per game kept every server picked: the newest of each stays.
  const [recents, setRecents] = useState<Game[]>(() =>
    readJson<Game[]>(RECENTS_KEY, []).filter((g, i, all) => all.findIndex((o) => sameRecent(o, g)) === i),
  );
  const [busy, setBusy] = useState<Set<string>>(new Set());
  const [startup, setStartup] = useState(false);
  const [autoClose, setAutoClose] = useState(true);
  const [stuck, setStuck] = useState(false);
  const [hotkeys, setHotkeys] = useState(true);
  const [vigia, setVigia] = useState<VigiaSettings>(NO_VIGIA);
  const [extras, setExtras] = useState<ExtrasSettings>(NO_EXTRAS);
  const [screens, setScreens] = useState(false);
  /** The name of the account that just dropped, while the duck panics. */
  const [alarm, setAlarm] = useState<string | null>(null);
  const [cheer, setCheer] = useState(false);
  const [savingSettings, setSavingSettings] = useState(false);
  const [clients, setClients] = useState<Clients>(NO_CLIENTS);
  const [presence, setPresence] = useState<Record<string, Presence>>({});
  const [sameServer, setSameServer] = useState(() => localStorage.getItem(SAME_SERVER_KEY) === "1");
  /** The accounts waiting for room in a full server, with how full it was at the last look. */
  const [waiting, setWaiting] = useState<Record<string, { playing: number; max: number }>>({});
  const [friendsOf, setFriendsOf] = useState<{ key: string; name: string } | null>(null);
  const [lists, setLists] = useState<Lists>({ groups: [], profiles: [], schedule: [] });
  const [palette, setPalette] = useState(false);
  const alarmTimer = useRef<number | undefined>(undefined);
  // Read by the listeners set up once: the names as they are now.
  const nameOf = useRef((key: string) => key);
  nameOf.current = (key) => (key === "app" ? appUser?.displayName : accounts.find((a) => String(a.id) === key)?.displayName) ?? "Roblox";

  const loadAccounts = useCallback(() => api.accounts().then(setAccounts).catch(toast), []);
  const loadAppAccount = useCallback(
    () =>
      api
        .appAccount()
        .then((u) => {
          setAppUser(u);
          setAppError(null);
        })
        .catch((e) => {
          setAppUser(null);
          setAppError(String(e));
        }),
    [],
  );

  useEffect(() => {
    api.startup().then(setStartup);
    api.settings().then((s) => {
      setAutoClose(s.autoClose);
      setHotkeys(s.hotkeys);
      setVigia(vigiaOf(s));
      setExtras(extrasOf(s));
    });
    api.clients().then(setClients);
    loadAccounts();
    loadAppAccount();
    // An event after the status was read wins over it.
    let heard = false;
    const offs = [
      on.multi((m) => {
        heard = true;
        setMulti(m);
        if (m) setStuck(false);
      }),
      on.multiStuck(() => setStuck(true)),
      on.startup(setStartup),
      on.robloxClosed(() => notify(texts().closedRoblox, "info")),
      on.accountAdded((u) => {
        notify(texts().accountAdded(u.displayName));
        loadAccounts();
      }),
      on.accountError(toast),
      on.clients(setClients),
      // A drop, a crash or a join that never happened (vigia.rs). Leaving on purpose is none.
      on.clientFell((e) => {
        toast(fellText(e, texts()));
        setAlarm(e.name);
        clearTimeout(alarmTimer.current);
        alarmTimer.current = window.setTimeout(() => setAlarm(null), PANIC_MS);
      }),
      on.clientRejoin((e) => (e.error ? toast(texts().rejoinFailed(e.name, e.error)) : notify(texts().rejoined(e.name), "info"))),
      on.accountsChanged(loadAccounts),
      on.hotkeysFailed((keys) => toast(texts().hotkeysFailed(keys))),
    ];
    // Read only once every listener is in, so nothing that happens in between is missed. A
    // Roblox closed before the window loaded still gets its notice.
    Promise.all(offs).then(() =>
      api.multiStatus().then((s) => {
        if (!heard) setMulti(s.active);
        // Only ever raises it: a "multi-stuck" heard meanwhile is never undone by an older read.
        setStuck((was) => was || s.stuck);
        if (s.closed > 0) notify(texts().closedRoblox, "info");
      }),
    );
    // The app's login, and the tray's "start with Windows", can change while the window is hidden.
    const refresh = () => {
      if (document.visibilityState !== "visible") return;
      loadAppAccount();
      api.startup().then(setStartup);
    };
    document.addEventListener("visibilitychange", refresh);
    return () => {
      offs.forEach((p) => p.then((off) => off()));
      document.removeEventListener("visibilitychange", refresh);
    };
  }, [loadAccounts, loadAppAccount]);

  // Groups, profiles and the schedule; read again when a slot ran (its last day changed).
  useEffect(() => {
    const load = () => api.lists().then(setLists).catch(toast);
    load();
    const off = on.scheduleRan(load);
    return () => {
      off.then((f) => f());
    };
  }, []);
  const saveLists = (next: Lists) =>
    api
      .setLists(next)
      .then((l) => {
        setLists(l);
        return true;
      })
      .catch((e) => {
        toast(e);
        return false;
      });

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPalette((o) => !o);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);

  // Avatars for every account shown, asked once each: an id with no image (or a failed
  // request) is not asked again until the app restarts.
  const asked = useRef(new Set<number>());
  useEffect(() => {
    const ids = [...accounts.map((a) => a.id), ...(appUser ? [appUser.id] : [])].filter((id) => !asked.current.has(id));
    if (!ids.length) return;
    ids.forEach((id) => asked.current.add(id));
    api.avatars(ids).then((pairs) => pairs.length && setAvatars((m) => ({ ...m, ...Object.fromEntries(pairs) })));
  }, [accounts, appUser]);

  // Where each account is, asked again while the window shows.
  useEffect(() => {
    const load = () => {
      if (document.visibilityState !== "visible") return;
      api.presence().then(setPresence).catch(() => {});
      // The Roblox app's login turns into the last account that played: its card follows.
      loadAppAccount();
    };
    load();
    const timer = setInterval(load, PRESENCE_MS);
    document.addEventListener("visibilitychange", load);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", load);
    };
  }, [accounts.length, appUser?.id]);

  const pickGame = (g: Game | null) => {
    setGame(g);
    localStorage.setItem(GAME_KEY, JSON.stringify(g));
    if (!g) return;
    setRecents((rs) => {
      const next = [g, ...rs.filter((r) => !sameRecent(r, g))].slice(0, 8);
      localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
      return next;
    });
  };

  // The phone page offers the same recent games.
  useEffect(() => {
    api.phoneGames(recents.map((r) => ({ game: gameArg(r), name: r.name }))).catch(() => {});
  }, [recents]);

  const forgetRecent = (g: Game) =>
    setRecents((rs) => {
      const next = rs.filter((r) => !sameRecent(r, g));
      localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
      return next;
    });

  // Read synchronously by play(), so a second click on a busy account does nothing.
  const busyNow = useRef(new Set<string>());
  const [playingAll, setPlayingAll] = useState(false);

  // Stopping a wait for room: the card lets go at once, the run hears it at its next look (launch.rs).
  const stopWaiting = (key: string) => {
    setWaiting(({ [key]: _, ...rest }) => rest);
    api.stopWait(key).catch(toast);
  };

  const markBusy = (key: string, on: boolean) => {
    if (on) busyNow.current.add(key);
    else busyNow.current.delete(key);
    setBusy((b) => {
      const n = new Set(b);
      if (on) n.add(key);
      else n.delete(key);
      return n;
    });
  };

  // Each step of a run (launch.rs), wherever it came from: a card, a group, a profile, the schedule.
  useEffect(() => {
    const off = on.launch((s) => {
      const tx = texts();
      const clearWait = () => setWaiting(({ [s.account]: _, ...rest }) => rest);
      switch (s.kind) {
        case "seat":
          markBusy(s.account, true);
          setWaiting((w) => ({ ...w, [s.account]: { playing: s.playing, max: s.max } }));
          break;
        case "opening":
          markBusy(s.account, true);
          clearWait();
          break;
        case "opened":
          notify(s.multi ? tx.openingGame(s.game, s.name) : tx.openingWithoutMulti(s.name), s.multi ? "success" : "info");
          if (s.account !== "app") loadAccounts();
          // The client takes a few seconds to show; the button stays busy meanwhile.
          setTimeout(() => markBusy(s.account, false), 4000);
          break;
        case "failed":
          toast(`${s.name}: ${s.error ?? ""}`);
          markBusy(s.account, false);
          clearWait();
          if (s.account !== "app") loadAccounts();
          break;
        case "stopped":
          notify(tx.waitStopped(s.name), "info");
          markBusy(s.account, false);
          clearWait();
          break;
        case "seatOpened":
          notify(tx.seatOpened(s.name));
          break;
        case "firstServer":
          notify(tx.waitingFirstServer(s.name), "info");
          break;
        case "sameServer":
          notify(tx.sameServerFound);
          break;
        case "sameServerTimeout":
          notify(tx.sameServerTimeout, "info");
          break;
        case "grid":
          notify(tx.gridDone(s.count), "info");
          break;
      }
    });
    // A window opened in the middle of a run (the schedule's, from the tray) shows who is busy.
    api.launchBusy().then((keys) => keys.forEach((k) => markBusy(k, true)));
    return () => {
      off.then((f) => f());
    };
  }, [loadAccounts]);

  /** Opens a client of the account in the game picked, or in `target`. */
  const play = async (key: string, target: Game | null = game) => {
    if (!target) {
      toast(t.pickGameFirst);
      return;
    }
    if (busyNow.current.has(key)) return;
    markBusy(key, true);
    let opened = 0;
    try {
      opened = (await api.launch({ accounts: [key], game: gameArg(target), gameName: target.name })).opened;
    } catch (e) {
      toast(e);
    } finally {
      // An open lets go 4 s after (the "opened" step); a failure or a stopped wait, at once.
      if (!opened) markBusy(key, false);
    }
  };

  /** Several accounts one after another: "Jogar com todas", a group, a profile. */
  const runMany = async (run: () => Promise<Summary>) => {
    if (playingAll) return;
    setPlayingAll(true);
    try {
      const s = await run();
      if (s.failed.length || s.opened > 1) notify(t.runDone(s.opened, s.failed.length), s.failed.length ? "info" : "success");
    } catch (e) {
      toast(e);
    } finally {
      setPlayingAll(false);
    }
  };
  const planFor = (keys: string[], g: Game): Plan => ({ accounts: keys, game: gameArg(g), gameName: g.name, sameServer });
  // The Roblox app's card first, as before; one saved account equal to it opens once (launch.rs).
  const playAll = () => game && runMany(() => api.launch(planFor(["app", ...accounts.filter((a) => !a.expired).map((a) => String(a.id))], game)));
  const playGroup = (g: Group) => {
    if (!game) return toast(t.pickGameFirst);
    runMany(() => api.launch(planFor(g.accounts, game)));
  };
  const runProfile = (p: Profile) => runMany(() => api.runProfile(p.id));

  const toggleSameServer = (on: boolean) => {
    setSameServer(on);
    localStorage.setItem(SAME_SERVER_KEY, on ? "1" : "0");
  };

  const remove = async (a: Account) => {
    try {
      await api.removeAccount(a.id);
      notify(t.accountRemoved(a.displayName), "info");
    } catch (e) {
      toast(e);
    }
    loadAccounts();
  };

  const moveUp = async (a: Account, index: number) => {
    await api.moveAccount(a.id, Math.max(0, index - 1)).catch(toast);
    loadAccounts();
  };

  // Shown at once; saved with the other settings, and put back if saving fails.
  const changeLanguage = (l: Lang) => {
    const before = getLang();
    if (l === before) return;
    setLang(l);
    api
      .setSettings({ autoClose, hotkeys, ...vigia, ...extras, language: l })
      .then((s) => setLang(s.language))
      .catch((e) => {
        setLang(before);
        toast(e);
      });
  };

  // The window sends every setting each time (settings.rs): the ones not changing go as they are.
  const saveSettings = (change: Partial<Omit<Settings, "language">>) => {
    setSavingSettings(true);
    return api
      .setSettings({ autoClose, hotkeys, ...vigia, ...extras, language: getLang(), ...change })
      .then((s) => {
        setAutoClose(s.autoClose);
        setHotkeys(s.hotkeys);
        setVigia(vigiaOf(s));
        setExtras(extrasOf(s));
        return true;
      })
      .catch((e) => {
        toast(e);
        return false;
      })
      .finally(() => setSavingSettings(false));
  };

  const clientsOf = (key: string) => clients.clients.filter((c) => c.account === key);
  const windowsOpen = clients.clients.filter((c) => c.window).length + clients.others;
  // Shown at once; the answer (and the watcher's next snapshot) confirms it.
  const toggleEco = (key: string) => {
    const on = !clients.eco.includes(key);
    setClients((c) => ({ ...c, eco: on ? [...c.eco, key] : c.eco.filter((k) => k !== key) }));
    api.setEco(key, on).then(setClients).catch(toast);
  };
  const toggleRejoin = (key: string) => {
    const on = !clients.rejoin.includes(key);
    setClients((c) => ({ ...c, rejoin: on ? [...c.rejoin, key] : c.rejoin.filter((k) => k !== key) }));
    api.setRejoin(key, on).then(setClients).catch(toast);
  };
  /** `place`: the saved account's place, from 0, for its Ctrl+Shift number; none for the app's. */
  const live = (key: string, place?: number) => ({
    presence: presence[key],
    waiting: waiting[key],
    onStopWait: () => stopWaiting(key),
    clients: clientsOf(key),
    eco: clients.eco.includes(key),
    onEco: () => toggleEco(key),
    rejoin: clients.rejoin.includes(key),
    onRejoin: () => toggleRejoin(key),
    onFocus: () => api.focusClient(key).catch(toast),
    hotkey: hotkeys && place !== undefined && place < 9 ? place + 1 : undefined,
  });

  // Hold and drag a saved account's card to give it another place. The list moves under the
  // pointer while it is held, and the new place is saved when it is let go.
  // `order` is the list of ids as it stands under the pointer, kept here so the drop reads the
  // place moved to even before React has drawn it; `pointer` ignores a second finger.
  const drag = useRef<{ id: number; pointer: number; x: number; y: number; moved: boolean; order: number[] } | null>(null);
  const [dragId, setDragId] = useState<number | null>(null);
  const dragHandlers = (a: Account) => ({
    onPointerDown: (e: React.PointerEvent<HTMLDivElement>) => {
      // The card's own buttons keep their clicks.
      if (drag.current || e.button !== 0 || (e.target as HTMLElement).closest("button, .menu")) return;
      drag.current = { id: a.id, pointer: e.pointerId, x: e.clientX, y: e.clientY, moved: false, order: accounts.map((x) => x.id) };
      e.currentTarget.setPointerCapture(e.pointerId);
    },
    onPointerMove: (e: React.PointerEvent<HTMLDivElement>) => {
      const d = drag.current;
      if (!d || e.pointerId !== d.pointer) return;
      if (!d.moved) {
        if (Math.hypot(e.clientX - d.x, e.clientY - d.y) < 6) return;
        d.moved = true;
        setDragId(d.id);
      }
      const over = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>("[data-account]");
      const overId = over ? Number(over.dataset.account) : null;
      if (overId === null || overId === d.id) return;
      const from = d.order.indexOf(d.id);
      const to = d.order.indexOf(overId);
      if (from < 0 || to < 0) return;
      d.order.splice(to, 0, ...d.order.splice(from, 1));
      const order = [...d.order];
      setAccounts((list) => order.map((id) => list.find((x) => x.id === id)).filter((x): x is Account => !!x));
    },
    onPointerUp: (e: React.PointerEvent<HTMLDivElement>) => {
      const d = drag.current;
      if (!d || e.pointerId !== d.pointer) return;
      drag.current = null;
      setDragId(null);
      if (!d.moved) return;
      api.moveAccount(d.id, Math.max(0, d.order.indexOf(d.id))).catch(toast).finally(loadAccounts);
    },
    onPointerCancel: (e: React.PointerEvent<HTMLDivElement>) => {
      if (drag.current?.pointer !== e.pointerId) return;
      drag.current = null;
      setDragId(null);
      loadAccounts();
    },
  });

  const playable = (appUser ? 1 : 0) + accounts.filter((a) => !a.expired && a.id !== appUser?.id).length;

  // The duck cheers once every client open is inside a game (from one that was not), and panics
  // after a drop or while a client sits on Roblox's error dialog.
  const inGame = clients.clients.filter((c) => c.state === "inGame").length;
  // "Todas": two or more. One account joining alone is no cheer, nor the first of a "Jogar com todas".
  const allIn = inGame > 1 && inGame === clients.clients.length;
  const wasAllIn = useRef(allIn);
  const cheerTimer = useRef<number | undefined>(undefined);
  useEffect(() => {
    // The timer is not cleared when a client leaves meanwhile: the cheer always ends.
    if (allIn && !wasAllIn.current) {
      setCheer(true);
      clearTimeout(cheerTimer.current);
      cheerTimer.current = window.setTimeout(() => setCheer(false), CHEER_MS);
    }
    wasAllIn.current = allIn;
  }, [allIn]);
  useEffect(() => () => clearTimeout(cheerTimer.current), []);
  const downName = clients.clients.find((c) => c.state === "dropped" || c.state === "joinFailed");
  const panicName = alarm ?? (downName ? nameOf.current(downName.account) : null);
  const mood: Mood = panicName ? "panic" : cheer ? "cheering" : windowsOpen ? "playing" : "sleeping";
  const moodText = panicName ? t.mascotPanic(panicName) : cheer ? t.mascotCheering : windowsOpen ? t.mascotPlaying(windowsOpen) : t.mascotSleeping;

  // The accounts the pickers and the palette name: the saved ones in card order, the Roblox app's last.
  const people: Person[] = [
    // The @name too when the display name differs: two accounts often share one (Enderex and EnderexDev).
    ...accounts.map((a) => ({ key: String(a.id), name: a.displayName === a.name ? a.name : `${a.displayName} @${a.name}`, avatar: avatars[a.id] })),
    { key: "app", name: appUser ? `${t.robloxApp} (${appUser.displayName})` : t.robloxApp, avatar: appUser ? avatars[appUser.id] : undefined },
  ];
  // The camera wall: each account with a client window, in card order.
  const screenList = [...accounts.map((a) => ({ key: String(a.id), name: a.displayName })), { key: "app", name: appUser?.displayName ?? t.robloxApp }].filter((s) =>
    clientsOf(s.key).some((c) => c.window),
  );
  const closeScreens = useCallback(() => setScreens(false), []);
  const commands = (): Command[] => {
    // Two accounts may share a display name (the Roblox app's is often a saved one): the hint says which.
    const playable = [
      ...(appUser ? [{ key: "app", name: appUser.displayName, who: t.robloxApp, expired: false }] : []),
      ...accounts.map((a) => ({ key: String(a.id), name: a.displayName, who: `@${a.name}`, expired: a.expired })),
    ];
    // More than one account opens only with the multi on (launch.rs).
    const canRun = (n: number) => !playingAll && (n < 2 || multi === true);
    const out: Command[] = [];
    if (game) for (const a of playable.filter((a) => !a.expired)) out.push({ id: `play-${a.key}`, label: t.cmdPlay(a.name), hint: `${a.who} · ${game.name}`, run: () => play(a.key) });
    if (game && multi === true && !playingAll) out.push({ id: "play-all", label: t.playAll, hint: game.name, run: playAll });
    if (game) for (const g of lists.groups.filter((g) => canRun(g.accounts.length))) out.push({ id: `group-${g.id}`, label: t.cmdPlayGroup(g.name), hint: game.name, run: () => playGroup(g) });
    for (const pr of lists.profiles.filter((p) => canRun(p.accounts.length))) out.push({ id: `profile-${pr.id}`, label: t.cmdRunProfile(pr.name), hint: pr.game.name, run: () => runProfile(pr) });
    for (const r of recents) out.push({ id: `game-${r.placeId}-${r.linkCode}`, label: t.cmdGame(r.name), run: () => pickGame(r) });
    if (windowsOpen) out.push({ id: "grid", label: t.grid, run: () => api.grid().then((n) => notify(t.gridDone(n), "info")).catch(toast) });
    if (screenList.length) out.push({ id: "screens", label: t.wallTitle, run: () => setScreens(true) });
    if (screenList.length) out.push({ id: "print-all", label: t.printAll, run: () => print(null, t) });
    if (windowsOpen > 1) out.push({ id: "minimize", label: t.minimizeOthers, hint: "Ctrl+Shift+0", run: () => api.minimizeOthers().catch(toast) });
    for (const a of playable) {
      if (clientsOf(a.key).some((c) => c.window)) out.push({ id: `focus-${a.key}`, label: t.cmdFocus(a.name), hint: a.who, run: () => api.focusClient(a.key).catch(toast) });
      out.push({ id: `eco-${a.key}`, label: t.cmdEco(a.name, !clients.eco.includes(a.key)), hint: a.who, run: () => toggleEco(a.key) });
      out.push({ id: `rejoin-${a.key}`, label: t.cmdRejoin(a.name, !clients.rejoin.includes(a.key)), hint: a.who, run: () => toggleRejoin(a.key) });
      if (a.key !== "app" && !a.expired) out.push({ id: `site-${a.key}`, label: t.cmdSite(a.name), hint: a.who, run: () => api.openSite(a.key).catch(toast) });
    }
    out.push({ id: "add", label: t.addAccount, run: () => api.addAccount().catch(toast) });
    out.push({ id: "lang", label: t.cmdLanguage, run: () => changeLanguage(getLang() === "en" ? "pt" : "en") });
    return out;
  };

  return (
    <>
      <div className="aura" aria-hidden>
        <div className="aura-blob a" />
        <div className="aura-blob b" />
        <div className="aura-blob c" />
      </div>
      <div className="app">
        <header className="topbar">
          <div className="brand">
            <Mascot key={mood} mood={mood} size={50} />
            <div className="brand-text">
              <span className="brand-name">Quackblox</span>
              <span className={`brand-mood ${mood}`}>{moodText}</span>
            </div>
          </div>
          <MultiPill active={multi} autoClose={autoClose} stuck={stuck} />
          <button className="round-btn palette-btn" onClick={() => setPalette(true)} title={t.paletteButton} aria-label={t.paletteButton}>
            <CommandIcon size={16} />
          </button>
          <LanguageSwitch onChange={changeLanguage} />
        </header>

        <main className="panel page">
          <section className="block">
            <div className="block-head">
              <h2>{t.gameTitle}</h2>
              <span className="muted small">{t.gameHint}</span>
            </div>
            <GamePicker game={game} recents={recents} onPick={pickGame} onForget={forgetRecent} />
          </section>

          <section className="block">
            <div className="block-head">
              <h2>{t.accountsTitle}</h2>
              <span className="muted small">{t.accountsHint}</span>
              <label className="same-server" title={t.sameServerTitle}>
                <Toggle on={sameServer} onChange={toggleSameServer} />
                {t.sameServer}
              </label>
              <button className="pill-btn ghost" disabled={!game || playable < 2 || playingAll || multi !== true} onClick={playAll} title={t.playAllTitle}>
                <UsersIcon size={16} /> {t.playAll}
              </button>
            </div>

            {windowsOpen > 0 && (
              <div className="windows-bar">
                <span className="live-dot" />
                <span className="windows-count">{t.windowsOpen(windowsOpen)}</span>
                <button className="pill-btn ghost small" onClick={() => api.grid().then((n) => notify(t.gridDone(n), "info")).catch(toast)} title={t.gridTitle}>
                  <GridIcon size={15} /> {t.grid}
                </button>
                <button className="pill-btn ghost small" disabled={windowsOpen < 2} onClick={() => api.minimizeOthers().catch(toast)} title={t.minimizeOthersTitle}>
                  <MinimizeIcon size={15} /> {t.minimizeOthers}
                </button>
                <button className="pill-btn ghost small" onClick={() => setScreens(true)} title={t.screensTitle}>
                  <ScreenIcon size={15} /> {t.screens}
                </button>
              </div>
            )}

            <div className="cards">
              <AccountCard
                tag={
                  <>
                    <PcIcon size={13} /> {t.robloxApp}
                  </>
                }
                user={appUser}
                avatar={appUser ? avatars[appUser.id] : undefined}
                sub={appError ?? (appUser ? t.appAccountSub : t.readingAppLogin)}
                missing={!!appError}
                busy={busy.has("app")}
                canPlay={!!game && !!appUser}
                onPlay={() => play("app")}
                {...live("app")}
                menu={appUser ? [{ icon: <FollowIcon size={16} />, label: t.followFriend, run: () => setFriendsOf({ key: "app", name: appUser.displayName }) }] : undefined}
              />
              {accounts.map((a, i) => (
                <AccountCard
                  key={a.id}
                  user={a}
                  avatar={avatars[a.id]}
                  sub={a.expired ? t.loginExpired : ago(a.lastPlayed, t)}
                  missing={a.expired}
                  busy={busy.has(String(a.id))}
                  canPlay={!!game && !a.expired}
                  onPlay={() => play(String(a.id))}
                  onRelogin={a.expired ? () => api.addAccount().catch(toast) : undefined}
                  {...live(String(a.id), i)}
                  dragId={a.id}
                  dragging={dragId === a.id}
                  dragHandlers={dragHandlers(a)}
                  menu={[
                    ...(a.expired ? [] : [{ icon: <FollowIcon size={16} />, label: t.followFriend, run: () => setFriendsOf({ key: String(a.id), name: a.displayName }) }]),
                    ...(a.expired ? [] : [{ icon: <GlobeIcon size={16} />, label: t.openSite, run: () => api.openSite(String(a.id)).catch(toast) }]),
                    ...(i > 0 ? [{ icon: <ArrowUpIcon size={16} />, label: t.moveUp, run: () => moveUp(a, i) }] : []),
                    { icon: <TrashIcon size={16} />, label: t.removeAccount, run: () => remove(a), danger: true },
                  ]}
                />
              ))}
              <button className="card add-card" onClick={() => api.addAccount().catch(toast)}>
                <span className="add-icon">
                  <PlusIcon size={22} />
                </span>
                <span className="add-title">{t.addAccount}</span>
                <span className="muted small">{t.addAccountHint}</span>
              </button>
            </div>
          </section>

          <OrganizeBlock
            lists={lists}
            onSave={saveLists}
            people={people}
            game={game}
            eco={clients.eco}
            sameServer={sameServer}
            running={playingAll}
            multi={multi}
            onPlayGroup={playGroup}
            onRunProfile={runProfile}
          />
          <VigiaBlock settings={vigia} saving={savingSettings} onSave={saveSettings} />
          <ExtrasBlock phone={extras.phone} richPresence={extras.richPresence} saving={savingSettings} onSave={saveSettings} />
          <StatsBlock accounts={accounts} appUser={appUser} />
          <ErrorLookup />
        </main>

        <footer className="footer">
          <label className="footer-toggle">
            <Toggle on={startup} onChange={(v) => api.setStartup(v).then(setStartup).catch(toast)} />
            {t.startWithWindows}
          </label>
          <label className="footer-toggle" title={t.autoCloseTitle}>
            <Toggle
              on={autoClose}
              disabled={savingSettings}
              onChange={(v) => saveSettings({ autoClose: v })}
            />
            {t.autoClose}
          </label>
          <label className="footer-toggle" title={t.hotkeysTitle}>
            <Toggle on={hotkeys} disabled={savingSettings} onChange={(v) => saveSettings({ hotkeys: v })} />
            {t.hotkeys}
          </label>
          <UpdateLink />
        </footer>
      </div>
      {friendsOf && (
        <FriendsSheet
          account={friendsOf.key}
          name={friendsOf.name}
          onClose={() => setFriendsOf(null)}
          onPick={(g, friend) => {
            pickGame(g);
            setFriendsOf(null);
            notify(t.friendPicked(friend));
          }}
        />
      )}
      {screens && <ScreensSheet screens={screenList} onClose={closeScreens} />}
      {palette && <Palette commands={commands()} onClose={() => setPalette(false)} />}
      <Toasts />
    </>
  );
}

/** The version, and the update: ask for one, or install the one found. */
function UpdateLink() {
  const t = useT();
  const u = useUpdate();
  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion().then(setVersion);
  }, []);
  return (
    <span className="muted small" title={t.trayTitle}>
      Quackblox {version} ·{" "}
      {u.kind === "available" || u.kind === "starting" ? (
        <button className="update-link available" disabled={u.kind === "starting"} onClick={installUpdate}>
          {u.kind === "starting" ? t.updateStarting : t.updateTo(u.version)}
        </button>
      ) : (
        <button className="update-link" disabled={u.kind === "checking"} onClick={() => checkForUpdate()}>
          {u.kind === "checking" ? t.checking : t.checkUpdate}
        </button>
      )}
    </span>
  );
}

/** PT | EN, beside the multi's pill. */
function LanguageSwitch({ onChange }: { onChange: (l: Lang) => void }) {
  const t = useT();
  const lang = getLang();
  return (
    <div className="lang-switch" role="group" aria-label={t.languageLabel}>
      {(["pt", "en"] as const).map((l) => (
        <button key={l} className={lang === l ? "on" : ""} aria-pressed={lang === l} onClick={() => onChange(l)}>
          {l.toUpperCase()}
        </button>
      ))}
    </div>
  );
}

function MultiPill({ active, autoClose, stuck }: { active: boolean | null; autoClose: boolean; stuck: boolean }) {
  const t = useT();
  if (active === null) return null;
  // Closing on its own: unless that already failed, then the button is back.
  const closing = autoClose && !stuck;
  return (
    <div className={`multi-pill ${active ? "on" : "wait"}`} title={active ? t.multiOnTitle : t.multiWaitTitle}>
      <span className="dot" />
      {active ? t.multiOn : closing ? t.multiClosing : stuck ? t.multiStuck : t.multiBlocked}
      {!active && !closing && (
        <button className="pill-link" onClick={() => api.closeRobloxNow().catch(toast)}>
          {t.closeNow}
        </button>
      )}
    </div>
  );
}

function GamePicker({ game, recents, onPick, onForget }: { game: Game | null; recents: Game[]; onPick: (g: Game | null) => void; onForget: (g: Game) => void }) {
  const t = useT();
  const [text, setText] = useState("");
  const [loading, setLoading] = useState(false);
  const [browsing, setBrowsing] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  // Bumped by every search and every pick: a search that answers after a newer one, or after a
  // recent was picked, is dropped.
  const latest = useRef(0);

  const resolve = async (value: string) => {
    if (!value.trim()) return;
    const mine = ++latest.current;
    setLoading(true);
    try {
      const g = await api.game(value);
      if (mine !== latest.current) return;
      onPick(g);
      setText("");
    } catch (e) {
      if (mine === latest.current) toast(e);
    } finally {
      if (mine === latest.current) setLoading(false);
    }
  };

  const pickRecent = (g: Game) => {
    latest.current++;
    setLoading(false);
    onPick(g);
  };

  return (
    <div className="game-picker">
      {game ? (
        <div className="game-chosen">
          <GameIcon game={game} size={64} />
          <div className="game-chosen-text">
            <span className="game-name">{game.name}</span>
            <span className="muted small game-where">
              ID {game.placeId}
              {" · "}
              {game.linkCode ? (
                <>
                  <LockIcon size={13} /> {t.privateServer}
                </>
              ) : game.jobId ? (
                t.serverChosen
              ) : (
                t.anyServer
              )}
            </span>
          </div>
          {!game.linkCode && (
            <button className={`pill-btn ghost small ${browsing ? "on" : ""}`} onClick={() => setBrowsing((b) => !b)} title={t.serversTitle} aria-expanded={browsing}>
              <ServerIcon size={15} /> {t.servers}
            </button>
          )}
          <button className="round-btn" onClick={() => { onPick(null); setTimeout(() => input.current?.focus()); }} title={t.changeGame} aria-label={t.changeGame}>
            <CloseIcon size={16} />
          </button>
        </div>
      ) : (
        <form className="game-search" onSubmit={(e) => { e.preventDefault(); resolve(text); }}>
          <SearchIcon size={18} />
          <input
            ref={input}
            value={text}
            placeholder={t.gamePlaceholder}
            onChange={(e) => setText(e.target.value)}
            onPaste={(e) => {
              const pasted = e.clipboardData.getData("text");
              e.preventDefault();
              setText(pasted);
              resolve(pasted);
            }}
            autoFocus
            spellCheck={false}
          />
          <button className="pill-btn" disabled={loading || !text.trim()}>
            {loading ? t.searching : t.use}
          </button>
        </form>
      )}
      {game && browsing && !game.linkCode && <ServerBrowser game={game} onPick={onPick} />}
      {recents.length > 0 && (
        <div className="recents">
          <span className="muted small">{t.recents}</span>
          <div className="recent-row">
            {recents.map((r) => (
              <div key={`${r.placeId}-${r.jobId}-${r.linkCode}`} className={`recent ${game && sameGame(game, r) ? "on" : ""}`}>
                <button className="recent-pick" onClick={() => pickRecent(r)} title={r.name}>
                  <GameIcon game={r} size={24} />
                  <span>{r.name}</span>
                </button>
                <button className="recent-x" onClick={() => onForget(r)} aria-label={t.forgetRecent(r.name)}>
                  <CloseIcon size={12} />
                </button>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function GameIcon({ game, size }: { game: Game; size: number }) {
  return game.icon ? (
    <img className="game-icon" src={game.icon} width={size} height={size} alt="" />
  ) : (
    <span className="game-icon empty" style={{ width: size, height: size }} />
  );
}

type MenuItem = { icon: React.ReactNode; label: string; run: () => void; danger?: boolean };

function AccountCard(p: {
  user: User | null;
  avatar?: string;
  sub: string;
  tag?: React.ReactNode;
  missing: boolean;
  busy: boolean;
  canPlay: boolean;
  onPlay: () => void;
  onRelogin?: () => void;
  menu?: MenuItem[];
  presence?: Presence;
  /** Set while it waits for room in a full server. */
  waiting?: { playing: number; max: number };
  onStopWait: () => void;
  /** This account's open clients. */
  clients: Client[];
  eco: boolean;
  onEco: () => void;
  /** Goes back in by itself when it drops. */
  rejoin: boolean;
  onRejoin: () => void;
  onFocus: () => void;
  /** N of its Ctrl+Shift+N, when the hotkeys are on. */
  hotkey?: number;
  /** A saved account's card can be dragged to another place. */
  dragId?: number;
  dragging?: boolean;
  dragHandlers?: React.HTMLAttributes<HTMLDivElement>;
}) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false);
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [open]);

  return (
    <div
      className={`card account ${p.missing ? "missing" : ""} ${p.dragHandlers ? "movable" : ""} ${p.dragging ? "dragging" : ""}`}
      data-account={p.dragId}
      {...p.dragHandlers}
    >
      <div className="account-top">
        <span className="avatar-wrap">
          {p.avatar ? <img className="avatar" src={p.avatar} alt="" draggable={false} /> : <span className="avatar empty" />}
          {p.presence && <span className={`presence-dot ${p.presence.kind}`} title={presenceText(p.presence, t)} />}
          {p.hotkey && (
            <span className="hotkey-chip" title={t.hotkeyChip(p.hotkey)}>
              {p.hotkey}
            </span>
          )}
        </span>
        {p.tag && <span className="tag">{p.tag}</span>}
        {p.menu && (
          <div className="menu-wrap" ref={ref}>
            <button className="round-btn small-btn" onClick={() => setOpen((o) => !o)} aria-label={t.moreOptions} aria-haspopup="menu">
              <MoreIcon size={16} />
            </button>
            {open && (
              <div className="menu" role="menu">
                {p.menu.map((m) => (
                  <button key={m.label} role="menuitem" className={m.danger ? "danger" : ""} onClick={() => { setOpen(false); m.run(); }}>
                    {m.icon}
                    {m.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
      <div className="account-text">
        <span className="account-name">{p.user?.displayName ?? t.robloxAccount}</span>
        {p.user && <span className="muted small">@{p.user.name}</span>}
        <span className={`small ${p.missing ? "warn" : "muted"}`}>{p.sub}</span>
        {p.presence && p.presence.kind !== "offline" && <span className={`small presence-text ${p.presence.kind}`}>{presenceText(p.presence, t)}</span>}
      </div>
      {p.clients.length > 0 && <LiveLine clients={p.clients} onFocus={p.onFocus} />}
      <div className="card-actions">
        {p.onRelogin ? (
          <button className="pill-btn ghost wide" onClick={p.onRelogin}>
            <LoginIcon size={16} /> {t.signInAgain}
          </button>
        ) : p.waiting ? (
          <button className="pill-btn ghost wide waiting" onClick={p.onStopWait} title={t.waitingSeatTitle}>
            {t.waitingSeat(p.waiting.playing, p.waiting.max)}
          </button>
        ) : (
          <button className="pill-btn wide" disabled={!p.canPlay || p.busy} onClick={p.onPlay}>
            <PlayIcon size={16} /> {p.busy ? t.opening : t.play}
          </button>
        )}
        <button className={`round-btn rejoin-btn ${p.rejoin ? "on" : ""}`} onClick={p.onRejoin} title={p.rejoin ? t.rejoinOn : t.rejoinOff} aria-label={t.rejoinLabel} aria-pressed={p.rejoin}>
          <RejoinIcon size={16} />
        </button>
        <button className={`round-btn eco-btn ${p.eco ? "on" : ""}`} onClick={p.onEco} title={p.eco ? t.ecoOn : t.ecoOff} aria-label={t.ecoLabel} aria-pressed={p.eco}>
          <LeafIcon size={16} />
        </button>
      </div>
    </div>
  );
}

/** The open clients of an account: how many, their CPU and RAM summed, a click to bring one forward. */
function LiveLine({ clients, onFocus }: { clients: Client[]; onFocus: () => void }) {
  const t = useT();
  const cpu = clients.reduce((n, c) => n + c.cpu, 0);
  const ram = clients.reduce((n, c) => n + c.ram, 0);
  const shown = clients.some((c) => c.window);
  // The worst state among its clients names the line: one dropped is what he needs to see.
  const down = clients.find((c) => c.state === "dropped") ?? clients.find((c) => c.state === "joinFailed");
  const starting = !down && clients.some((c) => c.state === "starting");
  const label = down
    ? down.state === "dropped"
      ? t.stateDropped(down.reason)
      : t.stateJoinFailed
    : starting
      ? t.stateStarting
      : clients.length > 1
        ? t.openMany(clients.length)
        : t.stateInGame;
  return (
    <button className={`live-line ${down ? "down" : starting || !shown ? "starting" : ""}`} onClick={onFocus} disabled={!shown} title={t.focusWindow}>
      <span className="live-dot" />
      <span className="live-label">{label}</span>
      <span className="live-stats">
        <span className="live-stat">
          CPU <b>{cpu < 10 ? cpu.toFixed(1) : Math.round(cpu)}%</b>
        </span>
        <span className="live-stat">
          RAM <b>{memory(ram)}</b>
        </span>
      </span>
    </button>
  );
}

function presenceText(p: Presence, t: Texts) {
  if (p.kind === "game") return t.presenceGame(p.location);
  if (p.kind === "site") return t.presenceSite;
  if (p.kind === "studio") return t.presenceStudio(p.location === "Studio" ? "" : p.location);
  return t.presenceOffline;
}

/** The game's public servers, to pick one, or the emptiest or fullest with room at once. Also
 * takes a pasted JobId. */
function ServerBrowser({ game, onPick }: { game: Game; onPick: (g: Game) => void }) {
  const t = useT();
  const [fullest, setFullest] = useState(false);
  const [list, setList] = useState<Server[]>([]);
  const [next, setNext] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [job, setJob] = useState("");
  // A page that answers after the order changed is dropped.
  const latest = useRef(0);

  const load = async (cursor: string | null) => {
    const mine = ++latest.current;
    setLoading(true);
    try {
      const page = await api.servers(game.placeId, fullest, false, cursor);
      if (mine !== latest.current) return;
      setList((l) => (cursor ? [...l, ...page.servers] : page.servers));
      setNext(page.next);
    } catch (e) {
      if (mine === latest.current) toast(e);
    } finally {
      if (mine === latest.current) setLoading(false);
    }
  };
  useEffect(() => {
    setList([]);
    load(null);
  }, [game.placeId, fullest]);

  const choose = (jobId: string | null) => {
    onPick({ ...game, jobId, linkCode: null });
    if (jobId) notify(t.serverPicked);
  };
  const quick = (fullestFirst: boolean) =>
    api
      .servers(game.placeId, fullestFirst, true, null)
      .then((p) => (p.servers[0] ? choose(p.servers[0].id) : notify(t.noRoom, "info")))
      .catch(toast);

  return (
    <div className="servers">
      <div className="servers-head">
        <div className="lang-switch" role="group">
          <button className={fullest ? "" : "on"} aria-pressed={!fullest} onClick={() => setFullest(false)}>
            {t.emptiestFirst}
          </button>
          <button className={fullest ? "on" : ""} aria-pressed={fullest} onClick={() => setFullest(true)}>
            {t.fullestFirst}
          </button>
        </div>
        <button className="pill-btn ghost small" onClick={() => quick(false)}>
          {t.pickEmptiest}
        </button>
        <button className="pill-btn ghost small" onClick={() => quick(true)}>
          {t.pickFullest}
        </button>
        <button className="pill-btn ghost small" disabled={!game.jobId} onClick={() => choose(null)}>
          {t.anyServerButton}
        </button>
      </div>
      <div className="server-list">
        {list.map((s) => {
          const full = s.playing >= s.maxPlayers;
          const on = game.jobId === s.id;
          return (
            <div key={s.id} className={`server ${on ? "on" : ""}`}>
              <span className="server-fill" title={`${s.playing}/${s.maxPlayers}`}>
                <span style={{ width: `${Math.min(100, (s.playing / Math.max(1, s.maxPlayers)) * 100)}%` }} />
              </span>
              <span className="server-count">
                {s.playing}/{s.maxPlayers}
              </span>
              <span className="server-stat muted">{s.ping != null ? `${s.ping} ms` : "–"}</span>
              <span className="server-stat muted">{s.fps != null ? `${Math.round(s.fps)} FPS` : "–"}</span>
              <button className="pill-btn ghost small" disabled={on} onClick={() => choose(s.id)}>
                {on ? t.chosen : full ? t.full : t.choose}
              </button>
            </div>
          );
        })}
        {!loading && list.length === 0 && <span className="muted small">{t.noServers}</span>}
      </div>
      <div className="servers-foot">
        <span className="muted small">{t.pingNote}</span>
        {next && (
          <button className="pill-btn ghost small" disabled={loading} onClick={() => load(next)}>
            {loading ? t.loading : t.loadMore}
          </button>
        )}
      </div>
      <form
        className="job-paste"
        onSubmit={(e) => {
          e.preventDefault();
          if (JOB_ID.test(job.trim())) {
            choose(job.trim().toLowerCase());
            setJob("");
          }
        }}
      >
        <input value={job} placeholder={t.pasteJob} onChange={(e) => setJob(e.target.value)} spellCheck={false} />
        <button className="pill-btn small" disabled={!JOB_ID.test(job.trim())}>
          {t.use}
        </button>
      </form>
    </div>
  );
}

/** The friends of an account who are in a game now, to go to the server of one. */
function FriendsSheet(p: { account: string; name: string; onClose: () => void; onPick: (g: Game, friend: string) => void }) {
  const t = useT();
  const [friends, setFriends] = useState<Friend[] | null>(null);
  const [faces, setFaces] = useState<Record<number, string>>({});
  const [going, setGoing] = useState<number | null>(null);
  useEffect(() => {
    api
      .friendsInGame(p.account)
      .then((list) => {
        setFriends(list);
        if (list.length) api.avatars(list.map((f) => f.id)).then((pairs) => setFaces(Object.fromEntries(pairs)));
      })
      .catch((e) => {
        toast(e);
        p.onClose();
      });
  }, [p.account]);
  useEffect(() => {
    const esc = (e: KeyboardEvent) => e.key === "Escape" && p.onClose();
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [p.onClose]);

  const go = async (f: Friend) => {
    setGoing(f.id);
    try {
      const g = await api.game(`https://www.roblox.com/games/start?placeId=${f.placeId}&gameInstanceId=${f.jobId}`);
      p.onPick(g, f.displayName || f.name);
    } catch (e) {
      toast(e);
    } finally {
      setGoing(null);
    }
  };

  return (
    <div className="sheet-backdrop" onMouseDown={(e) => e.target === e.currentTarget && p.onClose()}>
      <div className="sheet" role="dialog" aria-label={t.friendsTitle(p.name)}>
        <div className="sheet-head">
          <h3>{t.friendsTitle(p.name)}</h3>
          <button className="round-btn small-btn" onClick={p.onClose} aria-label={t.close}>
            <CloseIcon size={16} />
          </button>
        </div>
        {friends === null ? (
          <span className="muted small">{t.loading}</span>
        ) : friends.length === 0 ? (
          <span className="muted small">{t.noFriendsInGame}</span>
        ) : (
          <div className="friend-list">
            {friends.map((f) => (
              <div key={f.id} className="friend">
                {faces[f.id] ? <img className="friend-face" src={faces[f.id]} alt="" /> : <span className="friend-face" />}
                <div className="friend-text">
                  <span className="friend-name">{f.displayName || f.name}</span>
                  <span className="muted small">
                    {f.location}
                    {!f.jobId && ` · ${t.serverHidden}`}
                  </span>
                </div>
                <button className="pill-btn small" disabled={!f.jobId || going !== null} onClick={() => go(f)}>
                  {going === f.id ? t.searching : t.goWith}
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

/** A Roblox error code typed or pasted, explained. */
function ErrorLookup() {
  const t = useT();
  const [code, setCode] = useState("");
  const n = Number(code.replace(/\D/g, ""));
  return (
    <section className="error-lookup">
      <label className="muted small" htmlFor="error-code">
        {t.errorLookup}
      </label>
      <input id="error-code" value={code} inputMode="numeric" placeholder={t.errorPlaceholder} onChange={(e) => setCode(e.target.value)} />
      {n > 0 && <span className="small">{errorText(n, t)}</span>}
    </section>
  );
}

/** What to do when an account drops: the notices, and the login check. */
function VigiaBlock({ settings, saving, onSave }: { settings: VigiaSettings; saving: boolean; onSave: (change: Partial<VigiaSettings>) => Promise<boolean> }) {
  const t = useT();
  const [webhook, setWebhook] = useState(settings.discordWebhook);
  const [checking, setChecking] = useState(false);
  useEffect(() => setWebhook(settings.discordWebhook), [settings.discordWebhook]);
  const typed = webhook.trim();
  const checkNow = () => {
    setChecking(true);
    api
      .checkLogins()
      .then((names) => (names.length ? toast(t.loginsExpired(names.join(", "))) : notify(t.loginsFine, "info")))
      .catch(toast)
      .finally(() => setChecking(false));
  };
  return (
    <section className="block vigia">
      <div className="block-head">
        <h2>{t.vigiaTitle}</h2>
        <span className="muted small">{t.vigiaHint}</span>
      </div>
      <div className="vigia-rows">
        <div className="vigia-row">
          <label className="footer-toggle">
            <Toggle on={settings.alertWindows} disabled={saving} onChange={(v) => onSave({ alertWindows: v })} />
            {t.alertWindows}
          </label>
          <button className="pill-btn ghost small" onClick={() => api.testWindows().catch(toast)}>
            {t.test}
          </button>
        </div>
        <div className="vigia-row">
          <label className="footer-toggle" title={t.alertDiscordTitle}>
            <Toggle
              on={settings.alertDiscord}
              disabled={saving}
              onChange={(v) => onSave(v && typed !== settings.discordWebhook ? { alertDiscord: v, discordWebhook: typed } : { alertDiscord: v })}
            />
            {t.alertDiscord}
          </label>
        </div>
        <form
          className="job-paste webhook"
          onSubmit={(e) => {
            e.preventDefault();
            onSave({ discordWebhook: typed, alertDiscord: typed ? settings.alertDiscord : false }).then((ok) => ok && notify(t.webhookSaved, "info"));
          }}
        >
          <input type="password" value={webhook} placeholder={t.webhookPlaceholder} onChange={(e) => setWebhook(e.target.value)} spellCheck={false} autoComplete="off" />
          <button className="pill-btn ghost small" type="button" disabled={!typed} onClick={() => api.testDiscord(typed).then(() => notify(t.testSent, "info")).catch(toast)}>
            {t.test}
          </button>
          <button className="pill-btn small" disabled={saving || typed === settings.discordWebhook}>
            {t.webhookSave}
          </button>
        </form>
        <div className="vigia-row">
          <label className="footer-toggle" title={t.loginCheckTitle}>
            <Toggle on={settings.loginCheck} disabled={saving} onChange={(v) => onSave({ loginCheck: v })} />
            {t.loginCheck}
          </label>
          <button className="pill-btn ghost small" disabled={checking} onClick={checkNow}>
            {checking ? t.checking2 : t.checkNow}
          </button>
        </div>
      </div>
    </section>
  );
}

/** Hours inside a game per account and per game, the most played first. */
function StatsBlock({ accounts, appUser }: { accounts: Account[]; appUser: User | null }) {
  const t = useT();
  const [stats, setStats] = useState<Stats | null>(null);
  const [names, setNames] = useState<Record<string, string>>({});
  const [confirm, setConfirm] = useState(false);
  const asked = useRef(new Set<string>());
  const failed = useRef(new Map<string, number>());
  useEffect(() => {
    const load = () => document.visibilityState === "visible" && api.stats().then(setStats).catch(() => {});
    load();
    const timer = setInterval(load, 60000);
    return () => clearInterval(timer);
  }, []);
  // The name of each game shown (the three most played of each account), asked once.
  useEffect(() => {
    if (!stats) return;
    const shown = Object.values(stats.users).flatMap((p) =>
      Object.entries(p)
        .sort((a, b) => b[1] - a[1])
        .slice(0, 3)
        .map(([id]) => id),
    );
    const places = [...new Set(shown)].filter((id) => !asked.current.has(id));
    places.forEach((id) => {
      asked.current.add(id);
      api
        .game(id)
        .then((g) => setNames((n) => ({ ...n, [id]: g.name })))
        // Asked again at the next reload (a minute), 3 times at most: a refusal at the start is
        // not for ever, and a game gone for good is not asked every minute.
        .catch(() => {
          const n = (failed.current.get(id) ?? 0) + 1;
          failed.current.set(id, n);
          if (n < 3) asked.current.delete(id);
        });
    });
  }, [stats]);
  useEffect(() => {
    if (!confirm) return;
    const timer = setTimeout(() => setConfirm(false), 4000);
    return () => clearTimeout(timer);
  }, [confirm]);
  if (!stats) return null;
  const who = (id: string) => accounts.find((a) => String(a.id) === id)?.displayName ?? (appUser && String(appUser.id) === id ? appUser.displayName : `#${id}`);
  const rows = Object.entries(stats.users)
    .map(([id, places]) => {
      const games = Object.entries(places).sort((a, b) => b[1] - a[1]);
      return { id, games, total: games.reduce((n, [, secs]) => n + secs, 0) };
    })
    .filter((r) => r.total > 0)
    .sort((a, b) => b.total - a.total);
  const since = new Date(stats.since * 1000).toLocaleDateString(getLang() === "en" ? "en" : "pt-BR");
  return (
    <section className="block stats">
      <div className="block-head">
        <h2>{t.statsTitle}</h2>
        <span className="muted small">{t.statsHint(since)}</span>
        {rows.length > 0 && (
          <button
            className="pill-btn ghost small"
            onClick={() => {
              if (!confirm) return setConfirm(true);
              setConfirm(false);
              api.resetStats().then(setStats).catch(toast);
            }}
          >
            {confirm ? t.statsResetConfirm : t.statsReset}
          </button>
        )}
      </div>
      {rows.length === 0 ? (
        <span className="muted small">{t.statsEmpty}</span>
      ) : (
        <div className="stats-list">
          {rows.map((r) => (
            <div key={r.id} className="stats-row">
              <span className="stats-name">{who(r.id)}</span>
              <span className="stats-total">{t.hours(r.total)}</span>
              <span className="stats-games muted small">
                {r.games
                  .slice(0, 3)
                  .map(([place, secs]) => `${names[place] ?? place} ${t.hours(secs)}`)
                  .join(" · ")}
                {r.games.length > 3 && ` · ${t.statsOthers(r.games.length - 3)}`}
              </span>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
