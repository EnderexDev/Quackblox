// The Roblox clients Quackblox opened, each tied to the account it opened it with. The link comes
// from the launch itself: ShellExecuteEx hands back the player's process (win::open_link), so no
// command line is ever read. A thread looks at every client once a second: its window (found
// about 7 s after the click, measured 28/10/26) gets the account's name in its title, its CPU
// and RAM go to the window, the economy mode stays applied, and its end is heard with the exit
// code (phase 3 tells a crash from a close with it). The links live in clients.json, so a
// Quackblox restarted (an update) finds the clients still open.
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, FILETIME, HANDLE, HWND, LPARAM, RECT, WAIT_OBJECT_0, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromWindow};
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, GetExitCodeProcess, GetPriorityClass, GetProcessId, GetProcessTimes, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, OpenProcess,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, PROCESS_VM_READ,
    ProcessPowerThrottling, SetPriorityClass, SetProcessInformation, TerminateProcess, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GW_OWNER, GetForegroundWindow, GetWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, IsZoomed,
    SMTO_ABORTIFHUNG, SW_MINIMIZE, SW_RESTORE, HWND_TOP, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageTimeoutW, SetForegroundWindow,
    SetWindowPos, ShowWindowAsync, WM_SETTEXT,
};
use windows::core::BOOL;

const PLAYER_EXE: &str = "RobloxPlayerBeta.exe";
/// 1601-01-01 to 1970-01-01 in FILETIME's 100 ns units.
const UNIX_TO_FILETIME: u64 = 116_444_736_000_000_000;

/// A process handle owned by Quackblox, closed with it.
struct Proc(HANDLE);
// Process handles are plain kernel handles, usable from any thread.
unsafe impl Send for Proc {}
impl Drop for Proc {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

struct Client {
    pid: u32,
    /// Process creation time: a pid reused by another process after a restart has another.
    created: u64,
    /// "app" or a saved account's id, as `play` takes it.
    account: String,
    /// The account's display name, for the title.
    name: String,
    /// The Roblox user it joined as, for finding its log. The app's account is whoever was logged
    /// in to the Roblox app at the click, which changes, so it is kept with the client.
    user: Option<u64>,
    proc: Proc,
    hwnd: Option<isize>,
    had_window: bool,
    /// The economy mode set on it, so turning it off restores only what Quackblox changed.
    eco_on: bool,
    /// Busy time (kernel plus user, 100 ns units) at the last look, for the CPU share.
    busy: u64,
    at: Instant,
    cpu: f32,
    ram: u64,
}

#[derive(Serialize, Deserialize)]
struct Link {
    pid: u32,
    created: u64,
    account: String,
    name: String,
    #[serde(default)]
    user: Option<u64>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct File {
    links: Vec<Link>,
    /// Accounts with the economy mode on, kept for their next clients too.
    eco: Vec<String>,
}

pub struct Clients {
    path: PathBuf,
    list: Mutex<Vec<Client>>,
    eco: Mutex<HashSet<String>>,
    /// Clients seen ending (pid, when, as a FILETIME), newest last: a player process one of
    /// them started before it ended is a tray one. The time keeps a reused pid from counting.
    gone: Mutex<Vec<(u32, u64)>>,
}

/// What the window sees of a client.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pid: u32,
    account: String,
    /// Share of the whole machine, 0 to 100, as Task Manager shows it.
    cpu: f32,
    /// Working set in bytes.
    ram: u64,
    window: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    clients: Vec<ClientInfo>,
    eco: Vec<String>,
    /// Roblox player windows Quackblox did not open (the site's Play button, the app's own icon).
    others: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Exited {
    pid: u32,
    account: String,
    /// The process's exit code: 0 for a window closed by him, other values for a crash or a kill.
    code: u32,
    /// False when it ended before its window ever showed (a join that failed at the start).
    had_window: bool,
    /// Why it left its last server, from its log (Enum.ConnectionError, 285 when he left).
    reason: Option<u32>,
    /// Creation time and user, for finding its log; not sent.
    #[serde(skip)]
    created: u64,
    #[serde(skip)]
    user: Option<u64>,
}

fn filetime(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

/// Creation time and busy time of a process.
fn times(h: HANDLE) -> Option<(u64, u64)> {
    let (mut c, mut e, mut k, mut u) = Default::default();
    unsafe { GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) }.ok()?;
    Some((filetime(c), filetime(k) + filetime(u)))
}

/// When an ended process ended, from Windows itself.
fn exit_time(h: HANDLE) -> Option<u64> {
    let (mut c, mut e, mut k, mut u) = Default::default();
    unsafe { GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) }.ok()?;
    Some(filetime(e))
}

/// A FILETIME as Unix seconds.
fn unix(filetime: u64) -> u64 {
    filetime.saturating_sub(UNIX_TO_FILETIME) / 10_000_000
}

/// Now as a FILETIME, the clock process creation times use.
fn now_filetime() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| (d.as_nanos() / 100) as u64).unwrap_or(0) + UNIX_TO_FILETIME
}

/// Whether a process with this pid may be running. Only "no such process" reads as gone: a
/// process Quackblox may not open (access denied) counts as running, and so does a reused pid,
/// which only ever spares a tray player.
fn running(pid: u32) -> bool {
    match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE, false, pid) } {
        Ok(h) => exit_code(Proc(h).0).is_none(),
        Err(e) => e.code() != ERROR_INVALID_PARAMETER.to_hresult(),
    }
}

fn exit_code(h: HANDLE) -> Option<u32> {
    if unsafe { WaitForSingleObject(h, 0) } != WAIT_OBJECT_0 {
        return None;
    }
    let mut code = 0;
    let _ = unsafe { GetExitCodeProcess(h, &mut code) };
    Some(code)
}

fn ram(h: HANDLE) -> u64 {
    let mut m = PROCESS_MEMORY_COUNTERS::default();
    let ok = unsafe { K32GetProcessMemoryInfo(h, &mut m, std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32) };
    if ok.as_bool() { m.WorkingSetSize as u64 } else { 0 }
}

/// Idle priority and Windows' Eco (EcoQoS: the CPU runs the process at its most efficient clock),
/// or back to normal priority with the throttling left for Windows to decide. Measured 28/10/26:
/// Roblox does not undo either.
fn set_eco(h: HANDLE, on: bool) {
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: if on { PROCESS_POWER_THROTTLING_EXECUTION_SPEED } else { 0 },
        StateMask: if on { PROCESS_POWER_THROTTLING_EXECUTION_SPEED } else { 0 },
    };
    let size = std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32;
    let _ = unsafe { SetProcessInformation(h, ProcessPowerThrottling, &state as *const _ as _, size) };
    let _ = unsafe { SetPriorityClass(h, if on { IDLE_PRIORITY_CLASS } else { NORMAL_PRIORITY_CLASS }) };
}

/// The process's main window: its largest visible window with no owner (the crash reporter's
/// dialogs and tool windows have one, or are smaller).
fn window_of(pid: u32) -> Option<HWND> {
    struct Find {
        pid: u32,
        best: Option<HWND>,
        area: i64,
    }
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let find = unsafe { &mut *(lparam.0 as *mut Find) };
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid != find.pid || !unsafe { IsWindowVisible(hwnd) }.as_bool() || unsafe { GetWindow(hwnd, GW_OWNER) }.is_ok_and(|o| !o.is_invalid()) {
            return true.into();
        }
        let mut r = RECT::default();
        let _ = unsafe { GetWindowRect(hwnd, &mut r) };
        let area = (r.right - r.left) as i64 * (r.bottom - r.top) as i64;
        if find.best.is_none() || area > find.area {
            find.best = Some(hwnd);
            find.area = area;
        }
        true.into()
    }
    let mut find = Find { pid, best: None, area: 0 };
    let _ = unsafe { EnumWindows(Some(each), LPARAM(&mut find as *mut Find as isize)) };
    find.best
}

/// Every Roblox player process running, opened by Quackblox or not.
fn player_pids() -> Vec<u32> {
    player_procs().into_iter().map(|(pid, _)| pid).collect()
}

/// Every Roblox player process running, with its parent's pid.
fn player_procs() -> Vec<(u32, u32)> {
    let Ok(snap) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else { return vec![] };
    let mut out = vec![];
    let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    let mut more = unsafe { Process32FirstW(snap, &mut e) }.is_ok();
    while more {
        let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
        if String::from_utf16_lossy(&e.szExeFile[..len]).eq_ignore_ascii_case(PLAYER_EXE) {
            out.push((e.th32ProcessID, e.th32ParentProcessID));
        }
        more = unsafe { Process32NextW(snap, &mut e) }.is_ok();
    }
    let _ = unsafe { CloseHandle(snap) };
    out
}

fn title_of(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// Puts the account's name in the window's title when it is not there yet. Sent with a time
/// limit: a Roblox window that stopped answering never holds the thread.
fn ensure_title(hwnd: HWND, name: &str) {
    let want = format!("{name} · Roblox");
    if title_of(hwnd) == want {
        return;
    }
    let wide: Vec<u16> = want.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { SendMessageTimeoutW(hwnd, WM_SETTEXT, WPARAM(0), LPARAM(wide.as_ptr() as isize), SMTO_ABORTIFHUNG, 300, None) };
}

/// Brings a window forward, restored if it was minimized. Windows lets a process do it only
/// right after input of its own: a click in Quackblox or one of its hotkeys.
fn focus(hwnd: HWND) {
    // Even right after a hotkey Windows refused the focus now and then (2 of 3 times with
    // Discord in front, 28/10/26). Joined for a moment to the input of the window in front,
    // this thread is let through. No key is sent to anyone.
    let front = unsafe { GetWindowThreadProcessId(GetForegroundWindow(), None) };
    let me = unsafe { GetCurrentThreadId() };
    let joined = front != 0 && front != me && unsafe { AttachThreadInput(me, front, true) }.as_bool();
    // Foreground first, then restored: the other way round (measured 28/10/26) a minimized
    // window comes back behind the one in front.
    let _ = unsafe { SetForegroundWindow(hwnd) };
    let _ = unsafe { BringWindowToTop(hwnd) };
    if joined {
        let _ = unsafe { AttachThreadInput(me, front, false) };
    }
    if unsafe { IsIconic(hwnd) }.as_bool() {
        let _ = unsafe { ShowWindowAsync(hwnd, SW_RESTORE) };
    }
}

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut _)
}

impl Clients {
    fn save(&self, list: &[Client]) {
        let file = File {
            links: list.iter().map(|c| Link { pid: c.pid, created: c.created, account: c.account.clone(), name: c.name.clone(), user: c.user }).collect(),
            eco: self.eco.lock().unwrap().iter().cloned().collect(),
        };
        let Ok(text) = serde_json::to_string_pretty(&file) else { return };
        let tmp = self.path.with_extension("json.tmp");
        // Only links: a lost write costs the CPU/RAM line of clients already open, never an account.
        if let Err(e) = std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &self.path)) {
            eprintln!("clients: clients.json not saved: {e}");
        }
    }

    fn snapshot(&self, others: usize) -> Snapshot {
        let list = self.list.lock().unwrap();
        Snapshot {
            clients: list
                .iter()
                .map(|c| ClientInfo { pid: c.pid, account: c.account.clone(), cpu: c.cpu, ram: c.ram, window: c.hwnd.is_some() })
                .collect(),
            eco: self.eco.lock().unwrap().iter().cloned().collect(),
            others,
        }
    }

    /// Ties a client just launched to its account.
    pub fn add(&self, process: HANDLE, account: &str, name: &str, user: Option<u64>) {
        let proc = Proc(process);
        let pid = unsafe { GetProcessId(process) };
        let Some((created, busy)) = times(process) else { return };
        let eco_on = self.eco.lock().unwrap().contains(account);
        if eco_on {
            set_eco(process, true);
        }
        let mut list = self.list.lock().unwrap();
        list.push(Client {
            pid,
            created,
            account: account.into(),
            name: name.into(),
            user,
            proc,
            hwnd: None,
            had_window: false,
            eco_on,
            busy,
            at: Instant::now(),
            cpu: 0.0,
            ram: 0,
        });
        self.save(&list);
    }

    /// Ends the Roblox players left in the tray: the process a client leaves behind when its
    /// window closes. Measured 28/10/26: with one running, the client a link opens ends at start
    /// with 0xC0000409 and hands the game to it (a fresh tray process opens it as a window of no
    /// account) or loses it (one 20 min old did, 5 launches in a row, the site's way too). Only a
    /// player with no window and of no account counts: one started by a client seen ending,
    /// before it ended, or one over 30 s old whose parent is gone. A client the site just opened
    /// has no window yet either, but its parent (the browser) is still running. Waits until each
    /// one has ended, up to 3 s: the new client must not find it. Says how many it ended.
    pub fn end_tray_players(&self) -> usize {
        let ours: HashSet<u32> = self.list.lock().unwrap().iter().map(|c| c.pid).collect();
        let gone = self.gone.lock().unwrap().clone();
        let now = now_filetime();
        let mut ended = 0;
        for (pid, parent) in player_procs() {
            if ours.contains(&pid) || window_of(pid).is_some() {
                continue;
            }
            let access = PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE;
            let Ok(h) = (unsafe { OpenProcess(access, false, pid) }) else { continue };
            let proc = Proc(h);
            let Some((created, _)) = times(proc.0) else { continue };
            let left_by_client = gone.iter().any(|&(p, at)| p == parent && created <= at);
            let orphan = now.saturating_sub(created) > 30 * 10_000_000 && !running(parent);
            if (left_by_client || orphan) && unsafe { TerminateProcess(proc.0, 0) }.is_ok() {
                let _ = unsafe { WaitForSingleObject(proc.0, 3000) };
                ended += 1;
            }
        }
        ended
    }

    /// When the account's newest client started, as Unix seconds, and the user it joined as.
    pub fn newest_of(&self, account: &str) -> Option<(u64, Option<u64>)> {
        self.list.lock().unwrap().iter().filter(|c| c.account == account).max_by_key(|c| c.created).map(|c| (unix(c.created), c.user))
    }

    /// The main window of each client of the account, in launch order.
    fn windows_of(&self, account: &str) -> Vec<HWND> {
        self.list.lock().unwrap().iter().filter(|c| c.account == account).filter_map(|c| c.hwnd.map(hwnd)).collect()
    }
}

/// Reads clients.json, keeps the links whose process still runs with the same creation time, and
/// starts the thread that looks at them.
pub fn start(app: &AppHandle) {
    let dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
    let path = dir.join("clients.json");
    let file: File = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let access = PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_VM_READ;
    let eco: HashSet<String> = file.eco.into_iter().collect();
    let list: Vec<Client> = file
        .links
        .into_iter()
        .filter_map(|l| {
            let h = unsafe { OpenProcess(access, false, l.pid) }.ok()?;
            let proc = Proc(h);
            let (created, busy) = times(h)?;
            if created != l.created || exit_code(h).is_some() {
                return None;
            }
            let eco_on = eco.contains(&l.account);
            // Links saved before the user was kept: a saved account's key is its user id.
            let user = l.user.or(l.account.parse().ok());
            Some(Client {
                pid: l.pid,
                created,
                account: l.account,
                name: l.name,
                user,
                proc,
                hwnd: None,
                had_window: false,
                eco_on,
                busy,
                at: Instant::now(),
                cpu: 0.0,
                ram: 0,
            })
        })
        .collect();
    let clients = Clients { path, list: Mutex::new(list), eco: Mutex::new(eco), gone: Mutex::new(vec![]) };
    clients.save(&clients.list.lock().unwrap());
    app.manage(clients);

    let app = app.clone();
    std::thread::spawn(move || {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let clients = app.state::<Clients>();
            let mut exited = vec![];
            let mut eco_gone = vec![];
            let mut ended_at = vec![];
            {
                let mut list = clients.list.lock().unwrap();
                let before = list.len();
                list.retain_mut(|c| {
                    let h = c.proc.0;
                    if let Some(code) = exit_code(h) {
                        if c.eco_on {
                            eco_gone.push(c.pid);
                        }
                        // The real end, not this look's: a pid reused in between must not count.
                        ended_at.push((c.pid, exit_time(h).unwrap_or_else(now_filetime)));
                        exited.push(Exited { pid: c.pid, account: c.account.clone(), code, had_window: c.had_window, reason: None, created: c.created, user: c.user });
                        return false;
                    }
                    if !c.hwnd.is_some_and(|w| unsafe { IsWindow(Some(hwnd(w))) }.as_bool()) {
                        c.hwnd = window_of(c.pid).map(|w| w.0 as isize);
                    }
                    if let Some(w) = c.hwnd {
                        c.had_window = true;
                        ensure_title(hwnd(w), &c.name);
                    }
                    // Kept applied: another tool (or the game's own launcher) may put the priority back.
                    if c.eco_on && unsafe { GetPriorityClass(h) } != IDLE_PRIORITY_CLASS.0 {
                        set_eco(h, true);
                    }
                    if let Some((_, busy)) = times(h) {
                        let wall = c.at.elapsed().as_nanos() as f64 / 100.0;
                        if wall > 0.0 {
                            c.cpu = ((busy.saturating_sub(c.busy)) as f64 / wall / cores * 100.0).clamp(0.0, 100.0) as f32;
                        }
                        c.busy = busy;
                        c.at = Instant::now();
                    }
                    c.ram = ram(h);
                    true
                });
                if list.len() != before {
                    clients.save(&list);
                }
                let mut gone = clients.gone.lock().unwrap();
                gone.extend(ended_at);
                let extra = gone.len().saturating_sub(64);
                gone.drain(..extra);
            }
            // A client closing leaves a player process in the tray, born with its parent's idle
            // priority (seen 28/10/26): it goes back to normal, so nothing of Roblox stays slow
            // with no Eco shown on any card.
            if !eco_gone.is_empty() {
                for (pid, _) in player_procs().into_iter().filter(|(_, parent)| eco_gone.contains(parent)) {
                    if let Ok(h) = unsafe { OpenProcess(PROCESS_SET_INFORMATION, false, pid) } {
                        let child = Proc(h);
                        set_eco(child.0, false);
                    }
                }
            }
            // The log is read off this thread: a long session's log is megabytes.
            for mut e in exited {
                // A later client of the same user that is still open: its log is not this one's.
                let next = {
                    let list = clients.list.lock().unwrap();
                    list.iter().filter(|c| c.user.is_some() && c.user == e.user && c.created > e.created).map(|c| unix(c.created)).min()
                };
                let app = app.clone();
                std::thread::spawn(move || {
                    e.reason = crate::logs::of(e.user, unix(e.created), next).and_then(|p| crate::logs::reason_in(&p));
                    let _ = app.emit("client-exited", e);
                });
            }
            let _ = app.emit("clients", clients.snapshot(others(&clients)));
        }
    });
}

/// Roblox player windows open that Quackblox did not open.
fn others(clients: &Clients) -> usize {
    let ours: HashSet<u32> = clients.list.lock().unwrap().iter().map(|c| c.pid).collect();
    player_pids().into_iter().filter(|p| !ours.contains(p) && window_of(*p).is_some()).count()
}

/// Every Roblox player window: Quackblox's own in the order of the account cards, then the rest.
fn all_windows(app: &AppHandle) -> Vec<HWND> {
    let clients = app.state::<Clients>();
    let mut out: Vec<HWND> = crate::accounts::order(app).iter().flat_map(|a| clients.windows_of(a)).collect();
    let ours: HashSet<isize> = out.iter().map(|w| w.0 as isize).collect();
    let mut rest: Vec<u32> = player_pids();
    rest.sort();
    out.extend(rest.into_iter().filter_map(window_of).filter(|w| !ours.contains(&(w.0 as isize))));
    out
}

/// The main client: the first open in card order (the Roblox app's account comes last).
fn principal(app: &AppHandle) -> Option<HWND> {
    let clients = app.state::<Clients>();
    crate::accounts::order(app).iter().find_map(|a| clients.windows_of(a).into_iter().next())
}

/// Focuses the client of the saved account N, in card order. Used by a hotkey; the Roblox app's
/// account has no number, since it changes with whoever is logged in to Roblox.
pub fn focus_nth(app: &AppHandle, n: usize) {
    let order = crate::accounts::saved_order(app);
    if let Some(w) = order.get(n.wrapping_sub(1)).and_then(|a| app.state::<Clients>().windows_of(a).into_iter().next()) {
        focus(w);
    }
}

/// Minimizes every Roblox window but the main one, which comes forward. Used by a hotkey too.
pub fn minimize_others(app: &AppHandle) -> usize {
    let main = principal(app);
    let mut n = 0;
    for w in all_windows(app) {
        if Some(w) == main {
            continue;
        }
        if !unsafe { IsIconic(w) }.as_bool() {
            let _ = unsafe { ShowWindowAsync(w, SW_MINIMIZE) };
            n += 1;
        }
    }
    if let Some(w) = main {
        focus(w);
    }
    n
}

#[tauri::command]
pub fn clients_get(clients: tauri::State<Clients>) -> Snapshot {
    clients.snapshot(others(&clients))
}

/// Turns the economy mode of an account on or off, on its open clients at once and on the ones
/// it opens later.
#[tauri::command]
pub fn clients_set_eco(clients: tauri::State<Clients>, account: String, on: bool) -> Snapshot {
    {
        let mut eco = clients.eco.lock().unwrap();
        if on { eco.insert(account.clone()) } else { eco.remove(&account) };
    }
    {
        let mut list = clients.list.lock().unwrap();
        for c in list.iter_mut().filter(|c| c.account == account) {
            if c.eco_on != on {
                set_eco(c.proc.0, on);
                c.eco_on = on;
            }
        }
        clients.save(&list);
    }
    clients.snapshot(others(&clients))
}

#[tauri::command]
pub fn clients_focus(clients: tauri::State<Clients>, account: String) {
    if let Some(w) = clients.windows_of(&account).into_iter().next() {
        focus(w);
    }
}

#[tauri::command]
pub fn clients_minimize_others(app: AppHandle) -> usize {
    minimize_others(&app)
}

/// Lays every Roblox window out in a grid over the main client's monitor (taskbar left free),
/// in the order of the account cards. Says how many it placed.
#[tauri::command]
pub async fn clients_grid(app: AppHandle) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || grid(&app)).await.map_err(|e| e.to_string())?
}

fn grid(app: &AppHandle) -> Result<usize, String> {
    let windows = all_windows(app);
    if windows.is_empty() {
        return Err(tr!("nenhum Roblox aberto pra organizar", "no Roblox open to arrange"));
    }
    let anchor = principal(app).unwrap_or(windows[0]);
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    let _ = unsafe { GetMonitorInfoW(MonitorFromWindow(anchor, MONITOR_DEFAULTTOPRIMARY), &mut info) };
    let area = info.rcWork;
    // Restored first: a maximized or minimized window ignores the size it is given, and its
    // frame is measured only once it is a plain window.
    for &w in &windows {
        if unsafe { IsIconic(w) }.as_bool() || unsafe { IsZoomed(w) }.as_bool() {
            let _ = unsafe { ShowWindowAsync(w, SW_RESTORE) };
        }
    }
    std::thread::sleep(Duration::from_millis(300));
    let n = windows.len();
    let cols = (n as f64).sqrt().ceil() as i32;
    let rows = (n as i32 + cols - 1) / cols;
    let (area_w, area_h) = (area.right - area.left, area.bottom - area.top);
    let (w, h) = (area_w / cols, area_h / rows);
    let flags = SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS;
    // First the size of a cell. Roblox refuses to shrink below about 800x630 (1004x789 pixels
    // at 125%, measured 28/10/26), so from 3 windows up a cell may be smaller than the window.
    for &win in &windows {
        let (l, t, r, b) = frame(win);
        let _ = unsafe { SetWindowPos(win, None, area.left - l, area.top - t, w + l + r, h + t + b, flags | SWP_NOMOVE) };
    }
    std::thread::sleep(Duration::from_millis(300));
    // Then the place, from the size each one took: one that did not fit its cell is spread
    // evenly across the monitor instead, overlapping its neighbours and never past the edge.
    let spread = |cell: i32, size: i32, span: i32, index: i32, count: i32| {
        if size <= cell || count < 2 { index * cell } else { index * (span - size).max(0) / (count - 1) }
    };
    for (i, &win) in windows.iter().enumerate() {
        let (col, row) = (i as i32 % cols, i as i32 / cols);
        let (l, t, _, _) = frame(win);
        let mut visible = RECT::default();
        let _ = unsafe { DwmGetWindowAttribute(win, DWMWA_EXTENDED_FRAME_BOUNDS, &mut visible as *mut _ as _, std::mem::size_of::<RECT>() as u32) };
        let x = area.left + spread(w, visible.right - visible.left, area_w, col, cols);
        let y = area.top + spread(h, visible.bottom - visible.top, area_h, row, rows);
        // Raised one after another without taking the focus, so a window that covered the grid
        // goes behind it, and where two overlap the later one (its title bar and chat) is on top.
        let _ = unsafe { SetWindowPos(win, Some(HWND_TOP), x - l, y - t, 0, 0, SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS | SWP_NOSIZE) };
    }
    Ok(n)
}

/// The invisible resize border Windows 11 draws around a window (left, top, right, bottom): the
/// visible frame (DWM's bounds) is what goes edge to edge.
fn frame(win: HWND) -> (i32, i32, i32, i32) {
    let (mut outer, mut visible) = (RECT::default(), RECT::default());
    let _ = unsafe { GetWindowRect(win, &mut outer) };
    let measured = unsafe { DwmGetWindowAttribute(win, DWMWA_EXTENDED_FRAME_BOUNDS, &mut visible as *mut _ as _, std::mem::size_of::<RECT>() as u32) };
    if measured.is_ok() {
        (visible.left - outer.left, visible.top - outer.top, outer.right - visible.right, outer.bottom - visible.bottom)
    } else {
        (0, 0, 0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pid_of(exe: &str) -> Option<u32> {
        let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?;
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut more = unsafe { Process32FirstW(snap, &mut e) }.is_ok();
        let mut found = None;
        while more && found.is_none() {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            if String::from_utf16_lossy(&e.szExeFile[..len]).eq_ignore_ascii_case(exe) {
                found = Some(e.th32ProcessID);
            }
            more = unsafe { Process32NextW(snap, &mut e) }.is_ok();
        }
        let _ = unsafe { CloseHandle(snap) };
        found
    }

    #[test]
    fn exit_time_is_when_the_process_ended() {
        use std::os::windows::io::AsRawHandle;
        let before = now_filetime();
        let mut child = std::process::Command::new("cmd").args(["/c", "exit 0"]).spawn().unwrap();
        child.wait().unwrap();
        // Read a while after the end: the time must still be the end's, not the reading's.
        std::thread::sleep(Duration::from_millis(600));
        let ended = exit_time(HANDLE(child.as_raw_handle())).unwrap();
        let read_at = now_filetime();
        assert!(ended >= before, "ended before the start");
        assert!(read_at - ended >= 5_000_000, "exit time is the reading time, not the end");
    }

    #[test]
    fn running_reads_only_a_missing_process_as_gone() {
        assert!(running(std::process::id()));
        // Pids are multiples of 4: this one never exists.
        assert!(!running(0xFFFF_FFF1));
        // csrss.exe denies even a limited query: it must still count as running.
        let csrss = pid_of("csrss.exe").expect("csrss.exe runs on every Windows");
        assert!(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE, false, csrss) }.is_err());
        assert!(running(csrss));
    }
}
