// Quackblox's installer window (setup.rs), adapted from Quackify's: the folder and the desktop
// shortcut, then Instalar, then Abrir o Quackblox.
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CloseIcon, DuckIcon, FolderIcon } from "./icons";
import { Toggle } from "./Toggle";

type Info = { version: string; dir: string; installed: boolean };
type Stage = { kind: "ready" } | { kind: "installing" } | { kind: "done" } | { kind: "error"; message: string };

const close = () => invoke("setup_close");

export function Setup() {
  const [info, setInfo] = useState<Info | null>(null);
  const [dir, setDir] = useState("");
  const [desktop, setDesktop] = useState(true);
  const [stage, setStage] = useState<Stage>({ kind: "ready" });

  useEffect(() => {
    invoke<Info>("setup_info").then((i) => {
      setInfo(i);
      setDir(i.dir);
    });
  }, []);

  const install = async () => {
    setStage({ kind: "installing" });
    try {
      await invoke("setup_install", { dir, desktop });
      setStage({ kind: "done" });
    } catch (e) {
      setStage({ kind: "error", message: String(e) });
    }
  };

  const pick = async () => {
    const picked = await invoke<string | null>("setup_pick_folder");
    if (!picked) return;
    setDir(picked);
    const installed = await invoke<boolean>("setup_installed", { dir: picked });
    setInfo((i) => i && { ...i, installed });
  };

  const open = () => invoke("setup_launch", { dir }).catch((e) => setStage({ kind: "error", message: String(e) }));

  if (!info) return <div className="setup" />;
  const busy = stage.kind === "installing";

  const title = stage.kind === "done" ? "Quackblox instalado" : stage.kind === "error" ? "A instalação não foi" : "Quackblox";
  const line =
    stage.kind === "installing"
      ? "Instalando…"
      : stage.kind === "done"
        ? "Várias contas, um pato só."
        : stage.kind === "error"
          ? stage.message
          : info.installed
            ? `O Quackblox ${info.version} por cima do que já está nesta pasta. Tuas contas e configurações ficam.`
            : "Várias contas do Roblox abertas ao mesmo tempo.";

  return (
    <div className="setup">
      <header className="setup-bar" data-tauri-drag-region>
        <span className="setup-bar-title" data-tauri-drag-region>
          Instalar o Quackblox
        </span>
        {!busy && (
          <button className="setup-close" aria-label="Fechar" onClick={close}>
            <CloseIcon size={14} />
          </button>
        )}
      </header>

      <main className="setup-body">
        <div className={`setup-duck ${busy ? "busy" : ""} ${stage.kind === "done" ? "done" : ""}`}>
          <DuckIcon size={busy ? 52 : 76} />
        </div>
        <h1>{title}</h1>
        <p className={`setup-line ${stage.kind === "error" ? "error" : "muted"}`}>{line}</p>

        {busy && (
          <div className="setup-progress endless">
            <i />
          </div>
        )}

        {/* Also after a failed install: a folder it could not write to can be changed before trying again. */}
        {(stage.kind === "ready" || stage.kind === "error") && (
          <div className="setup-options">
            <div className="setup-option">
              <FolderIcon size={16} />
              <span className="setup-path" title={dir}>
                {dir}
              </span>
              <button className="pill-button" onClick={pick}>
                Alterar…
              </button>
            </div>
            <label className="setup-option">
              <span>Atalho na área de trabalho</span>
              <Toggle on={desktop} onChange={setDesktop} />
            </label>
          </div>
        )}

        <div className="setup-actions">
          {stage.kind === "ready" && (
            <button className="primary" onClick={install}>
              {info.installed ? "Atualizar" : "Instalar"}
            </button>
          )}
          {stage.kind === "done" && (
            <button className="primary" onClick={open}>
              Abrir o Quackblox
            </button>
          )}
          {stage.kind === "error" && (
            <button className="primary" onClick={install}>
              Tentar de novo
            </button>
          )}
        </div>
      </main>
    </div>
  );
}
