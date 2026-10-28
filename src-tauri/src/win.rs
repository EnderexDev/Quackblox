// The few Windows calls Quackblox needs: DPAPI (the user's own encryption, the same one the
// Roblox app uses for its saved login) and opening a roblox-player: link.
use windows::Win32::Foundation::{HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

fn take(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let out = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec();
    unsafe { LocalFree(Some(HLOCAL(blob.pbData as _))) };
    out
}

/// Encrypts for the current Windows user: only this user on this PC can read it back.
pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe { CryptProtectData(&input, w!("Quackblox"), None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out) }
        .map_err(|e| format!("DPAPI: {e}"))?;
    Ok(take(out))
}

pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe { CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out) }
        .map_err(|e| format!("DPAPI: {e}"))?;
    Ok(take(out))
}

/// Hands a link to Windows, as a click on it would (roblox-player: opens the Roblox player), and
/// returns the process it started. Measured 28/10/26: the roblox-player: handler is the player
/// itself, so the process is the client, its parent this app. None when Windows started none
/// (a handler that hands the link to a process already running).
pub fn open_link(url: &str) -> Result<Option<HANDLE>, String> {
    let url = HSTRING::from(url);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("open"),
        lpFile: PCWSTR(url.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(|e| tr!("o Windows não abriu o link ({e})", "Windows didn't open the link ({e})"))?;
    Ok((!info.hProcess.is_invalid()).then_some(info.hProcess))
}
