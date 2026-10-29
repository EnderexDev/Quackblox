// The phone page (phase 5): commands from the phone over the home network, and the live view of
// each account, look only. Off until he turns it on. A small HTTP server of its own on a fixed
// port, answering only addresses of the home network, Tailscale's (away from home, with no port
// opened on the router) and this PC; any other address is dropped before its request is read.
//
// Pairing: a random key, kept encrypted beside the settings (never in settings.json, which the
// window sends whole). The QR in the window carries the address with the key after '#', a part the
// phone's browser never sends to any server; the page keeps it and sends it with each request. A
// wrong key is refused with a growing wait. A new key unpairs every phone.
//
// Plain HTTP: on the home network the key crosses the Wi-Fi in clear text, inside Tailscale it is
// already encrypted. Nothing here ever carries a login, a cookie or a ticket: the page gets names,
// states, games and pictures, and every command goes through the same Rust a click goes through
// (launch.rs, clients.rs).
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::capture::{self, Shot};
use crate::clients::{Clients, State};

/// The installed app's port; the Dev's is the next one, so both can be on at once.
const PORT: u16 = if cfg!(debug_assertions) { 38251 } else { 38250 };
const PAGE: &str = include_str!("phone.html");
const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// Connections handled at once; more wait in the system's queue.
const MAX_OPEN: usize = 24;
/// Wrong keys in a row from one address before it waits.
const FREE_TRIES: u32 = 5;
/// The width of a live picture on the phone, and of each one in a print of all.
const LIVE_WIDTH: u32 = 640;
const PRINT_WIDTH: u32 = 1920;
const PRINT_ALL_WIDTH: u32 = 960;

/// Bumped on each start and stop: a server thread of an older one stops.
static GENERATION: AtomicU64 = AtomicU64::new(0);
static LISTENING: AtomicBool = AtomicBool::new(false);
static OPEN: AtomicUsize = AtomicUsize::new(0);
/// Why the server is not listening while it should, for the window.
static ERROR: Mutex<Option<String>> = Mutex::new(None);
static KEY: Mutex<Option<String>> = Mutex::new(None);
/// Wrong keys per address: how many in a row, and until when it is refused.
static FAILS: Mutex<Option<HashMap<IpAddr, (u32, Instant)>>> = Mutex::new(None);
/// The games the window shows as recent (it sends them when they change).
static RECENTS: Mutex<Vec<GameItem>> = Mutex::new(Vec::new());
/// Game names by what `play` takes, for the games of open clients.
static NAMES: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
/// Games whose name lookup failed, and when: asked again only after a while.
static FAILED: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
const NAME_RETRY: Duration = Duration::from_secs(60);
/// What the phone's commands did, newest last, for the page to show as it polls.
static NOTICES: Mutex<Vec<(u64, String)>> = Mutex::new(Vec::new());
static NOTICE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameItem {
    /// What `play` takes.
    pub game: String,
    pub name: String,
}

fn notice(text: String) {
    // The id is taken under the lock, so the list stays in id order for the page's `since`.
    let mut n = NOTICES.lock().unwrap();
    let id = NOTICE_ID.fetch_add(1, Ordering::Relaxed) + 1;
    n.push((id, text));
    let extra = n.len().saturating_sub(20);
    n.drain(..extra);
}

fn key_path(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from(".")).join("phone.key")
}

fn new_key() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

/// The pairing key, made the first time and kept encrypted for this Windows user.
fn key(app: &AppHandle) -> Result<String, String> {
    let mut k = KEY.lock().unwrap();
    if let Some(k) = k.as_ref() {
        return Ok(k.clone());
    }
    let path = key_path(app);
    let read = std::fs::read_to_string(&path).ok().and_then(|t| B64.decode(t.trim()).ok()).and_then(|b| crate::win::unprotect(&b).ok()).and_then(|b| String::from_utf8(b).ok());
    let key = match read {
        Some(key) if key.len() == 64 => key,
        _ => {
            let key = new_key();
            save_key(&path, &key)?;
            key
        }
    };
    *k = Some(key.clone());
    Ok(key)
}

fn save_key(path: &PathBuf, key: &str) -> Result<(), String> {
    let text = B64.encode(crate::win::protect(key.as_bytes())?);
    std::fs::write(path, text).map_err(|e| tr!("não consegui salvar a chave do celular: {e}", "couldn't save the phone key: {e}"))
}

/// Home network (10/8, 172.16/12, 192.168/16), Tailscale (100.64/10) or this PC. IPv6 is not
/// listened on at all.
fn allowed(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || tailscale(v4),
        IpAddr::V6(v6) => v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback() || v4.is_private() || tailscale(v4)),
    }
}

fn tailscale(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    a == 100 && (64..128).contains(&b)
}

/// Starts or stops the server as the settings say. Run at start and after each settings change.
pub fn apply(app: &AppHandle) {
    static APPLYING: Mutex<()> = Mutex::new(());
    let _one = APPLYING.lock().unwrap_or_else(|e| e.into_inner());
    let on = crate::settings::current().phone;
    if on == LISTENING.load(Ordering::SeqCst) {
        return;
    }
    // Stops the thread of the server before, if any.
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    LISTENING.store(false, Ordering::SeqCst);
    *ERROR.lock().unwrap() = None;
    if !on {
        return;
    }
    if let Err(e) = key(app) {
        *ERROR.lock().unwrap() = Some(e);
        return;
    }
    // The server before lets go of the port at its next look (100 ms): turned off and on at once,
    // the bind is tried again for a moment.
    let bind = || TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], PORT)));
    let mut tried = bind();
    for _ in 0..20 {
        if tried.is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
        tried = bind();
    }
    let listener = match tried {
        Ok(l) => l,
        Err(e) => {
            *ERROR.lock().unwrap() = Some(tr!(
                "a porta {PORT} está ocupada (outro Quackblox com o celular ligado?): {e}",
                "port {PORT} is taken (another Quackblox with the phone on?): {e}"
            ));
            return;
        }
    };
    // Polled, so a stop ends the thread and frees the port within a moment.
    let _ = listener.set_nonblocking(true);
    LISTENING.store(true, Ordering::SeqCst);
    let app = app.clone();
    std::thread::spawn(move || {
        while GENERATION.load(Ordering::SeqCst) == generation {
            // Full: the next connection waits in the system's queue instead of being dropped.
            if OPEN.load(Ordering::SeqCst) >= MAX_OPEN {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            match listener.accept() {
                Ok((stream, peer)) => {
                    if !allowed(peer.ip()) {
                        continue;
                    }
                    OPEN.fetch_add(1, Ordering::SeqCst);
                    let app = app.clone();
                    std::thread::spawn(move || {
                        let _ = stream.set_nonblocking(false);
                        handle(&app, stream, peer.ip(), generation);
                        OPEN.fetch_sub(1, Ordering::SeqCst);
                    });
                }
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    });
}

struct Request {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    // One deadline for the whole request, not per read: a client sending a byte at a time does not
    // hold a slot for ever.
    let deadline = Instant::now() + Duration::from_secs(10);
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    let read = |stream: &mut TcpStream, chunk: &mut [u8]| -> Option<usize> {
        let left = deadline.checked_duration_since(Instant::now()).filter(|d| !d.is_zero())?;
        stream.set_read_timeout(Some(left)).ok()?;
        stream.read(chunk).ok()
    };
    let mut buf = vec![];
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let n = read(stream, &mut chunk)?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > 16 * 1024 {
            return None;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let target = first.next()?.to_string();
    let headers: HashMap<String, String> = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string())).collect();
    let length: usize = headers.get("content-length").and_then(|l| l.parse().ok()).unwrap_or(0);
    if length > 64 * 1024 {
        return None;
    }
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < length {
        let n = read(stream, &mut chunk)?;
        if n == 0 {
            return None;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(length);
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.split('&').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), decode(v))).collect()),
        None => (target, HashMap::new()),
    };
    Some(Request { method, path, query, headers, body })
}

/// %XX and '+' in a query value.
fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = vec![];
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(if b == b'+' { b' ' } else { b });
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8], extra: &str) {
    // No cache anywhere: the answers are live states and pictures. No CORS header: a page of another
    // address cannot read anything here, and its requests with the key header are refused by the
    // browser before they are sent.
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n{extra}\r\n",
        body.len()
    );
    // Written in pieces against one deadline: a phone reading a byte at a time does not hold the
    // connection past it (the write timeout alone counts each piece apart).
    let deadline = Instant::now() + Duration::from_secs(15);
    for mut piece in [head.as_bytes(), body] {
        while !piece.is_empty() {
            let Some(left) = deadline.checked_duration_since(Instant::now()).filter(|d| !d.is_zero()) else { return };
            if stream.set_write_timeout(Some(left)).is_err() {
                return;
            }
            match stream.write(piece) {
                Ok(0) | Err(_) => return,
                Ok(n) => piece = &piece[n..],
            }
        }
    }
}

fn json_reply(stream: &mut TcpStream, status: &str, value: serde_json::Value) {
    respond(stream, status, "application/json; charset=utf-8", value.to_string().as_bytes(), "");
}

fn error(stream: &mut TcpStream, status: &str, text: String) {
    json_reply(stream, status, json!({ "error": text }));
}

/// Same time whatever the first wrong character: the key is not found one letter at a time.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Whether the request carries the key. A wrong one waits a second, and from the sixth in a row the
/// address is refused for a while that doubles each time (30 s up to an hour).
fn authorized(app: &AppHandle, req: &Request, ip: IpAddr) -> Result<(), &'static str> {
    {
        let fails = FAILS.lock().unwrap();
        if let Some((_, until)) = fails.as_ref().and_then(|f| f.get(&ip)) {
            if *until > Instant::now() {
                return Err("429 Too Many Requests");
            }
        }
    }
    let Ok(want) = key(app) else { return Err("503 Service Unavailable") };
    // A request with no key header is refused without counting: a page of another site open on the
    // phone can send those (it cannot set the header without a preflight this server never
    // answers), and must not lock the phone out.
    let Some(given) = req.headers.get("x-quackblox-key").map(String::as_str) else { return Err("401 Unauthorized") };
    if same(given, &want) {
        if let Some(f) = FAILS.lock().unwrap().as_mut() {
            f.remove(&ip);
        }
        return Ok(());
    }
    {
        let mut guard = FAILS.lock().unwrap();
        let fails = guard.get_or_insert_with(HashMap::new);
        // Addresses whose wait ended and that stopped trying are forgotten.
        let now = Instant::now();
        // Kept an hour past its wait, so the next wrong key doubles the wait instead of starting over.
        fails.retain(|_, (_, until)| *until > now || now.duration_since(*until) < Duration::from_secs(3600));
        let e = fails.entry(ip).or_insert((0, Instant::now()));
        e.0 += 1;
        if e.0 >= FREE_TRIES {
            let wait = 30u64 << (e.0 - FREE_TRIES).min(7);
            e.1 = Instant::now() + Duration::from_secs(wait.min(3600));
        }
    }
    std::thread::sleep(Duration::from_secs(1));
    Err("401 Unauthorized")
}

fn handle(app: &AppHandle, mut stream: TcpStream, ip: IpAddr, generation: u64) {
    let Some(req) = read_request(&mut stream) else { return };
    // Turned off while this request came in: nothing runs.
    if GENERATION.load(Ordering::SeqCst) != generation {
        return;
    }
    if req.method == "GET" && (req.path == "/" || req.path == "/index.html") {
        let page = PAGE.replace("/*TEXTS*/null", &texts().to_string()).replace("__LANG__", if crate::settings::english() { "en" } else { "pt" });
        // The page runs only its own script, and sends nothing to any other address.
        let csp = "Content-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src blob:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\r\n";
        return respond(&mut stream, "200 OK", "text/html; charset=utf-8", page.as_bytes(), csp);
    }
    if !req.path.starts_with("/api/") {
        return respond(&mut stream, "404 Not Found", "text/plain", b"", "");
    }
    if let Err(status) = authorized(app, &req, ip) {
        let text = if status.starts_with("429") {
            tr!("chave errada muitas vezes: espera um pouco", "wrong key too many times: wait a while")
        } else {
            tr!("esse celular não está pareado: escaneia o QR do Quackblox de novo", "this phone isn't paired: scan Quackblox's QR again")
        };
        return error(&mut stream, status, text);
    }
    let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap_or(serde_json::Value::Null);
    let s = |k: &str| body.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/api/state") => {
            let since: u64 = req.query.get("since").and_then(|s| s.parse().ok()).unwrap_or(0);
            json_reply(&mut stream, "200 OK", state(app, since));
        }
        ("GET", "/api/shot") => {
            let account = req.query.get("account").cloned().unwrap_or_default();
            let width = req.query.get("w").and_then(|w| w.parse().ok()).unwrap_or(LIVE_WIDTH).clamp(160, 1280);
            shot_reply(app, &mut stream, &account, width, 70, false);
        }
        ("GET", "/api/print") => match req.query.get("account") {
            Some(account) => shot_reply(app, &mut stream, account, PRINT_WIDTH, 90, true),
            None => match print_all(app) {
                Ok(bytes) => respond(&mut stream, "200 OK", "image/jpeg", &bytes, &disposition()),
                Err(e) => error(&mut stream, "409 Conflict", e),
            },
        },
        ("POST", "/api/play") => {
            let started = play(app, &s("kind"), &s("id"), &s("game"), body.get("sameServer").and_then(|v| v.as_bool()).unwrap_or(false));
            reply_started(&mut stream, started);
        }
        ("POST", "/api/close") => {
            let account = s("account");
            let left = app.state::<Clients>().end_account(&account);
            if left > 0 {
                return error(&mut stream, "409 Conflict", tr!("o Roblox não fechou", "Roblox didn't close"));
            }
            json_reply(&mut stream, "200 OK", json!({ "ok": tr!("{} fechada", "{} closed", name_of(app, &account)) }));
        }
        ("POST", "/api/rejoin") => {
            let account = s("account");
            let started = match app.state::<Clients>().last_game(&account) {
                Some(game) => reopen(app, account, game, None),
                None => Err(tr!("não sei em que jogo ela estava", "I don't know which game it was in")),
            };
            reply_started(&mut stream, started);
        }
        ("POST", "/api/switch") => {
            let started = reopen(app, s("account"), s("game"), None);
            reply_started(&mut stream, started);
        }
        ("POST", "/api/follow") => {
            let started = follow(app, s("account"), s("target"));
            reply_started(&mut stream, started);
        }
        ("POST", "/api/eco") => {
            let on = body.get("on").and_then(|v| v.as_bool()).unwrap_or(false);
            crate::clients::clients_set_eco(app.state::<Clients>(), s("account"), on);
            json_reply(&mut stream, "200 OK", json!({ "ok": "" }));
        }
        _ => respond(&mut stream, "404 Not Found", "text/plain", b"", ""),
    }
}

fn reply_started(stream: &mut TcpStream, started: Result<String, String>) {
    match started {
        Ok(text) => json_reply(stream, "200 OK", json!({ "ok": text })),
        Err(e) => error(stream, "409 Conflict", e),
    }
}

fn disposition() -> String {
    let now = crate::win::local_now();
    format!("Content-Disposition: inline; filename=\"Quackblox {} {}.jpg\"\r\n", now.day, now.clock.replace(':', "-"))
}

fn shot_reply(app: &AppHandle, stream: &mut TcpStream, account: &str, width: u32, quality: u8, print: bool) {
    let Some(hwnd) = app.state::<Clients>().window(account) else {
        return error(stream, "404 Not Found", tr!("essa conta não tem Roblox aberto", "that account has no Roblox open"));
    };
    match capture::jpeg(hwnd, width, quality) {
        Shot::Jpeg(bytes) => respond(stream, "200 OK", "image/jpeg", &bytes, &if print { disposition() } else { String::new() }),
        Shot::Minimized => json_reply(stream, "409 Conflict", json!({ "error": tr!("a janela está minimizada", "the window is minimized"), "minimized": true })),
        Shot::Gone => error(stream, "409 Conflict", tr!("não consegui ver essa janela", "couldn't see that window")),
    }
}

/// Every open client's window side by side, as a JPEG. Minimized ones are left out.
pub fn print_all(app: &AppHandle) -> Result<Vec<u8>, String> {
    let clients = app.state::<Clients>();
    let mut shots = vec![];
    for key in crate::accounts::order(app) {
        if let Some(hwnd) = clients.window(&key) {
            if let Ok(rgb) = capture::pixels(hwnd, PRINT_ALL_WIDTH) {
                shots.push(rgb);
            }
        }
    }
    let all = capture::side_by_side(&shots).ok_or_else(|| tr!("nenhuma janela do Roblox à mostra (minimizada não aparece)", "no Roblox window showing (a minimized one doesn't)"))?;
    capture::encode(&all, 88).ok_or_else(|| tr!("não consegui montar a imagem", "couldn't make the picture"))
}

fn name_of(app: &AppHandle, key: &str) -> String {
    if key == "app" {
        return crate::accounts::app_user_cached().map(|u| u.display_name).unwrap_or_else(|| "Roblox".into());
    }
    crate::accounts::saved(app, key).map(|(n, _)| n).unwrap_or_else(|| key.to_string())
}

/// A game's name: from the recents, the profiles, the names already looked up; else looked up in
/// the background and the game's link shown meanwhile.
fn game_name(app: &AppHandle, game: &str) -> Option<String> {
    known_name(app, game).or_else(|| {
        look_up_name(app, game);
        None
    })
}

/// A game's name from what is already known, asking no one.
fn known_name(app: &AppHandle, game: &str) -> Option<String> {
    if let Some(g) = RECENTS.lock().unwrap().iter().find(|g| g.game == game) {
        return Some(g.name.clone());
    }
    if let Some(p) = crate::organize::organize_get(app.clone()).profiles.iter().find(|p| crate::organize::game_arg(&p.game) == game) {
        return Some(p.game.name.clone());
    }
    NAMES.lock().unwrap().as_ref().and_then(|n| n.get(game)).filter(|n| !n.is_empty()).cloned()
}

/// Looks the game's name up in the background, once (and again a minute after a failure).
fn look_up_name(app: &AppHandle, game: &str) {
    let mut names = NAMES.lock().unwrap();
    let names = names.get_or_insert_with(HashMap::new);
    if names.contains_key(game) {
        return;
    }
    if FAILED.lock().unwrap().as_ref().and_then(|f| f.get(game)).is_some_and(|at| at.elapsed() < NAME_RETRY) {
        return;
    }
    // Marked as asked, so a page polling every few seconds asks Roblox once.
    names.insert(game.to_string(), String::new());
    let (app, game) = (app.clone(), game.to_string());
    tauri::async_runtime::spawn(async move {
        let found = crate::accounts::game_info(app, game.clone()).await;
        let mut names = NAMES.lock().unwrap();
        let names = names.get_or_insert_with(HashMap::new);
        match found {
            Ok(g) => names.insert(game, g.name),
            // Asked again at a look a minute from now, not at every poll of the page.
            Err(_) => {
                FAILED.lock().unwrap().get_or_insert_with(HashMap::new).insert(game.clone(), Instant::now());
                names.remove(&game)
            }
        };
    });
}

fn state_text(state: State) -> String {
    match state {
        State::Starting => tr!("entrando", "joining"),
        State::InGame => tr!("no jogo", "in game"),
        State::Dropped => tr!("caiu", "dropped"),
        State::JoinFailed => tr!("não entrou", "didn't get in"),
    }
}

fn state(app: &AppHandle, since: u64) -> serde_json::Value {
    let clients = app.state::<Clients>();
    let live = clients.live();
    let mut people: Vec<(String, String, String, bool)> = crate::accounts::saved_names(app);
    if let Some(u) = crate::accounts::app_user_cached() {
        people.push(("app".into(), u.display_name.clone(), tr!("app do Roblox", "Roblox app"), false));
    } else if live.iter().any(|c| c.account == "app") {
        people.push(("app".into(), "Roblox".into(), tr!("app do Roblox", "Roblox app"), false));
    }
    let mut lines = vec![];
    let mut accounts = vec![];
    let mut games: Vec<GameItem> = RECENTS.lock().unwrap().clone();
    for (key, name, handle, expired) in &people {
        let mine: Vec<_> = live.iter().filter(|c| &c.account == key).collect();
        let eco = clients.eco_on(key);
        let list: Vec<_> = mine
            .iter()
            .map(|c| {
                let gname = game_name(app, &c.game);
                if let Some(n) = &gname {
                    if !games.iter().any(|g| g.game == c.game) {
                        games.push(GameItem { game: c.game.clone(), name: n.clone() });
                    }
                }
                let minimized = c.window.is_some_and(|w| unsafe { windows::Win32::UI::WindowsAndMessaging::IsIconic(windows::Win32::Foundation::HWND(w as *mut _)) }.as_bool());
                json!({
                    "state": c.state,
                    "stateText": state_text(c.state),
                    "reason": c.reason,
                    "cpu": (c.cpu * 10.0).round() / 10.0,
                    "ramMb": c.ram / (1024 * 1024),
                    "window": c.window.is_some(),
                    "minimized": minimized,
                    "game": c.game,
                    "gameName": gname,
                })
            })
            .collect();
        let line = match mine.first() {
            None if *expired => tr!("{name}: login expirado", "{name}: login expired"),
            None => tr!("{name}: fechada", "{name}: closed"),
            Some(c) => {
                let mut l = format!("{name}: {}", state_text(c.state));
                if let Some(g) = game_name(app, &c.game) {
                    l += &tr!(" em {g}", " in {g}");
                }
                if let Some(r) = c.reason {
                    l += &tr!(" (código {r})", " (code {r})");
                }
                l += &format!(", CPU {:.0}%, {} MB", c.cpu, c.ram / (1024 * 1024));
                if eco {
                    l += ", Eco";
                }
                if mine.len() > 1 {
                    l += &tr!(", {} janelas", ", {} windows", mine.len());
                }
                l
            }
        };
        lines.push(line);
        accounts.push(json!({ "key": key, "name": name, "handle": handle, "expired": expired, "eco": eco, "clients": list }));
    }
    let lists = crate::organize::organize_get(app.clone());
    for p in &lists.profiles {
        let g = crate::organize::game_arg(&p.game);
        if !games.iter().any(|x| x.game == g) {
            games.push(GameItem { game: g, name: p.game.name.clone() });
        }
    }
    let open = live.len();
    let header = if crate::multi::active() {
        tr!("{open} Roblox aberto(s), multi ligado", "{open} Roblox open, multi on")
    } else {
        tr!("{open} Roblox aberto(s), multi desligado", "{open} Roblox open, multi off")
    };
    let notices: Vec<_> = NOTICES.lock().unwrap().iter().filter(|(id, _)| *id > since).map(|(id, text)| json!({ "id": id, "text": text })).collect();
    json!({
        "multi": crate::multi::active(),
        "text": format!("{header}\n{}", lines.join("\n")),
        "accounts": accounts,
        "groups": lists.groups.iter().map(|g| json!({ "id": g.id, "name": g.name, "count": g.accounts.len() })).collect::<Vec<_>>(),
        "profiles": lists.profiles.iter().map(|p| json!({ "id": p.id, "name": p.name, "game": p.game.name, "count": p.accounts.len() })).collect::<Vec<_>>(),
        "games": games,
        "notices": notices,
    })
}

/// Turns a game the page sent (one it was offered, or a link pasted) into what `play` takes and
/// its name. A pasted link is looked up first (a share link needs it).
async fn resolve(app: &AppHandle, game: &str) -> Result<(String, String), String> {
    let game = game.trim();
    if game.is_empty() {
        return Err(tr!("escolhe um jogo primeiro", "pick a game first"));
    }
    // A share link is not something `play` takes: it is always resolved, even with its name known.
    if crate::roblox::share_code(game).is_none() {
        if let Some(name) = known_name(app, game) {
            return Ok((game.to_string(), name));
        }
    }
    let g = crate::accounts::game_info(app.clone(), game.to_string()).await?;
    let arg = crate::organize::game_arg(&g);
    NAMES.lock().unwrap().get_or_insert_with(HashMap::new).insert(arg.clone(), g.name.clone());
    Ok((arg, g.name))
}

/// Runs a launch in the background and tells the page how it went.
fn spawn_run(app: &AppHandle, plan: crate::launch::Plan) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let game = plan.game_name.clone();
        match crate::launch::run(&app, plan).await {
            Ok(s) if s.failed.is_empty() && s.opened > 0 => notice(tr!("{} aberta(s) em {game}", "{} opened in {game}", s.opened)),
            Ok(s) if s.failed.is_empty() => notice(tr!("nada abriu (já estava abrindo?)", "nothing opened (already opening?)")),
            Ok(s) => notice(tr!("{} aberta(s); não abriu: {}", "{} opened; didn't open: {}", s.opened, s.failed.join("; "))),
            Err(e) => notice(e),
        }
    });
}

fn play(app: &AppHandle, kind: &str, id: &str, game: &str, same_server: bool) -> Result<String, String> {
    if kind == "profile" {
        let app = app.clone();
        let id = id.to_string();
        tauri::async_runtime::spawn(async move {
            match crate::organize::profile_run(app, id).await {
                Ok(s) if s.failed.is_empty() => notice(tr!("perfil aberto: {} conta(s)", "profile opened: {} account(s)", s.opened)),
                Ok(s) => notice(tr!("{} aberta(s); não abriu: {}", "{} opened; didn't open: {}", s.opened, s.failed.join("; "))),
                Err(e) => notice(e),
            }
        });
        return Ok(tr!("abrindo o perfil...", "opening the profile..."));
    }
    let accounts = match kind {
        "group" => crate::organize::organize_get(app.clone()).groups.into_iter().find(|g| g.id == id).map(|g| g.accounts).ok_or_else(|| tr!("esse grupo não existe mais", "that group no longer exists"))?,
        "account" => vec![id.to_string()],
        _ => return Err(tr!("escolhe quem abrir", "pick who to open")),
    };
    let (app2, game) = (app.clone(), game.to_string());
    tauri::async_runtime::spawn(async move {
        match resolve(&app2, &game).await {
            Ok((game, game_name)) => spawn_run(&app2, crate::launch::Plan { accounts, game, game_name, same_server, eco: None, grid: false }),
            Err(e) => notice(e),
        }
    });
    Ok(tr!("abrindo...", "opening..."))
}

/// Closes the account's clients and opens it again in `game` (the same one for a rejoin, a new one
/// for a switch, another account's server for a follow).
fn reopen(app: &AppHandle, account: String, game: String, name: Option<String>) -> Result<String, String> {
    if account.is_empty() {
        return Err(tr!("escolhe uma conta", "pick an account"));
    }
    let who = name_of(app, &account);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let (game, game_name) = match name {
            Some(n) => (game, n),
            None => match resolve(&app, &game).await {
                Ok(r) => r,
                Err(e) => return notice(e),
            },
        };
        let handle = app.clone();
        let key = account.clone();
        let left = tauri::async_runtime::spawn_blocking(move || handle.state::<Clients>().end_account(&key)).await.unwrap_or(1);
        if left > 0 {
            return notice(tr!("{who}: o Roblox não fechou, não abri outro", "{who}: Roblox didn't close, didn't open another"));
        }
        spawn_run(&app, crate::launch::Plan { accounts: vec![account], game, game_name, same_server: false, eco: None, grid: false });
    });
    Ok(tr!("fechando e abrindo de novo...", "closing and opening again..."))
}

/// Sends the account to the server another account is in: from the other's client log, or else
/// from its presence.
fn follow(app: &AppHandle, account: String, target: String) -> Result<String, String> {
    if account == target {
        return Err(tr!("escolhe outra conta", "pick another account"));
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let server: Option<(u64, String)> = match crate::accounts::client_server(app2.clone(), target.clone()) {
            Some(j) => Some(j.into()),
            None => crate::accounts::presence(app2.clone()).await.ok().and_then(|mut p| p.remove(&target)).and_then(|p| Some((p.place_id?, p.job_id?))),
        };
        let Some((place, job)) = server else {
            return notice(tr!("não sei em que servidor {} está", "I don't know which server {} is in", name_of(&app2, &target)));
        };
        let game = format!("https://www.roblox.com/games/start?placeId={place}&gameInstanceId={job}");
        // Only a name already known: otherwise `reopen` looks the server's game up, once.
        let name = app2.state::<Clients>().last_game(&target).and_then(|g| known_name(&app2, &g));
        let _ = reopen(&app2, account, game, name);
    });
    Ok(tr!("indo pro servidor...", "going to the server..."))
}

fn texts() -> serde_json::Value {
    json!({
        "title": "Quackblox",
        "notPaired": tr!("Esse celular não está pareado. No Quackblox do PC, abre Celular e escaneia o QR.", "This phone isn't paired. In Quackblox on the PC, open Phone and scan the QR."),
        "offline": tr!("Sem ligação com o PC. O Quackblox está aberto e com o celular ligado?", "No connection to the PC. Is Quackblox open with the phone on?"),
        "status": tr!("Status", "Status"),
        "open": tr!("Abrir", "Open"),
        "who": tr!("Quem", "Who"),
        "accounts": tr!("Contas", "Accounts"),
        "groups": tr!("Grupos", "Groups"),
        "profiles": tr!("Perfis", "Profiles"),
        "game": tr!("Jogo", "Game"),
        "pasteLink": tr!("Colar link ou ID...", "Paste a link or ID..."),
        "linkPlaceholder": tr!("link do jogo, do servidor ou ID", "game or server link, or ID"),
        "sameServer": tr!("Mesmo servidor", "Same server"),
        "profileGame": tr!("o jogo do perfil", "the profile's game"),
        "go": tr!("Abrir", "Open"),
        "live": tr!("Tela", "Screen"),
        "wall": tr!("Ver todas as telas", "See all screens"),
        "print": tr!("Print", "Print"),
        "printAll": tr!("Print de todas", "Print of all"),
        "close": tr!("Fechar", "Close"),
        "confirmClose": tr!("Fechar o Roblox de {{name}}?", "Close {{name}}'s Roblox?"),
        "rejoin": tr!("Entrar de novo", "Rejoin"),
        "confirmRejoin": tr!("Fechar e abrir {{name}} de novo no mesmo jogo?", "Close {{name}} and open it again in the same game?"),
        "switchGame": tr!("Trocar de jogo", "Switch game"),
        "confirmSwitch": tr!("Fechar {{name}} e abrir em {{game}}?", "Close {{name}} and open it in {{game}}?"),
        "follow": tr!("Mandar pro servidor de", "Send to the server of"),
        "confirmFollow": tr!("Fechar {{name}} e mandar pro servidor de {{target}}?", "Close {{name}} and send it to {{target}}'s server?"),
        "eco": tr!("Eco", "Eco"),
        "closed": tr!("fechada", "closed"),
        "expired": tr!("login expirado", "login expired"),
        "minimized": tr!("minimizada: a janela não desenha enquanto está minimizada", "minimized: the window doesn't draw while minimized"),
        "noWindow": tr!("sem janela ainda", "no window yet"),
        "back": tr!("Voltar", "Back"),
        "saveHint": tr!("Segura a imagem pra salvar.", "Press and hold the picture to save it."),
        "pick": tr!("escolhe...", "pick..."),
        "lookOnly": tr!("Só olhar: nada daqui mexe na tela do PC.", "Look only: nothing here touches the PC's screen."),
        "multiOff": tr!("o multi está desligado: só uma conta por vez", "the multi is off: one account at a time"),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    /// "lan" or "tailscale".
    kind: &'static str,
    url: String,
    /// The QR of the url, as an SVG.
    qr: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    on: bool,
    listening: bool,
    port: u16,
    error: Option<String>,
    links: Vec<Link>,
}

/// The PC's IPv4 addresses: the home network's (the one its default route leaves by first) and
/// Tailscale's.
fn addresses() -> (Option<Ipv4Addr>, Option<Ipv4Addr>) {
    use windows::Win32::NetworkManagement::IpHelper::{GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH};
    use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};
    let mut all = vec![];
    let mut size: u32 = 32 * 1024;
    for _ in 0..3 {
        // u64s: the list is read as structs that need their alignment.
        let mut buf = vec![0u64; (size as usize).div_ceil(8)];
        let first = buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
        let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
        let r = unsafe { GetAdaptersAddresses(AF_INET.0 as u32, flags, None, Some(first), &mut size) };
        if r == 111 {
            continue; // ERROR_BUFFER_OVERFLOW: size now holds what it needs.
        }
        if r != 0 {
            break;
        }
        let mut a = first as *const IP_ADAPTER_ADDRESSES_LH;
        while !a.is_null() {
            let adapter = unsafe { &*a };
            let mut u = adapter.FirstUnicastAddress;
            while !u.is_null() {
                let unicast = unsafe { &*u };
                let sa = unicast.Address.lpSockaddr;
                if !sa.is_null() && unsafe { (*sa).sa_family } == AF_INET {
                    let b = unsafe { (*(sa as *const SOCKADDR_IN)).sin_addr.S_un.S_un_b };
                    all.push(Ipv4Addr::new(b.s_b1, b.s_b2, b.s_b3, b.s_b4));
                }
                u = unicast.Next;
            }
            a = adapter.Next;
        }
        break;
    }
    // The address the system would leave by for the internet: the home network's, not a virtual
    // switch's. Connecting a UDP socket sends nothing.
    let routed = std::net::UdpSocket::bind("0.0.0.0:0").and_then(|s| s.connect("1.1.1.1:80").and_then(|_| s.local_addr())).ok().and_then(|a| match a.ip() {
        IpAddr::V4(v4) => Some(v4),
        _ => None,
    });
    let lan = routed.filter(|ip| ip.is_private()).or_else(|| all.iter().copied().find(|ip| ip.is_private()));
    let ts = all.iter().copied().find(|ip| tailscale(*ip));
    (lan, ts)
}

fn qr_svg(text: &str) -> String {
    use qrcodegen::{QrCode, QrCodeEcc};
    let Ok(qr) = QrCode::encode_text(text, QrCodeEcc::Medium) else { return String::new() };
    let border = 2;
    let size = qr.size() + border * 2;
    let mut path = String::new();
    for y in 0..qr.size() {
        for x in 0..qr.size() {
            if qr.get_module(x, y) {
                path += &format!("M{},{}h1v1h-1z", x + border, y + border);
            }
        }
    }
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {size} {size}" shape-rendering="crispEdges"><rect width="100%" height="100%" fill="#fff"/><path d="{path}" fill="#000"/></svg>"##)
}

fn info(app: &AppHandle) -> Info {
    let on = crate::settings::current().phone;
    let error = ERROR.lock().unwrap().clone();
    let listening = LISTENING.load(Ordering::SeqCst);
    let mut links = vec![];
    if listening {
        if let Ok(key) = key(app) {
            let (lan, ts) = addresses();
            for (kind, ip) in [("lan", lan), ("tailscale", ts)] {
                if let Some(ip) = ip {
                    let url = format!("http://{ip}:{PORT}/#{key}");
                    links.push(Link { kind, qr: qr_svg(&url), url });
                }
            }
        }
    }
    Info { on, listening, port: PORT, error, links }
}

#[tauri::command]
pub fn phone_info(app: AppHandle) -> Info {
    info(&app)
}

/// A new key: every phone paired with the old one stops working until it scans the new QR.
#[tauri::command]
pub fn phone_new_key(app: AppHandle) -> Result<Info, String> {
    let key = new_key();
    save_key(&key_path(&app), &key)?;
    *KEY.lock().unwrap() = Some(key);
    *FAILS.lock().unwrap() = None;
    Ok(info(&app))
}

/// The games the window shows as recent, offered on the phone too.
#[tauri::command]
pub fn phone_games(games: Vec<GameItem>) {
    *RECENTS.lock().unwrap() = games;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_home_tailscale_and_this_pc() {
        let ok = |s: &str| allowed(s.parse().unwrap());
        assert!(ok("127.0.0.1"));
        assert!(ok("192.168.0.20"));
        assert!(ok("10.1.2.3"));
        assert!(ok("172.20.0.5"));
        assert!(ok("100.101.102.103"));
        assert!(ok("::1"));
        assert!(!ok("100.128.0.1"));
        assert!(!ok("100.63.255.255"));
        assert!(!ok("8.8.8.8"));
        assert!(!ok("172.32.0.1"));
        assert!(!ok("2001:db8::1"));
        assert!(!ok("fe80::1"));
    }

    #[test]
    fn key_compare() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "abcd"));
        assert!(!same("", "a"));
    }

    #[test]
    fn query_values_decode() {
        assert_eq!(decode("a%20b+c"), "a b c");
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz"), "%zz");
    }
}
