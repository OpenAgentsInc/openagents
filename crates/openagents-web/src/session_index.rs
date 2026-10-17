//! Computer sessions in the account's chat list (#11897).
//!
//! A desktop or phone engine publishes one summary row per session it
//! runs: its id, title, the computer's name, project, status, last
//! activity, and a short recap. A row never holds the transcript. The web
//! chat list mixes the rows in with the account's chats, and
//! `/v1/history` returns them beside its first page, marked
//! `source: "session"` with their computer home.
//!
//! The full transcript is opt-in, per chat or per project, and off by
//! default. While it is on, the host may upload a read-only copy, which
//! the web shows under the summary. Turning it off deletes the copy.
//!
//! | Route | What |
//! | --- | --- |
//! | `GET /v1/history/sessions[?project=]` | `{data: [row]}`: the caller's rows, or a shared project's rows from every member |
//! | `GET /v1/history/sessions/{id}[?project=]` | `{session, messages}`: one row, and its copy when it is shared with the caller |
//! | `PUT /v1/history/sessions/computers/{device}` `{name, sessions: [...]}` | The host's current rows for that computer, replacing the earlier ones. `200 {transcripts: [id]}`: the sessions whose transcript is on |
//! | `DELETE /v1/history/sessions/computers/{device}` | Remove the computer's rows and copies |
//! | `PUT /v1/history/sessions/{id}/transcript` `{on}` | The per-chat choice |
//! | `PUT /v1/history/sessions/projects/{project}/transcript` `{on}` | The per-project choice |
//! | `PUT /v1/history/sessions/{id}/messages` `{messages: [{role, text}]}` | The host's copy; `409 transcript_off` while the choice is off |
//!
//! Every route takes the account's app token (`Authorization: Bearer
//! sess_…`). Rows live in one record beside the account's chats
//! ([`KEY`]). Removing a computer from the account
//! ([`crate::phone_api::remove_computer_named`]) removes its rows.

use std::collections::{BTreeMap, BTreeSet};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, put};
use chat_home::{Home, PartRef};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::App;
use crate::chat_store::{Error, Store, now_unix};
use crate::coder_sync::{answer, line, refused, stored};

/// The record beside the account's chats that holds every row.
pub(crate) const KEY: &str = "sessions/index.json";
/// The most rows one account keeps; the least recently active go first.
pub(crate) const MAX_ROWS: usize = 400;
/// The most rows one publish may carry.
pub(crate) const MAX_PER_COMPUTER: usize = 200;
/// The most computers one account's record keeps.
pub(crate) const MAX_COMPUTERS: usize = 32;
const MAX_TITLE: usize = 256;
const MAX_RECAP: usize = 512;
const MAX_NAME: usize = 64;
const MAX_FOLDER: usize = 128;
/// The most messages one transcript copy keeps (the newest).
pub(crate) const MAX_MESSAGES: usize = 400;
/// The longest one copied message may be, in bytes.
pub(crate) const MAX_TEXT: usize = 16 * 1024;
/// The largest upload of a copy.
pub(crate) const MAX_BODY: usize = 4 * 1024 * 1024;
/// A computer that published within this long is online.
pub(crate) const ONLINE_SECONDS: u64 = 90;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/history/sessions", get(list))
        .route("/v1/history/sessions/{id}", get(one))
        .route(
            "/v1/history/sessions/computers/{device}",
            put(publish).delete(forget),
        )
        .route("/v1/history/sessions/{id}/transcript", put(choose_chat))
        .route(
            "/v1/history/sessions/projects/{project}/transcript",
            put(choose_project),
        )
        .route("/v1/history/sessions/{id}/messages", put(upload))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY))
}

/// What a session is doing, as its engine says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Status {
    Idle,
    Working,
    AwaitingInput,
    Errored,
}

impl Status {
    /// What the web shows, or nothing for an idle session.
    pub(crate) fn label(self) -> Option<&'static str> {
        match self {
            Self::Idle => None,
            Self::Working => Some("Working"),
            Self::AwaitingInput => Some("Waiting for you"),
            Self::Errored => Some("Failed"),
        }
    }
}

/// One session's summary. No transcript content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Row {
    pub id: String,
    /// The computer's device id, its chat home.
    pub device: String,
    /// The computer's name, for showing.
    pub computer: String,
    pub title: String,
    /// The account project (`prj_…`) the session belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The folder's name on the computer, for showing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    pub status: Status,
    pub last_activity_unix: u64,
    #[serde(default)]
    pub recap: String,
    /// The app's workspace locator, which its `openagents://` links need.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// The web chat that continues this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continued_by: Option<PartRef>,
}

impl Row {
    pub(crate) fn home(&self) -> Home {
        Home::Computer(self.device.clone())
    }

    /// The link that opens the session in the OpenAgents app, when the
    /// host said which workspace holds it.
    pub(crate) fn app_link(&self) -> Option<String> {
        self.workspace
            .as_ref()
            .map(|w| format!("openagents://open/chat/{}?workspace={w}", self.id))
    }
}

/// Every row of one account, and its transcript choices.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Index {
    #[serde(default)]
    pub rows: BTreeMap<String, Row>,
    /// When each computer (by device id) last published.
    #[serde(default)]
    pub published: BTreeMap<String, u64>,
    #[serde(default)]
    pub transcript_chats: BTreeSet<String>,
    #[serde(default)]
    pub transcript_projects: BTreeSet<String>,
}

impl Index {
    /// Whether the session's transcript may be on the web.
    pub(crate) fn transcript(&self, row: &Row) -> bool {
        self.transcript_chats.contains(&row.id)
            || row
                .project
                .as_ref()
                .is_some_and(|p| self.transcript_projects.contains(p))
    }

    pub(crate) fn online(&self, row: &Row, now: u64) -> bool {
        self.published
            .get(&row.device)
            .is_some_and(|at| now.saturating_sub(*at) <= ONLINE_SECONDS)
    }

    /// Rows newest first.
    pub(crate) fn newest(&self) -> Vec<&Row> {
        let mut rows: Vec<_> = self.rows.values().collect();
        rows.sort_by(|a, b| {
            b.last_activity_unix
                .cmp(&a.last_activity_unix)
                .then_with(|| a.id.cmp(&b.id))
        });
        rows
    }

    fn trim(&mut self) {
        if self.rows.len() > MAX_ROWS {
            let keep: BTreeSet<String> = self
                .newest()
                .into_iter()
                .take(MAX_ROWS)
                .map(|r| r.id.clone())
                .collect();
            self.rows.retain(|id, _| keep.contains(id));
        }
        while self.published.len() > MAX_COMPUTERS {
            let Some(oldest) = self
                .published
                .iter()
                .min_by_key(|(_, at)| **at)
                .map(|(d, _)| d.clone())
            else {
                break;
            };
            self.published.remove(&oldest);
            self.rows.retain(|_, r| r.device != oldest);
        }
        let ids: BTreeSet<&String> = self.rows.keys().collect();
        self.transcript_chats.retain(|id| ids.contains(id));
    }
}

/// One row as the API and the web read it.
pub(crate) fn view(index: &Index, row: &Row, now: u64, shared: bool) -> Value {
    json!({
        "id": row.id, "source": "session", "home": row.home(), "title": row.title,
        "computer": row.computer, "project": row.project, "folder": row.folder,
        "status": row.status, "last_activity_unix": row.last_activity_unix,
        "updated_unix": row.last_activity_unix, "recap": row.recap,
        "online": index.online(row, now), "transcript": shared,
        "app_link": row.app_link(), "continued_by": row.continued_by,
    })
}

fn key(owner: &str) -> Result<String, Error> {
    Store::owner_key(owner, KEY)
}

fn copy_key(owner: &str, id: &str) -> Result<String, Error> {
    if !chat_home::valid_id(id) {
        return Err(Error::Invalid("The session id is invalid."));
    }
    Store::owner_key(owner, &format!("sessions/{id}.json"))
}

/// The account's rows; none saved is empty.
pub(crate) async fn load(store: &Store, owner: &str) -> Result<Index, Error> {
    Ok(read(store, owner).await?.0)
}

async fn read(store: &Store, owner: &str) -> Result<(Index, Option<String>), Error> {
    match store.read_key(&key(owner)?).await? {
        Some((bytes, generation)) => Ok((
            serde_json::from_slice(&bytes)
                .map_err(|_| Error::Unavailable("The session list could not be read."))?,
            Some(generation),
        )),
        None => Ok((Index::default(), None)),
    }
}

/// Change the account's rows with `change`, which says whether it changed
/// anything. Returns the record as it now stands.
pub(crate) async fn update<F>(store: &Store, owner: &str, mut change: F) -> Result<Index, Error>
where
    F: FnMut(&mut Index) -> bool,
{
    for _ in 0..6 {
        let (mut index, generation) = read(store, owner).await?;
        if !change(&mut index) {
            return Ok(index);
        }
        index.trim();
        let bytes = serde_json::to_vec(&index)
            .map_err(|_| Error::Invalid("The session list could not be saved."))?;
        match store
            .write_key(&key(owner)?, bytes, generation.as_deref())
            .await
        {
            Ok(_) => return Ok(index),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}

/// One message of a transcript copy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Copied {
    pub role: CopiedRole,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CopiedRole {
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Copy {
    pub messages: Vec<Copied>,
    pub saved_unix: u64,
}

/// The transcript copy of session `id`, when the host uploaded one.
pub(crate) async fn copy(store: &Store, owner: &str, id: &str) -> Result<Option<Copy>, Error> {
    match store.read_key(&copy_key(owner, id)?).await? {
        Some((bytes, _)) => Ok(serde_json::from_slice(&bytes).ok()),
        None => Ok(None),
    }
}

async fn drop_copy(store: &Store, owner: &str, id: &str) -> Result<(), Error> {
    let key = copy_key(owner, id)?;
    for _ in 0..4 {
        let Some((_, generation)) = store.read_key(&key).await? else {
            return Ok(());
        };
        match store.delete_key(&key, &generation).await {
            Ok(_) => return Ok(()),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}

/// Remove computer `name` (its device id or its name) and its rows from
/// the account, with every transcript copy they had. Returns whether it
/// had any.
pub(crate) async fn forget_computer(store: &Store, owner: &str, name: &str) -> Result<bool, Error> {
    let before = load(store, owner).await?;
    let gone: Vec<String> = before
        .rows
        .values()
        .filter(|r| r.device == name || r.computer == name)
        .map(|r| r.id.clone())
        .collect();
    if gone.is_empty() && !before.published.contains_key(name) {
        return Ok(false);
    }
    update(store, owner, |index| {
        let rows = index.rows.len();
        index
            .rows
            .retain(|_, r| r.device != name && r.computer != name);
        let published = index.published.remove(name).is_some();
        published || rows != index.rows.len()
    })
    .await?;
    for id in gone {
        drop_copy(store, owner, &id).await?;
    }
    Ok(true)
}

/// What a host sends for one session.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Published {
    id: String,
    title: String,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    folder: Option<String>,
    status: Status,
    last_activity_unix: u64,
    #[serde(default)]
    recap: String,
    #[serde(default)]
    workspace: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Publish {
    name: String,
    sessions: Vec<Published>,
}

/// Screened and bounded text: credential-shaped parts are replaced.
fn clean(text: &str, bytes: usize) -> String {
    let mut text = secret_screen::redact(&line(text, bytes));
    while text.len() > bytes {
        text.pop();
    }
    text
}

fn valid_device(device: &str) -> bool {
    Home::computer(device).is_some()
}

fn valid_workspace(w: &str) -> bool {
    w.len() == 16
        && w.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn invalid(message: &str) -> Response {
    refused(StatusCode::BAD_REQUEST, "invalid", message)
}

fn missing() -> Response {
    refused(StatusCode::NOT_FOUND, "not_found", "No such session.")
}

/// Rows from a host, checked. `Err` names what's wrong.
fn checked(device: &str, body: &[u8]) -> Result<(String, Vec<Row>), &'static str> {
    let publish: Publish =
        serde_json::from_slice(body).map_err(|_| "Send the computer's name and its sessions.")?;
    let name = line(&publish.name, MAX_NAME);
    if name.is_empty() {
        return Err("Send the computer's name.");
    }
    if publish.sessions.len() > MAX_PER_COMPUTER {
        return Err("Send at most 200 sessions.");
    }
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for s in publish.sessions {
        if !chat_home::valid_id(&s.id) || !seen.insert(s.id.clone()) {
            return Err("Each session needs its own id.");
        }
        if s.project
            .as_deref()
            .is_some_and(|p| !oa_auth::repos::project_id(p))
        {
            return Err("A project id is invalid.");
        }
        if s.workspace.as_deref().is_some_and(|w| !valid_workspace(w)) {
            return Err("A workspace locator is invalid.");
        }
        let title = clean(&s.title, MAX_TITLE);
        rows.push(Row {
            id: s.id,
            device: device.to_owned(),
            computer: name.clone(),
            title: if title.is_empty() {
                "Untitled chat".into()
            } else {
                title
            },
            project: s.project,
            folder: s
                .folder
                .map(|f| clean(&f, MAX_FOLDER))
                .filter(|f| !f.is_empty()),
            status: s.status,
            last_activity_unix: s.last_activity_unix.min(now_unix() + 300),
            recap: clean(&s.recap, MAX_RECAP),
            workspace: s.workspace,
            continued_by: None,
        });
    }
    Ok((name, rows))
}

async fn publish(
    State(app): State<App>,
    headers: HeaderMap,
    Path(device): Path<String>,
    body: Bytes,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    if !valid_device(&device) {
        return invalid("Use the computer's device id.");
    }
    let (_, rows) = match checked(&device, &body) {
        Ok(v) => v,
        Err(message) => return invalid(message),
    };
    let now = now_unix();
    let ids: BTreeSet<String> = rows.iter().map(|r| r.id.clone()).collect();
    let mut dropped = Vec::new();
    let result = update(&app.config.chat_store, &owner, |index| {
        dropped = index
            .rows
            .values()
            .filter(|r| r.device == device && !ids.contains(&r.id))
            .map(|r| r.id.clone())
            .collect();
        index.rows.retain(|_, r| r.device != device);
        for row in &rows {
            let mut row = row.clone();
            // Another computer's session with this id stays its own.
            if let Some(theirs) = index.rows.get(&row.id) {
                if theirs.device != device {
                    continue;
                }
            }
            row.continued_by = None;
            index.rows.insert(row.id.clone(), row);
        }
        index.published.insert(device.clone(), now);
        true
    })
    .await;
    // Keep each session's web continuation across publishes.
    let index = match result {
        Ok(index) => index,
        Err(e) => return stored(&e),
    };
    let index = match restore_links(&app.config.chat_store, &owner, index).await {
        Ok(index) => index,
        Err(e) => return stored(&e),
    };
    for id in dropped {
        if let Err(e) = drop_copy(&app.config.chat_store, &owner, &id).await {
            return stored(&e);
        }
    }
    let transcripts: Vec<&String> = index
        .rows
        .values()
        .filter(|r| r.device == device && index.transcript(r))
        .map(|r| &r.id)
        .collect();
    answer(StatusCode::OK, json!({"transcripts": transcripts}))
}

/// Continuations are kept in their own record, so a host's publish never
/// drops one it doesn't know about.
const LINKS: &str = "sessions/links.json";

async fn links(
    store: &Store,
    owner: &str,
) -> Result<(BTreeMap<String, PartRef>, Option<String>), Error> {
    match store.read_key(&Store::owner_key(owner, LINKS)?).await? {
        Some((bytes, generation)) => Ok((
            serde_json::from_slice(&bytes).unwrap_or_default(),
            Some(generation),
        )),
        None => Ok((BTreeMap::new(), None)),
    }
}

async fn restore_links(store: &Store, owner: &str, mut index: Index) -> Result<Index, Error> {
    let (links, _) = links(store, owner).await?;
    for (id, part) in links {
        if let Some(row) = index.rows.get_mut(&id) {
            row.continued_by = Some(part);
        }
    }
    Ok(index)
}

/// The account's rows with their web continuations.
pub(crate) async fn load_linked(store: &Store, owner: &str) -> Result<Index, Error> {
    let index = load(store, owner).await?;
    restore_links(store, owner, index).await
}

/// Record that web chat `chat` continues session `id`. False when the
/// session already continues in another chat.
pub(crate) async fn link(store: &Store, owner: &str, id: &str, chat: &str) -> Result<bool, Error> {
    let key = Store::owner_key(owner, LINKS)?;
    for _ in 0..6 {
        let (mut links, generation) = links(store, owner).await?;
        if let Some(part) = links.get(id) {
            return Ok(part.id == chat);
        }
        links.insert(
            id.to_owned(),
            PartRef {
                id: chat.to_owned(),
                home: Home::Cloud,
            },
        );
        if links.len() > MAX_ROWS * 2 {
            let index = load(store, owner).await?;
            links.retain(|k, _| k == id || index.rows.contains_key(k));
        }
        let bytes = serde_json::to_vec(&links)
            .map_err(|_| Error::Invalid("The session links could not be saved."))?;
        match store.write_key(&key, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(true),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}

async fn forget(
    State(app): State<App>,
    headers: HeaderMap,
    Path(device): Path<String>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    if !valid_device(&device) {
        return invalid("Use the computer's device id.");
    }
    match forget_computer(&app.config.chat_store, &owner, &device).await {
        Ok(removed) => answer(StatusCode::OK, json!({"removed": removed})),
        Err(e) => stored(&e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Choice {
    on: bool,
}

async fn choose_chat(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let Ok(Choice { on }) = serde_json::from_slice(&body) else {
        return invalid("Send on: true or false.");
    };
    if !chat_home::valid_id(&id) {
        return missing();
    }
    let store = &app.config.chat_store;
    let mut known = false;
    let result = update(store, &owner, |index| {
        known = index.rows.contains_key(&id);
        known
            && if on {
                index.transcript_chats.insert(id.clone())
            } else {
                index.transcript_chats.remove(&id)
            }
    })
    .await;
    let index = match result {
        Ok(index) => index,
        Err(e) => return stored(&e),
    };
    if !known {
        return missing();
    }
    let row = &index.rows[&id];
    if !index.transcript(row) {
        if let Err(e) = drop_copy(store, &owner, &id).await {
            return stored(&e);
        }
    }
    answer(
        StatusCode::OK,
        json!({"on": on, "transcript": index.transcript(row)}),
    )
}

async fn choose_project(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
    body: Bytes,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let Ok(Choice { on }) = serde_json::from_slice(&body) else {
        return invalid("Send on: true or false.");
    };
    if !oa_auth::repos::project_id(&project) {
        return refused(StatusCode::NOT_FOUND, "not_found", "No such project.");
    }
    let store = &app.config.chat_store;
    let index = match update(store, &owner, |index| {
        if on {
            index.transcript_projects.insert(project.clone())
        } else {
            index.transcript_projects.remove(&project)
        }
    })
    .await
    {
        Ok(index) => index,
        Err(e) => return stored(&e),
    };
    if !on {
        for row in index
            .rows
            .values()
            .filter(|r| r.project.as_deref() == Some(project.as_str()) && !index.transcript(r))
        {
            if let Err(e) = drop_copy(store, &owner, &row.id).await {
                return stored(&e);
            }
        }
    }
    answer(StatusCode::OK, json!({"on": on}))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Upload {
    messages: Vec<Copied>,
}

async fn upload(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    if !chat_home::valid_id(&id) {
        return missing();
    }
    let Ok(Upload { messages }) = serde_json::from_slice(&body) else {
        return invalid("Send the messages as role and text.");
    };
    let store = &app.config.chat_store;
    let index = match load(store, &owner).await {
        Ok(index) => index,
        Err(e) => return stored(&e),
    };
    let Some(row) = index.rows.get(&id) else {
        return missing();
    };
    if !index.transcript(row) {
        return refused(
            StatusCode::CONFLICT,
            "transcript_off",
            "Turn on the transcript for this chat first.",
        );
    }
    let skip = messages.len().saturating_sub(MAX_MESSAGES);
    let copy = Copy {
        messages: messages
            .into_iter()
            .skip(skip)
            .map(|m| {
                let mut text = secret_screen::redact(prefix(&m.text, MAX_TEXT));
                while text.len() > MAX_TEXT {
                    text.pop();
                }
                Copied { role: m.role, text }
            })
            .collect(),
        saved_unix: now_unix(),
    };
    let saved = copy.messages.len();
    let key = match copy_key(&owner, &id) {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    let Ok(bytes) = serde_json::to_vec(&copy) else {
        return invalid("Send the messages as role and text.");
    };
    for _ in 0..4 {
        let generation = match store.read_key(&key).await {
            Ok(found) => found.map(|(_, g)| g),
            Err(e) => return stored(&e),
        };
        match store
            .write_key(&key, bytes.clone(), generation.as_deref())
            .await
        {
            Ok(_) => return answer(StatusCode::OK, json!({"saved": saved})),
            Err(Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    stored(&Error::Conflict)
}

fn prefix(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[derive(Default, Deserialize)]
struct Scope {
    project: Option<String>,
}

/// One row the caller may see, whose record it is in, and whether its
/// transcript is shared with the caller.
pub(crate) struct Visible {
    pub owner: String,
    pub index: Index,
    pub id: String,
}

impl Visible {
    pub(crate) fn row(&self) -> &Row {
        &self.index.rows[&self.id]
    }

    /// The owner sees a copy when either choice is on; a project member
    /// only when the project's choice is.
    pub(crate) fn transcript(&self, viewer: &str) -> bool {
        shared_with(&self.index, self.row(), self.owner == viewer)
    }
}

/// The owner sees a copy when either choice is on; a project member only
/// when the project's choice is.
fn shared_with(index: &Index, row: &Row, own: bool) -> bool {
    if own {
        index.transcript(row)
    } else {
        row.project
            .as_ref()
            .is_some_and(|p| index.transcript_projects.contains(p))
    }
}

/// The records the caller may read rows from: their own, or, for a shared
/// project, every member's.
async fn records(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    project: Option<&str>,
) -> Result<Vec<(String, Index)>, Response> {
    let store = &app.config.chat_store;
    let Some(project) = project else {
        return Ok(vec![(
            owner.to_owned(),
            load_linked(store, owner).await.map_err(|e| stored(&e))?,
        )]);
    };
    let access = crate::projects::api::access(app, headers, project).await?;
    let mut owners = BTreeSet::from([owner.to_owned()]);
    if crate::projects::shared_chats::shared(&access.project) {
        owners.extend(
            crate::projects::shared_chats::starters(app, headers, &access.project)
                .await
                .into_keys(),
        );
    }
    let mut found = Vec::new();
    for member in owners {
        let mut index = load_linked(store, &member).await.map_err(|e| stored(&e))?;
        index
            .rows
            .retain(|_, r| r.project.as_deref() == Some(access.project.id.as_str()));
        found.push((member, index));
    }
    Ok(found)
}

/// The rows the caller sees, newest first, as the API writes them.
pub(crate) async fn listed(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    project: Option<&str>,
) -> Result<Vec<Value>, Response> {
    let now = now_unix();
    let mut rows = Vec::new();
    for (member, index) in records(app, headers, owner, project).await? {
        for row in index.newest() {
            let shared = shared_with(&index, row, member == owner);
            rows.push((row.last_activity_unix, view(&index, row, now, shared)));
        }
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(rows.into_iter().map(|(_, v)| v).collect())
}

/// Session `id` as the caller may see it.
pub(crate) async fn find(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    id: &str,
    project: Option<&str>,
) -> Result<Visible, Response> {
    if !chat_home::valid_id(id) {
        return Err(missing());
    }
    for (member, index) in records(app, headers, owner, project).await? {
        if index.rows.contains_key(id) {
            return Ok(Visible {
                owner: member,
                index,
                id: id.to_owned(),
            });
        }
    }
    Err(missing())
}

async fn list(State(app): State<App>, headers: HeaderMap, Query(scope): Query<Scope>) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    match listed(&app, &headers, &owner, scope.project.as_deref()).await {
        Ok(rows) => answer(StatusCode::OK, json!({"data": rows})),
        Err(r) => r,
    }
}

async fn one(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(scope): Query<Scope>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let found = match find(&app, &headers, &owner, &id, scope.project.as_deref()).await {
        Ok(f) => f,
        Err(r) => return r,
    };
    let shared = found.transcript(&owner);
    let messages = if shared {
        match copy(&app.config.chat_store, &found.owner, &id).await {
            Ok(copy) => copy.map(|c| c.messages),
            Err(e) => return stored(&e),
        }
    } else {
        None
    };
    answer(
        StatusCode::OK,
        json!({"session": view(&found.index, found.row(), now_unix(), shared), "messages": messages}),
    )
}

/// The routes' public contract, beside the saved-history schemas.
pub(crate) fn openapi(doc: &mut Value) {
    let s = |name: &str| json!({"$ref": format!("#/components/schemas/{name}")});
    let schemas = doc["components"]["schemas"].as_object_mut().unwrap();
    schemas.insert("HistorySession".into(), json!({"type":"object","required":["id","source","home","title","computer","project","folder","status","last_activity_unix","updated_unix","recap","online","transcript","app_link","continued_by"],"properties":{
        "id":{"type":"string","format":"uuid"},"source":{"const":"session"},"home":s("ChatHome"),"title":{"type":"string","maxLength":MAX_TITLE},
        "computer":{"type":"string","maxLength":MAX_NAME},"project":{"type":["string","null"]},"folder":{"type":["string","null"]},
        "status":{"enum":["idle","working","awaiting_input","errored"]},"last_activity_unix":{"type":"integer","minimum":0},"updated_unix":{"type":"integer","minimum":0},
        "recap":{"type":"string","maxLength":MAX_RECAP},"online":{"type":"boolean"},"transcript":{"type":"boolean","description":"The transcript is on for this caller, so a copy may be read."},
        "app_link":{"type":["string","null"],"description":"Opens the chat in the OpenAgents app."},
        "continued_by":{"oneOf":[{"type":"null"},s("ChatPartRef")]}},
        "description":"A computer session's summary. It holds no transcript."}));
    schemas.insert("HistorySessions".into(), json!({"type":"object","required":["data"],"properties":{"data":{"type":"array","maxItems":MAX_ROWS,"items":s("HistorySession")}}}));
    schemas.insert("HistorySessionMessage".into(), json!({"type":"object","additionalProperties":false,"required":["role","text"],"properties":{"role":{"enum":["user","assistant","tool"]},"text":{"type":"string","maxLength":MAX_TEXT}}}));
    schemas.insert("HistorySessionDetail".into(), json!({"type":"object","required":["session","messages"],"properties":{"session":s("HistorySession"),"messages":{"oneOf":[{"type":"null"},{"type":"array","maxItems":MAX_MESSAGES,"items":s("HistorySessionMessage")}]}}}));
    schemas.insert("HistorySessionPublish".into(), json!({"type":"object","additionalProperties":false,"required":["name","sessions"],"properties":{"name":{"type":"string","maxLength":MAX_NAME},"sessions":{"type":"array","maxItems":MAX_PER_COMPUTER,"items":{"type":"object","additionalProperties":false,"required":["id","title","status","last_activity_unix"],"properties":{
        "id":{"type":"string","format":"uuid"},"title":{"type":"string"},"project":{"type":["string","null"]},"folder":{"type":["string","null"]},
        "status":{"enum":["idle","working","awaiting_input","errored"]},"last_activity_unix":{"type":"integer","minimum":0},"recap":{"type":"string"},
        "workspace":{"type":["string","null"],"pattern":"^[0-9a-f]{16}$"}}}}}}));
    schemas.insert("HistorySessionChoice".into(), json!({"type":"object","additionalProperties":false,"required":["on"],"properties":{"on":{"type":"boolean"}}}));
    schemas.insert("HistorySessionUpload".into(), json!({"type":"object","additionalProperties":false,"required":["messages"],"properties":{"messages":{"type":"array","items":s("HistorySessionMessage")}}}));
    if let Some(list) = schemas.get_mut("HistoryList") {
        list["properties"]["sessions"] = json!({"type":"array","maxItems":MAX_ROWS,"items":s("HistorySession"),"description":"Computer sessions, newest first, on the first page only. They never join data or its paging."});
    }
    let error = json!({"description":"Invalid, signed out, missing, transcript off, throttled, or unavailable.","content":{"application/json":{"schema":s("AppAccountError")}}});
    let ok = |schema: Value, what: &str| json!({"description":what,"content":{"application/json":{"schema":schema}}});
    let id =
        json!({"name":"id","in":"path","required":true,"schema":{"type":"string","format":"uuid"}});
    let project = json!({"name":"project","in":"query","description":"A project the caller is in. A shared project lists every member's rows in it.","schema":{"type":"string"}});
    let device = json!({"name":"device","in":"path","required":true,"schema":{"type":"string","pattern":"^[A-Za-z0-9._-]{1,128}$"}});
    let body =
        |schema: &str| json!({"required":true,"content":{"application/json":{"schema":s(schema)}}});
    let op = |operation: &str,
              parameters: Vec<Value>,
              request: Option<Value>,
              success: Value,
              description: &str| {
        let mut responses = json!({"200": success});
        for status in ["400", "401", "404", "409", "429", "503"] {
            responses[status] = error.clone();
        }
        let mut op = json!({"operationId":operation,"tags":["Saved history"],"security":[{"appBearer":[]}],"parameters":parameters,"responses":responses,"description":description});
        if let Some(request) = request {
            op["requestBody"] = request;
        }
        op
    };
    doc["paths"]["/v1/history/sessions"] = json!({"get": op("listComputerSessions", vec![project.clone()], None, ok(s("HistorySessions"), "Session rows"), "The caller's computer sessions, newest first, or a project's.")});
    doc["paths"]["/v1/history/sessions/{id}"] = json!({"get": op("readComputerSession", vec![id.clone(), project], None, ok(s("HistorySessionDetail"), "One session"), "One session row, and its transcript copy when the transcript is on for the caller.")});
    doc["paths"]["/v1/history/sessions/computers/{device}"] = json!({
        "put": op("publishComputerSessions", vec![device.clone()], Some(body("HistorySessionPublish")), ok(json!({"type":"object","required":["transcripts"],"properties":{"transcripts":{"type":"array","items":{"type":"string","format":"uuid"}}}}), "The sessions whose transcript is on"), "A host's current rows for this computer, replacing the ones it published before. Publishing also marks the computer online for 90 seconds."),
        "delete": op("forgetComputerSessions", vec![device], None, ok(json!({"type":"object","required":["removed"],"properties":{"removed":{"type":"boolean"}}}), "Removed"), "Remove this computer's rows and transcript copies."),
    });
    doc["paths"]["/v1/history/sessions/{id}/transcript"] = json!({"put": op("setSessionTranscript", vec![id.clone()], Some(body("HistorySessionChoice")), ok(json!({"type":"object","required":["on","transcript"],"properties":{"on":{"type":"boolean"},"transcript":{"type":"boolean"}}}), "The choice"), "Turn one chat's transcript on or off on the web. Off by default; off deletes the copy.")});
    doc["paths"]["/v1/history/sessions/projects/{project}/transcript"] = json!({"put": op("setProjectSessionTranscripts", vec![json!({"name":"project","in":"path","required":true,"schema":{"type":"string"}})], Some(body("HistorySessionChoice")), ok(s("HistorySessionChoice"), "The choice"), "Turn the transcripts of every session in a project on or off. Shared project members read a copy only by this choice.")});
    doc["paths"]["/v1/history/sessions/{id}/messages"] = json!({"put": op("uploadSessionTranscript", vec![id], Some(body("HistorySessionUpload")), ok(json!({"type":"object","required":["saved"],"properties":{"saved":{"type":"integer","minimum":0}}}), "Saved"), "Replace the read-only transcript copy. Refused with transcript_off while the transcript is off. Text is bounded and screened.")});
}
