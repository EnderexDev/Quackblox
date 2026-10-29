// The live screens (capture.rs), look only: the camera wall over the window, and the prints. A
// picture is asked only after the last one came, and only while the wall is open and the window
// shows, so nothing is captured while nobody watches. The phone page has its own copy (phone.html).
import { useEffect, useRef, useState } from "react";
import { api, type PhoneInfo } from "../api";
import { useT } from "../i18n";
import { notify, toast } from "../toast";
import { Toggle } from "./Toggle";
import { CloseIcon } from "./icons";

/** Between two pictures of one window (measured: the plan of phase 5). */
const LIVE_MS = 250;
const TILE_WIDTH = 640;

export type Screen = { key: string; name: string };

/** A print, saved and told with a button that shows it in its folder. */
export const print = (account: string | null, t: ReturnType<typeof useT>) =>
  api
    .capturePrint(account)
    .then((path) => notify(t.printSaved(path), "success", { label: t.showInFolder, run: () => api.reveal(path).catch(toast) }))
    .catch(toast);

function Tile({ screen }: { screen: Screen }) {
  const t = useT();
  const [url, setUrl] = useState<string | null>(null);
  const [why, setWhy] = useState<string | null>(null);
  const shown = useRef<string | null>(null);
  useEffect(() => {
    let live = true;
    (async () => {
      while (live) {
        if (document.visibilityState === "visible") {
          try {
            const bytes = await api.captureFrame(screen.key, TILE_WIDTH);
            if (!live) break;
            const next = URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
            if (shown.current) URL.revokeObjectURL(shown.current);
            shown.current = next;
            setUrl(next);
            setWhy(null);
          } catch (e) {
            if (!live) break;
            const code = String(e);
            setWhy(code === "minimized" ? t.wallMinimized : code === "none" ? t.wallNoWindow : t.wallGone);
          }
        }
        await new Promise((r) => setTimeout(r, LIVE_MS));
      }
    })();
    return () => {
      live = false;
      if (shown.current) URL.revokeObjectURL(shown.current);
      shown.current = null;
    };
  }, [screen.key]);
  return (
    <div className="screen-tile">
      <div className="screen-frame">{why || !url ? <span className="muted small">{why ?? "..."}</span> : <img src={url} alt={screen.name} />}</div>
      <div className="screen-foot">
        <span className="screen-name">{screen.name}</span>
        <button className="pill-btn ghost small" onClick={() => print(screen.key, t)}>
          {t.print}
        </button>
      </div>
    </div>
  );
}

export function ScreensSheet({ screens, onClose }: { screens: Screen[]; onClose: () => void }) {
  const t = useT();
  useEffect(() => {
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [onClose]);
  return (
    <div className="sheet-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="sheet screens-sheet" role="dialog" aria-label={t.wallTitle}>
        <div className="sheet-head">
          <h3>{t.wallTitle}</h3>
          <button className="pill-btn ghost small" disabled={!screens.length} onClick={() => print(null, t)}>
            {t.printAll}
          </button>
          <button className="round-btn small-btn" onClick={onClose} aria-label={t.close}>
            <CloseIcon size={16} />
          </button>
        </div>
        <span className="muted small">{t.wallHint}</span>
        {screens.length === 0 ? (
          <span className="muted small">{t.wallEmpty}</span>
        ) : (
          <div className="screen-wall">
            {screens.map((s) => (
              <Tile key={s.key} screen={s} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

/** The phone page and Discord's Rich Presence, both off until he turns them on. */
export function ExtrasBlock(p: { phone: boolean; richPresence: boolean; saving: boolean; onSave: (change: { phone?: boolean; richPresence?: boolean }) => Promise<boolean> }) {
  const t = useT();
  const [info, setInfo] = useState<PhoneInfo | null>(null);
  const [rpAvailable, setRpAvailable] = useState(false);
  useEffect(() => {
    api.richPresenceAvailable().then(setRpAvailable);
  }, []);
  // Read again after each change of the switch: the server starts or stops with the setting.
  useEffect(() => {
    api.phoneInfo().then(setInfo).catch(toast);
  }, [p.phone]);
  const newKey = () => {
    if (!confirm(t.phoneNewKeyConfirm)) return;
    api
      .phoneNewKey()
      .then((i) => {
        setInfo(i);
        notify(t.phoneKeyChanged, "info");
      })
      .catch(toast);
  };
  return (
    <section className="block extras">
      <div className="block-head">
        <h2>{t.extrasTitle}</h2>
        <span className="muted small">{t.extrasHint}</span>
      </div>
      <div className="vigia-rows">
        <div className="vigia-row">
          <label className="footer-toggle" title={t.phoneTitle}>
            <Toggle on={p.phone} disabled={p.saving} onChange={(v) => p.onSave({ phone: v })} />
            {t.phone}
          </label>
          {info?.listening && (
            <button className="pill-btn ghost small" onClick={newKey}>
              {t.phoneNewKey}
            </button>
          )}
        </div>
        {p.phone && info?.error && <span className="muted small error-text">{info.error}</span>}
        {info?.listening && (
          <>
            <span className="muted small">{t.phoneScan}</span>
            {info.links.length === 0 && <span className="muted small">{t.phoneNoAddress}</span>}
            <div className="phone-links">
              {info.links.map((l) => (
                <div key={l.kind} className="phone-link">
                  {/* Made by the Rust side from the link alone (qrcodegen): no outside markup. */}
                  <div className="phone-qr" dangerouslySetInnerHTML={{ __html: l.qr }} />
                  <span className="phone-link-kind">{l.kind === "lan" ? t.phoneLan : t.phoneTailscale}</span>
                  <button className="pill-btn ghost small" onClick={() => navigator.clipboard.writeText(l.url).then(() => notify(t.linkCopied, "info"))}>
                    {t.copyLink}
                  </button>
                </div>
              ))}
            </div>
            {!info.links.some((l) => l.kind === "tailscale") && <span className="muted small">{t.phoneTailscaleHint}</span>}
            <span className="muted small">{t.phoneFirewall}</span>
          </>
        )}
        {rpAvailable && (
          <div className="vigia-row">
            <label className="footer-toggle" title={t.richPresenceTitle}>
              <Toggle on={p.richPresence} disabled={p.saving} onChange={(v) => p.onSave({ richPresence: v })} />
              {t.richPresence}
            </label>
          </div>
        )}
      </div>
    </section>
  );
}
