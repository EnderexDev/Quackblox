# Changelog

All notable changes to Quackblox are listed here, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Each version is a tag and a GitHub
release of this repo; its section here is that release's notes. A change a user notices adds its
line under [Unreleased], and `npm run release` turns that section into the version.

## [Unreleased]

## [0.8.9] - 2026-11-12

### Changed
- Quackblox uses much less CPU and GPU while its window is open in the background: the colour glow behind the window no longer drifts, the pulsing dots pause while the window has no focus, and the accounts' status is refreshed when the window gets focus back instead of every 20 seconds.

## [0.8.8] - 2026-11-12

### Fixed
- The old Roblox closes again when the same account joins somewhere else. Since Roblox 0.746 its log no longer writes the error number for that drop, only the "joined from another device" text, and Quackblox now reads the text.

## [0.8.7] - 2026-11-05

### Fixed
- Quitting Quackblox from the tray no longer closes the open Roblox: a copy with no window keeps the multi-instance lock until the last Roblox window closes or Quackblox opens again. Closing the window of a failed update does the same.

## [0.8.6] - 2026-11-05

### Added
- Every Roblox that Quackblox closes is written to `ends.log` with the reason.

### Fixed
- Updating Quackblox no longer closes the open Roblox.

## [0.8.5] - 2026-11-05

A Roblox that lost its session because the same account joined somewhere else now closes by itself.

### Changed

- **clients:** when the same account joins another server or device, the old Roblox only shows the "same account" error, and its Reconnect would knock out the new session. Quackblox now closes it as soon as its log shows that drop (Roblox codes 264, 273, 276). This also covers Roblox opened from the site or the Roblox app: Quackblox reads the recent Roblox logs and closes the client on screen that wrote the drop. When two clients started within 10 s of each other and it cannot tell which one wrote it, it closes neither.
- **clients:** a Quackblox client closed this way is not told as a drop and does not rejoin.

## [0.8.4] - 2026-11-05

Roblox left idle in the tray no longer sits beside the game you are playing.

### Fixed

- **clients:** Roblox opens a spare copy of itself in the tray when a game starts from a link and again when a game closes, and one of them stayed idle beside the game being played. Tools that read the newest Roblox log (Discord presence tools) read the idle one's and lost the game. While a Roblox window is open, Quackblox now ends any Roblox in the tray with no window that is over 30 s old, including ones it did not open. With no game open, Roblox keeps its tray.

## [0.8.3] - 2026-11-01

The new logo in the top bar and in the installer.

### Changed

- **ui:** the top bar shows the Quackblox logo (the two gold Roblox tiles) where the duck was. The line under the name still says what the clients are doing: sleeping, playing, everyone in, or who dropped. The duck's animations are gone with it.
- **setup:** the installer window shows the logo in place of the duck, with the same progress ring around it.

### Internal

- **dev:** Quackblox Dev now tells apart from Quackblox by colour. Its icon (window, taskbar, tray) is the green version of the logo (`src-tauri/icons-dev`, set in `tauri.dev.conf.json`), its accent colours are green, and the top bar and the footer say `Quackblox Dev`. The official build carries none of this.

## [0.8.2] - 2026-11-01

New app icon: two gold Roblox tiles.

### Changed

- **icon:** the app icon is now the official Quackblox logo, two Roblox tiles set apart, black and raised with a metal edge on the Quack gold background. It shows on the window, the tray, the taskbar and the installer. The icons in `src-tauri/icons` were made again from the 1024 px logo with `tauri icon`.

### Internal

- **icon:** the old logo `icon.svg` was removed. Nothing read it.

## [0.8.1] - 2026-10-30

Closing a Roblox by its X now ends the tray player it leaves behind right away.

### Fixed

- **clients:** after closing a Roblox with its X, the other clients sometimes lost mouse and keyboard. The tray player that a close leaves running is now ended at once when the client is closed, instead of only at the next `Jogar`. This was reported but not reproduced here, so the fix is aimed at the likely cause and is not confirmed.

## [0.8.0] - 2026-10-29

Per-account controls for running clients, closing in batches, drop history and moving your accounts to another PC.

### Added

- **audio:** `Mudo` mutes any account on its own. Quackblox sets that client's volume in the Windows mixer (`audio.rs`), so the other accounts keep their sound. A client shows in the mixer only after it makes a sound, so the watcher sets the mute again every 2 s. Before you use the mute for the first time, a sound you turned off yourself in the mixer is left alone.
- **clients:** restart an account every N hours (or `Nunca`). The client closes and joins the same game again, counted from when the Roblox opened and only while it sits in the game, never while it joins or on a drop dialog. It goes to the game, not to the same server; a private server is kept.
- **clients:** each account remembers where its window sits on the screen and how big it is. `Salvar onde está agora` keeps the current place, `Esquecer o lugar` drops it, and `Pôr nos lugares` sends every open window back. Roblox sets its own size a moment after the window shows, so the place is set, checked 3 s later and set again if it moved. Roblox does not go smaller than about 800x630.
- **accounts:** give an account a nickname and a color. Both show on its card, in the Roblox window title and on the phone page, and the Roblox name stays below.
- **close:** `Close all` closes every Roblox Quackblox opened, one group's or one profile's, from the PC (palette included) or from the phone page. Roblox clients Quackblox did not open stay.
- **schedule:** a time can now close a profile, not only open it (`Action`: open or close).
- **servers:** save private servers as favorites, each with its own name.
- **memory:** when a Roblox client stays above your limit for a minute, Quackblox tells you (in Windows, on Discord if on, and on the phone) and can reopen it in the same game. The limit goes from 500 to 32768 MB, and 0 turns it off. A client that hovers at the limit is told once, and again only after it came well down.
- **history:** drop history (`drops.json`) with every drop, crash and failed join, with its time, the account and Roblox's error code. The newest 500 are kept, it can be filtered by account and cleared, and the card shows the drops of the last 7 days.
- **stats:** hours chart per day and per week, with each account listed by its nickname. Days are counted from this version on, and the last 120 days are kept.
- **transfer:** export your accounts to a file locked with a password (at least 8 characters) and import it on another PC (`transfer.rs`). The file carries the logins, nicknames, groups, profiles, schedule, favorites and the per-account switches. Logins are encrypted with XChaCha20-Poly1305 and a key made from your password with Argon2id, so a wrong password or a changed byte opens nothing. On import, an account already there gets the file's login, and groups, profiles, favorites and schedule times not there yet are added, with the schedule times turned off. Files over 8 MB are not read.
- **phone:** on the phone page, a drop plays a beep and makes the phone vibrate while the page is open.

### Changed

- **ui:** account cards now fit the `Jogar` button plus three round buttons.
- **stats:** the hours chart is aligned left, and the hours list names an account by its nickname.

### Fixed

- **accounts:** the accounts file is checked whole before any account is saved, has a size limit, and is written to a temporary file first and then renamed, so a crash in the middle no longer leaves it half written. The temporary name is new each time.
- **history:** an unreadable `drops.json` is moved aside instead of being written over, and one that cannot be read or moved aside is never written over.
- **transfer:** importing accounts tells the window which accounts were saved, even when the account lists fail to load.

### Internal

- **dev:** `QUACKBLOX_HOUR_SECS` shortens the hour of the restarts for tests, in Dev builds only. A mute probe was added for the tests.
- **deps:** `argon2` 0.5 and `chacha20poly1305` 0.10, plus the `Win32_Media_Audio`, `Win32_System_Com_StructuredStorage` and `Win32_System_Variant` features of the `windows` crate.

## [0.7.0] - 2026-10-29

Extras: a phone page on the home network paired by QR, live screens and prints of each account, and Discord Rich Presence. All of it starts off.

### Added

- **phone:** a phone page served by a small HTTP server of its own (`phone.rs`, port 38250; the Dev build uses 38251). It answers only addresses of the home network, Tailscale's and this PC, and drops any other before reading its request. It can open, close, rejoin and switch the game of an account, send an account to another's server (follow) and turn `Eco` on.
- **phone:** pairing by QR. A random key is kept encrypted beside the settings, never in `settings.json`. The QR carries the address with the key after `#`, a part the phone's browser never sends to a server. A wrong key is refused with a growing wait (after 5 in a row from one address), and `Change the key` unpairs every phone. Nothing on the page carries a login, a cookie or a ticket.
- **phone:** away from home it works over Tailscale, with no port opened on the router. The window shows the home-network and Tailscale addresses, and notes that Windows asks the first time whether to allow Quackblox on private networks.
- **screens:** a live view of each account's window, look only, on the phone and in the new `Screens` view of the app, plus a `Print` of one account or `Print of all` side by side, saved to `Pictures\Quackblox`. It uses Windows' own window capture (`capture.rs`, the one OBS uses), so a window behind others still shows and nothing is sent to it. A minimized window draws nothing and is told as minimized, never restored. A capture starts on a window's first request and ends 5 s after the last, so nothing is captured while nobody watches.
- **discord:** `Show on Discord the game the accounts are in` turns on Rich Presence: "Playing Quackblox" with the game and how many accounts are in it, never the accounts' names. It talks to the Discord desktop app over its local pipe (`discord.rs`), so nothing goes out to the network from there. The presence is sent again every minute.

### Fixed

- **phone:** a request with no key is refused without counting toward the wait, wrong-key waits are kept for an hour, answers are written against one deadline per request, notices come in id order, and a pasted game is looked up once. Name lookups that failed are retried a minute later.
- **phone:** requests are dropped once the page is turned off, and the capture waits for a first frame only on a new session.
- **prints:** two prints never share a file name, and a half-written print is removed.
- **discord:** the Discord pipe is answered within 5 s and its pings are answered, and games are counted by place.

### Internal

- **tools:** `cdp.mjs` picks a page by address. The capture's CPU measurement was added for the tests.

## [0.6.0] - 2026-10-29

Organization: account groups, one-click profiles, a schedule, a Ctrl+K command palette and the Roblox site signed in to a saved account.

### Added

- **groups:** a group gathers accounts to open at once. `Jogar com o grupo` opens the group's accounts in the picked game, one at a time, 3 seconds apart.
- **profiles:** a profile keeps the game, the accounts, which of them open in eco mode, the grid and the same-server option. `Abrir` runs it in one click. A group or profile whose accounts were all removed is refused.
- **schedule:** the schedule opens a profile at a set time and weekday, with Quackblox open or in the tray. It can run up to 5 min late, and with the PC off or asleep that day is skipped. Each time shows when it last ran.
- **palette:** `Ctrl+K` opens a command palette that does everything from the keyboard: play with an account, play with a group, open a profile, bring an account's window forward, turn eco or rejoin on or off for an account, open the site as an account, use a recent game and switch the language. Escape closes it from anywhere in it, and a picked line is kept when the list shrinks.
- **site:** `Open the site signed in` opens the Roblox site logged in to any saved account, in a window with its own WebView2 profile. The login is put in as an HttpOnly cookie from Rust, read back on close (a rotated login is kept) and the whole profile folder is deleted when the window closes.

### Changed

- **launch:** every `Jogar` (a card, all, a group, a profile, the schedule) now goes through `launch.rs`. The seat wait, the same server and the 3 s gap moved from the window to Rust, which is why the schedule can open accounts with the window in the tray.
- **palette and pickers:** when two accounts share a name, the palette and the pickers say which one is which, and a schedule slot reads "ran" instead of "opened".
- **stop wait:** stopping a wait for room lets the card go within a second.

### Fixed

- **site:** the login cookie is set through WebView2's cookie manager for `.roblox.com`, because the `cookie` crate dropped the leading dot and `www` never got it. It is looked for until it is stored. Tested in the Dev build with two site windows signed in to two accounts at once: the folders were deleted on close and the logins stayed valid.

### Internal

- **organize:** groups, profiles and the schedule live in `organize.json`, a file of its own, because the window sends every setting at each change and these lists would ride along on every switch.
- **ui:** saves go in order on the latest lists, an editor reopened for another group or profile starts from it, a save answering after its sheet closed does not close the next one, and an IME's Enter or Escape does not pick or leave the palette.

## [0.5.0] - 2026-10-28

`Vigia`: Quackblox notices when an account drops, can rejoin it on its own, warns you, and counts hours played.

### Added

- **vigia:** each client's log is read piece by piece while it runs, so its state (`Entrando...`, `No jogo`, `Caiu`, `Não entrou`) is known. A drop leaves the client open with Roblox's error dialog, so the reason is seen in the log before the process ends. The first reason after the last join is the cause, and a teleport's next join clears it within a 10 s grace.
- **vigia:** a drop counts after 10 s only when the server cut the client off, and after 60 s otherwise (a teleport). A client that never joins is a failed start after 2 minutes. Closing the Roblox window yourself (code 285) never counts as a drop.
- **rejoin:** `Reentrar` per account, a button beside `Eco`. When that account drops, Quackblox ends the dropped client and plays the same game again, 5, 30 and 120 s apart. It does not rejoin after your own close, or when the same account went in elsewhere. Each card's button acts only on that card.
- **alerts:** a drop notification in Windows, and optionally a message to a Discord webhook. Only the account name, what happened and the code go to Discord, never the login.
- **logins:** an optional check of every saved login every 30 min (off by default), with `Conferir agora`. It tells you as soon as one expires, since Roblox does not say beforehand.
- **stats:** hours played per account and per game, kept in `stats.json` (only the time inside a game, for accounts joined through Quackblox), with a reset button.
- **duck:** the mascot cheers when two or more accounts are in, and panics with the account's name when one drops.

### Fixed

- **language:** switching the language no longer reverts the watch settings, because the language is now sent last.
- **games:** a game name whose lookup keeps failing is asked at most 3 times, and a name that failed is asked again later instead of staying blank. A server that ended is not asked again.
- **rejoin:** a client that would not end blocks the open, each try is checked again before ending or opening, and the card being followed travels with the client (and in `clients.json`). Ending a dropped client waits on a handle of its own, off the async runtime, and marks it only once it ended.
- **clients:** a windowless player whose parent is an open client counts as its tray player, and a client gone before its window showed is a failed start. The cheer always ends.

### Internal

- **dev:** `QUACKBLOX_REJOIN_ANY` allows the rejoin in dev tests. The new watch lives in `vigia.rs`.

## [0.4.0] - 2026-10-28

Servers: presence dots, a server list, private server links, following a friend, waiting for a seat and a Roblox error code translator.

### Added

- **presence:** a dot on every account shows where it is: offline, on the site, in a game or in Studio. One request covers all accounts with one login, and an account's own login is used for a server the first one may not see.
- **servers:** `Servidores` lists the public servers of the picked game with players, and the average ping and FPS of each server's players (not yours). It sorts fullest or emptiest first, picks `O mais vazio` or `O mais cheio com vaga` in one click, loads more, and takes a pasted JobId.
- **servers:** private server share links (`roblox.com/share?code=`) are read with a saved login and joined by `RequestPrivateGame` with the link code. `privateServerLinkCode` links and `joinGameInstance` links work too.
- **friends:** `Seguir amigo` lists the friends of any saved account who are in a game and joins their server.
- **seat:** joining a full server waits for room, looking every 15 s, and joins on its own when a seat opens. Clicking the card stops the wait.
- **play all:** `Mesmo servidor` in `Jogar com todas`: the first account joins, and the others go to the server it joined, read from its client log (or its presence). If it does not join one in 90 s, the others go to any server.
- **errors:** when a client leaves, a notice gives the reason, read from the client log. A translator explains any Roblox error code typed in, with names from `Enum.ConnectionError` and plain explanations for 264 to 773 (277, 279, 285 and the teleport codes 769 to 773 among them).

### Fixed

- **play all:** `Jogar com todas` no longer opens the same account twice. The Roblox app's login changes as clients open (each writes its account there), so the app card's cached account was stale, and the second client kicked the first (code 276). The app's account is now read at the click.
- **clients:** each client keeps the Roblox user it joined as, so the app card no longer names a client wrongly and its log is found. A failed read of the app's account leaves the client with no user, never the one cached before.
- **errors:** the exit reason is read only after the client's last join. A client's log must start within 5 s of the client, in its own second or the next, never a later launch's, and its window ends at the next start of the same user, open or ended. Clients linked before the user was kept get it from the saved account's key.

### Internal

- **dev:** `QUACKBLOX_NO_LAUNCH` blocks play in dev builds, for testing the waits with no client opened. `tools/dev2.sh` starts a second Dev beside an open one, with launches blocked on request.
- **tests:** `cdp.mjs` (type-in) is found on any path.

## [0.3.0] - 2026-10-28

Every open Roblox is tied to its account: window title, CPU and RAM on the card, economy mode, a window grid, hotkeys and a drag-to-reorder account list.

### Added

- **clients:** each Roblox opened by Quackblox is tied to its account. `ShellExecuteEx` hands back the player's process at launch, and a thread watches each client: its window, CPU and RAM (shown on the card), economy mode and exit code. `clients.json` relinks the clients after a Quackblox restart.
- **clients:** the Roblox window title carries the account's name, and the card shows `Aberto` (or `N abertos`) with a button to bring the window forward.
- **eco:** per-account economy mode (`Eco`): that account's Roblox runs at low priority and in Windows' Eco mode, and it is kept on while the client lives.
- **grid:** `Organizar em grade` puts every Roblox window side by side on the main client's monitor, spreading windows that Roblox will not shrink. `Minimizar as outras` minimizes all but the main one.
- **hotkeys:** Ctrl+Shift+1 to 9 brings forward the Roblox of the saved account in that position, and Ctrl+Shift+0 minimizes all but the main one. If another program already uses a key, the app says which ones were left out.
- **accounts:** hold a card and drag it to change the order of the saved accounts. The list moves under the pointer and the place is saved on release.
- **duck:** the mascot sleeps when no Roblox is open and plays when there is one.

### Changed

- **hotkeys:** the hotkeys number the saved accounts only. The Roblox app's account changes with whoever is logged in, so it has no number and comes last in the grid and as the main client.
- **hotkeys:** focus joins the front window's input for a moment, so Windows lets the hotkey bring a window forward.

### Fixed

- **tray:** a Roblox left in the tray after its window closed no longer blocks the next one. With one running, the client opened by a link ended at start (`0xC0000409`) and the tray one took the game unlinked or lost it (measured 28/10/26). Before each play, Quackblox now ends the windowless players that belong to no account it started, when they come from a client it saw end or have run for over 30 s.
- **tray:** a tray player is only counted when it was born before its client ended (a reused pid no longer marks a new player), and an old windowless player is ended only when its parent is gone, so a site client still loading is spared. The launch waits on each ended process instead of a fixed 500 ms.
- **eco:** the tray children of an Eco client go back to normal priority when that client ends, instead of keeping the idle priority they inherited.
- **clients:** an ended client's time is its real exit time, read from its handle. The watcher's look came up to 1 s late, and a pid reused in that second could mark a new player as tray.
- **tray:** the tray cleanup runs off the async runtime (it waits up to 3 s per player). A parent Quackblox that cannot be opened now counts as running unless the error is "no such process", so access denied spares the player.
- **drag:** a card drag follows one pointer only (a second finger is ignored), a release before React redraws still saves the right place, and `touch-action: pan-y` lets a finger scroll the list over the cards.

### Internal

- **tests:** unit tests on a real short process, and on a live pid, a missing one and `csrss.exe` (which denies the query). A test script checks whether a Ctrl+Shift hotkey is free, and the second-pointer test moves in Y too, so it reaches a card on another row.

## [0.2.5] - 2026-10-28

English and Portuguese, switched from the top bar, plus a separate Quackblox Dev build that runs beside the installed app.

### Added

- **language:** the app speaks English and Portuguese, switched by `PT | EN` in the top bar. The choice is saved in `settings.json` (a missing field means Portuguese) and the window reads it before the first render.
- **language:** the Rust side follows the same choice through `tr!`: the tray menu, the login window, its error messages and the installer window.

### Changed

- **settings:** the window now sends every setting on each save, so changing one switch never resets another.
- **updater:** the held installer/update copy has its size checked before it is read, and update notices cancel their own timers.

### Internal

- **dev:** `npm run app:dev` runs as Quackblox Dev (`tauri.dev.conf.json`) with its own identifier, data, tray icon and startup entry, so it runs beside the installed app. A debug build never closes Roblox.

## [0.2.4] - 2026-10-28

The installer and updater copies are now held open from the check until the install starts, and links are refused.

### Changed

- **installer:** the installer and updater copies are written by `write_held`: it writes a new file, reopens it with `FILE_FLAG_OPEN_REPARSE_POINT` shared read-only, refuses a link, checks the bytes and keeps the handle until the copy has started (updater) or NSIS has finished (install). Nothing can swap the file between the check and the run.
- **installer:** a failed spawn deletes the copy, and the sweep also removes `Quackblox-Install-` and `Quackblox-Update-` leftovers.

## [0.2.3] - 2026-10-28

The updater's sweep also cleans up the fixed-name copy that 0.2.0 and 0.2.1 left behind.

### Fixed

- **updater:** the sweep of leftover updater copies now also removes the fixed-name copy that 0.2.0 and 0.2.1 left in the temp folder, which the random names from 0.2.2 never matched.

### Internal

- **tools:** `tools/update-run.sh` added (23 lines).

## [0.2.2] - 2026-10-28

Hardens the update and install window against a second window, a guessable temp file and a failed open.

### Changed

- **updater:** the updater's temp copy gets a random name, created new and shared read-only while it is written. Every leftover copy is swept at start and again a minute later.
- **updater:** a second installer or update window exits (named mutex). Installs and updates claim a guard, so a second one is refused, and the window stays open until the running one ends.

### Fixed

- **updater:** after a failed update, a failed open of the app now keeps `Tentar de novo` instead of dropping it.

### Internal

- **release:** `release.mjs` now refuses a tree with untracked files too, since a new source file would go into the build but not into master.

## [0.2.1] - 2026-10-28

A version bump with no code change, published to exercise the automatic update from 0.2.0.

### Internal

- **release:** the version goes from 0.2.0 to 0.2.1 in `package.json`, `Cargo.toml`, `Cargo.lock` and `tauri.conf.json`. Nothing else changed.

## [0.2.0] - 2026-10-28

Quackblox stops being a C# tray unlocker and becomes a Tauri app with saved accounts, its own installer and automatic updates.

### Added

- **multi:** holds `ROBLOX_singletonMutex` on its own thread, so each client opened from the site's Play button (`roblox-player:`) stays open instead of closing the previous one. Measured with real joins: 2 clients alive after the second join, against the first one closing without it.
- **multi:** `Fechar sozinho o Roblox que trava o multi` (on by default) ends the Roblox that holds the lock. It gives up after passes that end Roblox and still find the lock held, or when no Roblox player is running, and the status pill offers `Fechar agora` again. A failed `CreateMutexW` no longer counts as a Roblox, and a lock that cannot be opened ends as stuck.
- **accounts:** saved accounts come from a private Roblox login window. The login is kept DPAPI-encrypted in `accounts.json`, rotated logins are saved and expired ones are flagged. A broken `accounts.json` is set aside instead of being overwritten.
- **accounts:** the Roblox app's own account is read from `RobloxCookies.dat` (DPAPI) and never stored.
- **play:** play per account with an auth ticket and a `roblox-player:` link, the same way the site does it.
- **games:** game picker by ID or link (server links too), recent games and `Jogar com todas`.
- **app:** tray icon, close to tray and start with Windows.
- **installer:** `Quackblox-Setup.exe` is the release `quackblox.exe` with the NSIS installer appended and a `QBSETUP1` trailer. On start it finds that trailer and opens a frameless window (the duck, install folder, desktop shortcut, `Instalar`, `Abrir o Quackblox`) that runs NSIS silently. It installs to `%LOCALAPPDATA%\Quackblox`; over an existing install the button reads `Atualizar`.
- **installer:** the window refuses to close while NSIS runs. A failed write deletes its partial copy, and a picked folder named quackblox in any case counts as the install folder. The NSIS copy goes to a random name, created new and shared for reading only while it is written.
- **installer:** switching the desktop shortcut off removes it even when an earlier install made it. `Abrir` opens the folder the install really went to, `Instalar` and `Alterar` wait while the folder picker is open, and a failed open keeps `Abrir` instead of offering to install again.
- **updater:** the app checks `latest.json` 10 s after start, every 6 hours and from the footer, and offers the new version in a notice. The update goes to a temp copy of the app that downloads it, checks the signature against the public key in `tauri.conf.json` and runs NSIS with `/UPDATE /R`.

### Changed

- **app:** the C# tray unlocker (Roblox Multi) is replaced by a Tauri 2 + React app, in Quackify's look.

### Internal

- **release:** `npm run release -- X.Y.Z "notes"` builds, signs (`~/.tauri/quackblox.key`), packs, commits the version and creates the GitHub release. `npm run setup:build` builds the installer.
- **multi:** `taskkill` runs from System32 by full path, `settings.json` is written beside and renamed over, the window registers its listeners before reading the multi's status (a Roblox closed before it loaded still gets its notice), and the auto-close switch is disabled while its save is pending.
- **tests:** scripts for the Roblox presence of accounts and for two plays a given gap apart, used to check two different accounts in the same game at once (2 new clients at 3 s and at 10 s).
- **project:** renamed to Quackblox, with a `build.ps1` for the C# builds before the rewrite, and the Notion structure ids added.

[Unreleased]: https://github.com/EnderexDev/Quackblox/compare/v0.8.9...HEAD
[0.8.9]: https://github.com/EnderexDev/Quackblox/compare/v0.8.8...v0.8.9
[0.8.8]: https://github.com/EnderexDev/Quackblox/compare/v0.8.7...v0.8.8
[0.8.7]: https://github.com/EnderexDev/Quackblox/compare/v0.8.6...v0.8.7
[0.8.6]: https://github.com/EnderexDev/Quackblox/compare/v0.8.5...v0.8.6
[0.8.5]: https://github.com/EnderexDev/Quackblox/compare/v0.8.4...v0.8.5
[0.8.4]: https://github.com/EnderexDev/Quackblox/compare/v0.8.3...v0.8.4
[0.8.3]: https://github.com/EnderexDev/Quackblox/compare/v0.8.2...v0.8.3
[0.8.2]: https://github.com/EnderexDev/Quackblox/compare/v0.8.1...v0.8.2
[0.8.1]: https://github.com/EnderexDev/Quackblox/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/EnderexDev/Quackblox/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/EnderexDev/Quackblox/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/EnderexDev/Quackblox/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/EnderexDev/Quackblox/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/EnderexDev/Quackblox/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/EnderexDev/Quackblox/compare/v0.2.5...v0.3.0
[0.2.5]: https://github.com/EnderexDev/Quackblox/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/EnderexDev/Quackblox/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/EnderexDev/Quackblox/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/EnderexDev/Quackblox/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/EnderexDev/Quackblox/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/EnderexDev/Quackblox/releases/tag/v0.2.0
