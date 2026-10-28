// The command palette (Ctrl+K): every action of the window by name, typed and picked with the
// keyboard. Only inside the window: nothing here is a global hotkey, and nothing is sent to a game.
import { useEffect, useMemo, useRef, useState } from "react";
import { useT } from "../i18n";
import { SearchIcon } from "./icons";

export type Command = { id: string; label: string; hint?: string; run: () => void };

/** Lowercase with no accents, so "economico" finds "econômico". */
const plain = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

/** Every word typed is in the label (or its hint), in any order. */
function matches(c: Command, words: string[]) {
  const text = plain(`${c.label} ${c.hint ?? ""}`);
  return words.every((w) => text.includes(w));
}

export function Palette({ commands, onClose }: { commands: Command[]; onClose: () => void }) {
  const t = useT();
  const [text, setText] = useState("");
  const [pointed, setAt] = useState(0);
  const list = useRef<HTMLDivElement>(null);
  const shown = useMemo(() => {
    const words = plain(text).split(/\s+/).filter(Boolean);
    return commands.filter((c) => matches(c, words));
  }, [commands, text]);
  useEffect(() => setAt(0), [text]);
  // The list is made again at each render of the window: one that shrank keeps a line picked.
  const at = Math.max(0, Math.min(pointed, shown.length - 1));
  useEffect(() => {
    list.current?.querySelector<HTMLElement>(`[data-at="${at}"]`)?.scrollIntoView({ block: "nearest" });
  }, [at]);
  const run = (c: Command | undefined) => {
    if (!c) return;
    onClose();
    c.run();
  };
  return (
    <div className="sheet-backdrop palette-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        className="sheet palette"
        role="dialog"
        aria-label={t.paletteTitle}
        // On the box, not the input: Tab may have moved the focus to a line.
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            onClose();
          }
        }}
      >
        <div className="palette-search">
          <SearchIcon size={18} />
          <input
            autoFocus
            value={text}
            placeholder={t.palettePlaceholder}
            spellCheck={false}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setAt(Math.min(shown.length - 1, at + 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setAt(Math.max(0, at - 1));
              } else if (e.key === "Enter") {
                e.preventDefault();
                run(shown[at]);
              }
            }}
          />
        </div>
        <div className="palette-list" ref={list} role="listbox">
          {shown.length === 0 && <span className="muted small">{t.paletteEmpty}</span>}
          {shown.map((c, i) => (
            <button key={c.id} data-at={i} role="option" aria-selected={i === at} className={`palette-item ${i === at ? "on" : ""}`} onMouseMove={() => setAt(i)} onClick={() => run(c)}>
              <span>{c.label}</span>
              {c.hint && <span className="muted small">{c.hint}</span>}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
