// Quackblox's own installer window, adapted from Quackify's (its branch t3/quackify-self-update,
// setup.rs), install only. The NSIS installer Tauri builds still does the installing (files,
// registry, uninstaller, shortcuts), run silently under a window in Quackblox's look.
// Quackblox-Setup.exe is quackblox.exe with the NSIS installer appended and a trailer saying how
// long it is (scripts/build-setup.mjs): finding it in its own file, the exe opens the installer
// window instead of the app.
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "setup";
/// The end of Quackblox-Setup.exe: the installer's length (u64, little endian), then this.
const MAGIC: &[u8; 8] = b"QBSETUP1";
const NAME: &str = "Quackblox";
/// The NSIS script's binary (the crate's name).
const BINARY: &str = "quackblox.exe";

/// Where the installer sits inside this exe.
pub struct Payload {
    offset: u64,
    len: u64,
}

/// Some when this exe is Quackblox-Setup.exe, read before anything else starts.
pub fn payload() -> Option<Payload> {
    let exe = std::env::current_exe().ok()?;
    let mut file = File::open(exe).ok()?;
    let size = file.metadata().ok()?.len();
    if size < 16 {
        return None;
    }
    let mut tail = [0u8; 16];
    file.seek(SeekFrom::End(-16)).ok()?;
    file.read_exact(&mut tail).ok()?;
    if &tail[8..] != MAGIC {
        return None;
    }
    let len = u64::from_le_bytes(tail[..8].try_into().ok()?);
    // size >= 16 here, so the subtraction cannot wrap whatever the trailer claims.
    (len > 0 && len <= size - 16).then(|| Payload { offset: size - 16 - len, len })
}

/// Runs the installer window in place of the app: no tray, multi or single instance.
pub fn run(mut context: tauri::Context<tauri::Wry>, payload: Payload) {
    context.config_mut().app.windows.clear();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(payload)
        .setup(|app| {
            // Its own WebView2 data: the install may run while Quackblox is open.
            let data = std::env::temp_dir().join("Quackblox-Setup-WebView");
            WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
                .title(NAME)
                .inner_size(560.0, 440.0)
                .resizable(false)
                .maximizable(false)
                .decorations(false)
                .center()
                .background_color(tauri::window::Color(10, 10, 12, 255))
                .data_directory(data)
                .build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![setup_info, setup_installed, setup_pick_folder, setup_install, setup_launch, setup_close])
        .run(context)
        .expect("error while running the Quackblox installer");
}

#[derive(Serialize)]
pub struct Info {
    version: String,
    /// Where it installs: the folder of an earlier install, or the NSIS default.
    dir: String,
    /// Quackblox is already there (the button says Atualizar instead of Instalar).
    installed: bool,
}

#[tauri::command]
fn setup_info(app: AppHandle) -> Info {
    let dir = default_dir();
    Info { version: app.package_info().version.to_string(), installed: dir.join(BINARY).exists(), dir: dir.display().to_string() }
}

/// The folder of the last install (the key NSIS writes), or %LOCALAPPDATA%\Quackblox.
fn default_dir() -> PathBuf {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(format!("Software\\enderex\\{NAME}")) {
        if let Ok(dir) = key.get_value::<String, _>("") {
            if !dir.is_empty() {
                return PathBuf::from(dir);
            }
        }
    }
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    local.join(NAME)
}

#[tauri::command]
fn setup_installed(dir: String) -> bool {
    Path::new(&dir).join(BINARY).exists()
}

#[tauri::command]
async fn setup_pick_folder(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let dialog = app.dialog().clone();
    tauri::async_runtime::spawn_blocking(move || dialog.file().set_title("Onde instalar o Quackblox").blocking_pick_folder())
        .await
        .ok()
        .flatten()
        .and_then(|p| p.into_path().ok())
        // NSIS puts the files straight in the folder given: a Quackblox folder inside the one picked.
        .map(|p| if p.ends_with(NAME) { p } else { p.join(NAME) })
        .map(|p| p.display().to_string())
}

/// Installs from the installer inside this exe. `desktop`: keep the desktop shortcut NSIS makes.
#[tauri::command]
async fn setup_install(app: AppHandle, dir: String, desktop: bool) -> Result<(), String> {
    let (offset, len) = {
        let p = app.state::<Payload>();
        (p.offset, p.len)
    };
    let shortcut = desktop_shortcut();
    tauri::async_runtime::spawn_blocking(move || {
        let installer = std::env::temp_dir().join(format!("Quackblox-Install-{}.exe", std::process::id()));
        let had_shortcut = shortcut.as_ref().is_some_and(|p| p.exists());
        let bytes = extract(offset, len)?;
        install_from(&installer, &bytes, Path::new(&dir))?;
        // Silent NSIS always makes the desktop shortcut; /NS would skip the Start menu's too.
        if !desktop && !had_shortcut {
            if let Some(p) = shortcut {
                let _ = std::fs::remove_file(p);
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn extract(offset: u64, len: u64) -> Result<Vec<u8>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut from = File::open(exe).map_err(|e| e.to_string())?;
    from.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
    let mut bytes = Vec::with_capacity(len as usize);
    from.take(len).read_to_end(&mut bytes).map_err(|e| format!("não consegui ler o instalador: {e}"))?;
    Ok(bytes)
}

/// Writes `bytes` to `path` and runs them as NSIS, the file held so nothing can change or delete
/// it in between: reopened sharing only reading (Windows still runs it), then checked against `bytes`.
fn install_from(path: &Path, bytes: &[u8], dir: &Path) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("não consegui gravar o instalador: {e}"))?;
    let result = (|| {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ only: no writing or deleting while this handle lives.
        let mut held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0x1)
            .open(path)
            .map_err(|e| format!("não consegui abrir o instalador: {e}"))?;
        let mut on_disk = Vec::with_capacity(bytes.len());
        held.read_to_end(&mut on_disk).map_err(|e| format!("não consegui ler o instalador: {e}"))?;
        if on_disk != bytes {
            return Err("o instalador mudou no disco antes de rodar".to_string());
        }
        run_nsis(path, dir)
    })();
    let _ = std::fs::remove_file(path);
    result
}

/// Runs NSIS silently into `dir` and waits for it.
fn run_nsis(installer: &Path, dir: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // /D= has to be the last argument and unquoted, spaces and all: NSIS reads the rest of the
    // line as the folder. raw_arg keeps Rust from quoting it.
    let status = Command::new(installer)
        .arg("/S")
        .raw_arg(format!("/D={}", dir.display()))
        .status()
        .map_err(|e| format!("não consegui iniciar o instalador: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("o instalador parou com o código {}", status.code().unwrap_or(-1)))
    }
}

fn desktop_shortcut() -> Option<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Desktop, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
    unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None).ok()?;
        let desktop = path.to_string().ok();
        CoTaskMemFree(Some(path.0 as _));
        desktop.map(|d| PathBuf::from(d).join(format!("{NAME}.lnk")))
    }
}

/// Opens the Quackblox installed in `dir` and closes this window.
#[tauri::command]
fn setup_launch(app: AppHandle, dir: String) -> Result<(), String> {
    Command::new(Path::new(&dir).join(BINARY))
        .spawn()
        .map_err(|e| format!("não consegui abrir o Quackblox: {e}"))?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn setup_close(app: AppHandle) {
    app.exit(0);
}
