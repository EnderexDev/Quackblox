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
/// log that started closest after it, within a minute, whose join line names the account when it
/// has one. None when Roblox wrote no log for it (a client that ended at once writes none).
pub fn of(user_id: Option<u64>, created: u64) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(dir()?).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.contains("CrashHandler") {
            continue;
        }
        let Some(start) = started(&name) else { continue };
        // The name's time has whole seconds, the process's does not.
        if start + 2 < created || start > created + 60 {
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
    std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
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

fn reason_of(text: &str) -> Option<u32> {
    static REASON: OnceLock<Regex> = OnceLock::new();
    let re = REASON.get_or_init(|| Regex::new(r"(?:Disconnected from server for reason: \w+: |Sending disconnect with reason: )(\d+)").unwrap());
    re.captures_iter(text).last()?[1].parse().ok()
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
        assert_eq!(reason_of(text), Some(277));
        assert_eq!(account_in(text), Some(3845602757));
        assert_eq!(reason_of("nothing here"), None);
    }

    /// On this PC's real logs: the private server join of 28/10/26 20:11:55Z, as Enderexx_1.
    #[test]
    #[ignore]
    fn finds_a_real_log() {
        let log = of(Some(3845602757), 1_793_218_314).expect("log");
        assert!(log.to_string_lossy().contains("20261028T201155Z"));
        assert_eq!(server_in(&log), Some((94717504417144, "5cdd7bda-9174-447f-97f1-e4ef40c6d8c0".into())));
        assert!(of(Some(87918968), 1_793_218_314).is_none(), "another account's log");
        assert_eq!(reason_in(&log), None, "killed, so it wrote no reason");
        // 19:14:53Z: a window closed by him.
        let closed = of(Some(3845602757), 1_793_214_893).expect("log");
        assert_eq!(reason_in(&closed), Some(285));
    }
}
