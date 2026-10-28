// The account logged in to the Roblox app on this PC. The app keeps its login in
// %LOCALAPPDATA%\Roblox\LocalStorage\RobloxCookies.dat: JSON whose CookiesData is base64 of
// DPAPI-encrypted cookie lines (tab-separated, joined by "; "). Read on demand, never stored.
use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

use base64::Engine;

#[derive(Deserialize)]
struct CookieFile {
    #[serde(rename = "CookiesData")]
    cookies_data: String,
}

/// The app's login, or why there is none.
pub fn cookie() -> Result<String, String> {
    let base = std::env::var("LOCALAPPDATA").map_err(|_| tr!("LOCALAPPDATA não existe", "LOCALAPPDATA is not set"))?;
    let path = std::path::Path::new(&base).join("Roblox").join("LocalStorage").join("RobloxCookies.dat");
    let text = std::fs::read_to_string(&path).map_err(|_| tr!("o Roblox não está instalado ou nunca foi aberto", "Roblox isn't installed or was never opened"))?;
    let file: CookieFile = serde_json::from_str(&text).map_err(|_| tr!("o arquivo de login do Roblox mudou de formato", "Roblox's login file changed format"))?;
    let encrypted = base64::engine::general_purpose::STANDARD
        .decode(file.cookies_data.trim())
        .map_err(|_| tr!("o arquivo de login do Roblox mudou de formato", "Roblox's login file changed format"))?;
    let plain = String::from_utf8_lossy(&crate::win::unprotect(&encrypted)?).into_owned();
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\.ROBLOSECURITY\t([^;\s]+)").unwrap());
    re.captures(&plain)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| tr!("o app do Roblox está sem conta logada", "the Roblox app has no account signed in"))
}
