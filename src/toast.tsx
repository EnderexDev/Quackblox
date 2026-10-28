// Notices at the bottom, centered, as in Quackify: up to three stacked, gone after 5 s or on a
// click. `toast()` is for errors; `notify()` for what went right or is worth knowing.
import { useEffect, useState } from "react";
import { CheckIcon } from "./components/icons";

type Kind = "success" | "info" | "error";
type Toast = { id: number; text: string; kind: Kind };

const listeners = new Set<(t: Toast) => void>();
let nextId = 1;
const show = (text: string, kind: Kind) => listeners.forEach((l) => l({ id: nextId++, text, kind }));

export const toast = (error: unknown) => show(String(error).replace(/^Error: /, ""), "error");
export const notify = (text: string, kind: Exclude<Kind, "error"> = "success") => show(text, kind);

export function Toasts() {
  const [items, setItems] = useState<Toast[]>([]);
  useEffect(() => {
    const add = (t: Toast) => {
      setItems((xs) => [...xs.filter((x) => x.text !== t.text), t].slice(-3));
      setTimeout(() => setItems((xs) => xs.filter((x) => x.id !== t.id)), 5000);
    };
    listeners.add(add);
    return () => {
      listeners.delete(add);
    };
  }, []);
  return (
    <div className="toasts" role="status" aria-live="polite">
      {items.map((t) => (
        <div key={t.id} className={`toast ${t.kind}`} onClick={() => setItems((xs) => xs.filter((x) => x.id !== t.id))}>
          <span className="toast-icon" aria-hidden>
            {t.kind === "success" ? <CheckIcon size={14} /> : t.kind === "error" ? "!" : "i"}
          </span>
          {t.text}
        </div>
      ))}
    </div>
  );
}
