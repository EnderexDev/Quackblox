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

/// Whether Roblox still takes a login, and its newer value when Roblox rotated it in the answer.
/// An error when Roblox could not say (no connection, a refusal that is not 401).
pub async fn login_alive(cookie: &str) -> Result<(bool, Option<String>), String> {
    let res = client()
        .get("https://users.roblox.com/v1/users/authenticated")
        .header(COOKIE, cookie_header(cookie)?)
        .send()
        .await
        .map_err(|e| tr!("sem conexão com o Roblox: {e}", "no connection to Roblox: {e}"))?;
    let newer = rotated(res.headers());
    match res.status().as_u16() {
        401 => Ok((false, newer)),
        s if (200..300).contains(&s) => Ok((true, newer)),
        s => Err(tr!("o Roblox recusou agora (HTTP {s})", "Roblox refused right now (HTTP {s})")),
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub place_id: u64,
    /// A server picked inside the game (a link with gameInstanceId), or none for any server.
    pub job_id: Option<String>,
    /// A private server's link code (from a share link or a privateServerLinkCode link).
    pub link_code: Option<String>,
    pub name: String,
    pub icon: Option<String>,
}

/// Where a client goes: a game, and in it a server or a private server, or any server.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Place {
    pub place_id: u64,
    pub job_id: Option<String>,
    pub link_code: Option<String>,
}

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).unwrap())
}

/// A place id, or a link to a game ("roblox.com/games/123/..." or "...?placeId=123"), with the
/// server when the link names one (gameInstanceId, the site's joinGameInstance(123, "job") or a
/// privateServerLinkCode). A share link (roblox.com/share?code=...) is not here: it needs a
/// login to read, see `share_code`.
pub fn parse_game(input: &str) -> Option<Place> {
    let input = input.trim();
    if let Ok(id) = input.parse::<u64>() {
        return Some(Place { place_id: id, ..Default::default() });
    }
    static PLACE: OnceLock<Regex> = OnceLock::new();
    static JOB: OnceLock<Regex> = OnceLock::new();
    static CALL: OnceLock<Regex> = OnceLock::new();
    static CODE: OnceLock<Regex> = OnceLock::new();
    if let Some(c) = re(&CALL, r#"(?i)joinGameInstance\(\s*(\d+)\s*,\s*["']([0-9a-f-]{36})["']"#).captures(input) {
        return Some(Place { place_id: c[1].parse().ok()?, job_id: Some(c[2].to_lowercase()), link_code: None });
    }
    let place_id = re(&PLACE, r"(?i)(?:/games/|placeId=)(\d+)").captures(input)?.get(1)?.as_str().parse().ok()?;
    let job_id = re(&JOB, r"(?i)gameInstanceId=([0-9a-f-]{36})").captures(input).map(|c| c[1].to_lowercase());
    let link_code = re(&CODE, r"(?i)privateServerLinkCode=([A-Za-z0-9_-]+)").captures(input).map(|c| c[1].to_string());
    Some(Place { place_id, job_id, link_code })
}

/// The code of a share link ("roblox.com/share?code=X&type=Server"), when the input is one.
/// Any other type of share (a game, a profile) is refused with why.
pub fn share_code(input: &str) -> Option<Result<String, String>> {
    static SHARE: OnceLock<Regex> = OnceLock::new();
    static KIND: OnceLock<Regex> = OnceLock::new();
    let code = re(&SHARE, r"(?i)roblox\.com/share(?:-links)?\?(?:[^ ]*&)?code=([A-Za-z0-9]+)").captures(input)?[1].to_string();
    let kind = re(&KIND, r"(?i)[?&]type=([A-Za-z]+)").captures(input).map(|c| c[1].to_string());
    Some(match kind.as_deref() {
        None => Ok(code),
        Some(k) if k.eq_ignore_ascii_case("Server") => Ok(code),
        Some(_) => Err(tr!("esse link não é de servidor privado", "that link isn't a private server's")),
    })
}

/// A JobId on its own (the id of a server), as the in-game console or a bug report shows it.
pub fn bare_job(input: &str) -> Option<String> {
    static JOB: OnceLock<Regex> = OnceLock::new();
    re(&JOB, r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").is_match(input.trim()).then(|| input.trim().to_lowercase())
}

/// The private server a share link opens: its place and the link code the client joins with.
/// Measured 28/10/26: answers with any account's login (not only the owner's), 401 without.
pub async fn resolve_share(cookie: &str, code: &str) -> Result<Place, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Invite {
        status: String,
        place_id: u64,
        link_code: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        private_server_invite_data: Option<Invite>,
    }
    let res = send_with_csrf(cookie, "https://apis.roblox.com/sharelinks/v1/resolve-link", &serde_json::json!({ "linkId": code, "linkType": "Server" }))
        .await?;
    if !res.status().is_success() {
        return Err(tr!("o Roblox não abriu esse link (HTTP {})", "Roblox didn't open that link (HTTP {})", res.status().as_u16()));
    }
    let raw: Raw = res.json().await.map_err(|e| tr!("resposta estranha do Roblox: {e}", "unexpected answer from Roblox: {e}"))?;
    let invite = raw.private_server_invite_data.ok_or_else(|| tr!("esse link não é de servidor privado", "that link isn't a private server's"))?;
    match (invite.status.as_str(), invite.link_code) {
        ("Valid", Some(link_code)) if invite.place_id != 0 => Ok(Place { place_id: invite.place_id, job_id: None, link_code: Some(link_code) }),
        _ => Err(tr!("esse link de servidor privado expirou ou foi trocado pelo dono", "that private server link expired or was changed by its owner")),
    }
}

/// A POST that needs the CSRF token: the first answer is 403 with the token the second one carries.
async fn send_with_csrf(cookie: &str, url: &str, body: &serde_json::Value) -> Result<reqwest::Response, String> {
    let send = |csrf: Option<HeaderValue>| -> Result<_, String> {
        let mut req = client().post(url).header(COOKIE, cookie_header(cookie)?).json(body);
        if let Some(token) = csrf {
            req = req.header("x-csrf-token", token);
        }
        Ok(req.send())
    };
    let no_net = |e: reqwest::Error| tr!("sem conexão com o Roblox: {e}", "no connection to Roblox: {e}");
    let first = send(None)?.await.map_err(no_net)?;
    match first.headers().get("x-csrf-token").cloned() {
        Some(token) if first.status() == 403 => send(Some(token))?.await.map_err(no_net),
        _ => Ok(first),
    }
}

/// Where an account is, as Roblox's presence says it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    pub user_id: u64,
    /// "offline", "site", "game" or "studio".
    pub kind: &'static str,
    /// The game's name; empty when Roblox hides it from the login that asked.
    pub location: String,
    pub place_id: Option<u64>,
    /// The server, when the login that asked may see it: its own, or a friend's (28/10/26).
    pub job_id: Option<String>,
}

/// The presence of these users, asked with this login. Measured 28/10/26: with no login the
/// game and server come back empty; an account's own login and a friend's login see them.
pub async fn presence(cookie: &str, ids: &[u64]) -> Result<Vec<Presence>, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Item {
        user_id: u64,
        user_presence_type: u8,
        last_location: Option<String>,
        place_id: Option<u64>,
        game_id: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        user_presences: Vec<Item>,
    }
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let res = client()
        .post("https://presence.roblox.com/v1/presence/users")
        .header(COOKIE, cookie_header(cookie)?)
        .json(&serde_json::json!({ "userIds": ids }))
        .send()
        .await
        .map_err(|e| tr!("sem conexão com o Roblox: {e}", "no connection to Roblox: {e}"))?;
    if !res.status().is_success() {
        return Err(tr!("o Roblox recusou agora (HTTP {}), tenta de novo daqui a pouco", "Roblox refused right now (HTTP {}), try again in a moment", res.status().as_u16()));
    }
    let raw: Raw = res.json().await.map_err(|e| tr!("resposta estranha do Roblox: {e}", "unexpected answer from Roblox: {e}"))?;
    Ok(raw
        .user_presences
        .into_iter()
        .map(|p| Presence {
            user_id: p.user_id,
            kind: match p.user_presence_type {
                1 => "site",
                2 => "game",
                3 => "studio",
                _ => "offline",
            },
            location: p.last_location.unwrap_or_default(),
            place_id: p.place_id,
            job_id: p.game_id,
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: String,
    pub playing: u32,
    pub max_players: u32,
    pub fps: Option<f32>,
    /// The average ping of the players in it, as Roblox reports it: not this PC's.
    pub ping: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPage {
    pub servers: Vec<Server>,
    pub next: Option<String>,
}

/// One page of a game's public servers, the fullest first or the emptiest first. Measured
/// 28/10/26: with no login it answers 429 from the third call in a row; with a login, 10 in a
/// row passed. So it asks with one.
pub async fn servers(cookie: Option<&str>, place_id: u64, fullest_first: bool, skip_full: bool, cursor: Option<&str>) -> Result<ServerPage, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        data: Vec<Server>,
        next_page_cursor: Option<String>,
    }
    let mut url = format!(
        "https://games.roblox.com/v1/games/{place_id}/servers/Public?limit=100&sortOrder={}&excludeFullGames={skip_full}",
        if fullest_first { "Desc" } else { "Asc" }
    );
    if let Some(c) = cursor {
        url.push_str("&cursor=");
        url.push_str(&encode(c));
    }
    let mut req = client().get(url);
    if let Some(cookie) = cookie {
        req = req.header(COOKIE, cookie_header(cookie)?);
    }
    let res = req.send().await.map_err(|e| tr!("sem conexão com o Roblox: {e}", "no connection to Roblox: {e}"))?;
    if res.status() == 429 {
        return Err(tr!("o Roblox pediu pra esperar uns segundos antes de listar de novo", "Roblox asked to wait a few seconds before listing again"));
    }
    if !res.status().is_success() {
        return Err(tr!("o Roblox recusou agora (HTTP {}), tenta de novo daqui a pouco", "Roblox refused right now (HTTP {}), try again in a moment", res.status().as_u16()));
    }
    let raw: Raw = res.json().await.map_err(|e| tr!("resposta estranha do Roblox: {e}", "unexpected answer from Roblox: {e}"))?;
    Ok(ServerPage { servers: raw.data, next: raw.next_page_cursor })
}

/// A friend of a saved account who is in a game now.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    pub id: u64,
    pub name: String,
    pub display_name: String,
    pub location: String,
    pub place_id: u64,
    /// The friend's server; none when Roblox hides it.
    pub job_id: Option<String>,
}

/// The friends of this account who are in a game now, with their server. The friends list comes
/// with no names (28/10/26), so those are asked apart, only for the ones in a game.
pub async fn friends_in_game(cookie: &str, user_id: u64) -> Result<Vec<Friend>, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Where {
        #[serde(rename = "UserPresenceType")]
        kind: Option<String>,
        last_location: Option<String>,
        place_id: Option<u64>,
        game_instance_id: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Item {
        id: u64,
        user_presence: Option<Where>,
    }
    #[derive(Deserialize)]
    struct Raw {
        data: Vec<Item>,
    }
    let res = client()
        .get(format!("https://friends.roblox.com/v1/users/{user_id}/friends/online"))
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
    let mut friends: Vec<Friend> = raw
        .data
        .into_iter()
        .filter_map(|f| {
            let p = f.user_presence?;
            if p.kind.as_deref() != Some("InGame") {
                return None;
            }
            Some(Friend {
                id: f.id,
                name: String::new(),
                display_name: String::new(),
                location: p.last_location.unwrap_or_default(),
                place_id: p.place_id?,
                job_id: p.game_instance_id,
            })
        })
        .collect();
    for (id, name, display_name) in names(&friends.iter().map(|f| f.id).collect::<Vec<_>>()).await {
        if let Some(f) = friends.iter_mut().find(|f| f.id == id) {
            f.name = name;
            f.display_name = display_name;
        }
    }
    Ok(friends)
}

/// Names of users by id (a failed request leaves them out).
async fn names(ids: &[u64]) -> Vec<(u64, String, String)> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Item {
        id: u64,
        name: String,
        display_name: String,
    }
    #[derive(Deserialize)]
    struct Raw {
        data: Vec<Item>,
    }
    if ids.is_empty() {
        return vec![];
    }
    let body = serde_json::json!({ "userIds": ids, "excludeBannedUsers": true });
    let Ok(res) = client().post("https://users.roblox.com/v1/users").json(&body).send().await else { return vec![] };
    let Ok(raw) = res.json::<Raw>().await else { return vec![] };
    raw.data.into_iter().map(|i| (i.id, i.name, i.display_name)).collect()
}

/// The game's name and icon, for the picker and the recent games.
pub async fn game(input: &str) -> Result<Game, String> {
    let place = parse_game(input).ok_or_else(|| tr!("isso não parece um jogo: cola o link ou o ID", "that doesn't look like a game: paste the link or the ID"))?;
    game_of(place).await
}

/// The name and icon of the game a place is in.
pub async fn game_of(place: Place) -> Result<Game, String> {
    let Place { place_id, job_id, link_code } = place;
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
    Ok(Game { place_id, job_id, link_code, name, icon })
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

/// The roblox-player: link the site's Play button opens, for this ticket and place. A private
/// server goes by RequestPrivateGame with its link code (measured 28/10/26: a client of an account
/// that is not the owner joined one this way); a server picked, by RequestGameJob.
pub fn launch_link(ticket: &str, place: &Place) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let tracker: u64 = (now as u64 % 900_000_000) + 100_000_000;
    let attempt = uuid::Uuid::new_v4();
    let place_id = place.place_id;
    let (request, extra) = match (&place.link_code, &place.job_id) {
        (Some(code), _) => ("RequestPrivateGame".to_string(), format!("&linkCode={}", encode(code))),
        (None, Some(job)) => (format!("RequestGameJob&gameId={job}"), String::new()),
        (None, None) => ("RequestGame".to_string(), String::new()),
    };
    let launcher = format!(
        "https://www.roblox.com/Game/PlaceLauncher.ashx?request={request}&browserTrackerId={tracker}&placeId={place_id}{extra}&isPlayTogetherGame=false&joinAttemptId={attempt}&joinAttemptOrigin=PlayButton"
    );
    format!(
        "roblox-player:1+launchmode:play+gameinfo:{ticket}+launchtime:{now}+placelauncherurl:{}+browsertrackerid:{tracker}+robloxLocale:pt_br+gameLocale:pt_br+channel:+LaunchExp:InApp",
        encode(&launcher)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(place_id: u64, job: Option<&str>, code: Option<&str>) -> Option<Place> {
        Some(Place { place_id, job_id: job.map(Into::into), link_code: code.map(Into::into) })
    }

    #[test]
    fn parses_ids_and_links() {
        assert_eq!(parse_game("1818"), at(1818, None, None));
        assert_eq!(parse_game("https://www.roblox.com/games/920587237/Adopt-Me"), at(920587237, None, None));
        assert_eq!(parse_game("roblox://experiences/start?placeId=42"), at(42, None, None));
        assert_eq!(
            parse_game("https://www.roblox.com/games/start?placeId=7&gameInstanceId=0B7C2F3E-1d2a-4c5b-9e8f-112233445566"),
            at(7, Some("0b7c2f3e-1d2a-4c5b-9e8f-112233445566"), None)
        );
        assert_eq!(
            parse_game(r#"Roblox.GameLauncher.joinGameInstance(1818, "0b7c2f3e-1d2a-4c5b-9e8f-112233445566")"#),
            at(1818, Some("0b7c2f3e-1d2a-4c5b-9e8f-112233445566"), None)
        );
        assert_eq!(
            parse_game("https://www.roblox.com/games/94717504417144/Anime?privateServerLinkCode=12345678901234567890"),
            at(94717504417144, None, Some("12345678901234567890"))
        );
        assert_eq!(parse_game("abc"), None);
    }

    #[test]
    fn reads_share_links_and_job_ids() {
        assert_eq!(share_code("https://www.roblox.com/share?code=abc123DEF&type=Server"), Some(Ok("abc123DEF".into())));
        assert_eq!(share_code("roblox.com/share?type=Server&code=abc"), Some(Ok("abc".into())));
        assert!(matches!(share_code("https://www.roblox.com/share?code=abc&type=ExperienceDetails"), Some(Err(_))));
        assert_eq!(share_code("https://www.roblox.com/games/1818"), None);
        assert_eq!(bare_job(" 0B7C2F3E-1d2a-4c5b-9e8f-112233445566 "), Some("0b7c2f3e-1d2a-4c5b-9e8f-112233445566".into()));
        assert_eq!(bare_job("1818"), None);
    }

    #[test]
    fn link_encodes_the_launcher() {
        let link = launch_link("T", &Place { place_id: 1818, ..Default::default() });
        assert!(link.starts_with("roblox-player:1+launchmode:play+gameinfo:T+"));
        assert!(link.contains("placelauncherurl:https%3A%2F%2Fwww.roblox.com%2FGame%2FPlaceLauncher.ashx%3Frequest%3DRequestGame%26"));
        assert!(link.contains("placeId%3D1818%26"));
        let private = launch_link("T", &Place { place_id: 7, job_id: None, link_code: Some("abc".into()) });
        assert!(private.contains("request%3DRequestPrivateGame%26"));
        assert!(private.contains("placeId%3D7%26linkCode%3Dabc%26"));
        let job = launch_link("T", &Place { place_id: 7, job_id: Some("j".into()), link_code: None });
        assert!(job.contains("request%3DRequestGameJob%26gameId%3Dj%26"));
    }
}
