// The app's texts in Portuguese and English. The language is saved in settings.json by the Rust
// side (settings.rs), which also uses it for the tray menu and its own error messages; main.tsx
// reads it before the first render. `useT()` in components, `t()` anywhere else.
import { useSyncExternalStore } from "react";

export type Lang = "pt" | "en";

const pt = {
  // Top bar
  languageLabel: "Idioma",
  multiOn: "Multi ligado",
  multiOnTitle: "Cada Roblox aberto pelo Quackblox ou pelo site abre ao lado dos outros",
  multiWaitTitle: "Um Roblox aberto antes do Quackblox está segurando a trava",
  multiClosing: "Fechando o Roblox aberto...",
  multiStuck: "Não consegui fechar o que trava o multi",
  multiBlocked: "Um Roblox aberto está travando o multi",
  closeNow: "Fechar agora",

  // Game
  gameTitle: "Jogo",
  gameHint: "Cola o link do jogo ou o ID. Link de servidor também vale.",
  gamePlaceholder: "roblox.com/games/920587237 ou 920587237",
  searching: "Buscando...",
  use: "Usar",
  serverChosen: "servidor escolhido",
  anyServer: "qualquer servidor",
  changeGame: "Trocar de jogo",
  recents: "Recentes",
  forgetRecent: (name: string) => `Tirar ${name} dos recentes`,

  // Accounts
  accountsTitle: "Contas",
  accountsHint: "Cada uma abre num Roblox separado.",
  playAll: "Jogar com todas",
  playAllTitle: "Abre uma por vez, com 3 segundos entre elas",
  robloxApp: "App do Roblox",
  appAccountSub: "A conta do app do Roblox deste PC",
  readingAppLogin: "Lendo o login do Roblox...",
  loginExpired: "O login expirou. Entra de novo pra continuar usando.",
  moveUp: "Subir na lista",
  removeAccount: "Tirar do Quackblox",
  addAccount: "Adicionar conta",
  addAccountHint: "Entra pelo site do Roblox numa janela à parte. O login fica guardado só neste PC, criptografado pelo Windows.",
  robloxAccount: "Conta do Roblox",
  moreOptions: "Mais opções",
  signInAgain: "Entrar de novo",
  opening: "Abrindo...",
  play: "Jogar",
  neverPlayed: "Ainda não jogou por aqui",
  playedNow: "Jogou agora",
  playedMinutes: (n: number) => `Jogou há ${n} min`,
  playedHours: (n: number) => `Jogou há ${n} h`,
  playedDays: (n: number) => `Jogou há ${n} ${n === 1 ? "dia" : "dias"}`,

  // Open clients (the line on each card, the windows bar)
  openOne: "Aberto",
  openMany: (n: number) => `${n} abertos`,
  focusWindow: "Trazer a janela pra frente",
  ecoOn: "Modo econômico ligado: o Roblox dessa conta roda com prioridade baixa e no Eco do Windows. Clica pra desligar.",
  ecoOff: "Modo econômico: o Roblox dessa conta roda com prioridade baixa e no Eco do Windows, gastando menos CPU. Clica pra ligar.",
  ecoLabel: "Eco",
  hotkeyChip: (n: number) => `Ctrl+Shift+${n} traz o Roblox dessa conta pra frente`,
  windowsOpen: (n: number) => `${n} ${n === 1 ? "janela do Roblox aberta" : "janelas do Roblox abertas"}`,
  grid: "Organizar em grade",
  gridTitle: "Põe todas as janelas do Roblox lado a lado, no monitor da principal",
  gridDone: (n: number) => `${n} ${n === 1 ? "janela organizada" : "janelas organizadas"}`,
  minimizeOthers: "Minimizar as outras",
  minimizeOthersTitle: "Minimiza todas as janelas do Roblox menos a principal (Ctrl+Shift+0)",

  // Mascot
  mascotSleeping: "Dormindo. Nenhum Roblox aberto",
  mascotPlaying: (n: number) => `Jogando. ${n} ${n === 1 ? "Roblox aberto" : "Roblox abertos"}`,

  // Notices
  pickGameFirst: "Escolhe um jogo primeiro, lá em cima",
  openingGame: (game: string, account: string) => `Abrindo ${game} com ${account}`,
  openingWithoutMulti: (account: string) => `Abrindo com ${account}. O multi está esperando, então o Roblox que já estava aberto vai fechar`,
  closedRoblox: "Fechei o Roblox que estava aberto pra ligar o multi",
  accountAdded: (name: string) => `${name} entrou no Quackblox`,
  accountRemoved: (name: string) => `${name} saiu da lista`,

  // Footer
  startWithWindows: "Iniciar com o Windows",
  autoClose: "Fechar sozinho o Roblox que trava o multi",
  autoCloseTitle: "Quando um Roblox aberto antes do Quackblox trava o multi, ele é fechado sozinho",
  hotkeys: "Atalhos Ctrl+Shift",
  hotkeysTitle: "Ctrl+Shift+1 a 9 traz pra frente o Roblox da conta naquela posição; Ctrl+Shift+0 minimiza todas menos a principal",
  hotkeysFailed: (keys: string) => `Outro programa já usa ${keys}. Esses atalhos ficaram de fora`,
  trayTitle: "Fechar a janela deixa o Quackblox na bandeja, com o multi ligado.",
  updateTo: (v: string) => `Atualizar pro ${v}`,
  updateStarting: "Abrindo a atualização…",
  checking: "Procurando…",
  checkUpdate: "Procurar atualização",

  // Updates (update.ts)
  upToDate: "O Quackblox está na versão mais nova",
  newVersion: (v: string) => `Saiu o Quackblox ${v}`,
  update: "Atualizar",
  checkFailed: (e: string) => `Não consegui procurar atualização: ${e}`,
  updateFailed: (e: string) => `Não consegui atualizar: ${e}`,

  // Installer and update window (Setup.tsx)
  setupInstalled: "Quackblox instalado",
  setupUpdateFailed: "A atualização não foi",
  setupInstallFailed: "A instalação não foi",
  setupUpdatingTo: (v: string) => `Atualizando pro Quackblox ${v}`,
  setupLookingForVersion: "Procurando a versão nova…",
  setupDownloading: "Baixando…",
  setupDownloadingPercent: (p: number) => `Baixando… ${p}%`,
  setupInstallingUpdate: "Instalando. O Quackblox abre de novo sozinho.",
  setupInstalling: "Instalando…",
  setupDone: "Várias contas, um pato só.",
  setupAskingGitHub: "Perguntando pro GitHub…",
  setupOverExisting: (v: string) => `O Quackblox ${v} por cima do que já está nesta pasta. Tuas contas e configurações ficam.`,
  setupPitch: "Várias contas do Roblox abertas ao mesmo tempo.",
  setupUpdateTitle: "Atualização do Quackblox",
  setupInstallTitle: "Instalar o Quackblox",
  close: "Fechar",
  change: "Alterar…",
  desktopShortcut: "Atalho na área de trabalho",
  install: "Instalar",
  openQuackblox: "Abrir o Quackblox",
  tryAgain: "Tentar de novo",
};

export type Texts = typeof pt;

const en: Texts = {
  languageLabel: "Language",
  multiOn: "Multi on",
  multiOnTitle: "Every Roblox opened by Quackblox or by the website opens next to the others",
  multiWaitTitle: "A Roblox opened before Quackblox is holding the lock",
  multiClosing: "Closing the open Roblox...",
  multiStuck: "Couldn't close what is blocking the multi",
  multiBlocked: "An open Roblox is blocking the multi",
  closeNow: "Close now",

  gameTitle: "Game",
  gameHint: "Paste the game's link or its ID. A server link works too.",
  gamePlaceholder: "roblox.com/games/920587237 or 920587237",
  searching: "Searching...",
  use: "Use",
  serverChosen: "server picked",
  anyServer: "any server",
  changeGame: "Change game",
  recents: "Recent",
  forgetRecent: (name) => `Remove ${name} from recent`,

  accountsTitle: "Accounts",
  accountsHint: "Each one opens in its own Roblox.",
  playAll: "Play with all",
  playAllTitle: "Opens one at a time, 3 seconds apart",
  robloxApp: "Roblox app",
  appAccountSub: "The account of this PC's Roblox app",
  readingAppLogin: "Reading the Roblox login...",
  loginExpired: "The login expired. Sign in again to keep using it.",
  moveUp: "Move up",
  removeAccount: "Remove from Quackblox",
  addAccount: "Add account",
  addAccountHint: "Sign in on the Roblox website in a separate window. The login is kept only on this PC, encrypted by Windows.",
  robloxAccount: "Roblox account",
  moreOptions: "More options",
  signInAgain: "Sign in again",
  opening: "Opening...",
  play: "Play",
  neverPlayed: "Hasn't played here yet",
  playedNow: "Played just now",
  playedMinutes: (n) => `Played ${n} min ago`,
  playedHours: (n) => `Played ${n} h ago`,
  playedDays: (n) => `Played ${n} ${n === 1 ? "day" : "days"} ago`,

  openOne: "Open",
  openMany: (n) => `${n} open`,
  focusWindow: "Bring the window forward",
  ecoOn: "Economy mode on: this account's Roblox runs at low priority and in Windows' Eco mode. Click to turn it off.",
  ecoOff: "Economy mode: this account's Roblox runs at low priority and in Windows' Eco mode, using less CPU. Click to turn it on.",
  ecoLabel: "Eco",
  hotkeyChip: (n) => `Ctrl+Shift+${n} brings this account's Roblox forward`,
  windowsOpen: (n) => `${n} Roblox ${n === 1 ? "window" : "windows"} open`,
  grid: "Arrange in a grid",
  gridTitle: "Puts every Roblox window side by side, on the main one's monitor",
  gridDone: (n) => `${n} ${n === 1 ? "window" : "windows"} arranged`,
  minimizeOthers: "Minimize the others",
  minimizeOthersTitle: "Minimizes every Roblox window except the main one (Ctrl+Shift+0)",

  mascotSleeping: "Sleeping. No Roblox open",
  mascotPlaying: (n) => `Playing. ${n} Roblox open`,

  pickGameFirst: "Pick a game first, up top",
  openingGame: (game, account) => `Opening ${game} with ${account}`,
  openingWithoutMulti: (account) => `Opening with ${account}. The multi is waiting, so the Roblox that was already open will close`,
  closedRoblox: "Closed the Roblox that was open to turn the multi on",
  accountAdded: (name) => `${name} joined Quackblox`,
  accountRemoved: (name) => `${name} was removed from the list`,

  startWithWindows: "Start with Windows",
  autoClose: "Close the Roblox that blocks the multi on its own",
  autoCloseTitle: "When a Roblox opened before Quackblox blocks the multi, it gets closed on its own",
  hotkeys: "Ctrl+Shift hotkeys",
  hotkeysTitle: "Ctrl+Shift+1 to 9 brings forward the Roblox of the account in that spot; Ctrl+Shift+0 minimizes all but the main one",
  hotkeysFailed: (keys) => `Another program already uses ${keys}. Those hotkeys were left out`,
  trayTitle: "Closing the window keeps Quackblox in the tray, with the multi on.",
  updateTo: (v) => `Update to ${v}`,
  updateStarting: "Opening the update…",
  checking: "Checking…",
  checkUpdate: "Check for updates",

  upToDate: "Quackblox is on the newest version",
  newVersion: (v) => `Quackblox ${v} is out`,
  update: "Update",
  checkFailed: (e) => `Couldn't check for updates: ${e}`,
  updateFailed: (e) => `Couldn't update: ${e}`,

  setupInstalled: "Quackblox installed",
  setupUpdateFailed: "The update failed",
  setupInstallFailed: "The install failed",
  setupUpdatingTo: (v) => `Updating to Quackblox ${v}`,
  setupLookingForVersion: "Looking for the new version…",
  setupDownloading: "Downloading…",
  setupDownloadingPercent: (p) => `Downloading… ${p}%`,
  setupInstallingUpdate: "Installing. Quackblox opens again on its own.",
  setupInstalling: "Installing…",
  setupDone: "Many accounts, one duck.",
  setupAskingGitHub: "Asking GitHub…",
  setupOverExisting: (v) => `Quackblox ${v} over what is already in this folder. Your accounts and settings stay.`,
  setupPitch: "Many Roblox accounts open at the same time.",
  setupUpdateTitle: "Quackblox update",
  setupInstallTitle: "Install Quackblox",
  close: "Close",
  change: "Change…",
  desktopShortcut: "Desktop shortcut",
  install: "Install",
  openQuackblox: "Open Quackblox",
  tryAgain: "Try again",
};

const dicts = { pt, en };

let lang: Lang = "pt";
const listeners = new Set<() => void>();

export const getLang = () => lang;
export const t = () => dicts[lang];

export function setLang(l: Lang) {
  document.documentElement.lang = l === "en" ? "en" : "pt-BR";
  if (l === lang) return;
  lang = l;
  listeners.forEach((f) => f());
}

// Outside the hook: a new function each render would make React subscribe again every time.
const subscribe = (f: () => void) => {
  listeners.add(f);
  return () => {
    listeners.delete(f);
  };
};

/** The texts in the current language; the component renders again when it changes. */
export function useT() {
  return dicts[useSyncExternalStore(subscribe, getLang)];
}
