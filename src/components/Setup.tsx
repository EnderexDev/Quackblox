// Quackblox's installer and update window (setup.rs), adapted from Quackify's. Installing: the
// folder and the desktop shortcut, then Instalar, then Abrir o Quackblox. Updating: it starts by
// itself, shows the real download percentage, then installing (no number: silent NSIS reports
// none), and closes when the installer opens the new Quackblox.
import { useEffect, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { CloseIcon, DuckIcon, FolderIcon } from "./icons";
import { Toggle } from "./Toggle";
import { useT } from "../i18n";

type Info = { mode: "install" | "update"; version: string; dir: string; installed: boolean };
type Progress = { stage: "found"; version: string } | { stage: "downloading"; percent: number | null } | { stage: "installing" };
type Stage =
  | { kind: "ready" }
  | { kind: "checking" }
  | { kind: "downloading"; percent: number | null }
  | { kind: "installing" }
  | { kind: "done" }
  | { kind: "error"; message: string }
  /** Installed, but opening it failed: Abrir stays, nothing installs again. */
  | { kind: "openError"; message: string };

const close = () => invoke("setup_close");

export function Setup() {
  const t = useT();
  const [info, setInfo] = useState<Info | null>(null);
  const [dir, setDir] = useState("");
  const [desktop, setDesktop] = useState(true);
  const [version, setVersion] = useState("");
  const [stage, setStage] = useState<Stage>({ kind: "ready" });
  const [picking, setPicking] = useState(false);
  /** The folder the install really went to: Abrir opens that one. */
  const [installedDir, setInstalledDir] = useState("");
  const started = useRef(false);

  useEffect(() => {
    invoke<Info>("setup_info").then((i) => {
      setInfo(i);
      setDir(i.dir);
      setInstalledDir(i.dir);
      setVersion(i.version);
      if (i.mode === "update") update();
    });
  }, []);

  const update = async () => {
    // StrictMode runs the first effect twice: one update only.
    if (started.current) return;
    started.current = true;
    setStage({ kind: "checking" });
    const progress = new Channel<Progress>();
    progress.onmessage = (p) => {
      if (p.stage === "found") setVersion(p.version);
      else if (p.stage === "downloading") setStage({ kind: "downloading", percent: p.percent });
      else setStage({ kind: "installing" });
    };
    try {
      // On success the installer opens the new Quackblox and this window closes itself.
      await invoke("setup_update", { progress });
    } catch (e) {
      started.current = false;
      setStage({ kind: "error", message: String(e) });
    }
  };

  const install = async () => {
    if (picking) return;
    const target = dir;
    setStage({ kind: "installing" });
    try {
      await invoke("setup_install", { dir: target, desktop });
      setInstalledDir(target);
      setStage({ kind: "done" });
    } catch (e) {
      setStage({ kind: "error", message: String(e) });
    }
  };

  const pick = async () => {
    setPicking(true);
    try {
      const picked = await invoke<string | null>("setup_pick_folder");
      if (!picked) return;
      setDir(picked);
      const installed = await invoke<boolean>("setup_installed", { dir: picked });
      setInfo((i) => i && { ...i, installed });
    } finally {
      setPicking(false);
    }
  };

  // After a failed update, a failed open keeps that error (and its Tentar de novo).
  const open = () =>
    invoke("setup_launch", { dir: installedDir }).catch((e) =>
      setStage(info?.mode === "update" ? { kind: "error", message: String(e) } : { kind: "openError", message: String(e) }),
    );

  if (!info) return <div className="setup" />;
  const updating = info.mode === "update";
  const busy = stage.kind === "checking" || stage.kind === "downloading" || stage.kind === "installing";
  const failed = stage.kind === "error" || stage.kind === "openError";

  const title =
    stage.kind === "done" || stage.kind === "openError"
      ? t.setupInstalled
      : stage.kind === "error"
        ? updating
          ? t.setupUpdateFailed
          : t.setupInstallFailed
        : updating
          ? version
            ? t.setupUpdatingTo(version)
            : t.setupLookingForVersion
          : "Quackblox";

  const line =
    stage.kind === "downloading"
      ? stage.percent === null
        ? t.setupDownloading
        : t.setupDownloadingPercent(stage.percent)
      : stage.kind === "installing"
        ? updating
          ? t.setupInstallingUpdate
          : t.setupInstalling
        : stage.kind === "done"
          ? t.setupDone
          : failed
            ? stage.message
            : stage.kind === "checking"
              ? t.setupAskingGitHub
              : info.installed
                ? t.setupOverExisting(version)
                : t.setupPitch;

  return (
    <div className="setup">
      <header className="setup-bar" data-tauri-drag-region>
        <span className="setup-bar-title" data-tauri-drag-region>
          {updating ? t.setupUpdateTitle : t.setupInstallTitle}
        </span>
        {!busy && (
          <button className="setup-close" aria-label={t.close} onClick={close}>
            <CloseIcon size={14} />
          </button>
        )}
      </header>

      <main className="setup-body">
        <div className={`setup-duck ${busy ? "busy" : ""} ${stage.kind === "done" ? "done" : ""}`}>
          <DuckIcon size={busy ? 52 : 76} />
        </div>
        <h1>{title}</h1>
        <p className={`setup-line ${failed ? "error" : "muted"}`}>{line}</p>

        {busy && (
          <div className={`setup-progress ${stage.kind === "downloading" && stage.percent !== null ? "" : "endless"}`}>
            <i style={stage.kind === "downloading" && stage.percent !== null ? { width: `${stage.percent}%` } : undefined} />
          </div>
        )}

        {/* Also after a failed install: a folder it could not write to can be changed before trying again. */}
        {!updating && (stage.kind === "ready" || stage.kind === "error") && (
          <div className="setup-options">
            <div className="setup-option">
              <FolderIcon size={16} />
              <span className="setup-path" title={dir}>
                {dir}
              </span>
              <button className="pill-button" disabled={picking} onClick={pick}>
                {t.change}
              </button>
            </div>
            <label className="setup-option">
              <span>{t.desktopShortcut}</span>
              <Toggle on={desktop} onChange={setDesktop} />
            </label>
          </div>
        )}

        <div className="setup-actions">
          {stage.kind === "ready" && !updating && (
            <button className="primary" disabled={picking} onClick={install}>
              {info.installed ? t.update : t.install}
            </button>
          )}
          {(stage.kind === "done" || stage.kind === "openError") && (
            <button className="primary" onClick={open}>
              {t.openQuackblox}
            </button>
          )}
          {stage.kind === "error" && (
            <>
              <button className="primary" disabled={picking} onClick={updating ? update : install}>
                {t.tryAgain}
              </button>
              {updating && (
                <button className="secondary" onClick={open}>
                  {t.openQuackblox}
                </button>
              )}
            </>
          )}
        </div>
      </main>
    </div>
  );
}
