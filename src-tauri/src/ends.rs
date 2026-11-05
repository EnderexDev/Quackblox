// Every Roblox process Quackblox ends, one line each in ends.log in the app's data folder, with
// the reason. The release build has no console: before this, a client that vanished could not
// be told apart from one Quackblox closed (05/11/26, a client gone at 15:28 with no crash and
// no record of who ended it).
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager};

static PATH: OnceLock<PathBuf> = OnceLock::new();
/// One writer at a time: the watcher, the tray sweep and the multi run on threads of their own.
static WRITE: Mutex<()> = Mutex::new(());
/// Past this the file starts over, the old one kept as ends.old.log.
const MAX_BYTES: u64 = 512 * 1024;

pub fn init(app: &AppHandle) {
    let dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    let _ = PATH.set(dir.join("ends.log"));
}

/// Writes one line: local time, the app's version, then `what`.
pub fn note(what: &str) {
    // A name with a line break would start a false line.
    let what = what.replace(['\r', '\n'], " ");
    eprintln!("ends: {what}");
    let Some(path) = PATH.get() else { return };
    let _guard = WRITE.lock().unwrap_or_else(|e| e.into_inner());
    if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::rename(path, path.with_file_name("ends.old.log"));
    }
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let line = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}  v{}  {what}\n",
        t.wYear,
        t.wMonth,
        t.wDay,
        t.wHour,
        t.wMinute,
        t.wSecond,
        env!("CARGO_PKG_VERSION")
    );
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn note_appends_a_dated_line() {
        let path = std::env::temp_dir().join(format!("quackblox-ends-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        super::PATH.set(path.clone()).unwrap();
        super::note("pid 1: um");
        super::note("pid 2: dois\r\nfalsa");
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with(&format!("  v{}  pid 1: um", env!("CARGO_PKG_VERSION"))), "{}", lines[0]);
        assert!(lines[1].starts_with("20") && lines[1].as_bytes()[4] == b'-', "{}", lines[1]);
        assert!(lines[1].ends_with("pid 2: dois  falsa"), "{}", lines[1]);
    }
}
