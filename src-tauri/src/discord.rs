// Discord Rich Presence (phase 5): "Playing Quackblox" on his Discord profile with the game the
// accounts are in and how many are in it. Off until he turns it on. Discord's desktop app is spoken
// to over its local pipe (\\.\pipe\discord-ipc-N), the same way games do it: nothing goes out to
// the network from here. Everyone who sees his profile sees it, so it never names an account: only
// the game and a count.
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::clients::{Clients, State};

/// Quackblox's application in Discord's developer portal: its name is what the profile shows.
const CLIENT_ID: &str = "";
/// A change shows within this; the game names are looked up once each.
const EVERY: Duration = Duration::from_secs(5);
const RESEND: Duration = Duration::from_secs(60);
/// Shown beside the game's picture.
const ICON: &str = "https://raw.githubusercontent.com/EnderexDev/Quackblox/master/src-tauri/icons/128x128.png";

pub fn available() -> bool {
    !CLIENT_ID.is_empty()
}

struct Pipe(File);

impl Pipe {
    fn open() -> Option<Pipe> {
        (0..10).find_map(|i| std::fs::OpenOptions::new().read(true).write(true).open(format!(r"\\?\pipe\discord-ipc-{i}")).ok()).map(Pipe)
    }

    fn send(&mut self, op: u32, body: &serde_json::Value) -> std::io::Result<serde_json::Value> {
        let data = body.to_string();
        let mut frame = Vec::with_capacity(8 + data.len());
        frame.extend_from_slice(&op.to_le_bytes());
        frame.extend_from_slice(&(data.len() as u32).to_le_bytes());
        frame.extend_from_slice(data.as_bytes());
        self.0.write_all(&frame)?;
        loop {
            let (op, reply) = self.read_frame()?;
            match op {
                // 2 is Discord closing the pipe, with the reason.
                2 => return Err(std::io::Error::other(String::from_utf8_lossy(&reply).to_string())),
                // 3 is a ping: answered with a pong (4) carrying the same data; the answer is still
                // to come.
                3 => self.write_frame(4, &reply)?,
                _ => return Ok(serde_json::from_slice(&reply).unwrap_or(serde_json::Value::Null)),
            }
        }
    }

    fn read_frame(&mut self) -> std::io::Result<(u32, Vec<u8>)> {
        let mut head = [0u8; 8];
        self.0.read_exact(&mut head)?;
        let len = u32::from_le_bytes(head[4..8].try_into().unwrap()) as usize;
        if len > 1 << 20 {
            return Err(std::io::Error::other("frame too big"));
        }
        let mut data = vec![0u8; len];
        self.0.read_exact(&mut data)?;
        Ok((u32::from_le_bytes(head[0..4].try_into().unwrap()), data))
    }

    fn write_frame(&mut self, op: u32, data: &[u8]) -> std::io::Result<()> {
        let mut frame = Vec::with_capacity(8 + data.len());
        frame.extend_from_slice(&op.to_le_bytes());
        frame.extend_from_slice(&(data.len() as u32).to_le_bytes());
        frame.extend_from_slice(data);
        self.0.write_all(&frame)
    }

    /// `send` with a time limit: a Discord that stopped answering never holds the presence thread.
    /// The pipe comes back only when it answered; a stuck one stays with its own thread, which lets
    /// it go when Discord closes it.
    fn send_within(mut self, op: u32, body: serde_json::Value) -> Option<(Pipe, std::io::Result<serde_json::Value>)> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let r = self.send(op, &body);
            let _ = tx.send((self, r));
        });
        rx.recv_timeout(Duration::from_secs(5)).ok()
    }

    fn handshake() -> Option<Pipe> {
        let (p, r) = Pipe::open()?.send_within(0, json!({ "v": 1, "client_id": CLIENT_ID }))?;
        r.ok()?;
        Some(p)
    }

    /// Sets the activity; the pipe comes back when it worked.
    fn activity(self, activity: serde_json::Value) -> std::io::Result<Pipe> {
        let nonce = uuid::Uuid::new_v4().to_string();
        let body = json!({ "cmd": "SET_ACTIVITY", "args": { "pid": std::process::id(), "activity": activity }, "nonce": nonce });
        let (p, reply) = self.send_within(1, body).ok_or_else(|| std::io::Error::other("no answer from Discord in 5 s"))?;
        let reply = reply?;
        if reply.get("evt").and_then(|e| e.as_str()) == Some("ERROR") {
            return Err(std::io::Error::other(reply.to_string()));
        }
        Ok(p)
    }
}

/// What the profile shows now: the game (or how many games) and how many accounts are in one.
/// Null while no account is in a game.
fn now_playing(app: &AppHandle, names: &mut HashMap<String, (String, Option<String>)>, failed: &mut HashMap<String, std::time::Instant>, since: u64) -> serde_json::Value {
    let live = app.state::<Clients>().live();
    let playing: Vec<_> = live.iter().filter(|c| c.state == State::InGame).collect();
    if playing.is_empty() {
        return serde_json::Value::Null;
    }
    let mut games: Vec<(String, Option<String>)> = vec![];
    let mut places = vec![];
    for c in &playing {
        // A failed lookup is not kept: the game is asked again a minute later.
        if !names.contains_key(&c.game) && failed.get(&c.game).is_none_or(|at| at.elapsed() >= Duration::from_secs(60)) {
            match tauri::async_runtime::block_on(crate::accounts::game_info(app.clone(), c.game.clone())) {
                Ok(g) => {
                    names.insert(c.game.clone(), (g.name, g.icon));
                }
                Err(_) => {
                    failed.insert(c.game.clone(), std::time::Instant::now());
                }
            }
        }
        let Some(entry) = names.get(&c.game) else { continue };
        // One game per place, whatever server each account is in; two games of the same name still
        // count as two.
        let place = crate::roblox::parse_game(&c.game).map(|p| p.place_id);
        if !entry.0.is_empty() && !places.contains(&place) {
            places.push(place);
            games.push(entry.clone());
        }
    }
    // Accounts, not windows: one account with two clients in game counts once.
    let n = playing.iter().map(|c| &c.account).collect::<std::collections::HashSet<_>>().len();
    let details = match games.as_slice() {
        [(name, _)] => name.clone(),
        [] => "Roblox".into(),
        many => tr!("{} jogos", "{} games", many.len()),
    };
    let state = if n == 1 { tr!("1 conta no jogo", "1 account in game") } else { tr!("{n} contas no jogo", "{n} accounts in game") };
    let mut assets = json!({ "small_image": ICON, "small_text": "Quackblox" });
    if let [(name, Some(icon))] = games.as_slice() {
        assets["large_image"] = json!(icon);
        assets["large_text"] = json!(name);
    } else {
        assets = json!({ "large_image": ICON, "large_text": "Quackblox" });
    }
    json!({ "type": 0, "details": details, "state": state, "timestamps": { "start": since }, "assets": assets })
}

/// The thread that keeps the profile up to date while the setting is on, and clears it when it is
/// turned off or nothing plays.
pub fn start(app: AppHandle) {
    if !available() {
        return;
    }
    std::thread::spawn(move || {
        let mut pipe: Option<Pipe> = None;
        let mut names = HashMap::new();
        let mut failed = HashMap::new();
        let mut sent = std::time::Instant::now();
        let mut shown = serde_json::Value::Null;
        let mut since = 0;
        loop {
            let on = crate::settings::current().rich_presence;
            let mut want = if on { now_playing(&app, &mut names, &mut failed, since) } else { serde_json::Value::Null };
            if want.is_null() {
                since = 0;
            } else if since == 0 {
                // Time in game counts from the first look that found an account in one.
                since = crate::win::unix_now();
                want = now_playing(&app, &mut names, &mut failed, since);
            }
            // Sent again now and then even unchanged: a Discord restarted meanwhile has lost it, and
            // the failed send is how a closed pipe is noticed.
            let stale = !want.is_null() && sent.elapsed() >= RESEND;
            if want != shown || stale {
                if pipe.is_none() && !want.is_null() {
                    pipe = Pipe::handshake();
                }
                match pipe.take().map(|p| p.activity(want.clone())) {
                    Some(Ok(p)) => {
                        pipe = Some(p);
                        shown = want.clone();
                        sent = std::time::Instant::now();
                    }
                    Some(Err(e)) => eprintln!("discord: presence not set: {e}"),
                    // Discord closed: nothing shows there anyway.
                    None => shown = serde_json::Value::Null,
                }
            }
            // Cleared: the pipe is let go, so Discord forgets Quackblox at once.
            if want.is_null() && shown.is_null() {
                pipe = None;
            }
            std::thread::sleep(EVERY);
        }
    });
}
