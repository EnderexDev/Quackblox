import { useCallback, useEffect, useRef, useState } from "react";
import { api, on, type Account, type Client, type Clients, type Friend, type Game, type Presence, type Server, type User } from "./api";
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
  SearchIcon,
  ServerIcon,
  TrashIcon,
  UsersIcon,
} from "./components/icons";
import { getVersion } from "@tauri-apps/api/app";
import { Toggle } from "./components/Toggle";
import { Mascot } from "./components/Mascot";
import { checkForUpdate, installUpdate, useUpdate } from "./update";
import { notify, toast, Toasts } from "./toast";
import { getLang, setLang, t as texts, useT, type Lang, type Texts } from "./i18n";
import { errorNames } from "./errorCodes";

const NO_CLIENTS: Clients = { clients: [], eco: [], others: 0 };

const RECENTS_KEY = "quackblox.recents";
const GAME_KEY = "quackblox.game";
const SAME_SERVER_KEY = "quackblox.sameServer";
/** Between two clients of "Jogar com todas", as the Account Manager does (2.5 s there). */
const JOIN_GAP_MS = 3000;
/** How often the accounts' presence is asked while the window shows (15 in a row passed, 28/10/26). */
const PRESENCE_MS = 20000;
/** How often a full server is looked at again while an account waits for room in it. */
const SEAT_MS = 15000;
/** How long "Mesmo servidor" waits for the first account to join a server. */
const FIRST_SERVER_MS = 90000;
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
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

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
  const [savingSettings, setSavingSettings] = useState(false);
  const [clients, setClients] = useState<Clients>(NO_CLIENTS);
  const [presence, setPresence] = useState<Record<string, Presence>>({});
  const [sameServer, setSameServer] = useState(() => localStorage.getItem(SAME_SERVER_KEY) === "1");
  /** The accounts waiting for room in a full server, with how full it was at the last look. */
  const [waiting, setWaiting] = useState<Record<string, { playing: number; max: number }>>({});
  const [friendsOf, setFriendsOf] = useState<{ key: string; name: string } | null>(null);
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
      // Why a client left, from its log. Leaving on purpose (285) needs no notice.
      on.clientExited((e) => {
        if (e.reason === null || e.reason === 285) return;
        toast(texts().clientLeft(nameOf.current(e.account), errorText(e.reason, texts())));
      }),
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
      if (document.visibilityState === "visible") api.presence().then(setPresence).catch(() => {});
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

  const forgetRecent = (g: Game) =>
    setRecents((rs) => {
      const next = rs.filter((r) => !sameRecent(r, g));
      localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
      return next;
    });

  // Read synchronously by play(), so a second click on a busy account does nothing.
  const busyNow = useRef(new Set<string>());
  const [playingAll, setPlayingAll] = useState(false);

  // An account waiting for room is in here; taking it out stops the wait.
  const waitingNow = useRef(new Set<string>());
  const stopWaiting = (key: string) => {
    waitingNow.current.delete(key);
    setWaiting(({ [key]: _, ...rest }) => rest);
  };

  /** Holds the join while the picked server is full, looking again every 15 s. False when he
   * stopped the wait. A server not in the list (it ended, or it is past the first pages) does not
   * hold the join: Roblox has the last word on it. */
  const waitForSeat = async (key: string, label: string, g: Game) => {
    const clear = () => {
      waitingNow.current.delete(key);
      setWaiting(({ [key]: _, ...rest }) => rest);
    };
    for (let looked = 0; ; looked++) {
      const seat = await api.serverSeat(g.placeId, g.jobId!).catch(() => null);
      // A failed look while already waiting keeps waiting; before that, the join goes ahead.
      if (seat ? !seat.found || seat.playing < seat.maxPlayers : looked === 0) {
        if (looked) notify(texts().seatOpened(label));
        clear();
        return true;
      }
      if (looked === 0) waitingNow.current.add(key);
      if (seat) setWaiting((w) => ({ ...w, [key]: { playing: seat.playing, max: seat.maxPlayers } }));
      await sleep(SEAT_MS);
      if (!waitingNow.current.has(key)) {
        notify(texts().waitStopped(label), "info");
        clear();
        return false;
      }
    }
  };

  /** Opens a client of the account in the game picked, or in `target`. True when it opened. */
  const play = async (key: string, label: string, target: Game | null = game) => {
    if (!target) {
      toast(t.pickGameFirst);
      return false;
    }
    if (busyNow.current.has(key)) return false;
    let launched = false;
    busyNow.current.add(key);
    setBusy((b) => new Set(b).add(key));
    try {
      if (target.jobId && !target.linkCode && !(await waitForSeat(key, label, target))) return false;
      launched = true;
      const r = await api.play(key, gameArg(target));
      notify(r.multi ? texts().openingGame(target.name, label) : texts().openingWithoutMulti(label), r.multi ? "success" : "info");
      if (key !== "app") loadAccounts();
      return true;
    } catch (e) {
      toast(`${label}: ${String(e)}`);
      if (key !== "app") loadAccounts();
      return false;
    } finally {
      // The client takes a few seconds to show; the button stays busy meanwhile. A wait for room
      // that he stopped opened nothing, so the button comes back at once.
      setTimeout(() => {
        busyNow.current.delete(key);
        setBusy((b) => {
          const n = new Set(b);
          n.delete(key);
          return n;
        });
      }, launched ? 4000 : 0);
    }
  };

  const playAll = async () => {
    if (playingAll) return;
    setPlayingAll(true);
    const list: [string, string][] = [
      ...(appUser ? [["app", appUser.displayName] as [string, string]] : []),
      ...accounts.filter((a) => !a.expired && a.id !== appUser?.id).map((a) => [String(a.id), a.displayName] as [string, string]),
    ];
    try {
      let target = game;
      for (const [i, [key, label]] of list.entries()) {
        if (i) await sleep(JOIN_GAP_MS);
        const opened = await play(key, label, target);
        // "Mesmo servidor" with no server picked: the others go where the first one landed.
        if (i === 0 && opened && sameServer && target && !target.jobId && !target.linkCode && list.length > 1) {
          notify(t.waitingFirstServer(label), "info");
          const where = await firstServer(key);
          if (where) {
            target = { ...target, ...where };
            notify(texts().sameServerFound);
          } else notify(texts().sameServerTimeout, "info");
        }
      }
    } finally {
      setPlayingAll(false);
    }
  };

  /** The server the account's new client joined: from its log, which says it as soon as it joins,
   * or else from its presence. Null after 90 s. */
  const firstServer = async (key: string) => {
    for (let waited = 0; waited < FIRST_SERVER_MS; waited += 2000) {
      await sleep(2000);
      const s = await api.clientServer(key).catch(() => null);
      if (s) return s;
    }
    const p = (await api.presence().catch(() => ({}) as Record<string, Presence>))[key];
    return p?.jobId && p.placeId ? { placeId: p.placeId, jobId: p.jobId } : null;
  };

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
      .setSettings({ autoClose, language: l, hotkeys })
      .then((s) => setLang(s.language))
      .catch((e) => {
        setLang(before);
        toast(e);
      });
  };

  // The window sends every setting each time (settings.rs): the ones not changing go as they are.
  const saveSettings = (change: { autoClose?: boolean; hotkeys?: boolean }) => {
    setSavingSettings(true);
    api
      .setSettings({ autoClose, hotkeys, language: getLang(), ...change })
      .then((s) => {
        setAutoClose(s.autoClose);
        setHotkeys(s.hotkeys);
      })
      .catch(toast)
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
  /** `place`: the saved account's place, from 0, for its Ctrl+Shift number; none for the app's. */
  const live = (key: string, place?: number) => ({
    presence: presence[key],
    waiting: waiting[key],
    onStopWait: () => stopWaiting(key),
    clients: clientsOf(key),
    eco: clients.eco.includes(key),
    onEco: () => toggleEco(key),
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
            <Mascot key={windowsOpen ? "playing" : "sleeping"} mood={windowsOpen ? "playing" : "sleeping"} size={50} />
            <div className="brand-text">
              <span className="brand-name">Quackblox</span>
              <span className="brand-mood">{windowsOpen ? t.mascotPlaying(windowsOpen) : t.mascotSleeping}</span>
            </div>
          </div>
          <MultiPill active={multi} autoClose={autoClose} stuck={stuck} />
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
                onPlay={() => appUser && play("app", appUser.displayName)}
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
                  onPlay={() => play(String(a.id), a.displayName)}
                  onRelogin={a.expired ? () => api.addAccount().catch(toast) : undefined}
                  {...live(String(a.id), i)}
                  dragId={a.id}
                  dragging={dragId === a.id}
                  dragHandlers={dragHandlers(a)}
                  menu={[
                    ...(a.expired ? [] : [{ icon: <FollowIcon size={16} />, label: t.followFriend, run: () => setFriendsOf({ key: String(a.id), name: a.displayName }) }]),
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
  return (
    <button className={`live-line ${shown ? "" : "starting"}`} onClick={onFocus} disabled={!shown} title={t.focusWindow}>
      <span className="live-dot" />
      <span className="live-label">{clients.length > 1 ? t.openMany(clients.length) : t.openOne}</span>
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
