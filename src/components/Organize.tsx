// Groups of accounts, one-click profiles and the schedule (organize.rs). The lists are saved whole
// at each change; a slot's last run is the Rust side's to keep.
import { useEffect, useState } from "react";
import type { Game, Group, Lists, Profile, Slot } from "../api";
import { useT } from "../i18n";
import { toast } from "../toast";
import { Toggle } from "./Toggle";
import { ClockIcon, CloseIcon, LeafIcon, PlayIcon, PlusIcon, TrashIcon, UsersIcon } from "./icons";

/** An account as the pickers show it: its key ("app" or a saved id), its name and face. */
export type Person = { key: string; name: string; avatar?: string };

const newId = () => crypto.randomUUID();
const hhmm = (m: number) => `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`;
const minuteOf = (v: string) => {
  const [h, m] = v.split(":").map(Number);
  return Number.isFinite(h) && Number.isFinite(m) ? h * 60 + m : null;
};

export function OrganizeBlock(p: {
  lists: Lists;
  onSave: (lists: Lists) => Promise<boolean>;
  people: Person[];
  game: Game | null;
  /** The accounts in economy mode now, a new profile's start. */
  eco: string[];
  sameServer: boolean;
  /** A run of several accounts is going (one at a time). */
  running: boolean;
  multi: boolean | null;
  onPlayGroup: (g: Group) => void;
  onRunProfile: (p: Profile) => void;
}) {
  const t = useT();
  const [group, setGroup] = useState<Group | "new" | null>(null);
  const [profile, setProfile] = useState<Profile | "new" | null>(null);
  const [confirm, setConfirm] = useState<string | null>(null);
  useEffect(() => {
    if (!confirm) return;
    const timer = setTimeout(() => setConfirm(null), 4000);
    return () => clearTimeout(timer);
  }, [confirm]);
  const { groups, profiles, schedule } = p.lists;
  const nameOf = (key: string) => p.people.find((x) => x.key === key)?.name;
  /** Deleted on the second click. */
  const removing = (id: string, run: () => void) => () => {
    if (confirm !== id) return setConfirm(id);
    setConfirm(null);
    run();
  };
  const save = (change: Partial<Lists>) => p.onSave({ ...p.lists, ...change });
  const saveSlot = (slot: Slot) => save({ schedule: schedule.map((s) => (s.id === slot.id ? slot : s)) });
  // More than one account needs the multi on (launch.rs).
  const canRun = (keys: string[]) => !p.running && (keys.length < 2 || p.multi === true);

  return (
    <section className="block organize">
      <div className="block-head">
        <h2>{t.orgTitle}</h2>
        <span className="muted small">{t.orgHint}</span>
      </div>

      <div className="org-part">
        <div className="org-head">
          <h3>{t.groups}</h3>
          <button className="pill-btn ghost small" onClick={() => setGroup("new")}>
            <PlusIcon size={14} /> {t.newGroup}
          </button>
        </div>
        {groups.length === 0 && <span className="muted small">{t.noGroups}</span>}
        {groups.map((g) => (
          <div key={g.id} className="org-row">
            <UsersIcon size={16} />
            <span className="org-name">{g.name}</span>
            <span className="muted small org-sub">{g.accounts.map((k) => nameOf(k) ?? "?").join(", ")}</span>
            <button
              className="pill-btn small"
              disabled={!p.game || !canRun(g.accounts)}
              onClick={() => p.onPlayGroup(g)}
              title={p.game ? t.playGroupTitle(p.game.name) : t.pickGameFirst}
            >
              <PlayIcon size={14} /> {t.playGroup}
            </button>
            <button className="pill-btn ghost small" onClick={() => setGroup(g)}>
              {t.edit}
            </button>
            <button className="round-btn small-btn" title={confirm === g.id ? t.removeConfirm : t.remove} aria-label={t.remove} onClick={removing(g.id, () => save({ groups: groups.filter((x) => x.id !== g.id) }))}>
              {confirm === g.id ? "?" : <TrashIcon size={14} />}
            </button>
          </div>
        ))}
      </div>

      <div className="org-part">
        <div className="org-head">
          <h3>{t.profiles}</h3>
          <button className="pill-btn ghost small" onClick={() => setProfile("new")}>
            <PlusIcon size={14} /> {t.newProfile}
          </button>
        </div>
        {profiles.length === 0 && <span className="muted small">{t.noProfiles}</span>}
        {profiles.map((pr) => (
          <div key={pr.id} className="org-row">
            {pr.game.icon ? <img className="game-icon" src={pr.game.icon} width={22} height={22} alt="" /> : <span className="game-icon empty" style={{ width: 22, height: 22 }} />}
            <span className="org-name">{pr.name}</span>
            <span className="muted small org-sub">
              {pr.game.name} · {t.accountsCount(pr.accounts.length)}
              {pr.eco.length > 0 && ` · ${t.ecoLabel} ${pr.eco.length}`}
              {pr.grid && ` · ${t.grid}`}
            </span>
            <button className="pill-btn small" disabled={!canRun(pr.accounts)} onClick={() => p.onRunProfile(pr)} title={t.runProfileTitle}>
              <PlayIcon size={14} /> {t.runProfile}
            </button>
            <button className="pill-btn ghost small" onClick={() => setProfile(pr)}>
              {t.edit}
            </button>
            <button
              className="round-btn small-btn"
              title={confirm === pr.id ? t.removeConfirm : t.remove}
              aria-label={t.remove}
              // A slot of a deleted profile goes with it: the schedule has nothing to open.
              onClick={removing(pr.id, () => save({ profiles: profiles.filter((x) => x.id !== pr.id), schedule: schedule.filter((s) => s.profile !== pr.id) }))}
            >
              {confirm === pr.id ? "?" : <TrashIcon size={14} />}
            </button>
          </div>
        ))}
      </div>

      <div className="org-part">
        <div className="org-head">
          <h3>{t.schedule}</h3>
          <button
            className="pill-btn ghost small"
            disabled={profiles.length === 0}
            title={profiles.length ? undefined : t.slotNeedsProfile}
            onClick={() => save({ schedule: [...schedule, { id: newId(), profile: profiles[0].id, minute: 9 * 60, days: [], on: false, last: "" }] })}
          >
            <PlusIcon size={14} /> {t.newSlot}
          </button>
        </div>
        <span className="muted small">{profiles.length ? t.scheduleHint : t.slotNeedsProfile}</span>
        {schedule.length === 0 && profiles.length > 0 && <span className="muted small">{t.noSlots}</span>}
        {schedule.map((slot) => (
          <div key={slot.id} className="org-row slot">
            <ClockIcon size={16} />
            <input
              type="time"
              className="slot-time"
              aria-label={t.slotTime}
              value={hhmm(slot.minute)}
              onChange={(e) => {
                const m = minuteOf(e.target.value);
                if (m !== null) saveSlot({ ...slot, minute: m });
              }}
            />
            <div className="slot-days" role="group">
              {t.days.map((d, i) => {
                const onDay = slot.days.includes(i);
                return (
                  <button
                    key={d}
                    className={`day ${onDay || slot.days.length === 0 ? "on" : ""}`}
                    aria-pressed={onDay || slot.days.length === 0}
                    onClick={() => {
                      // From "every day", a click keeps only that day; the last one off is every day again.
                      const days = slot.days.length === 0 ? [i] : onDay ? slot.days.filter((x) => x !== i) : [...slot.days, i].sort();
                      saveSlot({ ...slot, days: days.length === 7 ? [] : days });
                    }}
                  >
                    {d}
                  </button>
                );
              })}
            </div>
            <select className="slot-profile" aria-label={t.slotProfile} value={slot.profile} onChange={(e) => saveSlot({ ...slot, profile: e.target.value })}>
              {profiles.map((pr) => (
                <option key={pr.id} value={pr.id}>
                  {pr.name}
                </option>
              ))}
            </select>
            <span className="muted small org-sub">{slot.last ? t.slotLast(slot.last.split("-").reverse().join("/")) : slot.days.length === 0 ? t.everyDay : ""}</span>
            <label className="footer-toggle" title={t.slotOn}>
              <Toggle on={slot.on} onChange={(v) => saveSlot({ ...slot, on: v })} />
            </label>
            <button className="round-btn small-btn" title={confirm === slot.id ? t.removeConfirm : t.remove} aria-label={t.remove} onClick={removing(slot.id, () => save({ schedule: schedule.filter((s) => s.id !== slot.id) }))}>
              {confirm === slot.id ? "?" : <TrashIcon size={14} />}
            </button>
          </div>
        ))}
      </div>

      {group && (
        <GroupSheet
          group={group === "new" ? null : group}
          people={p.people}
          onClose={() => setGroup(null)}
          onSave={(g) => save({ groups: group === "new" ? [...groups, g] : groups.map((x) => (x.id === g.id ? g : x)) })}
        />
      )}
      {profile && (
        <ProfileSheet
          profile={profile === "new" ? null : profile}
          people={p.people}
          game={p.game}
          eco={p.eco}
          sameServer={p.sameServer}
          onClose={() => setProfile(null)}
          onSave={(pr) => save({ profiles: profile === "new" ? [...profiles, pr] : profiles.map((x) => (x.id === pr.id ? pr : x)) })}
        />
      )}
    </section>
  );
}

function Sheet(p: { title: string; onClose: () => void; children: React.ReactNode }) {
  const t = useT();
  useEffect(() => {
    const esc = (e: KeyboardEvent) => e.key === "Escape" && p.onClose();
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [p.onClose]);
  return (
    <div className="sheet-backdrop" onMouseDown={(e) => e.target === e.currentTarget && p.onClose()}>
      <div className="sheet" role="dialog" aria-label={p.title}>
        <div className="sheet-head">
          <h3>{p.title}</h3>
          <button className="round-btn small-btn" onClick={p.onClose} aria-label={t.close}>
            <CloseIcon size={16} />
          </button>
        </div>
        {p.children}
      </div>
    </div>
  );
}

/** The accounts to tick, in card order; with `eco`, a leaf beside each one ticked. */
function PeoplePicker(p: { people: Person[]; picked: string[]; onPick: (keys: string[]) => void; eco?: string[]; onEco?: (keys: string[]) => void }) {
  const t = useT();
  return (
    <div className="people">
      {p.people.map((x) => {
        const on = p.picked.includes(x.key);
        const eco = p.eco?.includes(x.key) ?? false;
        return (
          <div key={x.key} className={`person ${on ? "on" : ""}`}>
            <button className="person-pick" aria-pressed={on} onClick={() => p.onPick(on ? p.picked.filter((k) => k !== x.key) : [...p.picked, x.key])}>
              {x.avatar ? <img className="friend-face small-face" src={x.avatar} alt="" /> : <span className="friend-face small-face" />}
              <span>{x.name}</span>
            </button>
            {p.onEco && on && (
              <button
                className={`round-btn small-btn eco-btn ${eco ? "on" : ""}`}
                aria-pressed={eco}
                aria-label={t.ecoLabel}
                title={t.profileEcoTitle}
                onClick={() => p.onEco!(eco ? p.eco!.filter((k) => k !== x.key) : [...p.eco!, x.key])}
              >
                <LeafIcon size={14} />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}

function GroupSheet(p: { group: Group | null; people: Person[]; onClose: () => void; onSave: (g: Group) => Promise<boolean> }) {
  const t = useT();
  const [name, setName] = useState(p.group?.name ?? "");
  const [picked, setPicked] = useState<string[]>(p.group?.accounts ?? []);
  const submit = async () => {
    if (!name.trim()) return toast(t.needName);
    // Card order, so the group opens in the order he sees. An account removed since is not kept.
    const accounts = p.people.map((x) => x.key).filter((k) => picked.includes(k));
    if (!accounts.length) return toast(t.pickOneAccount);
    if (await p.onSave({ id: p.group?.id ?? newId(), name: name.trim(), accounts })) p.onClose();
  };
  return (
    <Sheet title={p.group ? p.group.name : t.newGroup} onClose={p.onClose}>
      <input className="sheet-input" value={name} placeholder={t.groupName} onChange={(e) => setName(e.target.value)} autoFocus />
      <span className="muted small">{t.pickAccounts}</span>
      <PeoplePicker people={p.people} picked={picked} onPick={setPicked} />
      <div className="sheet-foot">
        <button className="pill-btn ghost small" onClick={p.onClose}>
          {t.cancel}
        </button>
        <button className="pill-btn small" onClick={submit}>
          {t.save}
        </button>
      </div>
    </Sheet>
  );
}

function ProfileSheet(p: {
  profile: Profile | null;
  people: Person[];
  game: Game | null;
  eco: string[];
  sameServer: boolean;
  onClose: () => void;
  onSave: (pr: Profile) => Promise<boolean>;
}) {
  const t = useT();
  const [name, setName] = useState(p.profile?.name ?? "");
  const [game, setGame] = useState<Game | null>(p.profile?.game ?? p.game);
  const [picked, setPicked] = useState<string[]>(p.profile?.accounts ?? []);
  const [eco, setEco] = useState<string[]>(p.profile?.eco ?? p.eco);
  const [grid, setGrid] = useState(p.profile?.grid ?? true);
  const [same, setSame] = useState(p.profile?.sameServer ?? p.sameServer);
  const submit = async () => {
    if (!name.trim()) return toast(t.needName);
    if (!game) return toast(t.profileNeedsGame);
    const accounts = p.people.map((x) => x.key).filter((k) => picked.includes(k));
    if (!accounts.length) return toast(t.pickOneAccount);
    const saved = await p.onSave({ id: p.profile?.id ?? newId(), name: name.trim(), game, accounts, eco: eco.filter((k) => accounts.includes(k)), grid, sameServer: same });
    if (saved) p.onClose();
  };
  return (
    <Sheet title={p.profile ? p.profile.name : t.newProfile} onClose={p.onClose}>
      <input className="sheet-input" value={name} placeholder={t.profileName} onChange={(e) => setName(e.target.value)} autoFocus />
      <span className="muted small">{t.profileGame}</span>
      <div className="org-row">
        {game?.icon ? <img className="game-icon" src={game.icon} width={28} height={28} alt="" /> : <span className="game-icon empty" style={{ width: 28, height: 28 }} />}
        <span className="org-name">{game?.name ?? t.profileNeedsGame}</span>
        {p.game && (!game || game.placeId !== p.game.placeId || game.jobId !== p.game.jobId || game.linkCode !== p.game.linkCode) && (
          <button className="pill-btn ghost small" onClick={() => setGame(p.game)}>
            {t.profileUseGame}
          </button>
        )}
      </div>
      <span className="muted small">{t.pickAccounts}</span>
      <PeoplePicker people={p.people} picked={picked} onPick={setPicked} eco={eco} onEco={setEco} />
      <label className="footer-toggle">
        <Toggle on={grid} onChange={setGrid} />
        {t.profileGrid}
      </label>
      <label className="footer-toggle" title={t.sameServerTitle}>
        <Toggle on={same} onChange={setSame} />
        {t.profileSameServer}
      </label>
      <div className="sheet-foot">
        <button className="pill-btn ghost small" onClick={p.onClose}>
          {t.cancel}
        </button>
        <button className="pill-btn small" onClick={submit}>
          {t.save}
        </button>
      </div>
    </Sheet>
  );
}
