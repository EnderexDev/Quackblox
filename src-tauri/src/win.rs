// The few Windows calls Quackblox needs: DPAPI (the user's own encryption, the same one the
// Roblox app uses for its saved login) and opening a roblox-player: link.
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows::Win32::UI::Shell::ShellExecuteW;
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

/// Hands a link to Windows, as a click on it would (roblox-player: opens the Roblox player).
pub fn open_link(url: &str) -> Result<(), String> {
    let url = HSTRING::from(url);
    let r = unsafe { ShellExecuteW(None, w!("open"), &url, PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
    // ShellExecute reports success as a value above 32.
    if r.0 as isize > 32 { Ok(()) } else { Err(format!("o Windows não abriu o link (código {})", r.0 as isize)) }
}
