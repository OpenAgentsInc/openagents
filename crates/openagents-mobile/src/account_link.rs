//! The phone on the person's openagents.com account (#11107, #11165).
//!
//! **Sign in.** The device-code flow Coder uses (`docs/auth/README.md`,
//! "Device sign-in"): `POST /v1/device/code` with `app: "OpenAgents"` and the
//! phone's name, then `POST /v1/device/token` at the site's interval until the
//! person approves the code on `openagents.com/device` (typed, opened from
//! the phone, or scanned from the QR the phone shows). Only accounts the
//! site lets sign in can approve one, so the owner-only allowlist holds
//! here too. The answer is an ordinary app session (`sess_…`): Rust keeps it
//! in memory, hands it to the host once ([`Packet::store`]) for the
//! platform's protected store (Keychain, or a Keystore-encrypted file), and
//! gets it back from the host at launch ([`Action::Session`]). It never
//! reaches a log line, a `Debug` string, or the view.
//!
//! **One set of chats.** `GET /v1/threads` lists the account's web chats,
//! the terminal chats Coder synced (by computer), and the chats other
//! phones synced; `GET /v1/threads/{id}` reads one, and
//! `POST /v1/threads/{id}/messages` replies. A reply to a terminal chat
//! waits on the website until Coder on that computer takes it, the same
//! path as a reply typed on the website (#11048). The phone asks once where
//! its own chats live ("Sync all my chats" or "Keep chats on this phone",
//! the per-device record `/v1/computers/{name}/sync`); with "all", each
//! phone chat uploads like Coder's (`PUT /v1/threads/synced/phone-{id}`),
//! screened for credential shapes first.
//!
//! Sign-in and the upload ask the `/v1` path first (#11158); a site from
//! before that serves them only at `/device/*` and `/coder/sessions/*`,
//! asked when the `/v1` path answers `404`.
//!
//! **Photos.** A reply to a web chat can carry up to four photos (#11174):
//! **Add photo** asks the host for its picker ([`Link::take_pick`]), the
//! picked image joins the open chat's draft ([`Link::attach`]), and the
//! send uploads each one (`POST /v1/threads/{id}/files`) before the reply
//! names them by id (`files`). The website keeps them with the chat, and
//! the answer reads them.
//!
//! **Projects** (#11558). The Projects screen lists `GET /v1/projects`
//! (each with zero or more repositories, one primary), makes one from a
//! name alone (`POST /v1/projects {name}`), and a tap on a project starts
//! a chat inside it (`POST /v1/threads` with `project`, #11542).
//!
//! **Supervise.** `GET /v1/agents` lists what Coder runs on each computer
//! (status, elapsed, cost, a pending question), and
//! `POST /v1/agents/actions` approves, denies, stops, or messages one. The
//! `OpenAgents Cloud` board's runs (Solver in the cloud among them,
//! #11561) take Stop only, so they show no Message. A
//! change to finished, failed, or asking becomes a notice the host shows
//! as a local notification ([`Packet::notify`]).
//!
//! **Memory** (#11182). `GET /coder/memory` reads the notes Coder keeps
//! about the person on their account (from computers with sync on); the
//! ones that apply everywhere go with each phone chat turn
//! ([`Link::memory_notes`]), as the web chat sends them. The Memory screen
//! lists them; a note opens to change what it says
//! (`PUT /coder/memory/{id}`) or delete it (`DELETE /coder/memory/{id}`),
//! and a new note is `PUT /coder/memory/new`.
//!
//! **Scheduled prompts** (#11177). `GET /v1/schedules` reads the
//! person's scheduled prompts (each runs on one of their computers, as a
//! background rule there), the computers that run Coder, and those
//! computers' chats. The Scheduled prompts screen lists them with Pause or
//! Resume (`POST /v1/schedules/{id}/pause`) and Delete
//! (`DELETE /v1/schedules/{id}`); Add makes one
//! (`PUT /v1/schedules/new`): the prompt, the computer, when (every
//! day, weekdays, or weekends at a time, or every few hours), and where it
//! goes (a new Coder run, or one of that computer's chats). The computer
//! picks the change up at its next sync and runs it, even while Coder is
//! closed there.
//!
//! Everything polls with backoff: faster while a screen that needs it
//! shows, slower in the background, never while signed out.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The production website, where release builds sign in.
pub const PRODUCTION: &str = "https://openagents.com";
/// The app name the approval page shows ("Sign in to OpenAgents on …").
pub const APP_NAME: &str = "OpenAgents";
/// A phone chat's session id on the account: `phone-` and the chat's id.
pub const PHONE_SESSION: &str = "phone-";
/// The `GET /v1/agents` board that lists the account's cloud work runs,
/// Solver in the cloud's among them (#11561); they take Stop only.
pub const CLOUD_BOARD: &str = "OpenAgents Cloud";
/// The board that lists the person's environment runs (#11307).
pub const ENVIRONMENTS_BOARD: &str = "Environments";

/// Whether a `GET /v1/agents` board is one of the person's computers,
/// rather than a list of runs elsewhere.
#[must_use]
pub fn is_computer(board: &str) -> bool {
    board != CLOUD_BOARD && board != ENVIRONMENTS_BOARD
}

/// Who an action's answer names: Coder on a computer, or the run itself
/// on a board that isn't a computer.
fn runner(board: &str) -> String {
    if is_computer(board) {
        format!("Coder on {board}")
    } else {
        "The run".to_owned()
    }
}
/// What a message that looked like it held a credential says instead
/// (`coder_sync::LEFT_OUT`).
pub const LEFT_OUT: &str = "(Left out: this looked like it held a password or key.)";
/// The longest message a phone chat uploads; longer ones are cut.
const MAX_UPLOAD_TEXT: usize = 64 * 1024;
/// The most messages one phone chat uploads (the newest).
const MAX_UPLOAD_MESSAGES: usize = 1000;
/// The most phone chats one pass uploads.
const UPLOADS_PER_PASS: usize = 8;
/// The longest reply typed here, in bytes (the website's own bound).
pub const MAX_REPLY_BYTES: usize = 16 * 1024;
/// The most notices kept for the host at once.
const MAX_NOTIFY: usize = 16;
/// How often the memory notes are read while the Memory screen isn't up.
const MEMORY_EVERY: u64 = 120;
/// The longest memory note, as the website keeps it.
pub const MAX_NOTE_BYTES: usize = 8 * 1024;
/// The longest note name, in characters.
const MAX_NOTE_NAME: usize = 80;

/// One memory note on the account, as `GET /coder/memory` lists it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Note {
    pub id: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub project_name: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub updated: u64,
    #[serde(default)]
    pub deleted: bool,
}

impl Note {
    /// Whether the note applies everywhere (not to one project).
    #[must_use]
    pub fn everywhere(&self) -> bool {
        self.scope == "user"
    }
}

/// What a person reads for a note's kind.
#[must_use]
pub fn kind_label(kind: Option<&str>) -> &'static str {
    match kind {
        Some("user") => "About you",
        Some("feedback") => "How you like things done",
        Some("project") => "About a project",
        Some("reference") => "Where to look",
        _ => "Note",
    }
}
/// The most photos one reply carries (the website's own bound).
pub const MAX_PHOTOS: usize = 4;

/// How often the scheduled prompts are read while their screen isn't up.
const SCHEDULES_EVERY: u64 = 300;
/// The longest scheduled prompt, as the website keeps it.
pub const MAX_SCHEDULE_PROMPT: usize = 4000;
/// Monday to Friday, 0 being Sunday.
pub const WEEKDAYS: [u8; 5] = [1, 2, 3, 4, 5];
/// Saturday and Sunday.
pub const WEEKENDS: [u8; 2] = [0, 6];

/// One scheduled prompt on the account, as `GET /v1/schedules` lists it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Schedule {
    pub id: String,
    #[serde(default)]
    pub computer: String,
    #[serde(default)]
    pub prompt: String,
    /// 0 Sunday to 6 Saturday; empty is every day. Only with `time`.
    #[serde(default)]
    pub days: Vec<u8>,
    /// `HH:MM`, 24-hour, on the computer's clock.
    #[serde(default)]
    pub time: Option<String>,
    #[serde(default)]
    pub every_secs: Option<u64>,
    /// The Coder chat it posts in; a new Coder run when unset.
    #[serde(default)]
    pub chat: Option<String>,
    #[serde(default)]
    pub chat_title: Option<String>,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub updated: u64,
    #[serde(default)]
    pub deleted: bool,
}

impl Schedule {
    /// When it runs, in a few words.
    #[must_use]
    pub fn when(&self) -> String {
        when_words(&self.days, self.time.as_deref(), self.every_secs)
    }

    /// Where its answer goes.
    #[must_use]
    pub fn goes(&self) -> String {
        match (&self.chat, &self.chat_title) {
            (Some(_), Some(title)) if !title.trim().is_empty() => format!("Posts in {title}"),
            (Some(_), _) => "Posts in a chat".into(),
            (None, _) => "Starts a new Coder run".into(),
        }
    }
}

/// "Every day at 09:00", "Weekdays at 09:00", "Every 2 hours".
#[must_use]
pub fn when_words(days: &[u8], time: Option<&str>, every_secs: Option<u64>) -> String {
    if let Some(time) = time {
        let mut sorted = days.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        return if sorted.is_empty() || sorted.len() == 7 {
            format!("Every day at {time}")
        } else if sorted == WEEKDAYS {
            format!("Weekdays at {time}")
        } else if sorted == WEEKENDS {
            format!("Weekends at {time}")
        } else {
            const NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
            let names: Vec<&str> = sorted
                .iter()
                .filter_map(|day| NAMES.get(usize::from(*day)).copied())
                .collect();
            format!("{} at {time}", names.join(", "))
        };
    }
    match every_secs.unwrap_or(0) {
        0 => "Not scheduled".into(),
        3600 => "Every hour".into(),
        secs if secs % 3600 == 0 => format!("Every {} hours", secs / 3600),
        60 => "Every minute".into(),
        secs if secs % 60 == 0 => format!("Every {} minutes", secs / 60),
        secs => format!("Every {secs} seconds"),
    }
}

/// One of a computer's chats a scheduled prompt can post in.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct ScheduleChat {
    pub session: String,
    #[serde(default)]
    pub computer: String,
    #[serde(default)]
    pub title: String,
}

/// How often a new scheduled prompt repeats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    #[default]
    Daily,
    Weekdays,
    Weekends,
    /// Every this many hours.
    Hours(u64),
}

/// The new scheduled prompt being made: everything but its words, which
/// the composer sends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduleDraft {
    pub computer: String,
    pub repeat: Repeat,
    /// The time of day, in minutes after midnight.
    pub minutes: u32,
    /// The chat it posts in; a new Coder run when unset.
    pub chat: Option<String>,
}

impl Default for ScheduleDraft {
    fn default() -> Self {
        Self {
            computer: String::new(),
            repeat: Repeat::Daily,
            minutes: 9 * 60,
            chat: None,
        }
    }
}

impl ScheduleDraft {
    /// `HH:MM`.
    #[must_use]
    pub fn time(&self) -> String {
        format!("{:02}:{:02}", self.minutes / 60, self.minutes % 60)
    }

    /// The days, time, and interval the website keeps.
    #[must_use]
    pub fn when(&self) -> (Vec<u8>, Option<String>, Option<u64>) {
        match self.repeat {
            Repeat::Daily => (vec![], Some(self.time()), None),
            Repeat::Weekdays => (WEEKDAYS.to_vec(), Some(self.time()), None),
            Repeat::Weekends => (WEEKENDS.to_vec(), Some(self.time()), None),
            Repeat::Hours(hours) => (vec![], None, Some(hours.max(1) * 3600)),
        }
    }
}

/// The signed-in account. The token never appears in `Debug`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// The site this session belongs to.
    pub origin: String,
    pub account: String,
    pub label: String,
    /// When the session stops working, in Unix seconds.
    pub expires_at: u64,
    token: String,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("origin", &self.origin)
            .field("account", &self.account)
            .field("label", &self.label)
            .field("token", &"[redacted]")
            .finish()
    }
}

impl Session {
    #[cfg(test)]
    pub(crate) fn for_test(origin: &str, token: &str) -> Self {
        Self {
            origin: origin.into(),
            account: "acct_test".into(),
            label: "Test".into(),
            expires_at: u64::MAX,
            token: token.into(),
        }
    }

    fn live(&self, now: u64) -> bool {
        self.token.starts_with("sess_") && self.expires_at > now
    }
}

/// Chats fetched per page of the list (the website's page size).
pub const THREADS_PAGE: usize = 200;

/// One chat on the account, as `GET /v1/threads` lists it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Thread {
    pub id: String,
    pub title: String,
    pub line: Option<String>,
    /// `web`, `terminal`, or `phone`.
    pub surface: String,
    pub computer: Option<String>,
    pub session: Option<String>,
    pub updated_unix: u64,
    pub working: bool,
    pub online: bool,
    pub can_reply: bool,
    pub pinned: bool,
    /// The project (`prj_…`) the chat is filed under (#11541).
    pub project: Option<String>,
}

/// One project on the account, as `GET /v1/projects` lists it (#11558):
/// a name, and zero or more repositories with one primary.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Project {
    pub id: String,
    pub name: String,
    /// The primary repository (`owner/name`), when it has any.
    pub primary: Option<String>,
    pub repositories: usize,
    /// How many of the account's chats are filed under it.
    pub chats: u64,
}

impl Project {
    fn from_value(value: &Value) -> Option<Self> {
        let repositories = value["repositories"].as_array();
        let primary = repositories
            .and_then(|repos| {
                repos
                    .iter()
                    .find(|r| r["primary"].as_bool() == Some(true))
                    .or_else(|| repos.first())
            })
            .and_then(|r| r["repository"].as_str())
            .or_else(|| value["repository"].as_str())
            .map(str::to_owned);
        Some(Self {
            id: value["id"].as_str()?.to_owned(),
            name: value["name"].as_str()?.to_owned(),
            repositories: repositories.map_or(usize::from(primary.is_some()), Vec::len),
            primary,
            chats: value["counts"]["threads"].as_u64().unwrap_or(0),
        })
    }
}

/// How many views before the newest still take a tap.
const EARLIER_VIEWS: usize = 4;

/// The most characters in a project's name.
pub const MAX_PROJECT_NAME: usize = 80;

/// A computer (or phone) on the account.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Computer {
    pub name: String,
    pub online: bool,
    pub seen_unix: Option<u64>,
    /// `all`, `local`, or none when it hasn't been asked.
    pub sync: Option<String>,
}

/// One message of an open chat.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Message {
    /// `user`, `assistant`, or `tool`.
    pub role: String,
    pub text: String,
    /// The files attached to a sent message.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<MessageFile>,
}

/// A file attached to a message.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MessageFile {
    pub id: String,
    pub name: String,
    /// `png`, `jpeg`, `gif`, `webp`, `pdf`, or `text`.
    pub kind: String,
    /// Its size in bytes.
    pub size: u64,
}

/// A question an agent asks before it goes on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Question {
    pub id: String,
    pub text: String,
}

/// One piece of work Coder runs on a computer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Item {
    pub id: String,
    /// `chat` or `agent`.
    pub kind: String,
    pub title: String,
    pub engine: Option<String>,
    /// `working`, `asking`, `done`, `failed`, or `stopped`.
    pub status: String,
    pub started_unix: u64,
    pub finished_unix: Option<u64>,
    pub cost_usd: Option<f64>,
    pub tokens: Option<u64>,
    /// The Coder session, when its chat syncs to the account.
    pub session: Option<String>,
    pub question: Option<Question>,
    pub line: Option<String>,
    /// While a usage limit pauses it: `your_key` (Continue on your key)
    /// and `openagents` (Continue with OpenAgents), the ones it offers
    /// now (#11295).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
}

impl Item {
    pub(crate) fn running(&self) -> bool {
        matches!(self.status.as_str(), "working" | "asking")
    }
}

/// What one computer runs, as `GET /v1/agents` lists it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Agents {
    pub name: String,
    pub online: bool,
    pub updated_unix: u64,
    pub items: Vec<Item>,
}

/// The website's answer to one call.
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub body: Value,
}

impl Reply {
    fn message(&self, fallback: &str) -> String {
        self.body["error"]["message"]
            .as_str()
            .or_else(|| self.body["error_description"].as_str())
            .filter(|m| !m.is_empty())
            .unwrap_or(fallback)
            .to_owned()
    }

    fn code(&self) -> &str {
        self.body["error"]["code"]
            .as_str()
            .or_else(|| self.body["error"].as_str())
            .unwrap_or_default()
    }
}

/// The future one call returns.
pub type Calling = Pin<Box<dyn Future<Output = Result<Reply, String>> + Send>>;

/// How the phone reaches the website: one JSON call. `token` is the bearer.
pub trait Http: Send + Sync {
    fn call(
        &self,
        method: &'static str,
        url: String,
        token: Option<String>,
        body: Option<Value>,
    ) -> Calling;

    /// `POST url` with `bytes` as the body (a photo for a chat, #11174).
    fn upload(&self, _url: String, _token: Option<String>, _bytes: Vec<u8>) -> Calling {
        Box::pin(async { Err("Photos can't be sent from here.".to_owned()) })
    }

    /// Open the event stream at `url` and answer when something changes
    /// there (the stream's second event; its first is the state on
    /// connecting), when it ends, or after five minutes. The reply's
    /// status is the stream's; `204` when it ended with no change.
    fn changed(&self, _url: String, _token: Option<String>) -> Calling {
        Box::pin(async { Err("Live updates aren't available here.".to_owned()) })
    }
}

/// How long one live-update stream is held open.
const STREAM_SECS: u64 = 300;

/// `method` at `path` on the site at `origin`, then at `older` when the
/// site doesn't serve `path` (a `404` that isn't the API's own `unknown`):
/// a site from before #11158 serves our apps' routes only at their older
/// paths (`/device/*`, `/coder/sessions/*`).
async fn call_v1(
    http: &dyn Http,
    method: &'static str,
    origin: &str,
    path: &str,
    older: &str,
    token: Option<String>,
    body: Option<Value>,
) -> Result<Reply, String> {
    let reply = http
        .call(
            method,
            format!("{origin}{path}"),
            token.clone(),
            body.clone(),
        )
        .await?;
    if reply.status == 404 && reply.code() != "unknown" {
        return http
            .call(method, format!("{origin}{older}"), token, body)
            .await;
    }
    Ok(reply)
}

/// The real client, over HTTPS.
pub struct Https(reqwest::Client);

impl Https {
    #[must_use]
    pub fn new() -> Self {
        Self(
            reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(25))
                .build()
                .unwrap_or_default(),
        )
    }
}

impl Default for Https {
    fn default() -> Self {
        Self::new()
    }
}

impl Http for Https {
    fn call(
        &self,
        method: &'static str,
        url: String,
        token: Option<String>,
        body: Option<Value>,
    ) -> Calling {
        let client = self.0.clone();
        Box::pin(async move {
            let method = reqwest::Method::from_bytes(method.as_bytes())
                .map_err(|_| "That request isn't valid.".to_owned())?;
            let mut request = client.request(method, &url);
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            if let Some(body) = body {
                request = request.json(&body);
            }
            let response = request
                .send()
                .await
                .map_err(|_| "Couldn't reach openagents.com. Check your connection.".to_owned())?;
            let status = response.status().as_u16();
            let bytes = response
                .bytes()
                .await
                .map_err(|_| "Couldn't reach openagents.com. Check your connection.".to_owned())?;
            let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            Ok(Reply { status, body })
        })
    }

    fn changed(&self, url: String, token: Option<String>) -> Calling {
        let client = self.0.clone();
        Box::pin(async move {
            let mut request = client
                .get(&url)
                .timeout(Duration::from_secs(STREAM_SECS + 30))
                .header("accept", "text/event-stream");
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            let mut response = request
                .send()
                .await
                .map_err(|_| "Couldn't reach openagents.com.".to_owned())?;
            let status = response.status().as_u16();
            if status != 200 {
                return Ok(Reply {
                    status,
                    body: Value::Null,
                });
            }
            let mut events = 0usize;
            let mut tail = String::new();
            while let Ok(Some(chunk)) = response.chunk().await {
                tail.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(at) = tail.find("\n\n") {
                    let event: String = tail.drain(..at + 2).collect();
                    if event.lines().any(|line| line.starts_with("event:")) {
                        events += 1;
                        if events >= 2 {
                            return Ok(Reply {
                                status: 200,
                                body: Value::Null,
                            });
                        }
                    }
                }
                if tail.len() > 64 * 1024 {
                    tail.clear();
                }
            }
            Ok(Reply {
                status: 204,
                body: Value::Null,
            })
        })
    }

    fn upload(&self, url: String, token: Option<String>, bytes: Vec<u8>) -> Calling {
        let client = self.0.clone();
        Box::pin(async move {
            let mut request = client
                .post(&url)
                .timeout(Duration::from_secs(120))
                .header("content-type", "application/octet-stream")
                .body(bytes);
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            let response = request
                .send()
                .await
                .map_err(|_| "Couldn't reach openagents.com. Check your connection.".to_owned())?;
            let status = response.status().as_u16();
            let bytes = response
                .bytes()
                .await
                .map_err(|_| "Couldn't reach openagents.com. Check your connection.".to_owned())?;
            let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            Ok(Reply { status, body })
        })
    }
}

/// Where sign-in stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SignIn {
    #[default]
    Idle,
    /// Asking the site for a code.
    Starting,
    /// Waiting for the person to approve `user_code`.
    Waiting {
        device_code: String,
        user_code: String,
        /// The page to approve on, with the code in it.
        page: String,
        interval: u64,
        deadline: u64,
    },
    Failed(String),
}

/// Which screen of the account surface shows.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    /// Sign in, the account, and where this phone's chats live.
    #[default]
    Account,
    /// The account's chats.
    Chats,
    /// One chat.
    Chat,
    /// What runs on the computers.
    Running,
    /// Write a message to one running agent.
    Message,
    /// The memory notes on the account (#11182).
    Memory,
    /// One memory note, to change or delete, or a new one.
    Note,
    /// The scheduled prompts on the account (#11177).
    Schedules,
    /// A new scheduled prompt.
    NewSchedule,
    /// The account's projects, and a new one by name (#11558).
    Projects,
}

/// An open chat.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Open {
    pub id: String,
    pub loaded: bool,
    pub thread: Option<Thread>,
    pub messages: Vec<Message>,
    pub earlier: usize,
    /// Replies sent here or on the website that Coder hasn't taken yet.
    pub waiting: usize,
    /// Replies sent from this phone that the next read hasn't shown yet,
    /// each with the files it carried.
    pub sent: Vec<Message>,
    pub sending: bool,
    pub error: Option<String>,
    /// Photos for the next reply: each one's name and bytes, as the host's
    /// picker read them ([`Link::attach`]).
    pub photos: Vec<(String, Vec<u8>)>,
    /// A chat this phone is starting on openagents.com (#11349): its id is
    /// the first message's request id, and the first send makes it
    /// (`POST /v1/threads`); false once the website has it.
    pub new: bool,
    /// The project a new chat starts in (`prj_…`, #11542).
    pub project: Option<String>,
}

/// A local notification for the host to show once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Notify {
    /// Stable per change, so the host replaces rather than repeats it.
    pub id: String,
    pub title: String,
    pub body: String,
    pub computer: String,
    pub item: String,
    /// A question to approve or deny from the notification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
}

/// One account chat in the drawer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DrawerRow {
    pub id: String,
    pub title: String,
    pub detail: String,
}

/// What the host reads from the account surface each packet.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Packet {
    pub signed_in: bool,
    /// The account's name, for Settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The site, when it isn't openagents.com (a test build on staging).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    /// The surface's Rust Native view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<Value>,
    /// The sign-in QR code (the approval page with the code), while it
    /// shows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qr: Option<crate::app::QrModules>,
    /// The account's newest chats, for the drawer.
    pub drawer: Vec<DrawerRow>,
    /// How many agents run or ask now, for the drawer's **Running**.
    pub running: usize,
    /// How many ask a question.
    pub asking: usize,
    /// Keep this session in the protected store (sent once).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<Session>,
    /// Remove the session from the protected store (sent once).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub forget: bool,
    /// Local notifications to show (sent once).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notify: Vec<Notify>,
    /// Open this page in the browser (sent once).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_url: Option<String>,
    /// Open the drawer: the top bar's menu button was tapped (sent once).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub menu: bool,
    /// The surface changes on its own: ask for packets.
    pub live: bool,
}

/// What the host asks of the account surface.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// At launch: this phone's name and the session the host kept.
    Hello {
        name: String,
        #[serde(default)]
        session: Option<String>,
    },
    /// A screen of the surface shows; `id` opens a chat.
    Show {
        screen: Screen,
        #[serde(default)]
        id: Option<String>,
    },
    /// No screen of the surface shows.
    Hide,
    /// The drawer opened or closed: its rows refresh while open.
    Drawer { open: bool },
    /// The host's background fetch: read once now, for notices.
    Refresh,
    /// A notification's button: approve or deny a question.
    Answer {
        computer: String,
        item: String,
        question: String,
        approve: bool,
    },
}

/// What a tap on the surface asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    SignIn,
    CancelSignIn,
    OpenPage,
    SignOut,
    Choose {
        all: bool,
    },
    Show {
        screen: Screen,
    },
    Open {
        id: String,
    },
    /// Start a chat on openagents.com from the phone, which takes photos
    /// and files with its first message (#11349).
    NewChat,
    Back,
    /// Open the host's drawer: the top bar's menu button (#11472).
    Menu,
    /// Ask the website for the next page of older chats.
    MoreThreads,
    Act {
        computer: String,
        item: String,
        action: String,
        question: Option<String>,
    },
    Message {
        computer: String,
        item: String,
    },
    /// Open a memory note to change or delete it.
    EditNote {
        id: String,
    },
    /// Write a new memory note.
    NewNote,
    /// Start a chat on openagents.com inside a project (#11542).
    ProjectChat {
        id: String,
    },
    /// Delete a memory note everywhere.
    DeleteNote {
        id: String,
    },
    /// Make a new scheduled prompt.
    NewSchedule,
    /// Pause or resume a scheduled prompt.
    PauseSchedule {
        id: String,
        paused: bool,
    },
    /// Delete a scheduled prompt.
    DeleteSchedule {
        id: String,
    },
    /// The computer a new scheduled prompt runs on.
    ScheduleComputer {
        name: String,
    },
    /// How often a new scheduled prompt runs.
    ScheduleRepeat {
        repeat: Repeat,
    },
    /// Move a new scheduled prompt's time by `minutes` (earlier when
    /// negative).
    ScheduleTime {
        minutes: i32,
    },
    /// Where a new scheduled prompt goes: one of the computer's chats, or
    /// a new Coder run when unset.
    ScheduleChat {
        session: Option<String>,
    },
    Retry,
    /// Add a photo to the open chat's reply: the host opens its picker.
    AddPhoto,
    /// Remove the photo at `index` from the open chat's reply.
    RemovePhoto {
        index: usize,
    },
    /// Open a GitHub page an agent row names (its issue, commit, or pull
    /// request, #11228) in the browser.
    OpenLink {
        url: String,
    },
}

/// The surface's state, shared with its background reads.
#[derive(Default)]
pub struct State {
    pub origin: String,
    pub name: String,
    pub session: Option<Session>,
    pub sign_in: SignIn,
    /// This phone's choice: `Some(true)` sync all, `Some(false)` keep
    /// here, `None` not asked (or not read yet).
    pub choice: Option<bool>,
    /// The choice was read from the website (so `None` means ask).
    pub choice_read: bool,
    pub threads: Vec<Thread>,
    pub computers: Vec<Computer>,
    pub threads_read: bool,
    /// How many chats the list asks the website for (`THREADS_PAGE` more
    /// each "Show older chats").
    pub threads_wanted: usize,
    /// The website has older chats than the list holds.
    pub threads_more: bool,
    /// The account's projects, id and name (`GET /v1/projects`; a project
    /// may have no repository, and only these two fields are read), to name
    /// the project a chat is filed under (#11541).
    pub projects: BTreeMap<String, String>,
    /// The account's projects in the website's order, for the Projects
    /// screen.
    pub project_list: Vec<Project>,
    /// The projects were read at least once.
    pub projects_read: bool,
    /// A new project is on its way.
    pub projects_busy: bool,
    pub agents: Vec<Agents>,
    pub agents_read: bool,
    pub screen: Screen,
    /// The screen on view was opened from **Your account**, so its top bar
    /// leads with a back button there rather than the menu (#11472).
    pub from_account: bool,
    /// The open chat was opened from **Your chats**, so its top bar leads
    /// with a back button to the list rather than the menu.
    pub from_list: bool,
    /// A screen of the surface shows.
    pub shown: bool,
    pub drawer: bool,
    pub active: bool,
    pub open: Option<Open>,
    /// The agent a message goes to: computer, item, title.
    pub messaging: Option<(String, String, String)>,
    /// The last line to show (a refusal, an action's result).
    pub notice: Option<String>,
    /// Errors in a row, for backoff.
    pub failures: u32,
    pub store: Option<Session>,
    pub forget: bool,
    pub notify: Vec<Notify>,
    /// Each item's last status (and question), to notice changes.
    pub seen: BTreeMap<(String, String), (String, Option<String>)>,
    /// Phone chats waiting to upload, by chat id: title and messages.
    pub uploads: BTreeMap<String, (String, Vec<Message>, u64)>,
    /// Per phone chat, the `updated` stamp last uploaded.
    pub uploaded: BTreeMap<String, u64>,
    /// Phone chats the website refused for good.
    pub refused: BTreeSet<String>,
    /// Bumped by every change, so the poller and the view see it.
    pub revision: u64,
    /// Actions already sent, by request id, so a double tap sends once.
    pub acted: BTreeSet<String>,
    /// The account's memory notes, newest first (#11182).
    pub memory: Vec<Note>,
    /// The notes were read at least once.
    pub memory_read: bool,
    /// When the notes were last read, in Unix seconds.
    pub memory_at: u64,
    /// The note the Note screen shows: its id, or empty for a new one.
    pub editing: Option<String>,
    /// A note's change or delete is on its way.
    pub memory_busy: bool,
    /// The account's scheduled prompts, newest first (#11177).
    pub schedules: Vec<Schedule>,
    /// The computers a scheduled prompt can run on, most recent first.
    pub schedule_computers: Vec<String>,
    /// The chats a scheduled prompt can post in.
    pub schedule_chats: Vec<ScheduleChat>,
    /// The scheduled prompts were read at least once.
    pub schedules_read: bool,
    /// When they were last read, in Unix seconds.
    pub schedules_at: u64,
    /// A scheduled prompt's change is on its way.
    pub schedules_busy: bool,
    /// The new scheduled prompt the NewSchedule screen makes.
    pub draft: Option<ScheduleDraft>,
}

impl State {
    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn signed(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.live(unix_now()))
    }

    /// How long until the next read: quick while a screen that needs it
    /// shows, slow in the background, slower after failures.
    fn interval(&self) -> Duration {
        let base = if !self.active {
            60
        } else if self.shown && matches!(self.screen, Screen::Chat | Screen::Running) {
            3
        } else if self.shown || self.drawer {
            5
        } else {
            20
        };
        let backoff = 2u64.saturating_pow(self.failures.min(5));
        Duration::from_secs((base * backoff).min(60))
    }
}

/// The account surface.
pub struct Link {
    pub(crate) state: Arc<Mutex<State>>,
    http: Arc<dyn Http>,
    runtime: Option<tokio::runtime::Handle>,
    nudge: Arc<tokio::sync::Notify>,
    /// The poller runs.
    polling: Arc<std::sync::atomic::AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
    store: Option<Arc<coder_computers::cache::Cache>>,
    /// The view last drawn, to resolve taps.
    pub(crate) view: Option<rust_native::ValidatedView<Intent>>,
    /// The views drawn just before `view`, newest first: a tap the host
    /// sent for one of them still counts, since a packet may redraw the
    /// surface between the host showing a view and the person's tap.
    earlier_views: std::collections::VecDeque<rust_native::ValidatedView<Intent>>,
    view_revision: u64,
    /// Bumped after each accepted send, so the composer clears.
    pub(crate) composer: u64,
    /// **Add photo** was tapped: the host opens its picker once.
    picking: bool,
    /// The top bar's menu button was tapped: the host opens its drawer.
    menu: bool,
}

/// What the encrypted store keeps: the choice and what was uploaded.
#[derive(Default, Serialize, Deserialize)]
struct Kept {
    #[serde(default)]
    choice: Option<bool>,
    #[serde(default)]
    uploaded: BTreeMap<String, u64>,
}

impl Link {
    /// The surface for `origin` (openagents.com in a release build).
    pub fn new(
        origin: &str,
        http: Arc<dyn Http>,
        runtime: Option<tokio::runtime::Handle>,
        wake: Arc<dyn Fn() + Send + Sync>,
        store: Option<coder_computers::cache::Cache>,
    ) -> Self {
        let store = store.map(Arc::new);
        let kept: Kept = store
            .as_ref()
            .and_then(|store| store.read("link").ok().flatten())
            .unwrap_or_default();
        let state = State {
            threads_wanted: THREADS_PAGE,
            origin: origin.trim_end_matches('/').to_owned(),
            name: "Phone".into(),
            choice: kept.choice,
            uploaded: kept.uploaded,
            active: true,
            ..State::default()
        };
        Self {
            state: Arc::new(Mutex::new(state)),
            http,
            runtime,
            nudge: Arc::new(tokio::sync::Notify::new()),
            polling: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            wake,
            store,
            view: None,
            earlier_views: std::collections::VecDeque::new(),
            view_revision: 0,
            composer: 0,
            picking: false,
            menu: false,
        }
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn keep(&self) {
        let Some(store) = &self.store else { return };
        let state = self.lock();
        let kept = Kept {
            choice: state.choice,
            uploaded: state.uploaded.clone(),
        };
        drop(state);
        let _ = store.write("link", &kept);
    }

    /// Whether a session is signed in.
    #[must_use]
    pub fn signed_in(&self) -> bool {
        self.lock().signed()
    }

    /// Whether this phone's chats go to the account.
    #[must_use]
    pub fn syncs(&self) -> bool {
        let state = self.lock();
        state.signed() && state.choice == Some(true)
    }

    /// The app moved to the foreground or the background.
    pub fn set_active(&self, active: bool) {
        self.lock().active = active;
        if active {
            self.nudge.notify_one();
        }
    }

    /// Carry out a host action.
    pub fn act(&mut self, action: Action) {
        match action {
            Action::Hello { name, session } => {
                let name: String = name.chars().filter(|c| !c.is_control()).take(64).collect();
                let session = session.and_then(|text| serde_json::from_str::<Session>(&text).ok());
                let mut state = self.lock();
                if !name.trim().is_empty() {
                    state.name = name.trim().to_owned();
                }
                if let Some(session) = session {
                    if session.origin == state.origin && session.live(unix_now()) {
                        state.session = Some(session);
                    } else {
                        // A session for another site, or an old one.
                        state.forget = true;
                    }
                }
                state.changed();
                drop(state);
                self.start();
            }
            Action::Show { screen, id } => {
                let mut state = self.lock();
                state.shown = true;
                // The drawer opened it: its top bar leads with the menu.
                state.from_account = false;
                state.from_list = false;
                if let Some(id) = id {
                    open_chat(&mut state, &id);
                    state.screen = Screen::Chat;
                } else {
                    state.screen = if state.signed() || screen == Screen::Account {
                        screen
                    } else {
                        Screen::Account
                    };
                }
                state.notice = None;
                state.changed();
                drop(state);
                self.nudge.notify_one();
            }
            Action::Hide => {
                let mut state = self.lock();
                state.shown = false;
                state.changed();
            }
            Action::Drawer { open } => {
                self.lock().drawer = open;
                if open {
                    self.nudge.notify_one();
                }
            }
            Action::Refresh => self.refresh_now(Duration::from_secs(20)),
            Action::Answer {
                computer,
                item,
                question,
                approve,
            } => {
                let action = if approve { "approve" } else { "deny" };
                self.send_action(computer, item, action.into(), Some(question), None);
            }
        }
    }

    /// Carry out a tap on the surface.
    pub fn tap(&mut self, intent: Intent) -> Option<String> {
        match intent {
            Intent::SignIn | Intent::Retry if !self.signed_in() => self.begin_sign_in(),
            Intent::MoreThreads => {
                let mut state = self.lock();
                state.threads_wanted += THREADS_PAGE;
                state.threads_read = false;
                drop(state);
                self.nudge.notify_one();
            }
            Intent::Retry => {
                let mut state = self.lock();
                state.failures = 0;
                state.notice = None;
                if let Some(open) = &mut state.open {
                    open.error = None;
                    open.loaded = false;
                }
                state.changed();
                drop(state);
                self.nudge.notify_one();
            }
            Intent::SignIn => {}
            Intent::CancelSignIn => {
                let mut state = self.lock();
                state.sign_in = SignIn::Idle;
                state.changed();
            }
            Intent::OpenPage => {
                if let SignIn::Waiting { page, .. } = &self.lock().sign_in {
                    return Some(page.clone());
                }
            }
            Intent::OpenLink { url } => {
                if crate::link_view::github_link(&url) {
                    return Some(url);
                }
            }
            Intent::SignOut => self.sign_out(),
            Intent::Choose { all } => self.choose(all),
            Intent::Show { screen } => {
                let mut state = self.lock();
                state.from_account = state.screen == Screen::Account;
                state.screen = screen;
                state.notice = None;
                state.changed();
                drop(state);
                self.nudge.notify_one();
            }
            Intent::Open { id } => {
                let mut state = self.lock();
                open_chat(&mut state, &id);
                state.from_list = state.screen == Screen::Chats;
                state.screen = Screen::Chat;
                state.changed();
                drop(state);
                self.nudge.notify_one();
            }
            Intent::Menu => self.menu = true,
            Intent::NewChat => {
                let mut state = self.lock();
                if state.signed() {
                    if state.screen != Screen::Chat {
                        state.from_list = state.screen == Screen::Chats;
                    }
                    state.open = Some(Open {
                        id: uuid::Uuid::new_v4().to_string(),
                        loaded: true,
                        new: true,
                        ..Open::default()
                    });
                    state.screen = Screen::Chat;
                    state.notice = None;
                    state.changed();
                }
            }
            Intent::Back => {
                let mut state = self.lock();
                state.screen = match state.screen {
                    Screen::Message => Screen::Running,
                    Screen::Chat => Screen::Chats,
                    Screen::Note => Screen::Memory,
                    Screen::NewSchedule => Screen::Schedules,
                    _ => Screen::Account,
                };
                state.editing = None;
                state.draft = None;
                if state.screen != Screen::Chat {
                    state.open = None;
                }
                state.messaging = None;
                state.notice = None;
                state.changed();
            }
            Intent::Act {
                computer,
                item,
                action,
                question,
            } => self.send_action(computer, item, action, question, None),
            Intent::EditNote { id } => {
                let mut state = self.lock();
                if state
                    .memory
                    .iter()
                    .any(|note| note.id == id && !note.deleted)
                {
                    state.editing = Some(id);
                    state.screen = Screen::Note;
                    state.notice = None;
                    state.changed();
                }
            }
            Intent::NewNote => {
                let mut state = self.lock();
                state.editing = Some(String::new());
                state.screen = Screen::Note;
                state.notice = None;
                state.changed();
            }
            Intent::DeleteNote { id } => self.delete_note(id),
            Intent::ProjectChat { id } => {
                let mut state = self.lock();
                if state.signed() && state.project_list.iter().any(|p| p.id == id) {
                    // Back goes to Your chats, where the new chat lists.
                    state.from_list = true;
                    state.open = Some(Open {
                        id: uuid::Uuid::new_v4().to_string(),
                        loaded: true,
                        new: true,
                        project: Some(id),
                        ..Open::default()
                    });
                    state.screen = Screen::Chat;
                    state.notice = None;
                    state.changed();
                }
            }
            Intent::NewSchedule => {
                let mut state = self.lock();
                let computer = state
                    .schedule_computers
                    .first()
                    .cloned()
                    .unwrap_or_default();
                state.draft = Some(ScheduleDraft {
                    computer,
                    ..ScheduleDraft::default()
                });
                state.screen = Screen::NewSchedule;
                state.notice = None;
                state.changed();
            }
            Intent::PauseSchedule { id, paused } => self.pause_schedule(id, paused),
            Intent::DeleteSchedule { id } => self.delete_schedule(id),
            Intent::ScheduleComputer { name } => {
                let mut state = self.lock();
                if !state.schedule_computers.contains(&name) {
                    return None;
                }
                if let Some(draft) = &mut state.draft
                    && draft.computer != name
                {
                    draft.computer = name;
                    // A chat belongs to one computer.
                    draft.chat = None;
                }
                state.changed();
            }
            Intent::ScheduleRepeat { repeat } => {
                let mut state = self.lock();
                if let Some(draft) = &mut state.draft {
                    draft.repeat = match repeat {
                        Repeat::Hours(hours) => Repeat::Hours(hours.clamp(1, 24)),
                        other => other,
                    };
                }
                state.changed();
            }
            Intent::ScheduleTime { minutes } => {
                let mut state = self.lock();
                if let Some(draft) = &mut state.draft {
                    let day = 24 * 60;
                    let moved = (i64::from(draft.minutes) + i64::from(minutes)).rem_euclid(day);
                    draft.minutes = u32::try_from(moved).unwrap_or(0);
                }
                state.changed();
            }
            Intent::ScheduleChat { session } => {
                let mut state = self.lock();
                let computer = state.draft.as_ref().map(|d| d.computer.clone());
                let known = session.as_ref().is_none_or(|session| {
                    state
                        .schedule_chats
                        .iter()
                        .any(|c| &c.session == session && Some(&c.computer) == computer.as_ref())
                });
                if known && let Some(draft) = &mut state.draft {
                    draft.chat = session;
                }
                state.changed();
            }
            Intent::Message { computer, item } => {
                let mut state = self.lock();
                let title = state
                    .agents
                    .iter()
                    .find(|c| c.name == computer)
                    .and_then(|c| c.items.iter().find(|i| i.id == item))
                    .map(|i| i.title.clone())
                    .unwrap_or_default();
                state.messaging = Some((computer, item, title));
                state.screen = Screen::Message;
                state.notice = None;
                state.changed();
            }
            Intent::AddPhoto => {
                let state = self.lock();
                if let Some(open) = &state.open
                    && takes_photos(open)
                    && open.photos.len() < MAX_PHOTOS
                {
                    drop(state);
                    self.picking = true;
                }
            }
            Intent::RemovePhoto { index } => {
                let mut state = self.lock();
                if let Some(open) = &mut state.open
                    && index < open.photos.len()
                {
                    open.photos.remove(index);
                    state.changed();
                }
            }
        }
        None
    }

    /// A composer's send: a reply to the open chat, or a message to the
    /// agent being messaged.
    pub fn input(&mut self, token: &str, value: &str) {
        let text = value.trim();
        if text.is_empty() {
            return;
        }
        if token.starts_with("link-note-") {
            self.save_note(text.to_owned());
            return;
        }
        if token.starts_with("link-schedule-") {
            self.save_schedule(text.to_owned());
            return;
        }
        if token.starts_with("link-project-") {
            self.create_project(text.to_owned());
            return;
        }
        if text.len() > MAX_REPLY_BYTES {
            let mut state = self.lock();
            state.notice = Some("That message is too long to send.".into());
            state.changed();
            return;
        }
        if token.starts_with("link-reply-") {
            self.reply(text.to_owned());
        } else if token.starts_with("link-message-") {
            let target = self.lock().messaging.clone();
            if let Some((computer, item, _)) = target {
                self.send_action(
                    computer,
                    item,
                    "message".into(),
                    None,
                    Some(text.to_owned()),
                );
            }
        }
    }

    fn spawn(&self, work: impl Future<Output = ()> + Send + 'static) {
        if let Some(runtime) = &self.runtime {
            runtime.spawn(work);
        }
    }

    /// Ask the site for a code, then wait for approval in the background.
    fn begin_sign_in(&mut self) {
        let (origin, name) = {
            let mut state = self.lock();
            if matches!(state.sign_in, SignIn::Starting | SignIn::Waiting { .. }) {
                return;
            }
            state.sign_in = SignIn::Starting;
            state.screen = Screen::Account;
            state.changed();
            (state.origin.clone(), state.name.clone())
        };
        (self.wake)();
        let http = self.http.clone();
        let state = self.state.clone();
        let wake = self.wake.clone();
        let nudge = self.nudge.clone();
        let polling = self.polling.clone();
        let runtime = self.runtime.clone();
        let poller = self.poller();
        self.spawn(async move {
            let started = call_v1(
                &*http,
                "POST",
                &origin,
                "/v1/device/code",
                "/device/code",
                None,
                Some(json!({"app": APP_NAME, "computer": name})),
            )
            .await;
            let waiting = match started {
                Ok(reply) if reply.status == 200 => {
                    let body = &reply.body;
                    let user_code = body["user_code"].as_str().unwrap_or_default().to_owned();
                    let device_code = body["device_code"].as_str().unwrap_or_default().to_owned();
                    let page = body["verification_uri_complete"]
                        .as_str()
                        .or_else(|| body["verification_uri"].as_str())
                        .unwrap_or_default()
                        .to_owned();
                    if user_code.is_empty() || device_code.is_empty() || !page.starts_with("http") {
                        Err("The site didn't start sign-in. Try again.".to_owned())
                    } else {
                        Ok(SignIn::Waiting {
                            device_code,
                            user_code,
                            page,
                            interval: body["interval"].as_u64().unwrap_or(5).clamp(1, 60),
                            deadline: unix_now() + body["expires_in"].as_u64().unwrap_or(600),
                        })
                    }
                }
                Ok(reply) => Err(reply.message("The site didn't start sign-in. Try again.")),
                Err(error) => Err(error),
            };
            {
                let mut state = lock(&state);
                state.sign_in = match waiting {
                    Ok(waiting) => waiting,
                    Err(error) => SignIn::Failed(error),
                };
                state.changed();
            }
            wake();
            wait_for_approval(http, state, wake, nudge, polling, runtime, poller).await;
        });
    }

    /// End the session here and on the site.
    fn sign_out(&mut self) {
        let session = {
            let mut state = self.lock();
            let session = state.session.take();
            state.forget = true;
            state.sign_in = SignIn::Idle;
            state.threads.clear();
            state.projects.clear();
            state.project_list.clear();
            state.projects_read = false;
            state.computers.clear();
            state.agents.clear();
            state.threads_read = false;
            state.agents_read = false;
            state.memory.clear();
            state.memory_read = false;
            state.schedules.clear();
            state.schedule_computers.clear();
            state.schedule_chats.clear();
            state.schedules_read = false;
            state.draft = None;
            state.editing = None;
            state.open = None;
            state.seen.clear();
            state.choice_read = false;
            state.screen = Screen::Account;
            state.notice = Some("Signed out.".into());
            state.changed();
            session
        };
        if let Some(session) = session {
            let http = self.http.clone();
            self.spawn(async move {
                let _ = call_v1(
                    &*http,
                    "POST",
                    &session.origin,
                    "/v1/device/sign-out",
                    "/device/sign-out",
                    Some(session.token.clone()),
                    Some(json!({})),
                )
                .await;
            });
        }
    }

    /// Answer "Where should this phone's chats live?".
    fn choose(&mut self, all: bool) {
        let (origin, name, token) = {
            let mut state = self.lock();
            state.choice = Some(all);
            state.choice_read = true;
            state.notice = Some(if all {
                "This phone's chats will sync to your account.".into()
            } else {
                "This phone's chats stay on this phone.".into()
            });
            state.changed();
            let Some(session) = state.session.clone() else {
                return;
            };
            (state.origin.clone(), state.name.clone(), session.token)
        };
        self.keep();
        let http = self.http.clone();
        self.spawn(async move {
            let _ = http
                .call(
                    "PUT",
                    format!("{origin}/v1/computers/{}/sync", encode(&name)),
                    Some(token),
                    Some(json!({"choice": if all { "all" } else { "local" }})),
                )
                .await;
        });
        self.nudge.notify_one();
    }

    /// Whether the host should open its photo picker for the open chat
    /// (**Add photo** was tapped); true once.
    pub fn take_pick(&mut self) -> bool {
        std::mem::take(&mut self.picking)
    }

    /// Whether the open chat takes photos and files now: a web chat that
    /// takes replies.
    pub fn takes_attachments(&self) -> bool {
        self.lock().open.as_ref().is_some_and(takes_photos)
    }

    /// Add a photo or file the host's picker read to the open chat's
    /// reply. A photo goes through the shared attachments code (PNG or
    /// JPEG, 8 MiB, 4096 pixels a side); a PDF or UTF-8 text file goes as
    /// it is, within the website's bounds ([`attachable_file`]). A refusal
    /// shows as the notice.
    pub fn attach(&mut self, name: &str, bytes: Vec<u8>) {
        let checked = attachable(name, bytes);
        let mut state = self.lock();
        let Some(open) = state.open.as_mut().filter(|open| takes_photos(open)) else {
            return;
        };
        let notice = match checked {
            Ok(_) if open.photos.len() >= MAX_PHOTOS => {
                Some("Send up to four photos or files with one reply.".to_owned())
            }
            Ok(file) => {
                open.photos.push(file);
                None
            }
            Err(message) => Some(message),
        };
        state.notice = notice;
        state.changed();
    }

    /// A reply to the open chat.
    fn reply(&mut self, text: String) {
        if secret_screen::credential_in(&text).is_some() {
            let mut state = self.lock();
            state.notice =
                Some("This looks like it holds a password or key, so it wasn't sent.".into());
            state.changed();
            return;
        }
        let (origin, token, id, photos, new, project) = {
            let mut state = self.lock();
            let Some(session) = state.session.clone() else {
                return;
            };
            let Some(open) = &mut state.open else {
                return;
            };
            if open.sending {
                return;
            }
            open.sending = true;
            let id = open.id.clone();
            let photos = open.photos.clone();
            let new = open.new;
            let project = open.project.clone().filter(|_| new);
            state.notice = None;
            state.changed();
            (
                state.origin.clone(),
                session.token,
                id,
                photos,
                new,
                project,
            )
        };
        self.composer += 1;
        // A new chat's first message is the request the chat is named by.
        let request = if new {
            id.clone()
        } else {
            uuid::Uuid::new_v4().to_string()
        };
        let http = self.http.clone();
        let state = self.state.clone();
        let wake = self.wake.clone();
        let nudge = self.nudge.clone();
        let attached: Vec<(String, u64)> = photos
            .iter()
            .map(|(name, bytes)| (name.clone(), bytes.len() as u64))
            .collect();
        self.spawn(async move {
            // The photos first, each by its own upload; the reply names
            // them by the ids the website gave them.
            let mut files = Vec::with_capacity(photos.len());
            let mut kinds = Vec::with_capacity(photos.len());
            let mut failed = None;
            for (name, bytes) in photos {
                let url = format!("{origin}/v1/threads/{id}/files?name={}", query_value(&name));
                match http.upload(url, Some(token.clone()), bytes).await {
                    Ok(reply) if (200..300).contains(&reply.status) => {
                        match reply.body["id"].as_str() {
                            Some(file) => {
                                files.push(file.to_owned());
                                let kind = reply.body["kind"].as_str().unwrap_or("");
                                kinds.push(kind.to_owned());
                            }
                            None => {
                                failed = Some(Ok(reply));
                                break;
                            }
                        }
                    }
                    other => {
                        failed = Some(other);
                        break;
                    }
                }
            }
            let reply = match failed {
                Some(Ok(reply)) => Ok(Reply {
                    status: reply.status.max(400),
                    body: if reply.body["error"]["message"].is_string() {
                        reply.body
                    } else {
                        json!({"error": {"message": "That file wasn't sent. Try again."}})
                    },
                }),
                Some(Err(error)) => Err(error),
                None => {
                    let mut body = json!({"request_id": request, "text": text});
                    if !files.is_empty() {
                        body["files"] = json!(files);
                    }
                    // A new chat in a project starts inside it (#11542).
                    if let Some(project) = &project {
                        body["project"] = json!(project);
                    }
                    http.call(
                        "POST",
                        if new {
                            format!("{origin}/v1/threads")
                        } else {
                            format!("{origin}/v1/threads/{id}/messages")
                        },
                        Some(token),
                        Some(body),
                    )
                    .await
                }
            };
            {
                let mut state = lock(&state);
                let notice = match &reply {
                    Ok(reply) if (200..300).contains(&reply.status) => None,
                    Ok(reply) => Some(reply.message("That reply wasn't sent. Try again.")),
                    Err(error) => Some(error.clone()),
                };
                if let Some(open) = state.open.as_mut().filter(|open| open.id == id) {
                    open.sending = false;
                    if notice.is_none() && new {
                        // The website has the chat now: read it as any
                        // web chat that takes replies.
                        open.new = false;
                        open.loaded = false;
                        open.thread = Some(Thread {
                            id: id.clone(),
                            title: text.chars().take(64).collect(),
                            surface: "web".into(),
                            can_reply: true,
                            project: project.clone(),
                            ..Thread::default()
                        });
                    }
                    if notice.is_none() {
                        // The sent photos show by name until the chat has them.
                        let files = attached
                            .iter()
                            .zip(files.iter().zip(&kinds))
                            .map(|((name, size), (file, kind))| MessageFile {
                                id: file.clone(),
                                name: name.clone(),
                                kind: kind.clone(),
                                size: *size,
                            })
                            .collect();
                        open.sent.push(Message {
                            role: "user".into(),
                            text: text.clone(),
                            files,
                        });
                        open.photos.clear();
                    }
                }
                state.notice = notice;
                state.changed();
            }
            wake();
            nudge.notify_one();
        });
    }

    /// Make a project with only a name (`POST /v1/projects {name}`,
    /// #11558); it lists at the top of the Projects screen.
    fn create_project(&mut self, name: String) {
        let name: String = name.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.chars().count() > MAX_PROJECT_NAME {
            let mut state = self.lock();
            state.notice = Some(format!(
                "A project's name can be at most {MAX_PROJECT_NAME} characters."
            ));
            state.changed();
            return;
        }
        let (origin, token) = {
            let mut state = self.lock();
            let Some(session) = state.session.clone() else {
                return;
            };
            if state.projects_busy {
                return;
            }
            state.projects_busy = true;
            state.notice = None;
            state.changed();
            (state.origin.clone(), session.token)
        };
        self.composer += 1;
        let http = self.http.clone();
        let state = self.state.clone();
        let wake = self.wake.clone();
        let nudge = self.nudge.clone();
        self.spawn(async move {
            let reply = http
                .call(
                    "POST",
                    format!("{origin}/v1/projects"),
                    Some(token),
                    Some(json!({"name": name})),
                )
                .await;
            {
                let mut state = lock(&state);
                state.projects_busy = false;
                state.notice = Some(match &reply {
                    Ok(reply) if (200..300).contains(&reply.status) => {
                        if let Some(project) = Project::from_value(&reply.body) {
                            state
                                .projects
                                .insert(project.id.clone(), project.name.clone());
                            state.project_list.retain(|p| p.id != project.id);
                            state.project_list.insert(0, project);
                        }
                        format!("Made {name}.")
                    }
                    Ok(reply) => reply.message("That project wasn't made. Try again."),
                    Err(error) => error.clone(),
                });
                state.changed();
            }
            wake();
            nudge.notify_one();
        });
    }

    /// Approve, deny, stop, or message one agent.
    fn send_action(
        &mut self,
        computer: String,
        item: String,
        action: String,
        question: Option<String>,
        text: Option<String>,
    ) {
        if text
            .as_deref()
            .is_some_and(|text| secret_screen::credential_in(text).is_some())
        {
            let mut state = self.lock();
            state.notice =
                Some("This looks like it holds a password or key, so it wasn't sent.".into());
            state.changed();
            return;
        }
        let key = format!(
            "{computer}\0{item}\0{action}\0{}\0{}",
            question.as_deref().unwrap_or_default(),
            text.as_deref().unwrap_or_default()
        );
        let (origin, token) = {
            let mut state = self.lock();
            let Some(session) = state.session.clone() else {
                return;
            };
            // A double tap sends once; a message can be sent again.
            if action != "message" && !state.acted.insert(key) {
                return;
            }
            if state.acted.len() > 256 {
                state.acted.clear();
            }
            state.notice = Some(match action.as_str() {
                "approve" => "Approving…".into(),
                "deny" => "Denying…".into(),
                "stop" => "Stopping…".into(),
                "your_key" | "openagents" => "Continuing…".into(),
                _ => "Sending…".into(),
            });
            state.changed();
            (state.origin.clone(), session.token)
        };
        if action == "message" {
            self.composer += 1;
        }
        let request = uuid::Uuid::new_v4().to_string();
        let http = self.http.clone();
        let state = self.state.clone();
        let wake = self.wake.clone();
        let nudge = self.nudge.clone();
        let body = json!({
            "request_id": request,
            "computer": computer,
            "item": item,
            "action": action,
            "question": question,
            "text": text,
        });
        self.spawn(async move {
            let reply = http
                .call(
                    "POST",
                    format!("{origin}/v1/agents/actions"),
                    Some(token),
                    Some(body),
                )
                .await;
            {
                let mut state = lock(&state);
                state.notice = Some(match &reply {
                    Ok(reply) if (200..300).contains(&reply.status) => match action.as_str() {
                        "approve" => format!("Approved. {} goes on.", runner(&computer)),
                        "deny" => format!("Denied. {} won't run it.", runner(&computer)),
                        "stop" => format!("{} stops in a few seconds.", runner(&computer)),
                        _ => reply.body["message"]
                            .as_str()
                            .filter(|m| !m.is_empty())
                            .map_or_else(
                                || {
                                    if is_computer(&computer) {
                                        format!("Sent to Coder on {computer}.")
                                    } else {
                                        "Sent.".to_owned()
                                    }
                                },
                                str::to_owned,
                            ),
                    },
                    Ok(reply) => reply.message("That didn't reach your computer. Try again."),
                    Err(error) => error.clone(),
                });
                if action == "message"
                    && reply
                        .as_ref()
                        .is_ok_and(|reply| (200..300).contains(&reply.status))
                    && state.screen == Screen::Message
                {
                    state.screen = Screen::Running;
                    state.messaging = None;
                }
                state.changed();
            }
            wake();
            nudge.notify_one();
        });
    }

    /// The memory notes that apply everywhere, newest first, for each
    /// phone chat turn (#11182); none while signed out.
    #[must_use]
    pub fn memory_notes(&self) -> Vec<openagents_chat::router::MemoryNote> {
        let state = self.lock();
        if !state.signed() {
            return Vec::new();
        }
        state
            .memory
            .iter()
            .filter(|note| !note.deleted && note.everywhere())
            .filter_map(|note| {
                Some(openagents_chat::router::MemoryNote {
                    name: note.name.clone()?,
                    kind: note.kind.clone()?,
                    description: note.description.clone().unwrap_or_default(),
                    body: note.body.clone()?,
                })
            })
            .take(openagents_chat::router::MAX_MEMORY_NOTES)
            .collect()
    }

    /// Save the Note screen's text: what the open note says, or a new note
    /// that applies everywhere, named by its first line.
    fn save_note(&mut self, text: String) {
        let refuse = |link: &Self, notice: &str| {
            let mut state = link.lock();
            state.notice = Some(notice.to_owned());
            state.changed();
        };
        if text.len() > MAX_NOTE_BYTES {
            return refuse(&*self, "That note is longer than 8 KB.");
        }
        if secret_screen::credential_in(&text).is_some() {
            return refuse(
                &*self,
                "This looks like it holds a password or key, so it wasn't saved.",
            );
        }
        let (origin, token, id, body) = {
            let mut state = self.lock();
            let Some(session) = state.session.clone() else {
                return;
            };
            let Some(editing) = state.editing.clone() else {
                return;
            };
            if state.memory_busy {
                return;
            }
            let body = if editing.is_empty() {
                let name: String = text
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or_default()
                    .trim()
                    .chars()
                    .take(MAX_NOTE_NAME)
                    .collect();
                json!({"kind": "user", "name": name, "description": "", "body": text})
            } else {
                let Some(note) = state.memory.iter().find(|note| note.id == editing) else {
                    return;
                };
                json!({
                    "kind": note.kind,
                    "name": note.name,
                    "description": note.description,
                    "body": text,
                })
            };
            state.memory_busy = true;
            state.notice = Some("Saving…".into());
            state.changed();
            let id = if editing.is_empty() {
                "new".to_owned()
            } else {
                editing
            };
            (state.origin.clone(), session.token, id, body)
        };
        self.composer += 1;
        let http = self.http.clone();
        let state = self.state.clone();
        let wake = self.wake.clone();
        self.spawn(async move {
            let reply = http
                .call(
                    "PUT",
                    format!("{origin}/coder/memory/{}", encode(&id)),
                    Some(token),
                    Some(body),
                )
                .await;
            {
                let mut state = lock(&state);
                state.memory_busy = false;
                match &reply {
                    Ok(reply) if reply.status == 200 => {
                        if let Ok(note) = serde_json::from_value::<Note>(reply.body["note"].clone())
                        {
                            state.memory.retain(|have| have.id != note.id);
                            state.memory.insert(0, note);
                        }
                        state.editing = None;
                        state.screen = Screen::Memory;
                        state.notice = Some("Saved. Coder has it at its next sync.".into());
                    }
                    Ok(reply) => {
                        state.notice = Some(reply.message("That note wasn't saved. Try again."));
                    }
                    Err(error) => state.notice = Some(error.clone()),
                }
                state.changed();
            }
            wake();
        });
    }

    /// Delete a memory note everywhere.
    fn delete_note(&mut self, id: String) {
        let (origin, token) = {
            let mut state = self.lock();
            let Some(session) = state.session.clone() else {
                return;
            };
            if state.memory_busy {
                return;
            }
            state.memory_busy = true;
            state.notice = Some("Deleting…".into());
            state.changed();
            (state.origin.clone(), session.token)
        };
        let http = self.http.clone();
        let state = self.state.clone();
        let wake = self.wake.clone();
        self.spawn(async move {
            let reply = http
                .call(
                    "DELETE",
                    format!("{origin}/coder/memory/{}", encode(&id)),
                    Some(token),
                    None,
                )
                .await;
            {
                let mut state = lock(&state);
                state.memory_busy = false;
                match &reply {
                    Ok(reply) if reply.status == 200 => {
                        state.memory.retain(|note| note.id != id);
                        state.editing = None;
                        state.screen = Screen::Memory;
                        state.notice = Some("Deleted. Coder forgets it at its next sync.".into());
                    }
                    Ok(reply) => {
                        state.notice = Some(reply.message("That note wasn't deleted. Try again."));
                    }
                    Err(error) => state.notice = Some(error.clone()),
                }
                state.changed();
            }
            wake();
        });
    }

    /// Make the new scheduled prompt: the composer's words, and what the
    /// NewSchedule screen picked.
    fn save_schedule(&mut self, text: String) {
        let refuse = |link: &Self, notice: &str| {
            let mut state = link.lock();
            state.notice = Some(notice.to_owned());
            state.changed();
        };
        if text.len() > MAX_SCHEDULE_PROMPT {
            return refuse(&*self, "That prompt is longer than 4,000 characters.");
        }
        if secret_screen::credential_in(&text).is_some() {
            return refuse(
                &*self,
                "This looks like it holds a password or key, so it wasn't saved.",
            );
        }
        let (origin, token, body) = {
            let mut state = self.lock();
            let Some(session) = state.session.clone() else {
                return;
            };
            let Some(draft) = state.draft.clone() else {
                return;
            };
            if state.schedules_busy {
                return;
            }
            if draft.computer.is_empty() {
                state.notice = Some("Pick the computer it runs on.".into());
                state.changed();
                return;
            }
            let (days, time, every_secs) = draft.when();
            let mut body = json!({
                "computer": draft.computer,
                "prompt": text,
                "days": days,
                "paused": false,
            });
            if let Some(time) = time {
                body["time"] = json!(time);
            }
            if let Some(every) = every_secs {
                body["every_secs"] = json!(every);
            }
            if let Some(chat) = &draft.chat {
                body["chat"] = json!(chat);
            }
            state.schedules_busy = true;
            state.notice = Some("Saving…".into());
            state.changed();
            (state.origin.clone(), session.token, body)
        };
        self.composer += 1;
        let http = self.http.clone();
        let state = self.state.clone();
        let wake = self.wake.clone();
        self.spawn(async move {
            let reply = http
                .call(
                    "PUT",
                    format!("{origin}/v1/schedules/new"),
                    Some(token),
                    Some(body),
                )
                .await;
            {
                let mut state = lock(&state);
                state.schedules_busy = false;
                match &reply {
                    Ok(reply) if reply.status == 200 => {
                        if let Ok(schedule) =
                            serde_json::from_value::<Schedule>(reply.body["schedule"].clone())
                        {
                            state.schedules.retain(|have| have.id != schedule.id);
                            state.schedules.insert(0, schedule);
                        }
                        state.draft = None;
                        state.screen = Screen::Schedules;
                        state.notice =
                            Some("Saved. The computer picks it up at its next sync.".into());
                    }
                    Ok(reply) => {
                        state.notice =
                            Some(reply.message("That scheduled prompt wasn't saved. Try again."));
                    }
                    Err(error) => state.notice = Some(error.clone()),
                }
                state.changed();
            }
            wake();
        });
    }

    /// Pause or resume a scheduled prompt.
    fn pause_schedule(&mut self, id: String, paused: bool) {
        let (origin, token) = {
            let mut state = self.lock();
            let Some(session) = state.session.clone() else {
                return;
            };
            if state.schedules_busy || !state.schedules.iter().any(|s| s.id == id) {
                return;
            }
            state.schedules_busy = true;
            state.notice = Some(if paused { "Pausing…" } else { "Resuming…" }.into());
            state.changed();
            (state.origin.clone(), session.token)
        };
        let http = self.http.clone();
        let state = self.state.clone();
        let wake = self.wake.clone();
        self.spawn(async move {
            let reply = http
                .call(
                    "POST",
                    format!("{origin}/v1/schedules/{}/pause", encode(&id)),
                    Some(token),
                    Some(json!({ "paused": paused })),
                )
                .await;
            {
                let mut state = lock(&state);
                state.schedules_busy = false;
                match &reply {
                    Ok(reply) if reply.status == 200 => {
                        let updated =
                            serde_json::from_value::<Schedule>(reply.body["schedule"].clone()).ok();
                        if let Some(have) = state.schedules.iter_mut().find(|s| s.id == id) {
                            match updated {
                                Some(schedule) => *have = schedule,
                                None => have.paused = paused,
                            }
                        }
                        state.notice = Some(if paused { "Paused." } else { "Resumed." }.into());
                    }
                    Ok(reply) => {
                        state.notice = Some(reply.message("That didn't go through. Try again."));
                    }
                    Err(error) => state.notice = Some(error.clone()),
                }
                state.changed();
            }
            wake();
        });
    }

    /// Delete a scheduled prompt everywhere.
    fn delete_schedule(&mut self, id: String) {
        let (origin, token) = {
            let mut state = self.lock();
            let Some(session) = state.session.clone() else {
                return;
            };
            if state.schedules_busy {
                return;
            }
            state.schedules_busy = true;
            state.notice = Some("Deleting…".into());
            state.changed();
            (state.origin.clone(), session.token)
        };
        let http = self.http.clone();
        let state = self.state.clone();
        let wake = self.wake.clone();
        self.spawn(async move {
            let reply = http
                .call(
                    "DELETE",
                    format!("{origin}/v1/schedules/{}", encode(&id)),
                    Some(token),
                    None,
                )
                .await;
            {
                let mut state = lock(&state);
                state.schedules_busy = false;
                match &reply {
                    Ok(reply) if reply.status == 200 => {
                        state.schedules.retain(|s| s.id != id);
                        state.notice =
                            Some("Deleted. The computer stops running it at its next sync.".into());
                    }
                    Ok(reply) => {
                        state.notice =
                            Some(reply.message("That scheduled prompt wasn't deleted. Try again."));
                    }
                    Err(error) => state.notice = Some(error.clone()),
                }
                state.changed();
            }
            wake();
        });
    }

    /// Phone chats changed since they last uploaded: queue them. `chats`
    /// gives each chat's id, title, updated stamp, and messages.
    pub fn queue_uploads(&self, chats: Vec<(String, String, u64, Vec<Message>)>) {
        let mut state = self.lock();
        let mut added = false;
        for (id, title, updated, messages) in chats {
            if state.refused.contains(&id) || messages.is_empty() {
                continue;
            }
            if state.uploaded.get(&id).is_some_and(|sent| *sent >= updated) {
                continue;
            }
            if state
                .uploads
                .get(&id)
                .is_some_and(|(_, _, at)| *at >= updated)
            {
                continue;
            }
            state.uploads.insert(id, (title, messages, updated));
            added = true;
        }
        drop(state);
        if added {
            self.nudge.notify_one();
        }
    }

    /// The chats whose `updated` stamp is newer than what was uploaded,
    /// among `listed` (id and updated): the ones to read and queue.
    #[must_use]
    pub fn wanted_uploads(&self, listed: &[(String, u64)]) -> Vec<String> {
        let state = self.lock();
        if !state.signed() || state.choice != Some(true) {
            return vec![];
        }
        listed
            .iter()
            .filter(|(id, updated)| {
                !state.refused.contains(id)
                    && state.uploaded.get(id).is_none_or(|sent| sent < updated)
                    && state.uploads.get(id).is_none_or(|(_, _, at)| at < updated)
            })
            .map(|(id, _)| id.clone())
            .take(UPLOADS_PER_PASS)
            .collect()
    }

    /// Start the background reads, once, while signed in.
    fn start(&self) {
        if !self.signed_in() {
            return;
        }
        if self.polling.swap(true, std::sync::atomic::Ordering::SeqCst) {
            self.nudge.notify_one();
            return;
        }
        let poller = self.poller();
        self.spawn(async move { poller.run().await });
    }

    fn poller(&self) -> Poller {
        Poller {
            http: self.http.clone(),
            state: self.state.clone(),
            wake: self.wake.clone(),
            nudge: self.nudge.clone(),
            polling: self.polling.clone(),
            store: self.store.clone(),
        }
    }

    /// One read of everything now, waiting at most `limit` (the host's
    /// background fetch).
    pub fn refresh_now(&self, limit: Duration) {
        if !self.signed_in() {
            return;
        }
        let Some(runtime) = &self.runtime else {
            return;
        };
        let poller = self.poller();
        let _ = runtime.block_on(async move {
            tokio::time::timeout(limit, async move { poller.pass().await }).await
        });
    }

    /// The host's part of the packet; takes the once-only fields.
    pub fn packet(&mut self) -> Packet {
        self.start();
        let mut state = self.lock();
        let signed = state.signed();
        let store = state.store.take();
        let forget = std::mem::take(&mut state.forget);
        let notify = std::mem::take(&mut state.notify);
        let drawer = drawer_rows(&state);
        let running = state
            .agents
            .iter()
            .flat_map(|c| &c.items)
            .filter(|i| i.running())
            .count();
        let asking = state
            .agents
            .iter()
            .flat_map(|c| &c.items)
            .filter(|i| i.status == "asking")
            .count();
        let qr = match &state.sign_in {
            SignIn::Waiting { page, .. } if !signed => qr_modules(page),
            _ => None,
        };
        let label = state.session.as_ref().map(|s| s.label.clone());
        let site = (state.origin != PRODUCTION)
            .then(|| state.origin.trim_start_matches("https://").to_owned());
        let live = state.shown
            && (matches!(state.sign_in, SignIn::Starting | SignIn::Waiting { .. })
                || state.open.as_ref().is_some_and(|o| o.sending || !o.loaded));
        let shown = state.shown;
        drop(state);
        let view = shown.then(|| self.render()).flatten();
        Packet {
            signed_in: signed,
            label,
            site,
            view,
            qr,
            drawer,
            running,
            asking,
            store,
            forget,
            notify,
            open_url: None,
            menu: std::mem::take(&mut self.menu),
            live,
        }
    }

    fn render(&mut self) -> Option<Value> {
        self.view_revision += 1;
        let root = {
            let state = self.lock();
            crate::link_view::root(&state, self.composer)
        };
        let view = rust_native::View::new("openagents.link", self.view_revision, root)
            .validate()
            .or_else(|_| {
                // Too big to draw: the newest few messages only.
                let state = self.lock();
                let root = crate::link_view::root_small(&state, self.composer);
                rust_native::View::new("openagents.link", self.view_revision, root).validate()
            })
            .ok()?;
        let value = serde_json::to_value(view.view()).ok();
        if let Some(previous) = self.view.replace(view) {
            self.earlier_views.push_front(previous);
            self.earlier_views.truncate(EARLIER_VIEWS);
        }
        value
    }

    /// Resolve a tap on the last view drawn.
    pub fn activate(&mut self, event: &rust_native::Activation) -> Option<String> {
        let intent = self
            .view
            .iter()
            .chain(&self.earlier_views)
            .find_map(|view| view.activate(event).ok())?
            .clone();
        self.tap(intent)
    }
}

/// The largest PDF a reply carries (the website's `MAX_FILE_BYTES`).
pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
/// The largest text file a reply carries (the website's `MAX_TEXT_BYTES`).
pub const MAX_TEXT_BYTES: usize = 512 * 1024;

/// What a reply carries for `bytes` the host's picker read: a photo,
/// re-encoded and bounded by the shared attachments code, or a PDF or
/// text file as it is ([`attachable_file`]).
pub(crate) fn attachable(name: &str, bytes: Vec<u8>) -> Result<(String, Vec<u8>), String> {
    if let Some(kept) = attachable_file(&bytes) {
        return kept.map(|()| (name.to_owned(), bytes));
    }
    openagents_chat_app::attachments::Image::decode(name, bytes)
        .map(|image| (image.name.clone(), (*image.bytes).clone()))
        .map_err(|message| {
            if message.starts_with("Choose a PNG or JPEG") {
                "Add a photo (PNG or JPEG), a PDF, or a text file.".to_owned()
            } else {
                message
            }
        })
}

/// `Some` when `bytes` are a PDF or a text file, read from the bytes as
/// the website reads them: `Ok` when they fit its bounds, else why not.
/// `None` for anything else (a photo, or a kind no chat takes).
pub(crate) fn attachable_file(bytes: &[u8]) -> Option<Result<(), String>> {
    if bytes.starts_with(b"%PDF-") {
        return Some(if bytes.len() > MAX_FILE_BYTES {
            Err("That PDF is over 10 MB.".to_owned())
        } else {
            Ok(())
        });
    }
    let image = bytes.starts_with(&[0x89, b'P', b'N', b'G'])
        || bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || bytes.starts_with(b"GIF8")
        || (bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP");
    if image || bytes.is_empty() || bytes.contains(&0) {
        return None;
    }
    std::str::from_utf8(bytes).ok()?;
    Some(if bytes.len() > MAX_TEXT_BYTES {
        Err("That text file is over 512 KB.".to_owned())
    } else {
        Ok(())
    })
}

/// Whether `open` is a chat a reply with photos can go to: a web chat that
/// takes replies now. Coder's chats on a computer and the phone's own
/// take words only.
pub(crate) fn takes_photos(open: &Open) -> bool {
    open.new
        || open
            .thread
            .as_ref()
            .is_some_and(|thread| thread.surface == "web" && thread.can_reply)
}

/// `value` for a URL query: letters, digits, and `-._~` as they are, every
/// other byte as `%XX`.
fn query_value(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

fn lock(state: &Arc<Mutex<State>>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}

fn open_chat(state: &mut State, id: &str) {
    if state.open.as_ref().is_some_and(|open| open.id == id) {
        return;
    }
    let thread = state.threads.iter().find(|t| t.id == id).cloned();
    state.open = Some(Open {
        id: id.to_owned(),
        thread,
        ..Open::default()
    });
    state.notice = None;
}

/// Poll `/v1/device/token` until the person approves or denies, or the code
/// expires; then start reading.
async fn wait_for_approval(
    http: Arc<dyn Http>,
    state: Arc<Mutex<State>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    nudge: Arc<tokio::sync::Notify>,
    polling: Arc<std::sync::atomic::AtomicBool>,
    runtime: Option<tokio::runtime::Handle>,
    poller: Poller,
) {
    loop {
        let (origin, device_code, mut interval, deadline) = {
            let state = lock(&state);
            match &state.sign_in {
                SignIn::Waiting {
                    device_code,
                    interval,
                    deadline,
                    ..
                } => (
                    state.origin.clone(),
                    device_code.clone(),
                    *interval,
                    *deadline,
                ),
                _ => return,
            }
        };
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if unix_now() >= deadline {
            let mut state = lock(&state);
            if matches!(state.sign_in, SignIn::Waiting { .. }) {
                state.sign_in =
                    SignIn::Failed("The code expired before it was approved. Try again.".into());
                state.changed();
            }
            drop(state);
            wake();
            return;
        }
        let reply = call_v1(
            &*http,
            "POST",
            &origin,
            "/v1/device/token",
            "/device/token",
            None,
            Some(json!({
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
                "device_code": device_code,
            })),
        )
        .await;
        // A dropped connection is not the end of a sign-in.
        let Ok(reply) = reply else { continue };
        let mut state = lock(&state);
        // Cancelled, or another sign-in started, while this one waited.
        if !matches!(&state.sign_in, SignIn::Waiting { device_code: d, .. } if *d == device_code) {
            return;
        }
        if reply.status == 200 {
            let token = reply.body["access_token"].as_str().unwrap_or_default();
            if !token.starts_with("sess_") {
                state.sign_in = SignIn::Failed("Sign-in stopped. Try again.".into());
                state.changed();
                drop(state);
                wake();
                return;
            }
            let session = Session {
                origin: origin.clone(),
                account: reply.body["account"]["id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                label: reply.body["account"]["label"]
                    .as_str()
                    .unwrap_or("your account")
                    .to_owned(),
                expires_at: unix_now() + reply.body["expires_in"].as_u64().unwrap_or(30 * 86_400),
                token: token.to_owned(),
            };
            state.store = Some(session.clone());
            state.session = Some(session);
            state.sign_in = SignIn::Idle;
            state.choice_read = false;
            state.notice = None;
            state.changed();
            drop(state);
            wake();
            if !polling.swap(true, std::sync::atomic::Ordering::SeqCst) {
                if let Some(runtime) = runtime {
                    runtime.spawn(async move { poller.run().await });
                }
            } else {
                nudge.notify_one();
            }
            return;
        }
        match reply.code() {
            "authorization_pending" => {}
            "slow_down" => {
                interval = reply.body["interval"]
                    .as_u64()
                    .unwrap_or(interval + 5)
                    .clamp(interval, 60);
                if let SignIn::Waiting { interval: i, .. } = &mut state.sign_in {
                    *i = interval;
                }
            }
            "access_denied" => {
                state.sign_in = SignIn::Failed("Sign-in was denied on the website.".into());
                state.changed();
                drop(state);
                wake();
                return;
            }
            "expired_token" | "invalid_grant" => {
                state.sign_in =
                    SignIn::Failed("The code expired before it was approved. Try again.".into());
                state.changed();
                drop(state);
                wake();
                return;
            }
            _ => {
                state.sign_in = SignIn::Failed(reply.message("Sign-in stopped. Try again."));
                state.changed();
                drop(state);
                wake();
                return;
            }
        }
    }
}

/// The background reads.
#[derive(Clone)]
struct Poller {
    http: Arc<dyn Http>,
    state: Arc<Mutex<State>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    nudge: Arc<tokio::sync::Notify>,
    polling: Arc<std::sync::atomic::AtomicBool>,
    store: Option<Arc<coder_computers::cache::Cache>>,
}

impl Poller {
    /// Reads `wanted` chats, page by page (`GET /v1/threads?offset=&limit=`),
    /// and whether the website has more. An older website answers one
    /// unpaged list; then `more` is false. `Err` carries the HTTP status
    /// (0 when the call failed).
    async fn threads_pages(
        &self,
        origin: &str,
        token: &str,
        wanted: usize,
    ) -> Result<(Vec<Thread>, Vec<Computer>, bool), u16> {
        let mut threads = Vec::new();
        let mut computers = Vec::new();
        let mut offset = 0;
        loop {
            let limit = (wanted - offset).min(THREADS_PAGE);
            let reply = self
                .http
                .call(
                    "GET",
                    format!("{origin}/v1/threads?offset={offset}&limit={limit}"),
                    Some(token.to_owned()),
                    None,
                )
                .await
                .map_err(|_| 0u16)?;
            if reply.status != 200 {
                return Err(reply.status);
            }
            let page: Vec<Thread> =
                serde_json::from_value(reply.body["threads"].clone()).unwrap_or_default();
            if offset == 0 {
                computers =
                    serde_json::from_value(reply.body["computers"].clone()).unwrap_or_default();
            }
            let got = page.len();
            threads.extend(page);
            let more = reply.body["more"].as_bool().unwrap_or(false);
            offset += got;
            if !more || got == 0 || offset >= wanted {
                return Ok((threads, computers, more));
            }
        }
    }

    /// The account's projects (`GET /v1/projects`); `None` when they
    /// couldn't be read now.
    async fn projects(&self, origin: &str, token: &str) -> Option<Vec<Project>> {
        let reply = self
            .http
            .call(
                "GET",
                format!("{origin}/v1/projects?limit=200"),
                Some(token.to_owned()),
                None,
            )
            .await
            .ok()
            .filter(|reply| reply.status == 200)?;
        Some(
            reply.body["data"]
                .as_array()?
                .iter()
                .filter_map(Project::from_value)
                .collect(),
        )
    }

    async fn run(self) {
        tokio::spawn(self.clone().watch());
        loop {
            if !lock(&self.state).signed() {
                self.polling
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                return;
            }
            self.pass().await;
            let interval = lock(&self.state).interval();
            let _ = tokio::time::timeout(interval, self.nudge.notified()).await;
        }
    }

    /// Live updates (#11253): while signed in and in front, hold the Mac
    /// jobs' event stream open and read again the moment a job changes,
    /// rather than at the next poll. A site without the stream (`404`) is
    /// asked again only every ten minutes; polling goes on regardless.
    async fn watch(self) {
        loop {
            let (origin, token, active) = {
                let state = lock(&self.state);
                let Some(session) = state.session.clone().filter(|s| s.live(unix_now())) else {
                    return;
                };
                (state.origin.clone(), session.token, state.active)
            };
            if !active {
                tokio::time::sleep(Duration::from_secs(20)).await;
                continue;
            }
            let pause = match self
                .http
                .changed(format!("{origin}/v1/mac-jobs/events"), Some(token))
                .await
            {
                Ok(reply) if reply.status == 200 => {
                    self.nudge.notify_one();
                    Duration::ZERO
                }
                Ok(reply) if reply.status == 204 => Duration::ZERO,
                Ok(reply) if reply.status == 404 => Duration::from_secs(600),
                Ok(reply) if reply.status == 401 => return,
                _ => Duration::from_secs(30),
            };
            tokio::time::sleep(pause).await;
        }
    }

    /// One pass: the choice, the chat list, the open chat, the agents, and
    /// the uploads.
    async fn pass(&self) {
        let (origin, token, name, read_choice, open, uploads, read_memory, read_schedules) = {
            let state = lock(&self.state);
            let Some(session) = state.session.clone().filter(|s| s.live(unix_now())) else {
                return;
            };
            let uploads: Vec<(String, (String, Vec<Message>, u64))> = if state.choice == Some(true)
            {
                state
                    .uploads
                    .iter()
                    .take(UPLOADS_PER_PASS)
                    .map(|(id, value)| (id.clone(), value.clone()))
                    .collect()
            } else {
                vec![]
            };
            (
                state.origin.clone(),
                session.token,
                state.name.clone(),
                !state.choice_read,
                state
                    .open
                    .as_ref()
                    .filter(|open| !open.new)
                    .map(|open| open.id.clone()),
                uploads,
                !state.memory_read
                    || (state.shown && state.screen == Screen::Memory)
                    || unix_now().saturating_sub(state.memory_at) >= MEMORY_EVERY,
                !state.schedules_read
                    || (state.shown
                        && matches!(state.screen, Screen::Schedules | Screen::NewSchedule))
                    || unix_now().saturating_sub(state.schedules_at) >= SCHEDULES_EVERY,
            )
        };
        let before = lock(&self.state).revision;
        let mut failed = false;
        let mut signed_out = false;
        if read_choice {
            match self
                .http
                .call(
                    "GET",
                    format!("{origin}/v1/computers/{}/sync", encode(&name)),
                    Some(token.clone()),
                    None,
                )
                .await
            {
                Ok(reply) if reply.status == 200 => {
                    let web = match reply.body["choice"].as_str() {
                        Some("all") => Some(true),
                        Some("local") => Some(false),
                        _ => None,
                    };
                    let (tell, mine) = {
                        let mut state = lock(&self.state);
                        // A choice made here before the site had one goes there.
                        let tell = web.is_none() && state.choice.is_some();
                        if web.is_some() {
                            state.choice = web;
                        }
                        state.choice_read = true;
                        state.changed();
                        (tell, state.choice)
                    };
                    self.keep();
                    if tell && let Some(all) = mine {
                        let _ = self
                            .http
                            .call(
                                "PUT",
                                format!("{origin}/v1/computers/{}/sync", encode(&name)),
                                Some(token.clone()),
                                Some(json!({"choice": if all { "all" } else { "local" }})),
                            )
                            .await;
                    }
                }
                Ok(reply) if reply.status == 401 => signed_out = true,
                _ => failed = true,
            }
        }
        let wanted = lock(&self.state).threads_wanted.max(THREADS_PAGE);
        match self.threads_pages(&origin, &token, wanted).await {
            Ok((threads, computers, more)) => {
                let mut state = lock(&self.state);
                if state.threads != threads
                    || state.computers != computers
                    || state.threads_more != more
                    || !state.threads_read
                {
                    state.threads_more = more;
                    if let Some(open) = &mut state.open
                        && let Some(row) = threads.iter().find(|t| t.id == open.id)
                    {
                        open.thread = Some(row.clone());
                    }
                    state.threads = threads;
                    state.computers = computers;
                    state.threads_read = true;
                    state.changed();
                }
            }
            Err(401) => signed_out = true,
            Err(_) => failed = true,
        }
        // Name the projects the chats are filed under, read again when a
        // chat names one the list doesn't know (#11541).
        // The Projects screen reads them each pass while it shows.
        let unknown = {
            let state = lock(&self.state);
            (state.shown && state.screen == Screen::Projects && !state.projects_busy)
                || state
                    .threads
                    .iter()
                    .filter_map(|t| t.project.as_ref())
                    .any(|id| !state.projects.contains_key(id))
        };
        if unknown && let Some(list) = self.projects(&origin, &token).await {
            let mut state = lock(&self.state);
            let projects: BTreeMap<String, String> = list
                .iter()
                .map(|p| (p.id.clone(), p.name.clone()))
                .collect();
            if state.projects != projects || state.project_list != list || !state.projects_read {
                state.projects = projects;
                state.project_list = list;
                state.projects_read = true;
                state.changed();
            }
        }
        if let Some(id) = open {
            match self
                .http
                .call(
                    "GET",
                    format!("{origin}/v1/threads/{id}"),
                    Some(token.clone()),
                    None,
                )
                .await
            {
                Ok(reply) if reply.status == 200 => {
                    let messages: Vec<Message> =
                        serde_json::from_value(reply.body["messages"].clone()).unwrap_or_default();
                    let thread: Option<Thread> =
                        serde_json::from_value(reply.body["thread"].clone()).ok();
                    let mut state = lock(&self.state);
                    if let Some(open) = state.open.as_mut().filter(|open| open.id == id) {
                        let earlier = reply.body["earlier"].as_u64().unwrap_or(0) as usize;
                        let waiting = reply.body["waiting"].as_u64().unwrap_or(0) as usize;
                        // A reply sent from here shows until the chat has it.
                        let sent: Vec<Message> =
                            open.sent
                                .iter()
                                .filter(|sent| {
                                    !messages.iter().rev().take(8).any(|m| {
                                        m.role == "user" && m.text.trim() == sent.text.trim()
                                    })
                                })
                                .cloned()
                                .collect();
                        let fresh = !open.loaded
                            || open.messages != messages
                            || open.earlier != earlier
                            || open.waiting != waiting
                            || open.sent != sent
                            || thread.is_some() && open.thread != thread
                            || open.error.is_some();
                        if fresh {
                            open.messages = messages;
                            open.earlier = earlier;
                            open.waiting = waiting;
                            open.sent = sent;
                            if thread.is_some() {
                                open.thread = thread;
                            }
                            open.loaded = true;
                            open.error = None;
                            state.changed();
                        }
                    }
                }
                Ok(reply) if reply.status == 401 => signed_out = true,
                Ok(reply) if reply.status == 404 => {
                    let mut state = lock(&self.state);
                    if let Some(open) = state.open.as_mut().filter(|open| open.id == id) {
                        open.loaded = true;
                        open.error = Some("This chat was deleted.".into());
                        state.changed();
                    }
                }
                _ => failed = true,
            }
        }
        match self
            .http
            .call(
                "GET",
                format!("{origin}/v1/agents"),
                Some(token.clone()),
                None,
            )
            .await
        {
            Ok(reply) if reply.status == 200 => {
                let agents: Vec<Agents> =
                    serde_json::from_value(reply.body["computers"].clone()).unwrap_or_default();
                let mut state = lock(&self.state);
                notice_changes(&mut state, &agents);
                if state.agents != agents || !state.agents_read {
                    state.agents = agents;
                    state.agents_read = true;
                    state.changed();
                }
            }
            Ok(reply) if reply.status == 401 => signed_out = true,
            _ => failed = true,
        }
        if read_memory {
            // An older website keeps no memory: that is no failure.
            match self
                .http
                .call(
                    "GET",
                    format!("{origin}/coder/memory"),
                    Some(token.clone()),
                    None,
                )
                .await
            {
                Ok(reply) if reply.status == 200 => {
                    let mut notes: Vec<Note> = reply.body["notes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|note| serde_json::from_value::<Note>(note.clone()).ok())
                        .filter(|note| !note.deleted)
                        .collect();
                    notes.sort_by(|a, b| b.updated.cmp(&a.updated));
                    let mut state = lock(&self.state);
                    state.memory_at = unix_now();
                    if state.memory != notes || !state.memory_read {
                        state.memory = notes;
                        state.memory_read = true;
                        state.changed();
                    }
                }
                Ok(reply) if reply.status == 401 => signed_out = true,
                _ => {
                    let mut state = lock(&self.state);
                    state.memory_at = unix_now();
                    if !state.memory_read {
                        state.memory_read = true;
                        state.changed();
                    }
                }
            }
        }
        if read_schedules {
            // An older website keeps no scheduled prompts: that is no
            // failure.
            match self
                .http
                .call(
                    "GET",
                    format!("{origin}/v1/schedules"),
                    Some(token.clone()),
                    None,
                )
                .await
            {
                Ok(reply) if reply.status == 200 => {
                    let (schedules, computers, chats) = read_schedule_list(&reply.body);
                    let mut state = lock(&self.state);
                    state.schedules_at = unix_now();
                    if state.schedules != schedules
                        || state.schedule_computers != computers
                        || state.schedule_chats != chats
                        || !state.schedules_read
                    {
                        state.schedules = schedules;
                        state.schedule_computers = computers;
                        state.schedule_chats = chats;
                        state.schedules_read = true;
                        state.changed();
                    }
                }
                Ok(reply) if reply.status == 401 => signed_out = true,
                _ => {
                    let mut state = lock(&self.state);
                    state.schedules_at = unix_now();
                    if !state.schedules_read {
                        state.schedules_read = true;
                        state.changed();
                    }
                }
            }
        }
        for (id, (title, messages, updated)) in uploads {
            let body = json!({
                "computer": name,
                "title": title,
                "messages": messages
                    .iter()
                    .map(|m| json!({"role": m.role, "text": m.text}))
                    .collect::<Vec<_>>(),
            });
            match call_v1(
                &*self.http,
                "PUT",
                &origin,
                &format!("/v1/threads/synced/{PHONE_SESSION}{id}"),
                &format!("/coder/sessions/{PHONE_SESSION}{id}"),
                Some(token.clone()),
                Some(body),
            )
            .await
            {
                Ok(reply) if reply.status == 200 => {
                    let mut state = lock(&self.state);
                    if state
                        .uploads
                        .get(&id)
                        .is_some_and(|(_, _, at)| *at == updated)
                    {
                        state.uploads.remove(&id);
                    }
                    state.uploaded.insert(id, updated);
                }
                // Deleted on the website, too large, or refused: kept here.
                Ok(reply) if matches!(reply.status, 400 | 410 | 413 | 422) => {
                    let mut state = lock(&self.state);
                    state.uploads.remove(&id);
                    state.refused.insert(id);
                }
                Ok(reply) if reply.status == 401 => signed_out = true,
                _ => failed = true,
            }
            self.keep();
        }
        let mut state = lock(&self.state);
        if signed_out {
            state.session = None;
            state.forget = true;
            state.threads.clear();
            state.projects.clear();
            state.project_list.clear();
            state.projects_read = false;
            state.agents.clear();
            state.memory.clear();
            state.memory_read = false;
            state.schedules.clear();
            state.schedule_computers.clear();
            state.schedule_chats.clear();
            state.schedules_read = false;
            state.draft = None;
            state.editing = None;
            state.open = None;
            state.screen = Screen::Account;
            state.notice = Some("Your sign-in ended. Sign in again.".into());
            state.changed();
        }
        state.failures = if failed {
            state.failures.saturating_add(1)
        } else {
            0
        };
        let changed = state.revision != before;
        drop(state);
        if changed {
            (self.wake)();
        }
    }

    fn keep(&self) {
        let Some(store) = &self.store else { return };
        let state = lock(&self.state);
        let kept = Kept {
            choice: state.choice,
            uploaded: state.uploaded.clone(),
        };
        drop(state);
        let _ = store.write("link", &kept);
    }
}

/// Notices for items that finished, failed, or asked since the last read.
/// The first read only learns what is there.
fn notice_changes(state: &mut State, agents: &[Agents]) {
    let first = !state.agents_read;
    let mut seen = BTreeMap::new();
    for computer in agents {
        for item in &computer.items {
            let key = (computer.name.clone(), item.id.clone());
            let now = (
                item.status.clone(),
                item.question.as_ref().map(|q| q.id.clone()),
            );
            let before = state.seen.get(&key);
            if !first && before != Some(&now) {
                let title = if item.title.trim().is_empty() {
                    "Coder".to_owned()
                } else {
                    item.title.clone()
                };
                let (headline, question) = match item.status.as_str() {
                    "asking" => (
                        format!("{title} asks"),
                        item.question.as_ref().map(|q| q.id.clone()),
                    ),
                    "done" => (format!("{title} finished"), None),
                    "failed" => (format!("{title} failed"), None),
                    _ => (String::new(), None),
                };
                if !headline.is_empty() {
                    let body = match (&item.question, item.status.as_str()) {
                        (Some(q), "asking") => q.text.chars().take(200).collect(),
                        _ => item
                            .line
                            .clone()
                            .unwrap_or_else(|| format!("On {}", computer.name)),
                    };
                    state.notify.push(Notify {
                        id: format!(
                            "{}:{}:{}:{}",
                            computer.name,
                            item.id,
                            item.status,
                            question.as_deref().unwrap_or_default()
                        ),
                        title: headline,
                        body,
                        computer: computer.name.clone(),
                        item: item.id.clone(),
                        question,
                    });
                }
            }
            seen.insert(key, now);
        }
    }
    let excess = state.notify.len().saturating_sub(MAX_NOTIFY);
    state.notify.drain(..excess);
    state.seen = seen;
}

/// The account's newest chats for the drawer (not this phone's own, which
/// the drawer lists already).
fn drawer_rows(state: &State) -> Vec<DrawerRow> {
    if !state.signed() {
        return vec![];
    }
    let mut rows: Vec<&Thread> = state
        .threads
        .iter()
        .filter(|t| t.surface != "phone" || t.computer.as_deref() != Some(state.name.as_str()))
        .collect();
    rows.sort_by_key(|t| std::cmp::Reverse((t.pinned, t.updated_unix)));
    rows.into_iter()
        .take(8)
        .map(|t| DrawerRow {
            id: t.id.clone(),
            title: if t.title.trim().is_empty() {
                "New chat".into()
            } else {
                t.title.clone()
            },
            detail: detail(state, t),
        })
        .collect()
}

/// A chat's row's second line: its project, when it is filed under one
/// (#11541), then where it lives.
pub(crate) fn detail(state: &State, thread: &Thread) -> String {
    let place = place(thread);
    match thread
        .project
        .as_ref()
        .and_then(|id| state.projects.get(id))
    {
        Some(project) => format!("{project} · {place}"),
        None => place,
    }
}

/// Where a chat lives, as its row's second line.
pub(crate) fn place(thread: &Thread) -> String {
    match (thread.surface.as_str(), thread.computer.as_deref()) {
        ("terminal", Some(computer)) => format!("Terminal · {computer}"),
        ("phone", Some(computer)) => format!("Phone · {computer}"),
        _ => thread
            .line
            .clone()
            .unwrap_or_else(|| "openagents.com".into()),
    }
}

/// The sign-in page as QR modules, quiet zone included.
fn qr_modules(page: &str) -> Option<crate::app::QrModules> {
    let qr = qrcodegen::QrCode::encode_text(page, qrcodegen::QrCodeEcc::Medium).ok()?;
    let border = 2;
    let size = qr.size();
    let rows: Vec<String> = (-border..size + border)
        .map(|y| {
            (-border..size + border)
                .map(|x| if qr.get_module(x, y) { '1' } else { '0' })
                .collect()
        })
        .collect();
    Some(crate::app::QrModules {
        size: rows.len(),
        rows,
    })
}

/// `GET /v1/schedules`'s answer: the live scheduled prompts newest
/// first, the computers, and the chats (at most 100).
fn read_schedule_list(body: &Value) -> (Vec<Schedule>, Vec<String>, Vec<ScheduleChat>) {
    let mut schedules: Vec<Schedule> = body["schedules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| serde_json::from_value::<Schedule>(s.clone()).ok())
        .filter(|s| !s.deleted && !s.id.is_empty())
        .collect();
    schedules.sort_by(|a, b| b.updated.cmp(&a.updated));
    let computers: Vec<String> = body["computers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_owned)
        .take(32)
        .collect();
    let chats: Vec<ScheduleChat> = body["chats"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| serde_json::from_value::<ScheduleChat>(c.clone()).ok())
        .filter(|c| !c.session.is_empty())
        .take(100)
        .collect();
    (schedules, computers, chats)
}

/// A path segment, percent-encoded.
fn encode(segment: &str) -> String {
    url::form_urlencoded::byte_serialize(segment.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

/// A phone chat's turns as upload messages: screened, cut, the newest
/// [`MAX_UPLOAD_MESSAGES`].
#[must_use]
pub fn upload_messages(turns: &[openagents_chat::basic_coder::Turn]) -> Vec<Message> {
    let mut messages: Vec<Message> = turns
        .iter()
        .filter(|turn| !turn.text.trim().is_empty())
        .map(|turn| {
            let mut text = if secret_screen::credential_in(&turn.text).is_some() {
                LEFT_OUT.to_owned()
            } else {
                turn.text.clone()
            };
            if text.len() > MAX_UPLOAD_TEXT {
                let mut end = MAX_UPLOAD_TEXT;
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                text.truncate(end);
                text.push_str("\n…");
            }
            Message {
                role: match turn.role {
                    openagents_chat::basic_coder::Role::User => "user".into(),
                    openagents_chat::basic_coder::Role::Assistant => "assistant".into(),
                },
                text,
                files: Vec::new(),
            }
        })
        .collect();
    if messages.len() > MAX_UPLOAD_MESSAGES {
        messages.drain(..messages.len() - MAX_UPLOAD_MESSAGES);
    }
    messages
}

/// A chat title with no credential in it.
#[must_use]
pub fn upload_title(title: &str) -> String {
    let title: String = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if secret_screen::credential_in(&title).is_some() || title.is_empty() {
        "Phone chat".into()
    } else {
        title.chars().take(120).collect()
    }
}

pub(crate) fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
