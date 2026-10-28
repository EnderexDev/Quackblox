import { useCallback, useEffect, useRef, useState } from "react";
import { api, on, type Account, type Client, type Clients, type Game, type User } from "./api";
import {
  ArrowUpIcon,
  CloseIcon,
  GridIcon,
  LeafIcon,
  LoginIcon,
  MinimizeIcon,
  MoreIcon,
  PcIcon,
  PlayIcon,
  PlusIcon,
  SearchIcon,
  TrashIcon,
  UsersIcon,
} from "./components/icons";
import { getVersion } from "@tauri-apps/api/app";
import { Toggle } from "./components/Toggle";
import { Mascot } from "./components/Mascot";
import { checkForUpdate, installUpdate, useUpdate } from "./update";
import { notify, toast, Toasts } from "./toast";
import { getLang, setLang, t as texts, useT, type Lang, type Texts } from "./i18n";

const NO_CLIENTS: Clients = { clients: [], eco: [], others: 0 };

const RECENTS_KEY = "quackblox.recents";
const GAME_KEY = "quackblox.game";
/** Between two clients of "Jogar com todas", as the Account Manager does (2.5 s there). */
const JOIN_GAP_MS = 3000;

const readJson = <T,>(key: string, fallback: T): T => {
  try {
    return JSON.parse(localStorage.getItem(key) ?? "") as T;
  } catch {
    return fallback;
  }
};

/** What `play` takes for a game: the id, or a link that also names the server. */
const gameArg = (g: Game) => (g.jobId ? `https://www.roblox.com/games/start?placeId=${g.placeId}&gameInstanceId=${g.jobId}` : String(g.placeId));
const sameGame = (a: Game, b: Game) => a.placeId === b.placeId && a.jobId === b.jobId;
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
  const [recents, setRecents] = useState<Game[]>(() => readJson<Game[]>(RECENTS_KEY, []));
  const [busy, setBusy] = useState<Set<string>>(new Set());
  const [startup, setStartup] = useState(false);
  const [autoClose, setAutoClose] = useState(true);
  const [stuck, setStuck] = useState(false);
  const [hotkeys, setHotkeys] = useState(true);
  const [savingSettings, setSavingSettings] = useState(false);
  const [clients, setClients] = useState<Clients>(NO_CLIENTS);

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

  const pickGame = (g: Game | null) => {
    setGame(g);
    localStorage.setItem(GAME_KEY, JSON.stringify(g));
    if (!g) return;
    setRecents((rs) => {
      const next = [g, ...rs.filter((r) => !sameGame(r, g))].slice(0, 8);
      localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
      return next;
    });
  };

  const forgetRecent = (g: Game) =>
    setRecents((rs) => {
      const next = rs.filter((r) => !sameGame(r, g));
      localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
      return next;
    });

  // Read synchronously by play(), so a second click on a busy account does nothing.
  const busyNow = useRef(new Set<string>());
  const [playingAll, setPlayingAll] = useState(false);

  const play = async (key: string, label: string) => {
    if (!game) return toast(t.pickGameFirst);
    if (busyNow.current.has(key)) return;
    busyNow.current.add(key);
    setBusy((b) => new Set(b).add(key));
    try {
      const r = await api.play(key, gameArg(game));
      notify(r.multi ? t.openingGame(game.name, label) : t.openingWithoutMulti(label), r.multi ? "success" : "info");
      if (key !== "app") loadAccounts();
    } catch (e) {
      toast(`${label}: ${String(e)}`);
      if (key !== "app") loadAccounts();
    } finally {
      // The client takes a few seconds to show; the button stays busy meanwhile.
      setTimeout(() => {
        busyNow.current.delete(key);
        setBusy((b) => {
          const n = new Set(b);
          n.delete(key);
          return n;
        });
      }, 4000);
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
      for (const [i, [key, label]] of list.entries()) {
        if (i) await sleep(JOIN_GAP_MS);
        await play(key, label);
      }
    } finally {
      setPlayingAll(false);
    }
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
            <span className="muted small">
              ID {game.placeId}
              {" · "}
              {game.jobId ? t.serverChosen : t.anyServer}
            </span>
          </div>
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
      {recents.length > 0 && (
        <div className="recents">
          <span className="muted small">{t.recents}</span>
          <div className="recent-row">
            {recents.map((r) => (
              <div key={`${r.placeId}-${r.jobId}`} className={`recent ${game && sameGame(game, r) ? "on" : ""}`}>
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
      </div>
      {p.clients.length > 0 && <LiveLine clients={p.clients} onFocus={p.onFocus} />}
      <div className="card-actions">
        {p.onRelogin ? (
          <button className="pill-btn ghost wide" onClick={p.onRelogin}>
            <LoginIcon size={16} /> {t.signInAgain}
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
