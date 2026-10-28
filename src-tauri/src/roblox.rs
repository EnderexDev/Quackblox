// Roblox's web API, the parts Quackblox uses: who a login belongs to, avatars and game names
// for the window, and the one-use ticket that opens a client logged in as that account.
use regex::Regex;
use reqwest::Client;
use reqwest::header::{COOKIE, HeaderMap, HeaderValue, REFERER, SET_COOKIE};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    // Timeouts so a stalled connection ends as an error instead of a button busy forever.
    CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent("Quackblox/0.1")
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("http client")
    })
}

fn cookie_header(cookie: &str) -> Result<HeaderValue, String> {
    HeaderValue::from_str(&format!(".ROBLOSECURITY={cookie}")).map_err(|_| tr!("login inválido", "invalid login"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: u64,
    pub name: String,
    pub display_name: String,
}

/// The account a login belongs to; an error when Roblox no longer accepts it.
pub async fn user_of(cookie: &str) -> Result<User, String> {
    #[derive(Deserialize)]
    struct Raw {
        id: u64,
        name: String,
        #[serde(rename = "displayName")]
        display_name: String,
    }
    let res = client()
        .get("https://users.roblox.com/v1/users/authenticated")
        .header(COOKIE, cookie_header(cookie)?)
        .send()
        .await
        .map_err(|e| tr!("sem conexão com o Roblox: {e}", "no connection to Roblox: {e}"))?;
    if res.status() == 401 {
        return Err(tr!("o login dessa conta expirou", "this account's login expired"));
    }
    if !res.status().is_success() {
        return Err(tr!("o Roblox recusou agora (HTTP {}), tenta de novo daqui a pouco", "Roblox refused right now (HTTP {}), try again in a moment", res.status().as_u16()));
    }
    let raw: Raw = res.json().await.map_err(|e| tr!("resposta estranha do Roblox: {e}", "unexpected answer from Roblox: {e}"))?;
    Ok(User { id: raw.id, name: raw.name, display_name: raw.display_name })
}

/// Avatar headshots by user id, as image URLs (a missing one is left out).
pub async fn headshots(ids: &[u64]) -> Vec<(u64, String)> {
    if ids.is_empty() {
        return vec![];
    }
    #[derive(Deserialize)]
    struct Item {
        #[serde(rename = "targetId")]
        target_id: u64,
        #[serde(rename = "imageUrl")]
        image_url: Option<String>,
    }
    #[derive(Deserialize)]
    struct Raw {
        data: Vec<Item>,
    }
    let ids: Vec<String> = ids.iter().map(u64::to_string).collect();
    let url = format!(
        "https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={}&size=150x150&format=Png&isCircular=false",
        ids.join(",")
    );
    let Ok(res) = client().get(url).send().await else { return vec![] };
    let Ok(raw) = res.json::<Raw>().await else { return vec![] };
    raw.data.into_iter().filter_map(|i| Some((i.target_id, i.image_url?))).collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub place_id: u64,
    /// A server picked inside the game (a link with gameInstanceId), or none for any server.
    pub job_id: Option<String>,
    pub name: String,
    pub icon: Option<String>,
}

/// A place id, or a link to a game ("roblox.com/games/123/..." or "...?placeId=123"), with the
/// server when the link names one.
pub fn parse_game(input: &str) -> Option<(u64, Option<String>)> {
    let input = input.trim();
    if let Ok(id) = input.parse::<u64>() {
        return Some((id, None));
    }
    static PLACE: OnceLock<Regex> = OnceLock::new();
    static JOB: OnceLock<Regex> = OnceLock::new();
    let place = PLACE.get_or_init(|| Regex::new(r"(?i)(?:/games/|placeId=)(\d+)").unwrap());
    let job = JOB.get_or_init(|| Regex::new(r"(?i)gameInstanceId=([0-9a-f-]{36})").unwrap());
    let id = place.captures(input)?.get(1)?.as_str().parse().ok()?;
    let job_id = job.captures(input).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
    Some((id, job_id))
}

/// The game's name and icon, for the picker and the recent games.
pub async fn game(input: &str) -> Result<Game, String> {
    let (place_id, job_id) = parse_game(input).ok_or_else(|| tr!("isso não parece um jogo: cola o link ou o ID", "that doesn't look like a game: paste the link or the ID"))?;
    #[derive(Deserialize)]
    struct Universe {
        #[serde(rename = "universeId")]
        universe_id: Option<u64>,
    }
    let universe: Universe = client()
        .get(format!("https://apis.roblox.com/universes/v1/places/{place_id}/universe"))
        .send()
        .await
        .map_err(|e| tr!("sem conexão com o Roblox: {e}", "no connection to Roblox: {e}"))?
        .json()
        .await
        .map_err(|_| tr!("não achei esse jogo", "couldn't find that game"))?;
    let universe_id = universe.universe_id.ok_or_else(|| tr!("não achei esse jogo", "couldn't find that game"))?;

    #[derive(Deserialize)]
    struct Detail {
        name: String,
    }
    #[derive(Deserialize)]
    struct Details {
        data: Vec<Detail>,
    }
    let name = client()
        .get(format!("https://games.roblox.com/v1/games?universeIds={universe_id}"))
        .send()
        .await
        .ok()
        .and_then(|r| r.error_for_status().ok());
    let name = match name {
        Some(r) => r.json::<Details>().await.ok().and_then(|d| d.data.into_iter().next()).map(|d| d.name),
        None => None,
    }
    .unwrap_or_else(|| tr!("Jogo {place_id}", "Game {place_id}"));

    #[derive(Deserialize)]
    struct Icon {
        #[serde(rename = "imageUrl")]
        image_url: Option<String>,
    }
    #[derive(Deserialize)]
    struct Icons {
        data: Vec<Icon>,
    }
    let icon = match client()
        .get(format!(
            "https://thumbnails.roblox.com/v1/games/icons?universeIds={universe_id}&size=150x150&format=Png&isCircular=false"
        ))
        .send()
        .await
    {
        Ok(r) => r.json::<Icons>().await.ok().and_then(|i| i.data.into_iter().next()).and_then(|i| i.image_url),
        Err(_) => None,
    };
    Ok(Game { place_id, job_id, name, icon })
}

/// The newer login when Roblox rotated it in this response.
fn rotated(headers: &HeaderMap) -> Option<String> {
    headers.get_all(SET_COOKIE).iter().find_map(|v| {
        let v = v.to_str().ok()?;
        let rest = v.strip_prefix(".ROBLOSECURITY=")?;
        let value = rest.split(';').next()?;
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// A one-use ticket that logs a client in as this account, plus the login's newer value when
/// Roblox rotated it. The first call answers 403 with the CSRF token the second one needs.
/// The login's newer value comes back on failure too: Roblox may already have retired the old one.
pub async fn ticket(cookie: &str) -> (Result<String, String>, Option<String>) {
    let mut rotation = None;
    let ticket = ticket_inner(cookie, &mut rotation).await;
    (ticket, rotation)
}

async fn ticket_inner(cookie: &str, new_cookie: &mut Option<String>) -> Result<String, String> {
    let url = "https://auth.roblox.com/v1/authentication-ticket";
    let send = |cookie: &str, csrf: Option<HeaderValue>| {
        let mut req = client()
            .post(url)
            .header(COOKIE, cookie_header(cookie).unwrap_or(HeaderValue::from_static("")))
            .header(REFERER, "https://www.roblox.com/")
            .header("Origin", "https://www.roblox.com")
            .header("Content-Type", "application/json")
            .body("{}");
        if let Some(token) = csrf {
            req = req.header("x-csrf-token", token);
        }
        req.send()
    };
    cookie_header(cookie)?;
    let first = send(cookie, None).await.map_err(|e| tr!("sem conexão com o Roblox: {e}", "no connection to Roblox: {e}"))?;
    // A rotation in the first answer is the login the second request has to use.
    *new_cookie = rotated(first.headers());
    let res = match first.headers().get("x-csrf-token").cloned() {
        Some(token) if first.status() == 403 => {
            let current = new_cookie.clone().unwrap_or_else(|| cookie.to_string());
            send(&current, Some(token)).await.map_err(|e| tr!("sem conexão com o Roblox: {e}", "no connection to Roblox: {e}"))?
        }
        _ => first,
    };
    if let Some(latest) = rotated(res.headers()) {
        *new_cookie = Some(latest);
    }
    if res.status() == 401 {
        return Err(tr!("o login dessa conta expirou", "this account's login expired"));
    }
    res.headers()
        .get("rbx-authentication-ticket")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .ok_or_else(|| tr!("o Roblox não deu o ticket de entrada (HTTP {})", "Roblox didn't give the join ticket (HTTP {})", res.status().as_u16()))
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The roblox-player: link the site's Play button opens, for this ticket and game.
pub fn launch_link(ticket: &str, place_id: u64, job_id: Option<&str>) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let tracker: u64 = (now as u64 % 900_000_000) + 100_000_000;
    let attempt = uuid::Uuid::new_v4();
    let request = match job_id {
        Some(job) => format!("RequestGameJob&gameId={job}"),
        None => "RequestGame".to_string(),
    };
    let launcher = format!(
        "https://www.roblox.com/Game/PlaceLauncher.ashx?request={request}&browserTrackerId={tracker}&placeId={place_id}&isPlayTogetherGame=false&joinAttemptId={attempt}&joinAttemptOrigin=PlayButton"
    );
    format!(
        "roblox-player:1+launchmode:play+gameinfo:{ticket}+launchtime:{now}+placelauncherurl:{}+browsertrackerid:{tracker}+robloxLocale:pt_br+gameLocale:pt_br+channel:+LaunchExp:InApp",
        encode(&launcher)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ids_and_links() {
        assert_eq!(parse_game("1818"), Some((1818, None)));
        assert_eq!(parse_game("https://www.roblox.com/games/920587237/Adopt-Me"), Some((920587237, None)));
        assert_eq!(parse_game("roblox://experiences/start?placeId=42"), Some((42, None)));
        assert_eq!(
            parse_game("https://www.roblox.com/games/start?placeId=7&gameInstanceId=0b7c2f3e-1d2a-4c5b-9e8f-112233445566"),
            Some((7, Some("0b7c2f3e-1d2a-4c5b-9e8f-112233445566".into())))
        );
        assert_eq!(parse_game("abc"), None);
    }

    #[test]
    fn link_encodes_the_launcher() {
        let link = launch_link("T", 1818, None);
        assert!(link.starts_with("roblox-player:1+launchmode:play+gameinfo:T+"));
        assert!(link.contains("placelauncherurl:https%3A%2F%2Fwww.roblox.com%2FGame%2FPlaceLauncher.ashx%3Frequest%3DRequestGame%26"));
        assert!(link.contains("placeId%3D1818%26"));
    }
}
