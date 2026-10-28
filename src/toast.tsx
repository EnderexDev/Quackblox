// Notices at the bottom, centered, as in Quackify: up to three stacked, gone after 5 s (8 s with
// a button) or on a click. `toast()` is for errors; `notify()` for what went right or is worth
// knowing, optionally with one action (Atualizar).
import { useEffect, useState } from "react";
import { CheckIcon } from "./components/icons";

type Kind = "success" | "info" | "error";
export type ToastAction = { label: string; run: () => void };
type Toast = { id: number; text: string; kind: Kind; action?: ToastAction };

const listeners = new Set<(t: Toast) => void>();
let nextId = 1;
const show = (text: string, kind: Kind, action?: ToastAction) => listeners.forEach((l) => l({ id: nextId++, text, kind, action }));

export const toast = (error: unknown) => show(String(error).replace(/^Error: /, ""), "error");
export const notify = (text: string, kind: Exclude<Kind, "error"> = "success", action?: ToastAction) => show(text, kind, action);

export function Toasts() {
  const [items, setItems] = useState<Toast[]>([]);
  useEffect(() => {
    const add = (t: Toast) => {
      setItems((xs) => [...xs.filter((x) => x.text !== t.text), t].slice(-3));
      setTimeout(() => setItems((xs) => xs.filter((x) => x.id !== t.id)), t.action ? 8000 : 5000);
    };
    listeners.add(add);
    return () => {
      listeners.delete(add);
    };
  }, []);
  const close = (id: number) => setItems((xs) => xs.filter((x) => x.id !== id));
  return (
    <div className="toasts" role="status" aria-live="polite">
      {items.map((t) => (
        <div key={t.id} className={`toast ${t.kind}`} onClick={() => close(t.id)}>
          <span className="toast-icon" aria-hidden>
            {t.kind === "success" ? <CheckIcon size={14} /> : t.kind === "error" ? "!" : "i"}
          </span>
          {t.text}
          {t.action && (
            <button
              className="toast-action"
              onClick={(e) => {
                e.stopPropagation();
                close(t.id);
                t.action!.run();
              }}
            >
              {t.action.label}
            </button>
          )}
        </div>
      ))}
    </div>
  );
}
