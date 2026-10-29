// Carrying the accounts to another PC (phase 6). A saved login is sealed with DPAPI, which only this
// Windows user on this PC opens, so accounts.json is no use anywhere else. The export opens each
// login here, in Rust, and writes all of them, with the nicknames, groups, profiles, schedule,
// favorites and per-account switches, into one file locked by a password he types: the key comes
// from the password through Argon2id, the file is sealed with XChaCha20-Poly1305 (a wrong password
// or a changed byte opens nothing). The import seals each login again with this PC's DPAPI. The
// page sends only the password; the logins never go to it, to a log or anywhere else.
use argon2::Argon2;
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

const MAGIC: &[u8; 4] = b"QBX1";
const SALT: usize = 16;
const NONCE: usize = 24;
const PASSWORD_MIN: usize = 8;
const VERSION: u32 = 1;
/// A file of a few hundred accounts is well under this; a bigger one is not read at all.
const FILE_MAX: u64 = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Payload {
    version: u32,
    accounts: Vec<serde_json::Value>,
    #[serde(default)]
    lists: crate::organize::Lists,
    #[serde(default)]
    prefs: crate::clients::Prefs,
}

fn key_of(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    Argon2::default().hash_password_into(password.as_bytes(), salt, &mut key).map_err(|e| e.to_string())?;
    Ok(key)
}

/// From the operating system's random source.
fn random(n: usize) -> Vec<u8> {
    let mut out = vec![0u8; n];
    OsRng.fill_bytes(&mut out);
    out
}

fn seal(password: &str, plain: &[u8]) -> Result<Vec<u8>, String> {
    let salt = random(SALT);
    let nonce = random(NONCE);
    let cipher = XChaCha20Poly1305::new(&key_of(password, &salt)?.into());
    let sealed = cipher.encrypt(XNonce::from_slice(&nonce), plain).map_err(|_| tr!("não consegui trancar o arquivo", "couldn't lock the file"))?;
    Ok([MAGIC.as_slice(), &salt, &nonce, &sealed].concat())
}

fn open(password: &str, file: &[u8]) -> Result<Vec<u8>, String> {
    if file.len() < 4 + SALT + NONCE || &file[..4] != MAGIC {
        return Err(tr!("esse não é um arquivo de contas do Quackblox", "that isn't a Quackblox accounts file"));
    }
    let (salt, rest) = file[4..].split_at(SALT);
    let (nonce, sealed) = rest.split_at(NONCE);
    let cipher = XChaCha20Poly1305::new(&key_of(password, salt)?.into());
    cipher.decrypt(XNonce::from_slice(nonce), sealed).map_err(|_| tr!("senha errada, ou o arquivo foi mudado", "wrong password, or the file was changed"))
}

fn check_password(password: &str) -> Result<(), String> {
    if password.chars().count() < PASSWORD_MIN {
        return Err(tr!("a senha precisa de pelo menos {PASSWORD_MIN} letras", "the password needs at least {PASSWORD_MIN} characters"));
    }
    Ok(())
}

/// Writes every saved account (with its login) into a file locked by the password, where he picks.
/// Says where, or None when he closed the dialog.
#[tauri::command]
pub async fn transfer_export(app: AppHandle, password: String) -> Result<Option<String>, String> {
    check_password(&password)?;
    let accounts = crate::accounts::export_all(&app)?;
    if accounts.is_empty() {
        return Err(tr!("nenhuma conta salva pra levar", "no saved account to carry"));
    }
    let payload = Payload { version: VERSION, accounts, lists: crate::organize::organize_get(app.clone()), prefs: app.state::<crate::clients::Clients>().prefs() };
    let plain = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let dialog = app.dialog().clone();
    let day = crate::win::local_now().day;
    let path = tauri::async_runtime::spawn_blocking(move || {
        dialog
            .file()
            .set_title(tr!("Salvar as contas", "Save the accounts"))
            .set_file_name(format!("Quackblox {day}.quackblox"))
            .add_filter("Quackblox", &["quackblox"])
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?
    .and_then(|p| p.into_path().ok());
    let Some(path) = path else { return Ok(None) };
    // Argon2 takes a moment and memory: off the async runtime.
    let sealed = tauri::async_runtime::spawn_blocking(move || seal(&password, &plain)).await.map_err(|e| e.to_string())??;
    // Written beside and renamed over: a failed write never leaves an older file half overwritten.
    let tmp = path.with_extension("quackblox.tmp");
    std::fs::write(&tmp, sealed).and_then(|_| std::fs::rename(&tmp, &path)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        tr!("não consegui salvar o arquivo: {e}", "couldn't save the file: {e}")
    })?;
    Ok(Some(path.display().to_string()))
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    added: usize,
    updated: usize,
    groups: usize,
    profiles: usize,
    favorites: usize,
    slots: usize,
}

/// Reads a file another PC wrote, with its password: the accounts in it saved here (an account
/// already here gets the file's login), and the groups, profiles, favorites and schedule it has
/// that are not here yet. The schedule's times come turned off. None when he closed the dialog.
#[tauri::command]
pub async fn transfer_import(app: AppHandle, password: String) -> Result<Option<Imported>, String> {
    let dialog = app.dialog().clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        dialog.file().set_title(tr!("Abrir um arquivo de contas", "Open an accounts file")).add_filter("Quackblox", &["quackblox"]).blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?
    .and_then(|p| p.into_path().ok());
    let Some(path) = path else { return Ok(None) };
    let read_error = |e: std::io::Error| tr!("não consegui ler o arquivo: {e}", "couldn't read the file: {e}");
    if std::fs::metadata(&path).map_err(read_error)?.len() > FILE_MAX {
        return Err(tr!("esse não é um arquivo de contas do Quackblox", "that isn't a Quackblox accounts file"));
    }
    let file = std::fs::read(&path).map_err(read_error)?;
    let plain = tauri::async_runtime::spawn_blocking(move || open(&password, &file)).await.map_err(|e| e.to_string())??;
    let payload: Payload = serde_json::from_slice(&plain).map_err(|_| tr!("o arquivo está estragado", "the file is damaged"))?;
    if payload.version > VERSION {
        return Err(tr!("esse arquivo é de um Quackblox mais novo: atualiza este antes", "that file is from a newer Quackblox: update this one first"));
    }
    // Every account checked before any is saved: a file with one bad account changes nothing.
    if payload.accounts.iter().any(|a| a["id"].as_u64().is_none() || a["login"].as_str().is_none_or(str::is_empty)) {
        return Err(tr!("o arquivo tem uma conta incompleta: nada foi importado", "the file has an incomplete account: nothing was imported"));
    }
    let mut out = Imported::default();
    for a in &payload.accounts {
        match crate::accounts::import_one(&app, a) {
            Ok(true) => out.added += 1,
            Ok(false) => out.updated += 1,
            // A disk or DPAPI failure midway: the window still hears about the ones saved.
            Err(e) => {
                let _ = app.emit("accounts-changed", ());
                return Err(tr!(
                    "{e} ({} conta(s) importadas antes do erro; abre o arquivo de novo pra terminar)",
                    "{e} ({} account(s) imported before the error; open the file again to finish)",
                    out.added + out.updated
                ));
            }
        }
    }
    (out.groups, out.profiles, out.favorites, out.slots) = crate::organize::merge(&app, payload.lists).inspect_err(|_| {
        // The accounts are in already: the window shows them even when the lists failed.
        let _ = app.emit("accounts-changed", ());
    })?;
    app.state::<crate::clients::Clients>().merge_prefs(payload.prefs);
    let _ = app.emit("accounts-changed", ());
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_opens_only_with_its_password() {
        let file = seal("senha-certa", b"logins").unwrap();
        assert_eq!(open("senha-certa", &file).unwrap(), b"logins");
        assert!(open("senha-errada", &file).is_err());
        let mut changed = file.clone();
        *changed.last_mut().unwrap() ^= 1;
        assert!(open("senha-certa", &changed).is_err());
        assert!(open("senha-certa", b"QBX1").is_err());
        // The same text twice gives two different files (salt and nonce are new each time).
        assert_ne!(seal("senha-certa", b"logins").unwrap(), file);
    }
}
