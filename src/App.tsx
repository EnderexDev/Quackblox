import { useCallback, useEffect, useRef, useState } from "react";
import { api, on, type Account, type Game, type User } from "./api";
import {
  ArrowUpIcon,
  CloseIcon,
  DuckIcon,
  LoginIcon,
  MoreIcon,
  PcIcon,
  PlayIcon,
  PlusIcon,
  SearchIcon,
  TrashIcon,
  UsersIcon,
} from "./components/icons";
import { notify, toast, Toasts } from "./toast";

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

function ago(seconds: number | null) {
  if (!seconds) return "Ainda não jogou por aqui";
  const s = Date.now() / 1000 - seconds;
  if (s < 60) return "Jogou agora";
  if (s < 3600) return `Jogou há ${Math.floor(s / 60)} min`;
  if (s < 86400) return `Jogou há ${Math.floor(s / 3600)} h`;
  const d = Math.floor(s / 86400);
  return `Jogou há ${d} ${d === 1 ? "dia" : "dias"}`;
}

export default function App() {
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
  const [savingAutoClose, setSavingAutoClose] = useState(false);
  const [stuck, setStuck] = useState(false);

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
    api.settings().then((s) => setAutoClose(s.autoClose));
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
      on.robloxClosed(() => notify("Fechei o Roblox que estava aberto pra ligar o multi", "info")),
      on.accountAdded((u) => {
        notify(`${u.displayName} entrou no Quackblox`);
        loadAccounts();
      }),
      on.accountError(toast),
    ];
    // Read only once every listener is in, so nothing that happens in between is missed. A
    // Roblox closed before the window loaded still gets its notice.
    Promise.all(offs).then(() =>
      api.multiStatus().then((s) => {
        if (!heard) setMulti(s.active);
        // Only ever raises it: a "multi-stuck" heard meanwhile is never undone by an older read.
        setStuck((was) => was || s.stuck);
        if (s.closed > 0) notify("Fechei o Roblox que estava aberto pra ligar o multi", "info");
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
    if (!game) return toast("Escolhe um jogo primeiro, lá em cima");
    if (busyNow.current.has(key)) return;
    busyNow.current.add(key);
    setBusy((b) => new Set(b).add(key));
    try {
      const r = await api.play(key, gameArg(game));
      notify(r.multi ? `Abrindo ${game.name} com ${label}` : `Abrindo com ${label}. O multi está esperando, então o Roblox que já estava aberto vai fechar`, r.multi ? "success" : "info");
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
      notify(`${a.displayName} saiu da lista`, "info");
    } catch (e) {
      toast(e);
    }
    loadAccounts();
  };

  const moveUp = async (a: Account, index: number) => {
    await api.moveAccount(a.id, Math.max(0, index - 1)).catch(toast);
    loadAccounts();
  };

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
            <DuckIcon size={30} />
            <span className="brand-name">Quackblox</span>
          </div>
          <MultiPill active={multi} autoClose={autoClose} stuck={stuck} />
        </header>

        <main className="panel page">
          <section className="block">
            <div className="block-head">
              <h2>Jogo</h2>
              <span className="muted small">Cola o link do jogo ou o ID. Link de servidor também vale.</span>
            </div>
            <GamePicker game={game} recents={recents} onPick={pickGame} onForget={forgetRecent} />
          </section>

          <section className="block">
            <div className="block-head">
              <h2>Contas</h2>
              <span className="muted small">Cada uma abre num Roblox separado.</span>
              <button className="pill-btn ghost" disabled={!game || playable < 2 || playingAll || multi !== true} onClick={playAll} title="Abre uma por vez, com 3 segundos entre elas">
                <UsersIcon size={16} /> Jogar com todas
              </button>
            </div>

            <div className="cards">
              <AccountCard
                tag={
                  <>
                    <PcIcon size={13} /> App do Roblox
                  </>
                }
                user={appUser}
                avatar={appUser ? avatars[appUser.id] : undefined}
                sub={appError ?? (appUser ? "A conta do app do Roblox deste PC" : "Lendo o login do Roblox...")}
                missing={!!appError}
                busy={busy.has("app")}
                canPlay={!!game && !!appUser}
                onPlay={() => appUser && play("app", appUser.displayName)}
              />
              {accounts.map((a, i) => (
                <AccountCard
                  key={a.id}
                  user={a}
                  avatar={avatars[a.id]}
                  sub={a.expired ? "O login expirou. Entra de novo pra continuar usando." : ago(a.lastPlayed)}
                  missing={a.expired}
                  busy={busy.has(String(a.id))}
                  canPlay={!!game && !a.expired}
                  onPlay={() => play(String(a.id), a.displayName)}
                  onRelogin={a.expired ? () => api.addAccount().catch(toast) : undefined}
                  menu={[
                    ...(i > 0 ? [{ icon: <ArrowUpIcon size={16} />, label: "Subir na lista", run: () => moveUp(a, i) }] : []),
                    { icon: <TrashIcon size={16} />, label: "Tirar do Quackblox", run: () => remove(a), danger: true },
                  ]}
                />
              ))}
              <button className="card add-card" onClick={() => api.addAccount().catch(toast)}>
                <span className="add-icon">
                  <PlusIcon size={22} />
                </span>
                <span className="add-title">Adicionar conta</span>
                <span className="muted small">Entra pelo site do Roblox numa janela à parte. O login fica guardado só neste PC, criptografado pelo Windows.</span>
              </button>
            </div>
          </section>
        </main>

        <footer className="footer">
          <label className="footer-toggle">
            <Toggle on={startup} onChange={(v) => api.setStartup(v).then(setStartup).catch(toast)} />
            Iniciar com o Windows
          </label>
          <label className="footer-toggle" title="Quando um Roblox aberto antes do Quackblox trava o multi, ele é fechado sozinho">
            <Toggle
              on={autoClose}
              disabled={savingAutoClose}
              onChange={(v) => {
                setSavingAutoClose(true);
                api
                  .setSettings({ autoClose: v })
                  .then((s) => setAutoClose(s.autoClose))
                  .catch(toast)
                  .finally(() => setSavingAutoClose(false));
              }}
            />
            Fechar sozinho o Roblox que trava o multi
          </label>
          <span className="muted small">Fechar a janela deixa o Quackblox na bandeja, com o multi ligado.</span>
        </footer>
      </div>
      <Toasts />
    </>
  );
}

function MultiPill({ active, autoClose, stuck }: { active: boolean | null; autoClose: boolean; stuck: boolean }) {
  if (active === null) return null;
  // Closing on its own: unless that already failed, then the button is back.
  const closing = autoClose && !stuck;
  return (
    <div className={`multi-pill ${active ? "on" : "wait"}`} title={active ? "Cada Roblox aberto pelo Quackblox ou pelo site abre ao lado dos outros" : "Um Roblox aberto antes do Quackblox está segurando a trava"}>
      <span className="dot" />
      {active ? "Multi ligado" : closing ? "Fechando o Roblox aberto..." : stuck ? "Não consegui fechar o que trava o multi" : "Um Roblox aberto está travando o multi"}
      {!active && !closing && (
        <button className="pill-link" onClick={() => api.closeRobloxNow().catch(toast)}>
          Fechar agora
        </button>
      )}
    </div>
  );
}

function Toggle({ on, onChange, disabled = false }: { on: boolean; onChange: (on: boolean) => void; disabled?: boolean }) {
  return (
    <button role="switch" aria-checked={on} className={`toggle ${on ? "on" : ""}`} disabled={disabled} onClick={() => onChange(!on)}>
      <span className="toggle-knob" />
    </button>
  );
}

function GamePicker({ game, recents, onPick, onForget }: { game: Game | null; recents: Game[]; onPick: (g: Game | null) => void; onForget: (g: Game) => void }) {
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
              {game.jobId ? " · servidor escolhido" : " · qualquer servidor"}
            </span>
          </div>
          <button className="round-btn" onClick={() => { onPick(null); setTimeout(() => input.current?.focus()); }} title="Trocar de jogo" aria-label="Trocar de jogo">
            <CloseIcon size={16} />
          </button>
        </div>
      ) : (
        <form className="game-search" onSubmit={(e) => { e.preventDefault(); resolve(text); }}>
          <SearchIcon size={18} />
          <input
            ref={input}
            value={text}
            placeholder="roblox.com/games/920587237 ou 920587237"
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
            {loading ? "Buscando..." : "Usar"}
          </button>
        </form>
      )}
      {recents.length > 0 && (
        <div className="recents">
          <span className="muted small">Recentes</span>
          <div className="recent-row">
            {recents.map((r) => (
              <div key={`${r.placeId}-${r.jobId}`} className={`recent ${game && sameGame(game, r) ? "on" : ""}`}>
                <button className="recent-pick" onClick={() => pickRecent(r)} title={r.name}>
                  <GameIcon game={r} size={24} />
                  <span>{r.name}</span>
                </button>
                <button className="recent-x" onClick={() => onForget(r)} aria-label={`Tirar ${r.name} dos recentes`}>
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
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false);
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [open]);

  return (
    <div className={`card account ${p.missing ? "missing" : ""}`}>
      <div className="account-top">
        {p.avatar ? <img className="avatar" src={p.avatar} alt="" /> : <span className="avatar empty" />}
        {p.tag && <span className="tag">{p.tag}</span>}
        {p.menu && (
          <div className="menu-wrap" ref={ref}>
            <button className="round-btn small-btn" onClick={() => setOpen((o) => !o)} aria-label="Mais opções" aria-haspopup="menu">
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
        <span className="account-name">{p.user?.displayName ?? "Conta do Roblox"}</span>
        {p.user && <span className="muted small">@{p.user.name}</span>}
        <span className={`small ${p.missing ? "warn" : "muted"}`}>{p.sub}</span>
      </div>
      {p.onRelogin ? (
        <button className="pill-btn ghost wide" onClick={p.onRelogin}>
          <LoginIcon size={16} /> Entrar de novo
        </button>
      ) : (
        <button className="pill-btn wide" disabled={!p.canPlay || p.busy} onClick={p.onPlay}>
          <PlayIcon size={16} /> {p.busy ? "Abrindo..." : "Jogar"}
        </button>
      )}
    </div>
  );
}
