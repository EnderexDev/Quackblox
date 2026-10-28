/** An on/off switch (Quackify's). */
export function Toggle({ on, onChange, disabled = false }: { on: boolean; onChange: (on: boolean) => void; disabled?: boolean }) {
  return (
    <button role="switch" aria-checked={on} className={`toggle ${on ? "on" : ""}`} disabled={disabled} onClick={() => onChange(!on)}>
      <span className="toggle-knob" />
    </button>
  );
}
