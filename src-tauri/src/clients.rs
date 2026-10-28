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
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, HWND, LPARAM, RECT, WAIT_OBJECT_0, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromWindow};
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, GetExitCodeProcess, GetPriorityClass, GetProcessId, GetProcessTimes, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, OpenProcess,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_VM_READ, ProcessPowerThrottling,
    SetPriorityClass, SetProcessInformation, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GW_OWNER, GetForegroundWindow, GetWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, IsZoomed,
    SMTO_ABORTIFHUNG, SW_MINIMIZE, SW_RESTORE, HWND_TOP, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageTimeoutW, SetForegroundWindow,
    SetWindowPos, ShowWindowAsync, WM_SETTEXT,
};
use windows::core::BOOL;

const PLAYER_EXE: &str = "RobloxPlayerBeta.exe";

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
    let Ok(snap) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else { return vec![] };
    let mut out = vec![];
    let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    let mut more = unsafe { Process32FirstW(snap, &mut e) }.is_ok();
    while more {
        let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
        if String::from_utf16_lossy(&e.szExeFile[..len]).eq_ignore_ascii_case(PLAYER_EXE) {
            out.push(e.th32ProcessID);
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
            links: list.iter().map(|c| Link { pid: c.pid, created: c.created, account: c.account.clone(), name: c.name.clone() }).collect(),
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
    pub fn add(&self, process: HANDLE, account: &str, name: &str) {
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
            Some(Client {
                pid: l.pid,
                created,
                account: l.account,
                name: l.name,
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
    let clients = Clients { path, list: Mutex::new(list), eco: Mutex::new(eco) };
    clients.save(&clients.list.lock().unwrap());
    app.manage(clients);

    let app = app.clone();
    std::thread::spawn(move || {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let clients = app.state::<Clients>();
            let mut exited = vec![];
            {
                let mut list = clients.list.lock().unwrap();
                let before = list.len();
                list.retain_mut(|c| {
                    let h = c.proc.0;
                    if let Some(code) = exit_code(h) {
                        exited.push(Exited { pid: c.pid, account: c.account.clone(), code, had_window: c.had_window });
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
            }
            for e in exited {
                let _ = app.emit("client-exited", e);
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

/// The main client: the Roblox app account's, or else the first open in card order.
fn principal(app: &AppHandle) -> Option<HWND> {
    let clients = app.state::<Clients>();
    crate::accounts::order(app).iter().find_map(|a| clients.windows_of(a).into_iter().next())
}

/// Focuses the client of the account on card N (1 is the Roblox app's card). Used by a hotkey.
pub fn focus_nth(app: &AppHandle, n: usize) {
    let order = crate::accounts::order(app);
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
