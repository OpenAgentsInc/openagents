//! Coder chats saved to the signed-in openagents.com account (#11046).
//!
//! Off by default. When the person turns it on (`/sync on` in Coder), each
//! saved chat's messages go to the website's `/v1/threads/synced/{session}`
//! (`openagents-web` `coder_sync`) under the account's own app token
//! ([`openagents_login::Saved`]), in the background, retrying quietly.
//! While Coder replies it sends a "working" heartbeat so the website shows
//! the chat as Working.
//!
//! While sync is on, Coder also checks in every [`CHECK_IN_EVERY`]
//! ([`Job::Listen`]), so the website offers a reply box on this computer's
//! chats (#11048). A reply typed there waits on the website until Coder
//! takes it ([`Job::Take`]) and answers it here, with this computer's tools.
//! The same take brings the messages added to the chat on the website
//! while this computer was offline (a run on a Cloud computer, #11050), so
//! Coder adds them to its own copy, in order, before answering (#11052).
//!
//! What goes up is the chat's title, the computer's name, and its
//! messages: what the person wrote, what the model answered, and one line
//! per tool call (its name and what it acted on, not its output). Every
//! message passes the secret screen ([`secret_screen::Screen`]) first; a
//! message that looks like it holds a credential is left out
//! ([`LEFT_OUT`]), never uploaded. Plugin keys, model credentials, and
//! files are never read here.
//!
//! Where this computer's chats live is the person's choice, asked once
//! (#11089): "Sync all my chats" or "Keep chats on this computer"
//! ([`Choice`]). The website keeps the same choice per computer
//! (`/v1/computers/{name}/sync`, [`choice`], [`choose`]), so it can be
//! made or changed there too.
//!
//! Every call asks the `/v1` path first (#11158). A website from before
//! that serves only the older `/coder/*` paths, so a `404` that isn't the
//! API's own `unknown` is asked again there.
//!
//! The same take brings screenshots and files asked for on the website
//! from this computer (#11185, [`Ask`]): Coder runs each through this
//! computer's own host and sends back what came of it ([`Job::Capture`]).
//! The website never reaches the computer; it only answers what Coder
//! asks.
//!
//! [`Settings`] lives in `sync.json` (0600) beside the account file.

pub mod activity;
pub mod claude_session;
pub mod memory;
pub mod own_runs;
pub mod traces;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use openagents_login::Saved;
use secret_screen::Screen;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The settings file, in Coder's folder.
pub const FILE: &str = "sync.json";

/// What a message that looked like it held a secret says instead.
pub const LEFT_OUT: &str = "(Left out: this looked like it held a password or key.)";

/// The longest one message is sent; longer ones are cut.
pub const MAX_TEXT: usize = 64 * 1024;
/// The most messages sent for one chat (the newest).
pub const MAX_MESSAGES: usize = 4000;
/// The largest upload; the oldest messages are dropped to fit.
pub const MAX_BODY: usize = 7 * 1024 * 1024;

/// How often a running reply repeats its "working" heartbeat.
pub const HEARTBEAT: Duration = Duration::from_secs(30);
/// How often Coder asks which chats were deleted on the website.
pub const CHECK_EVERY: Duration = Duration::from_secs(300);
/// How often Coder, with sync on, tells the website this computer is
/// online and asks for replies sent there (#11048). The website offers a
/// reply box on this computer's chats while these keep coming.
pub const CHECK_IN_EVERY: Duration = Duration::from_secs(10);

/// The person's choice and what has been sent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Save chats to the account.
    #[serde(default)]
    pub on: bool,
    /// The person answered where this computer's chats live (#11089);
    /// until then Coder asks.
    #[serde(default)]
    pub chosen: bool,
    /// Per session, the digest of the last upload the website accepted.
    #[serde(default)]
    pub sent: BTreeMap<String, String>,
    /// Sessions deleted here whose copy on the website still has to go.
    #[serde(default)]
    pub to_delete: BTreeSet<String>,
    /// Sessions never sent again: deleted on the website while open here.
    #[serde(default)]
    pub kept_here: BTreeSet<String>,
}

impl Settings {
    /// The settings in `dir`; none saved (or unreadable) is off.
    #[must_use]
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(dir.join(FILE))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Keep the settings in `dir/sync.json`, readable only by this user.
    pub fn store(&self, dir: &Path) -> Result<(), String> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        write_private(&dir.join(FILE), &bytes)
    }

    /// Whether a chat should be sent now.
    #[must_use]
    pub fn sends(&self, session: &str, digest: &str) -> bool {
        self.on
            && !self.kept_here.contains(session)
            && self.sent.get(session).is_none_or(|sent| sent != digest)
    }
}

fn write_private(file: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let dir = file
        .parent()
        .ok_or_else(|| format!("{} has no folder.", file.display()))?;
    std::fs::create_dir_all(dir).map_err(|_| format!("Couldn't make {}.", dir.display()))?;
    let tmp = dir.join(format!(".{FILE}.{}.tmp", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let fail = || format!("Couldn't save {}.", file.display());
    let mut handle = options.open(&tmp).map_err(|_| fail())?;
    handle.write_all(bytes).map_err(|_| fail())?;
    handle.sync_all().map_err(|_| fail())?;
    drop(handle);
    std::fs::rename(&tmp, file).map_err(|_| {
        let _ = std::fs::remove_file(&tmp);
        fail()
    })
}

/// One chat, ready to send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upload {
    /// `{computer, title, messages: [{role, text}]}`.
    pub body: Value,
    /// A digest of `body`, so an unchanged chat isn't sent again.
    pub digest: String,
    /// How many messages were left out by the secret screen.
    pub left_out: usize,
}

/// The upload for a saved Coder chat (its ATIF document).
#[must_use]
pub fn upload(document: &Value, computer: &str, screen: &Screen) -> Upload {
    let mut left_out = 0;
    let mut screened = |text: String| -> String {
        if screen.check(&text).is_err() {
            left_out += 1;
            LEFT_OUT.to_owned()
        } else {
            cut(text)
        }
    };
    let mut messages = Vec::new();
    for step in document
        .get("steps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let text = message_text(step.get("message").unwrap_or(&Value::Null));
        match step.get("source").and_then(Value::as_str) {
            Some("user") if !text.trim().is_empty() => {
                messages.push(json!({"role": "user", "text": screened(text)}));
            }
            Some("agent") => {
                if !text.trim().is_empty() {
                    messages.push(json!({"role": "assistant", "text": screened(text)}));
                }
                for call in step
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    messages.push(json!({"role": "tool", "text": screened(tool_line(call))}));
                }
            }
            _ => {}
        }
    }
    if messages.len() > MAX_MESSAGES {
        messages.drain(..messages.len() - MAX_MESSAGES);
    }
    let title = document
        .pointer("/extra/title")
        .or_else(|| document.get("title"))
        .and_then(Value::as_str)
        .map(|title| line(title, 120))
        .filter(|title| !title.is_empty())
        .or_else(|| {
            messages
                .iter()
                .find(|m| m["role"] == "user" && m["text"] != LEFT_OUT)
                .and_then(|m| m["text"].as_str())
                .map(|text| line(text, 120))
        })
        .filter(|title| screen.check(title).is_ok())
        .unwrap_or_else(|| "Coder chat".to_owned());
    let computer = line(computer, 64);
    let mut body = json!({"computer": computer, "title": title, "messages": messages});
    while serde_json::to_vec(&body).map_or(0, |bytes| bytes.len()) > MAX_BODY {
        let Some(messages) = body["messages"].as_array_mut() else {
            break;
        };
        if messages.is_empty() {
            break;
        }
        let drop = messages.len().div_ceil(10);
        messages.drain(..drop);
    }
    let digest = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&body).unwrap_or_default())
    );
    Upload {
        body,
        digest,
        left_out,
    }
}

fn message_text(value: &Value) -> String {
    value.as_str().map(str::to_owned).unwrap_or_else(|| {
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// One line for a tool call: its name and what it acted on.
fn tool_line(call: &Value) -> String {
    if call.pointer("/extra/schema").and_then(Value::as_str) == Some("openagents.delegation.v1") {
        let agent = call
            .pointer("/arguments/agent")
            .and_then(Value::as_str)
            .unwrap_or("a helper");
        let task = call
            .pointer("/arguments/task")
            .and_then(Value::as_str)
            .unwrap_or("");
        return line(&format!("Asked {agent}: {task}"), 200);
    }
    let name = call
        .get("function_name")
        .and_then(Value::as_str)
        .unwrap_or("tool");
    let arguments = call.get("arguments").unwrap_or(&Value::Null);
    let subject = [
        "command",
        "cmd",
        "path",
        "file_path",
        "pattern",
        "query",
        "url",
    ]
    .iter()
    .find_map(|key| arguments.get(*key).and_then(Value::as_str))
    .map(str::to_owned)
    .or_else(|| arguments.as_str().map(str::to_owned));
    match subject {
        Some(subject) if !subject.trim().is_empty() => line(&format!("{name}: {subject}"), 200),
        _ => line(name, 200),
    }
}

/// One line of plain text, at most `limit` characters.
fn line(value: &str, limit: usize) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

/// `text`, cut to [`MAX_TEXT`] bytes on a character boundary.
fn cut(mut text: String) -> String {
    if text.len() <= MAX_TEXT {
        return text;
    }
    let mut end = MAX_TEXT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str("\n…");
    text
}

/// What the website said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Done,
    /// Deleted on the website.
    Deleted,
    /// The website has no such chat (a heartbeat before the first upload).
    Unknown,
    /// The account has no room for more chats.
    Full,
    /// The sign-in no longer works.
    SignedOut,
    /// Refused for good; the website's words.
    Refused(String),
    /// Not reached, or a passing failure: try again later.
    Retry,
}

fn client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .ok()
}

async fn call(
    http: &reqwest::Client,
    saved: &Saved,
    method: reqwest::Method,
    path: &str,
    body: Option<&Value>,
) -> (Answer, Value) {
    let method_has_body = matches!(method, reqwest::Method::POST | reqwest::Method::PUT);
    let mut request = http
        .request(method, format!("{}{path}", saved.origin))
        .bearer_auth(saved.token());
    if let Some(body) = body {
        request = request.json(body);
    } else if method_has_body {
        // Google's front end refuses a POST with no length (411), before
        // the website sees it: send an empty object.
        request = request.json(&json!({}));
    }
    let Ok(response) = request.send().await else {
        return (Answer::Retry, Value::Null);
    };
    let status = response.status().as_u16();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    let code = body["error"]["code"].as_str().unwrap_or_default();
    let answer = match status {
        200 => Answer::Done,
        401 => Answer::SignedOut,
        404 => Answer::Unknown,
        410 => Answer::Deleted,
        413 if code == "full" => Answer::Full,
        400..=499 if status != 408 && status != 429 => Answer::Refused(
            body["error"]["message"]
                .as_str()
                .unwrap_or("The website refused this chat.")
                .to_owned(),
        ),
        _ => Answer::Retry,
    };
    (answer, body)
}

/// Synced chats, under `/v1` (#11158).
const SYNCED: &str = "/v1/threads/synced";
/// Their path on a website from before #11158.
const OLDER_SYNCED: &str = "/coder/sessions";
/// A computer checking in, under `/v1`.
const CHECK_IN: &str = "/v1/computers/check-in";
/// Its path on a website from before #11158.
const OLDER_CHECK_IN: &str = "/coder/check-in";

/// The older path of a `/v1` one this crate calls, on a website from
/// before #11158.
fn older(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(SYNCED) {
        format!("{OLDER_SYNCED}{rest}")
    } else if path == CHECK_IN {
        OLDER_CHECK_IN.to_owned()
    } else {
        path.to_owned()
    }
}

/// A `404` that isn't the API's own `unknown` (no such chat): the website
/// doesn't serve the path at all.
fn no_route(answer: &Answer, body: &Value) -> bool {
    *answer == Answer::Unknown && body["error"]["code"] != "unknown"
}

/// [`call`] at a `/v1` path; when the website doesn't serve it (a site
/// from before #11158), the same call at its older path ([`older`]).
async fn call_v1(
    http: &reqwest::Client,
    saved: &Saved,
    method: reqwest::Method,
    path: &str,
    body: Option<&Value>,
) -> (Answer, Value) {
    let (answer, answered) = call(http, saved, method.clone(), path, body).await;
    if no_route(&answer, &answered) {
        return call(http, saved, method, &older(path), body).await;
    }
    (answer, answered)
}

fn session_path(session: &str) -> Option<String> {
    (!session.is_empty()
        && session.len() <= 128
        && session
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')))
    .then(|| format!("{SYNCED}/{session}"))
}

/// Send one chat.
pub async fn put(http: &reqwest::Client, saved: &Saved, session: &str, upload: &Upload) -> Answer {
    let Some(path) = session_path(session) else {
        return Answer::Refused("That isn't a Coder session id.".into());
    };
    call_v1(http, saved, reqwest::Method::PUT, &path, Some(&upload.body))
        .await
        .0
}

/// Say whether Coder is replying in this chat.
pub async fn status(http: &reqwest::Client, saved: &Saved, session: &str, working: bool) -> Answer {
    let Some(path) = session_path(session) else {
        return Answer::Refused("That isn't a Coder session id.".into());
    };
    let body = json!({"working": working});
    call_v1(
        http,
        saved,
        reqwest::Method::POST,
        &format!("{path}/status"),
        Some(&body),
    )
    .await
    .0
}

/// Delete one chat from the website.
pub async fn delete(http: &reqwest::Client, saved: &Saved, session: &str) -> Answer {
    let Some(path) = session_path(session) else {
        return Answer::Done;
    };
    call_v1(http, saved, reqwest::Method::DELETE, &path, None)
        .await
        .0
}

/// The chats deleted on the website that Coder still has.
pub async fn deleted_on_site(http: &reqwest::Client, saved: &Saved) -> Result<Vec<String>, Answer> {
    match call_v1(http, saved, reqwest::Method::GET, SYNCED, None).await {
        (Answer::Done, body) => Ok(body["sessions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|row| row["deleted"] == true)
            .filter_map(|row| row["session"].as_str().map(str::to_owned))
            .collect()),
        (answer, _) => Err(answer),
    }
}

/// Where a computer's chats live (#11089).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Sync all my chats.
    All,
    /// Keep chats on this computer.
    Local,
}

impl Choice {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Local => "local",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "all" => Some(Self::All),
            "local" => Some(Self::Local),
            _ => None,
        }
    }

    /// The choice `settings` stand for, once the person made one.
    #[must_use]
    pub fn of(settings: &Settings) -> Option<Self> {
        settings
            .chosen
            .then_some(if settings.on { Self::All } else { Self::Local })
    }
}

/// The website's record of this computer's choice; `None` when it has
/// none (or is a website without choices).
pub async fn choice(
    http: &reqwest::Client,
    saved: &Saved,
    computer: &str,
) -> Result<Option<Choice>, Answer> {
    let name = activity::segment(computer);
    let mut asked = call(
        http,
        saved,
        reqwest::Method::GET,
        &format!("/v1/computers/{name}/sync"),
        None,
    )
    .await;
    if no_route(&asked.0, &asked.1) {
        // A website from before #11158.
        asked = call(
            http,
            saved,
            reqwest::Method::GET,
            &format!("/coder/sync?computer={name}"),
            None,
        )
        .await;
    }
    match asked {
        (Answer::Done, body) => Ok(body["choice"].as_str().and_then(Choice::parse)),
        (Answer::Unknown, _) => Ok(None),
        (answer, _) => Err(answer),
    }
}

/// Tell the website this computer's choice.
pub async fn choose(
    http: &reqwest::Client,
    saved: &Saved,
    computer: &str,
    choice: Choice,
) -> Answer {
    let (answer, body) = call(
        http,
        saved,
        reqwest::Method::PUT,
        &format!("/v1/computers/{}/sync", activity::segment(computer)),
        Some(&json!({"choice": choice.as_str()})),
    )
    .await;
    if !no_route(&answer, &body) {
        return answer;
    }
    // A website from before #11158 takes the computer in the body.
    let body = json!({"computer": line(computer, 64), "choice": choice.as_str()});
    call(
        http,
        saved,
        reqwest::Method::PUT,
        "/coder/sync",
        Some(&body),
    )
    .await
    .0
}

/// [`choice`] from a plain thread, waiting at most a few seconds.
#[must_use]
pub fn choice_now(saved: &Saved, computer: &str) -> Option<Choice> {
    let http = client()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), choice(&http, saved, computer))
            .await
            .ok()?
            .ok()?
    })
}

/// [`choose`] from a plain thread; true when the website took it.
#[must_use]
pub fn choose_now(saved: &Saved, computer: &str, picked: Choice) -> bool {
    let Some(http) = client() else {
        return false;
    };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return false;
    };
    runtime.block_on(async {
        matches!(
            tokio::time::timeout(
                Duration::from_secs(10),
                choose(&http, saved, computer, picked)
            )
            .await,
            Ok(Answer::Done)
        )
    })
}

/// Send these chats now, oldest first, from a plain thread: `coder login`'s
/// "Sync all my chats". Returns how many the website has afterwards and
/// the digests it accepted, to keep in [`Settings::sent`].
#[must_use]
pub fn send_now(saved: &Saved, uploads: Vec<(String, Upload)>) -> Vec<(String, String)> {
    let Some(http) = client() else {
        return Vec::new();
    };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return Vec::new();
    };
    runtime.block_on(async {
        let mut sent = Vec::new();
        for (session, upload) in uploads {
            match put(&http, saved, &session, &upload).await {
                Answer::Done => sent.push((session, upload.digest)),
                Answer::SignedOut | Answer::Full => break,
                _ => {}
            }
        }
        sent
    })
}

/// A reply typed on the website for one of this computer's chats.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub id: String,
    pub text: String,
}

/// Say this computer is online; the answer is its chats with replies
/// from the website waiting, and the computer's choice there.
pub async fn check_in(
    http: &reqwest::Client,
    saved: &Saved,
    computer: &str,
) -> Result<(Vec<String>, Option<Choice>), Answer> {
    let body = json!({"computer": line(computer, 64)});
    match call_v1(http, saved, reqwest::Method::POST, CHECK_IN, Some(&body)).await {
        (Answer::Done, body) => Ok((
            body["waiting"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|session| session_path(session).is_some())
                .map(str::to_owned)
                .collect(),
            body["sync"].as_str().and_then(Choice::parse),
        )),
        (answer, _) => Err(answer),
    }
}

/// A message added to one of this computer's chats on the website while
/// it was offline: the person's words, or the answer of a run on a Cloud
/// computer (#11050).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Added {
    /// The person wrote it (else the assistant answered it).
    pub user: bool,
    pub text: String,
}

/// A screenshot or file asked for on the website from this computer
/// (#11185).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    /// The website's id for it, a version 4 UUID.
    pub id: String,
    pub action: AskAction,
}

/// What the person asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AskAction {
    /// A picture of this computer's main screen.
    Screenshot,
    /// A copy of the file at this path (absolute, or under `~/`).
    Pull { path: String },
}

/// The largest capture sent back; the website takes no more.
pub const MAX_CAPTURE_BYTES: usize = 15 * 1024 * 1024;

fn ask_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// The asks in a take's answer; a website without asks sends none, and a
/// malformed one is left out.
fn asks(body: &Value) -> Vec<Ask> {
    body["asks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|ask| {
            let id = ask["id"].as_str().filter(|id| ask_id(id))?.to_owned();
            let action = match ask["action"]["kind"].as_str()? {
                "screenshot" => AskAction::Screenshot,
                "pull" => {
                    let path = ask["action"]["path"].as_str()?;
                    if !(path.starts_with('/') || path.starts_with("~/"))
                        || path.len() > 1024
                        || path.chars().any(char::is_control)
                    {
                        return None;
                    }
                    AskAction::Pull {
                        path: path.to_owned(),
                    }
                }
                _ => return None,
            };
            Some(Ask { id, action })
        })
        .collect()
}

/// Send back what an ask brought: the bytes, or why it couldn't be done.
pub async fn send_capture(
    http: &reqwest::Client,
    saved: &Saved,
    session: &str,
    ask: &str,
    result: &Result<Vec<u8>, String>,
) -> Answer {
    let Some(path) = session_path(session) else {
        return Answer::Refused("That isn't a Coder session id.".into());
    };
    if !ask_id(ask) {
        return Answer::Refused("That isn't an ask id.".into());
    }
    let too_large: Result<Vec<u8>, String>;
    let result = match result {
        Ok(bytes) if bytes.len() > MAX_CAPTURE_BYTES => {
            too_large = Err(format!(
                "It is {} MB, over the {} MB this can bring back.",
                bytes.len() / (1024 * 1024),
                MAX_CAPTURE_BYTES / (1024 * 1024)
            ));
            &too_large
        }
        other => other,
    };
    match result {
        Ok(bytes) => {
            let send = |path: String| {
                http.put(format!("{}{path}", saved.origin))
                    .bearer_auth(saved.token())
                    .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                    .body(bytes.clone())
                    .send()
            };
            let path = format!("{path}/captures/{ask}");
            let Ok(mut response) = send(path.clone()).await else {
                return Answer::Retry;
            };
            if response.status().as_u16() == 404 {
                let body: Value = response.json().await.unwrap_or(Value::Null);
                if !no_route(&Answer::Unknown, &body) {
                    return Answer::Unknown;
                }
                // A website from before #11158.
                let Ok(again) = send(older(&path)).await else {
                    return Answer::Retry;
                };
                response = again;
            }
            match response.status().as_u16() {
                200 => Answer::Done,
                401 => Answer::SignedOut,
                404 => Answer::Unknown,
                410 => Answer::Deleted,
                408 | 429 | 500..=599 => Answer::Retry,
                _ => Answer::Refused("The website refused the capture.".into()),
            }
        }
        Err(message) => {
            let body = json!({"message": line(message, 300)});
            call_v1(
                http,
                saved,
                reqwest::Method::POST,
                &format!("{path}/captures/{ask}/failed"),
                Some(&body),
            )
            .await
            .0
        }
    }
}

/// What one take brought from the website.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Taken {
    /// Replies typed on the website, for Coder to answer here.
    pub replies: Vec<Reply>,
    /// Messages added there while this computer was offline, oldest first,
    /// for Coder to add to its own copy before answering `replies`
    /// (#11052).
    pub added: Vec<Added>,
    /// Screenshots and files asked for there (#11185).
    pub asks: Vec<Ask>,
}

/// Take what waits in one chat. The website shows replies in the chat at
/// once and hands nothing out twice.
pub async fn take(http: &reqwest::Client, saved: &Saved, session: &str) -> Result<Taken, Answer> {
    let Some(path) = session_path(session) else {
        return Err(Answer::Refused("That isn't a Coder session id.".into()));
    };
    match call_v1(
        http,
        saved,
        reqwest::Method::POST,
        &format!("{path}/replies"),
        None,
    )
    .await
    {
        (Answer::Done, body) => Ok(taken(&body)),
        (answer, _) => Err(answer),
    }
}

/// The replies and added messages in a take's answer. A website without
/// added messages sends none.
fn taken(body: &Value) -> Taken {
    let replies = body["replies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|reply| {
            let text = reply["text"].as_str()?.trim();
            (!text.is_empty()).then(|| Reply {
                id: reply["id"].as_str().unwrap_or_default().to_owned(),
                text: text.to_owned(),
            })
        })
        .collect();
    let added = body["continued"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|message| {
            let user = match message["role"].as_str()? {
                "user" => true,
                "assistant" => false,
                _ => return None,
            };
            let text = message["text"].as_str()?.trim();
            (!text.is_empty()).then(|| Added {
                user,
                text: text.to_owned(),
            })
        })
        .collect();
    Taken {
        replies,
        added,
        asks: asks(body),
    }
}

/// Delete one chat from the website now, waiting at most a few seconds
/// (`coder sessions delete`). False when it has to wait for later.
#[must_use]
pub fn delete_now(saved: &Saved, session: &str) -> bool {
    let Some(http) = client() else {
        return false;
    };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return false;
    };
    runtime.block_on(async {
        matches!(
            tokio::time::timeout(Duration::from_secs(10), delete(&http, saved, session)).await,
            Ok(Answer::Done | Answer::Unknown)
        )
    })
}

/// Work for the background sender.
#[derive(Debug)]
pub enum Job {
    Upload {
        session: String,
        upload: Upload,
    },
    Status {
        session: String,
        working: bool,
    },
    Delete {
        session: String,
    },
    /// Check in as this computer every [`CHECK_IN_EVERY`] and report
    /// chats with replies waiting ([`Event::Waiting`]); `None` stops.
    Listen {
        computer: Option<String>,
    },
    /// Take the replies waiting in this chat ([`Event::Replies`]).
    Take {
        session: String,
    },
    /// Send back what an ask brought (#11185): the bytes, or why the
    /// computer couldn't.
    Capture {
        session: String,
        ask: String,
        result: Result<Vec<u8>, String>,
    },
    /// What runs on this computer now, for the phone (#11165): reported
    /// at once when it changed, then again every [`activity::every`];
    /// actions from the phone come back as [`Event::Commands`]. `None`
    /// stops reporting.
    Activity {
        computer: String,
        items: Option<Vec<activity::Item>>,
    },
}

/// What the background sender reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The website has this version of the chat.
    Saved { session: String, digest: String },
    /// The chat is gone from the website after a [`Job::Delete`].
    Removed { session: String },
    /// The chat was deleted on the website: delete it here too.
    Gone { session: String },
    /// The account has no room for more chats.
    Full,
    /// The sign-in no longer works; the sender stopped.
    SignedOut,
    /// The website refused this chat for good.
    Refused { session: String, message: String },
    /// These chats have replies from the website waiting.
    Waiting { sessions: Vec<String> },
    /// The website says this computer's choice is this (#11089).
    Chosen { choice: Choice },
    /// The answer to a [`Job::Take`]: the replies taken from the website
    /// for this chat, oldest first, and the messages added there while
    /// this computer was offline (none when there was nothing to take).
    Replies {
        session: String,
        replies: Vec<Reply>,
        added: Vec<Added>,
    },
    /// Actions sent from the phone for this computer's running work
    /// (#11165), each handed out once.
    Commands { commands: Vec<activity::Command> },
    /// Screenshots and files asked for on the website in this chat
    /// (#11185), taken with its replies, each handed out once.
    Asks { session: String, asks: Vec<Ask> },
}

/// The background sender: one thread, its own runtime, quiet retries.
pub struct Worker {
    jobs: mpsc::Sender<Job>,
    events: mpsc::Receiver<Event>,
}

impl Worker {
    /// Start sending as `saved`.
    #[must_use]
    pub fn start(saved: Saved) -> Self {
        let (jobs, inbox) = mpsc::channel();
        let (outbox, events) = mpsc::channel();
        std::thread::spawn(move || run(&saved, &inbox, &outbox));
        Self { jobs, events }
    }

    /// Queue work. A newer upload or status for the same chat replaces an
    /// older one still waiting.
    pub fn send(&self, job: Job) {
        let _ = self.jobs.send(job);
    }

    /// A handle another thread can queue work with.
    #[must_use]
    pub fn sender(&self) -> mpsc::Sender<Job> {
        self.jobs.clone()
    }

    /// What happened since the last call.
    pub fn drain(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

#[derive(Default)]
struct Queue {
    uploads: BTreeMap<String, Upload>,
    statuses: BTreeMap<String, bool>,
    deletes: BTreeSet<String>,
    takes: BTreeSet<String>,
    /// What asks brought, to send back (#11185).
    captures: Vec<(String, String, Result<Vec<u8>, String>)>,
    /// The computer to check in as, when listening for replies.
    computer: Option<String>,
    /// The running work to report, and for which computer (#11165).
    activity: Option<(String, Vec<activity::Item>)>,
    /// The running work changed since the last report.
    activity_changed: bool,
}

impl Queue {
    fn add(&mut self, job: Job) {
        match job {
            Job::Upload { session, upload } => {
                self.deletes.remove(&session);
                self.uploads.insert(session, upload);
            }
            Job::Status { session, working } => {
                self.statuses.insert(session, working);
            }
            Job::Delete { session } => {
                self.uploads.remove(&session);
                self.statuses.remove(&session);
                self.takes.remove(&session);
                self.deletes.insert(session);
            }
            Job::Listen { computer } => self.computer = computer,
            Job::Take { session } => {
                self.takes.insert(session);
            }
            Job::Capture {
                session,
                ask,
                result,
            } => self.captures.push((session, ask, result)),
            Job::Activity { computer, items } => {
                let next = items.map(|items| (computer, items));
                if next != self.activity {
                    self.activity_changed = next.is_some();
                    self.activity = next;
                }
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.uploads.is_empty()
            && self.statuses.is_empty()
            && self.deletes.is_empty()
            && self.takes.is_empty()
            && self.captures.is_empty()
            && !self.activity_changed
    }
}

fn run(saved: &Saved, inbox: &mpsc::Receiver<Job>, outbox: &mpsc::Sender<Event>) {
    let (Some(http), Ok(runtime)) = (
        client(),
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build(),
    ) else {
        return;
    };
    let mut queue = Queue::default();
    let mut check_at = Instant::now();
    let mut listen_at = Instant::now();
    let mut activity_at = Instant::now();
    let mut retry_at: Option<Instant> = None;
    let mut backoff = Duration::ZERO;
    loop {
        let now = Instant::now();
        let ready = retry_at.is_none_or(|at| at <= now);
        let mut timer = if queue.computer.is_some() {
            check_at.min(listen_at)
        } else {
            check_at
        };
        if queue.activity.is_some() {
            timer = timer.min(activity_at);
        }
        let due = if ready && !queue.is_empty() {
            now
        } else {
            retry_at.map_or(timer, |at| at.min(timer))
        };
        match inbox.recv_timeout(due.saturating_duration_since(now)) {
            Ok(job) => {
                queue.add(job);
                while let Ok(job) = inbox.try_recv() {
                    queue.add(job);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        if retry_at.is_some_and(|at| at > Instant::now()) {
            continue;
        }
        let outcome = runtime.block_on(round(
            &http,
            saved,
            &mut queue,
            (&mut check_at, &mut listen_at, &mut activity_at),
            outbox,
        ));
        match outcome {
            Round::SignedOut => {
                let _ = outbox.send(Event::SignedOut);
                return;
            }
            Round::Failed => {
                backoff = (backoff * 2).clamp(Duration::from_secs(5), Duration::from_secs(120));
                retry_at = Some(Instant::now() + backoff);
            }
            Round::Done => {
                backoff = Duration::ZERO;
                retry_at = None;
            }
        }
    }
}

enum Round {
    Done,
    Failed,
    SignedOut,
}

/// Send what is queued: heartbeats first, then deletes, then uploads.
async fn round(
    http: &reqwest::Client,
    saved: &Saved,
    queue: &mut Queue,
    (check_at, listen_at, activity_at): (&mut Instant, &mut Instant, &mut Instant),
    outbox: &mpsc::Sender<Event>,
) -> Round {
    let mut failed = false;
    // Running work for the phone (#11165).
    if let Some((computer, items)) = queue.activity.clone()
        && (queue.activity_changed || Instant::now() >= *activity_at)
    {
        queue.activity_changed = false;
        *activity_at = Instant::now() + activity::every(&items);
        match activity::report(http, saved, &computer, &items).await {
            Ok(commands) => {
                if !commands.is_empty() {
                    let _ = outbox.send(Event::Commands { commands });
                }
            }
            Err(Answer::SignedOut) => return Round::SignedOut,
            // A website without the route: try again much later.
            Err(Answer::Unknown) => *activity_at = Instant::now() + CHECK_EVERY,
            // A missed report only leaves the phone's list a little old.
            Err(_) => {}
        }
    }
    // Listening for replies typed on the website (#11048).
    if let Some(computer) = queue.computer.clone()
        && Instant::now() >= *listen_at
    {
        *listen_at = Instant::now() + CHECK_IN_EVERY;
        match check_in(http, saved, &computer).await {
            Ok((sessions, chosen)) => {
                if !sessions.is_empty() {
                    let _ = outbox.send(Event::Waiting { sessions });
                }
                if let Some(choice) = chosen {
                    let _ = outbox.send(Event::Chosen { choice });
                }
            }
            Err(Answer::SignedOut) => return Round::SignedOut,
            // Missed check-ins only make the website say this computer
            // is offline until the next one.
            Err(_) => {}
        }
    }
    for session in std::mem::take(&mut queue.takes) {
        match take(http, saved, &session).await {
            Ok(Taken {
                replies,
                added,
                asks,
            }) => {
                if !asks.is_empty() {
                    let _ = outbox.send(Event::Asks {
                        session: session.clone(),
                        asks,
                    });
                }
                let _ = outbox.send(Event::Replies {
                    session,
                    replies,
                    added,
                });
            }
            Err(Answer::SignedOut) => return Round::SignedOut,
            Err(Answer::Deleted) => {
                let _ = outbox.send(Event::Gone { session });
            }
            Err(Answer::Retry) => {
                failed = true;
                queue.takes.insert(session);
            }
            // Nothing to take after all.
            Err(_) => {
                let _ = outbox.send(Event::Replies {
                    session,
                    replies: Vec::new(),
                    added: Vec::new(),
                });
            }
        }
    }
    for (session, ask, result) in std::mem::take(&mut queue.captures) {
        match send_capture(http, saved, &session, &ask, &result).await {
            Answer::SignedOut => return Round::SignedOut,
            Answer::Retry => {
                failed = true;
                queue.captures.push((session, ask, result));
            }
            // Taken, answered already, or the chat is gone: done either way.
            _ => {}
        }
    }
    for (session, working) in std::mem::take(&mut queue.statuses) {
        match status(http, saved, &session, working).await {
            Answer::SignedOut => return Round::SignedOut,
            Answer::Retry => {
                failed = true;
                queue.statuses.entry(session).or_insert(working);
            }
            _ => {}
        }
    }
    for session in std::mem::take(&mut queue.deletes) {
        match delete(http, saved, &session).await {
            Answer::SignedOut => return Round::SignedOut,
            Answer::Retry => {
                failed = true;
                queue.deletes.insert(session);
            }
            _ => {
                let _ = outbox.send(Event::Removed { session });
            }
        }
    }
    for (session, upload) in std::mem::take(&mut queue.uploads) {
        // A newer version that arrived meanwhile waits for the next round.
        match put(http, saved, &session, &upload).await {
            Answer::Done => {
                let _ = outbox.send(Event::Saved {
                    session,
                    digest: upload.digest,
                });
            }
            Answer::Deleted => {
                let _ = outbox.send(Event::Gone { session });
            }
            Answer::Full => {
                let _ = outbox.send(Event::Full);
            }
            Answer::SignedOut => return Round::SignedOut,
            Answer::Refused(message) => {
                let _ = outbox.send(Event::Refused { session, message });
            }
            Answer::Unknown | Answer::Retry => {
                failed = true;
                queue.uploads.entry(session).or_insert(upload);
            }
        }
    }
    if Instant::now() >= *check_at {
        match deleted_on_site(http, saved).await {
            Ok(sessions) => {
                *check_at = Instant::now() + CHECK_EVERY;
                for session in sessions {
                    let _ = outbox.send(Event::Gone { session });
                }
            }
            Err(Answer::SignedOut) => return Round::SignedOut,
            Err(_) => {
                *check_at = Instant::now() + CHECK_EVERY / 5;
            }
        }
    }
    if failed { Round::Failed } else { Round::Done }
}

#[cfg(test)]
mod tests;
