// What a Roblox client wrote in its log (%LOCALAPPDATA%\Roblox\logs): the server it joined and
// why it left. The log has no pid; its file name carries the time the client started, in UTC
// ("0.745.0.7451137_20261028T201155Z_Player_B2076_last.log"), and the join line carries the
// account ("userid:3845602757"). Only those two facts are read out of it: the launch line holds
// the client's ticket, so no line is ever copied anywhere.
use std::path::PathBuf;
use std::sync::OnceLock;

use regex::Regex;

fn dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Roblox").join("logs"))
}

/// The UTC start in a log's file name, as Unix seconds.
fn started(name: &str) -> Option<u64> {
    static STAMP: OnceLock<Regex> = OnceLock::new();
    let c = STAMP.get_or_init(|| Regex::new(r"_(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})Z_Player_").unwrap()).captures(name)?;
    let n = |i: usize| c[i].parse::<i64>().ok();
    let (y, m, d) = (n(1)?, n(2)?, n(3)?);
    // Days from 1970-01-01 to the date (Howard Hinnant's days_from_civil).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    u64::try_from(days * 86400 + n(4)? * 3600 + n(5)? * 60 + n(6)?).ok()
}

/// The log of the client that started at this time (Unix seconds) as this account: the player
/// log that started closest to it, within 10 s, whose join line names the account when it
/// has one. None when Roblox wrote no log for it (a client that ended at once writes none).
/// `next`: when a later client of the same user started, if one is known; a log from then on is its.
pub fn of(user_id: Option<u64>, created: u64, next: Option<u64>) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(dir()?).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.contains("CrashHandler") {
            continue;
        }
        let Some(start) = started(&name) else { continue };
        // The name's time has whole seconds, the process's does not. Measured 28/10/26: a client
        // started at 21:38:30.7 opened its log at 21:38:34. The join line's account tells two
        // accounts apart; two clients of the same account opened under 10 s apart can still
        // swap logs (the Play button stays busy 4 s after a click).
        if start + 1 < created || start > created + 10 || next.is_some_and(|n| start >= n) {
            continue;
        }
        let gap = start.abs_diff(created);
        if best.as_ref().is_some_and(|(g, _)| *g <= gap) {
            continue;
        }
        let path = entry.path();
        if let Some(id) = user_id {
            let text = read(&path);
            let other = account_in(&text).is_some_and(|a| a != id);
            if other {
                continue;
            }
        }
        best = Some((gap, path));
    }
    best.map(|(_, p)| p)
}

fn read(path: &PathBuf) -> String {
    // A few hundred KB each (760 KB the largest, 28/10/26), read once per client that ends.
    std::fs::read(path).map(|b| String::from_utf8(b).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())).unwrap_or_default()
}

/// The account the client joined as.
fn account_in(text: &str) -> Option<u64> {
    static USER: OnceLock<Regex> = OnceLock::new();
    USER.get_or_init(|| Regex::new(r"game_join_loadtime: .*?userid:(\d+)").unwrap()).captures_iter(text).last()?[1].parse().ok()
}

/// The server the client joined last: its place and JobId.
pub fn server_in(path: &PathBuf) -> Option<(u64, String)> {
    static JOIN: OnceLock<Regex> = OnceLock::new();
    let text = read(path);
    let c = JOIN.get_or_init(|| Regex::new(r"Joining game '([0-9a-f-]{36})' place (\d+)").unwrap()).captures_iter(&text).last()?;
    Some((c[2].parse().ok()?, c[1].to_string()))
}

/// Why the client left its last server: Roblox's code (Enum.ConnectionError), as the error
/// screen shows it ("Error Code: 277"). 285 is the player leaving on purpose.
pub fn reason_in(path: &PathBuf) -> Option<u32> {
    reason_of(&read(path))
}

fn reason_re() -> &'static Regex {
    static REASON: OnceLock<Regex> = OnceLock::new();
    REASON.get_or_init(|| Regex::new(r"(?:Disconnected from server for reason: \w+: |Sending disconnect with reason: )(\d+)").unwrap())
}

fn reason_of(text: &str) -> Option<u32> {
    // Only after the last join: a teleport joins another server, and a reason written for the
    // one before it is not why the client ended. The first one after it is the cause: what the
    // client writes later (closing the error dialog) is not.
    let text = text.rfind("Joining game '").map_or(text, |at| &text[at..]);
    reason_re().captures(text)?[1].parse().ok()
}

/// What a running client's log said so far, read a piece at a time as Roblox appends to it.
pub struct Tail {
    path: PathBuf,
    /// Bytes read up to the end of the last whole line.
    offset: u64,
    /// The place of the last server joined, once the join finished (`game_join_loadtime`).
    pub joined: Option<u64>,
    /// The first reason written after the last join: why it left that server.
    pub reason: Option<u32>,
    /// The server cut it off ("Lost connection with reason"), after the last join. Every real drop
    /// measured wrote it (273, 276, 28/10/26) and no close did: a reason without it may be a
    /// teleport on its way to the next server.
    pub lost: bool,
}

impl Tail {
    pub fn new(path: PathBuf) -> Self {
        Tail { path, offset: 0, joined: None, reason: None, lost: false }
    }

    /// Reads what was appended since the last call. A line Roblox is still writing waits for the
    /// next call.
    pub fn read(&mut self) {
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut f) = std::fs::File::open(&self.path) else { return };
        if f.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut bytes = vec![];
        // At most 4 MB a look: a client restored after a restart reads its whole log once.
        if f.take(4 << 20).read_to_end(&mut bytes).is_err() {
            return;
        }
        let Some(end) = bytes.iter().rposition(|&b| b == b'\n') else { return };
        self.offset += end as u64 + 1;
        self.feed(&String::from_utf8_lossy(&bytes[..end]));
    }

    fn feed(&mut self, text: &str) {
        static PLACE: OnceLock<Regex> = OnceLock::new();
        let place = PLACE.get_or_init(|| Regex::new(r"game_join_loadtime: placeid:(\d+)").unwrap());
        for line in text.lines() {
            // A new server (the first, or a teleport's): whatever ended the one before is over.
            if line.contains("Joining game '") {
                self.joined = None;
                self.reason = None;
                self.lost = false;
            } else if let Some(c) = place.captures(line) {
                self.joined = c[1].parse().ok();
            } else if line.contains("Lost connection with reason") {
                self.lost = true;
            } else if self.reason.is_none() {
                if let Some(c) = reason_re().captures(line) {
                    self.reason = c[1].parse().ok();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_start_from_the_name() {
        // 2026-10-28T20:11:55Z
        assert_eq!(started("0.745.0.7451137_20261028T201155Z_Player_B2076_last.log"), Some(1_793_218_315));
        assert_eq!(started("0.745.0.7451137_20261028T152803Z_Studio_6AB47_last.log"), None);
    }

    #[test]
    fn reads_reason_server_and_account() {
        let text = "x [FLog::Output] ! Joining game '5cdd7bda-9174-447f-97f1-e4ef40c6d8c0' place 94717504417144 at 10.34.9.224\n\
            x [FLog::GameJoinLoadTime] Report game_join_loadtime: placeid:94717504417144, join_time:3.6, universeid:1, referral_page:RequestPrivateGame, sid:a, userid:3845602757, \n\
            x [FLog::Network] Sending disconnect with reason: 285\n\
            x Info [DFLog::RbxTransportDummyClient] Disconnected from server for reason: Player: 277 (DisconnectConnectionLost)\n";
        // The first reason after the join is why it left: what comes after is the dialog's close.
        assert_eq!(reason_of(text), Some(285));
        assert_eq!(reason_of(&text.replace("reason: 285", "reason: 273")), Some(273));
        assert_eq!(account_in(text), Some(3845602757));
        assert_eq!(reason_of("nothing here"), None);
        // Left one server (a teleport), joined another, then ended with no reason written.
        let teleported = "Sending disconnect with reason: 288
x ! Joining game '5cdd7bda-9174-447f-97f1-e4ef40c6d8c0' place 1 at a
";
        assert_eq!(reason_of(teleported), None);
    }

    #[test]
    fn tail_reads_only_what_was_added() {
        use std::io::Write;
        let path = std::env::temp_dir().join(format!("quackblox-tail-{}.log", std::process::id()));
        let mut f = std::fs::File::create(&path).unwrap();
        let mut tail = Tail::new(path.clone());
        writeln!(f, "x ! Joining game '5cdd7bda-9174-447f-97f1-e4ef40c6d8c0' place 1818 at a").unwrap();
        tail.read();
        assert_eq!((tail.joined, tail.reason), (None, None), "joining, not in yet");
        // Half a line: it waits for the rest.
        write!(f, "x Report game_join_loadtime: placeid:18").unwrap();
        f.flush().unwrap();
        tail.read();
        assert_eq!(tail.joined, None);
        writeln!(f, "18, join_time:1.2, userid:1, ").unwrap();
        tail.read();
        assert_eq!(tail.joined, Some(1818));
        writeln!(f, "x Warning [FLog::Network] Lost connection with reason : another device\nx Sending disconnect with reason: 276").unwrap();
        writeln!(f, "x Sending disconnect with reason: 285").unwrap();
        tail.read();
        assert_eq!(tail.reason, Some(276), "the first reason stays");
        assert!(tail.lost);
        // A teleport: the next server's join clears what ended the last one.
        writeln!(f, "x ! Joining game '11111111-9174-447f-97f1-e4ef40c6d8c0' place 2 at a\nx game_join_loadtime: placeid:2, userid:1").unwrap();
        tail.read();
        assert_eq!((tail.joined, tail.reason, tail.lost), (Some(2), None, false));
        drop(f);
        let _ = std::fs::remove_file(path);
    }

    /// On this PC's real logs: the private server join of 28/10/26 20:11:55Z, as Enderexx_1.
    #[test]
    #[ignore]
    fn finds_a_real_log() {
        let log = of(Some(3845602757), 1_793_218_314, None).expect("log");
        assert!(log.to_string_lossy().contains("20261028T201155Z"));
        assert_eq!(server_in(&log), Some((94717504417144, "5cdd7bda-9174-447f-97f1-e4ef40c6d8c0".into())));
        assert!(of(Some(87918968), 1_793_218_314, None).is_none(), "another account's log");
        assert!(of(Some(3845602757), 1_793_218_314 - 10, None).is_none(), "a launch 10 s earlier that wrote no log");
        assert_eq!(reason_in(&log), None, "killed, so it wrote no reason");
        // 19:14:53Z: a window closed by him.
        let closed = of(Some(3845602757), 1_793_214_893, None).expect("log");
        assert_eq!(reason_in(&closed), Some(285));
        // A client started at 21:38:30.7 whose log opened at 21:38:34, joined by JobId.
        let late = of(Some(3845602757), 1_793_223_510, None).expect("log opened 4 s after the process");
        assert_eq!(server_in(&late), Some((1818, "f612cf45-0dd5-4241-8e87-6ffde87d877c".into())));
        // A later client of the same user started at 21:38:33: the log at :34 is that one's.
        assert!(of(Some(3845602757), 1_793_223_510, Some(1_793_223_513)).is_none());
        // Read as it ran: a client knocked out at 21:55:14 (276), whose dialog stayed up.
        let dir = dir().unwrap();
        let mut dropped = Tail::new(dir.join("0.745.0.7451137_20261028T215505Z_Player_7ADF9_last.log"));
        dropped.read();
        assert_eq!((dropped.joined, dropped.reason, dropped.lost), (Some(1818), Some(276), true));
        let mut closed = Tail::new(dir.join("0.745.0.7451137_20261028T191420Z_Player_C5015_last.log"));
        closed.read();
        assert_eq!((closed.joined, closed.reason, closed.lost), (Some(1818), Some(285), false));
    }
}
