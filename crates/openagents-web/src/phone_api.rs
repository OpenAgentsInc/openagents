//! The account's chats, computers, and running agents for the apps
//! (#11107, #11165): the phone reads and answers the same chats the web
//! sidebar lists, and supervises what Coder runs on each computer.
//!
//! Every route takes the app's own token (`Authorization: Bearer sess_…`,
//! [`crate::coder_sync::owner`]) and answers only that account's records.
//!
//! | Route | What |
//! | --- | --- |
//! | `GET /v1/threads` | `{threads: [Row], computers: [Computer]}`: the account's chats (web, terminal, phone), not archived, pinned first, then newest |
//! | `GET /v1/threads/{id}` | `{thread: Row, messages: [{role, text}], earlier, waiting}`: the newest [`SHOWN_MESSAGES`] messages |
//! | `POST /v1/threads/{id}/messages` `{request_id, text, files?}` | Reply. A terminal chat queues it for Coder on its computer (`202 {queued}`); a web chat is answered here (`202 {answering}`), with the files added below (`files`: their ids, up to four); a phone chat refuses (`409 phone`) |
//! | `POST /v1/threads/{id}/files?name=` (the file's bytes) | Add an image, PDF, or text file to a web chat for the next reply (`201 {id, name, kind, size, url}`; #11174, [`crate::chat_files`]) |
//! | `GET`/`DELETE /v1/threads/{id}/files/{file}` | A file of the chat; remove one not sent yet (`204`) |
//! | `GET /v1/computers` | `{computers: [Computer]}` |
//! | `GET`/`PUT /v1/computers/{name}/sync` `{choice}` | The computer's (or phone's) sync choice, as `/coder/sync` |
//! | `POST /v1/computers/{name}/activity` `{items: [Item]}` | Coder's running work on the computer; answers `{commands: [Command]}`, each handed out once |
//! | `GET /v1/agents` | `{computers: [{name, online, updated_unix, items}]}` |
//! | `POST /v1/agents/actions` `{request_id, computer, item, action, question?, text?}` | Stop, approve, deny, or message an item: `202 {queued}` |
//!
//! A phone chat is a synced chat ([`crate::chat_store::Terminal`]) whose
//! session starts with [`PHONE_PREFIX`], uploaded through
//! `PUT /coder/sessions/{session}` with the phone's name as the computer.
//! The running work lives in one small object per account beside its
//! chats ([`AGENTS_KEY`]), never in the chat or computer records.

use std::collections::BTreeMap;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::App;
use crate::chat_store::{Computers, Conversation, Error, Store, SyncChoice, now_unix};
use crate::coder_sync::{self, Queued, answer, line, online, refused, stored};
use crate::pages::chat::{AppSent, follow_from_app, line_two};

/// The session prefix of a chat synced from the phone.
pub(crate) const PHONE_PREFIX: &str = "phone-";
/// The most chats `GET /v1/threads` lists.
pub(crate) const SHOWN_THREADS: usize = 200;
/// The most messages `GET /v1/threads/{id}` returns (the newest).
pub(crate) const SHOWN_MESSAGES: usize = 200;
/// The longest reply, in characters (the web composer's limit).
const MAX_REPLY_CHARS: usize = 4_000;
/// The account's running-work object, under its folder.
pub(crate) const AGENTS_KEY: &str = "agents/activity.json";
const AGENTS_SCHEMA: &str = "openagents.web.agents.v1";
/// The most items one computer reports.
pub(crate) const MAX_ITEMS: usize = 32;
/// The most computers the record keeps (the least recently updated go).
const MAX_BOARDS: usize = 16;
/// The most commands that wait at once.
const MAX_COMMANDS: usize = 64;
/// A command not taken in this long is dropped.
const COMMAND_TTL: u64 = 600;
/// An unchanged report is still written this often, for its time.
const REWRITE_EVERY: u64 = 20;
/// How many handed-out command ids are remembered, so a resent action is
/// queued once.
const MAX_TAKEN_IDS: usize = 64;
/// What a text that looked like it held a credential says instead.
const LEFT_OUT: &str = "(Left out: this looked like it held a password or key.)";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/threads", get(threads))
        .route("/v1/threads/{id}", get(thread))
        .route("/v1/threads/{id}/messages", post(reply))
        .route(
            "/v1/threads/{id}/files",
            post(add_file).layer(axum::extract::DefaultBodyLimit::max(
                crate::chat_files::MAX_FILE_BYTES + 64 * 1024,
            )),
        )
        .route(
            "/v1/threads/{id}/files/{file}",
            get(read_file).delete(remove_file),
        )
        .route("/v1/computers", get(computers_route))
        .route("/v1/computers/{name}/sync", get(sync_get).put(sync_put))
        .route("/v1/computers/{name}/activity", post(activity))
        .route("/v1/agents", get(agents))
        .route("/v1/agents/actions", post(action))
}

/// Whether `path` is one of these routes (`crate::upstream::owned`).
pub(crate) fn owns(path: &str) -> bool {
    ["/v1/threads", "/v1/computers", "/v1/agents"]
        .iter()
        .any(|prefix| {
            path.strip_prefix(prefix)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        })
}

/// Whether a synced chat's session is a phone's.
pub(crate) fn phone_session(session: &str) -> bool {
    session.starts_with(PHONE_PREFIX)
}

/// One chat as the apps list it.
fn row(chat: &Conversation, computers: &Computers) -> Value {
    let terminal = chat.terminal.as_ref();
    let phone = terminal.is_some_and(|t| phone_session(&t.session));
    let surface = match terminal {
        None => "web",
        Some(_) if phone => "phone",
        Some(_) => "terminal",
    };
    let online_now = terminal.is_none_or(|t| online(computers, &t.computer));
    let can_reply = match terminal {
        None => {
            chat.pending.is_none()
                && !chat.requests.iter().any(|r| r.cloud.is_some())
                && chat.tasks.iter().all(|t| t.state.finished())
        }
        Some(_) if phone => false,
        Some(_) => online_now,
    };
    json!({
        "id": chat.id,
        "title": chat.title,
        "line": line_two(chat, true),
        "surface": surface,
        "computer": terminal.map(|t| t.computer.clone()),
        "session": terminal.map(|t| t.session.clone()),
        "updated_unix": chat.updated_unix,
        "working": chat.working(),
        "online": online_now,
        "can_reply": can_reply,
        "pinned": chat.pinned_unix.is_some(),
    })
}

/// The account's computers (and phones), as the apps list them.
fn computer_rows(computers: &Computers) -> Vec<Value> {
    let mut names: Vec<&String> = computers.seen.keys().chain(computers.sync.keys()).collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .map(|name| {
            json!({
                "name": name,
                "online": online(computers, name),
                "seen_unix": computers.seen.get(name),
                "sync": computers.sync.get(name).map(|choice| choice.as_str()),
            })
        })
        .collect()
}

/// The chats `GET /v1/threads` lists, in order.
pub(crate) async fn listed(store: &Store, owner: &str) -> Result<(Vec<Value>, Computers), Error> {
    let computers = store.computers(owner).await?;
    let mut chats: Vec<Conversation> = store
        .list(owner)
        .await?
        .into_iter()
        .filter(|chat| chat.archived_unix.is_none())
        .collect();
    chats.sort_by_key(|chat| {
        (
            chat.pinned_unix.is_none(),
            std::cmp::Reverse(chat.pinned_unix.unwrap_or(0)),
            std::cmp::Reverse(chat.updated_unix),
        )
    });
    chats.truncate(SHOWN_THREADS);
    Ok((
        chats.iter().map(|chat| row(chat, &computers)).collect(),
        computers,
    ))
}

async fn threads(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match listed(&app.config.chat_store, &owner).await {
        Ok((rows, computers)) => answer(
            StatusCode::OK,
            json!({"threads": rows, "computers": computer_rows(&computers)}),
        ),
        Err(error) => stored(&error),
    }
}

fn not_found() -> Response {
    refused(StatusCode::NOT_FOUND, "not_found", "No such chat.")
}

async fn thread(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let store = &app.config.chat_store;
    let chat = match store.load(&owner, &id).await {
        Ok(Some(loaded)) if !loaded.conversation.deleted() => loaded.conversation,
        Ok(_) | Err(Error::Invalid(_)) => return not_found(),
        Err(error) => return stored(&error),
    };
    let computers = match store.computers(&owner).await {
        Ok(computers) => computers,
        Err(error) => return stored(&error),
    };
    let messages: Vec<&crate::chat_store::Message> = chat
        .messages
        .iter()
        .filter(|message| !message.text.trim().is_empty())
        .collect();
    let earlier = messages.len().saturating_sub(SHOWN_MESSAGES);
    answer(
        StatusCode::OK,
        json!({
            "thread": row(&chat, &computers),
            "messages": messages[earlier..]
                .iter()
                .map(|message| {
                    let files = crate::chat_files::of_message(&chat, message);
                    if files.is_empty() {
                        json!({"role": message.role, "text": message.text})
                    } else {
                        json!({"role": message.role, "text": message.text, "files": files})
                    }
                })
                .collect::<Vec<_>>(),
            "earlier": earlier,
            "waiting": chat.terminal.as_ref().map_or(0, |t| t.replies.len()),
        }),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    request_id: String,
    text: String,
    /// Files added with `POST /v1/threads/{id}/files`, by id.
    #[serde(default)]
    files: Vec<String>,
}

/// A request id: a UUID the app made for this send, so a resend is
/// taken once.
fn valid_request(id: &str) -> bool {
    crate::chat_store::valid_id(id)
}

async fn reply(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let Some(sent) = serde_json::from_slice::<Reply>(&body)
        .ok()
        .filter(|sent| valid_request(&sent.request_id))
    else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {request_id (a UUID), text}.",
        );
    };
    let text = sent.text.trim();
    if text.is_empty() || text.chars().count() > MAX_REPLY_CHARS {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Enter a message of at most 4,000 characters.",
        );
    }
    let store = &app.config.chat_store;
    let chat = match store.load(&owner, &id).await {
        Ok(Some(loaded)) if !loaded.conversation.deleted() => loaded.conversation,
        Ok(_) | Err(Error::Invalid(_)) => return not_found(),
        Err(error) => return stored(&error),
    };
    if let Some(terminal) = &chat.terminal {
        if !sent.files.is_empty() {
            return refused(
                StatusCode::BAD_REQUEST,
                "files",
                crate::chat_files::CODER_CHAT,
            );
        }
        if phone_session(&terminal.session) {
            return refused(
                StatusCode::CONFLICT,
                "phone",
                "This chat lives on your phone.",
            );
        }
        return match coder_sync::queue_reply(store, &owner, &id, &sent.request_id, text).await {
            Ok(Queued::Queued) => answer(StatusCode::ACCEPTED, json!({"queued": true})),
            Ok(Queued::Offline(computer)) => refused(
                StatusCode::CONFLICT,
                "offline",
                &format!("Coder on {computer} isn't online now. Open Coder there to reply."),
            ),
            Ok(Queued::Full) => refused(
                StatusCode::CONFLICT,
                "busy",
                "Wait for Coder to answer your earlier replies.",
            ),
            Ok(Queued::Secret) => refused(
                StatusCode::UNPROCESSABLE_ENTITY,
                "secret",
                "This looks like it holds a password or key, so it wasn't sent.",
            ),
            Ok(Queued::Missing) => not_found(),
            Err(error) => stored(&error),
        };
    }
    if secret_screen::credential_in(text).is_some() {
        return refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "secret",
            "This looks like it holds a password or key, so it wasn't sent.",
        );
    }
    let files = match crate::chat_files::take(store, &owner, &id, &sent.files.join(",")).await {
        Ok(files) => files,
        Err(message) => return refused(StatusCode::BAD_REQUEST, "files", message),
    };
    match follow_from_app(&app, &owner, &id, &sent.request_id, text, files).await {
        AppSent::Answering => answer(StatusCode::ACCEPTED, json!({"answering": true})),
        AppSent::Busy(message) => refused(StatusCode::CONFLICT, "busy", message),
        AppSent::Invalid => refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Enter a message of at most 4,000 characters.",
        ),
        AppSent::Missing => not_found(),
        AppSent::Unavailable => refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "Try again later.",
        ),
    }
}

#[derive(Deserialize)]
struct FileName {
    #[serde(default)]
    name: String,
}

/// `POST /v1/threads/{id}/files?name=`: the web composer's upload
/// ([`crate::chat_files::add`]) for the apps.
async fn add_file(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    axum::extract::Query(named): axum::extract::Query<FileName>,
    body: Bytes,
) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    use crate::chat_files::Refused;
    match crate::chat_files::add(&app.config.chat_store, &owner, &id, &named.name, &body).await {
        Ok(file) => answer(
            StatusCode::CREATED,
            json!({
                "id": file.id,
                "name": file.name,
                "kind": file.kind,
                "size": crate::chat_files::size(file.size),
                "url": format!("/v1/threads/{id}/files/{}", file.id),
            }),
        ),
        Err(Refused::Plain(status, message)) => refused(status, "file", message),
        Err(Refused::Full) => refused(StatusCode::CONFLICT, "full", crate::chat_files::FULL),
        Err(Refused::Stored(error)) => stored(&error),
    }
}

/// `GET /v1/threads/{id}/files/{file}`: one of the chat's files.
async fn read_file(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, file)): Path<(String, String)>,
) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    if !crate::chat_store::valid_id(&id) {
        return not_found();
    }
    match crate::chat_files::read(&app.config.chat_store, &owner, &id, &file).await {
        Ok(Some((found, bytes))) => crate::chat_files::file_response(&found, bytes),
        Ok(None) | Err(Error::Invalid(_)) => not_found(),
        Err(error) => stored(&error),
    }
}

/// `DELETE /v1/threads/{id}/files/{file}`: a file added and not sent.
async fn remove_file(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, file)): Path<(String, String)>,
) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    if !crate::chat_store::valid_id(&id) {
        return not_found();
    }
    use crate::chat_files::Refused;
    match crate::chat_files::unsent_forget(&app.config.chat_store, &owner, &id, &file).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(Refused::Plain(status, message)) => refused(status, "file", message),
        Err(Refused::Full) => refused(StatusCode::CONFLICT, "full", crate::chat_files::FULL),
        Err(Refused::Stored(error)) => stored(&error),
    }
}

async fn computers_route(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match app.config.chat_store.computers(&owner).await {
        Ok(computers) => answer(
            StatusCode::OK,
            json!({"computers": computer_rows(&computers)}),
        ),
        Err(error) => stored(&error),
    }
}

async fn sync_get(
    State(app): State<App>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match coder_sync::choice(&app.config.chat_store, &owner, &name).await {
        Ok(found) => answer(
            StatusCode::OK,
            json!({"choice": found.map(SyncChoice::as_str)}),
        ),
        Err(error) => stored(&error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Choose {
    choice: String,
}

async fn sync_put(
    State(app): State<App>,
    headers: HeaderMap,
    Path(name): Path<String>,
    body: Bytes,
) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let Some(picked) = serde_json::from_slice::<Choose>(&body)
        .ok()
        .and_then(|sent| SyncChoice::parse(&sent.choice))
    else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {choice: all or local}.",
        );
    };
    match coder_sync::choose(&app.config.chat_store, &owner, &name, picked).await {
        Ok(Ok(picked)) => answer(StatusCode::OK, json!({"choice": picked.as_str()})),
        Ok(Err(_)) => refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send the computer's name.",
        ),
        Err(error) => stored(&error),
    }
}

/// A question an item waits on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Question {
    pub id: String,
    pub text: String,
}

/// One piece of running work Coder reports for a computer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Item {
    pub id: String,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub engine: Option<String>,
    pub status: String,
    #[serde(default)]
    pub started_unix: u64,
    #[serde(default)]
    pub finished_unix: Option<u64>,
    #[serde(default)]
    pub cost_usd: Option<f64>,
    #[serde(default)]
    pub tokens: Option<u64>,
    #[serde(default)]
    pub session: Option<String>,
    #[serde(default)]
    pub question: Option<Question>,
    #[serde(default)]
    pub line: Option<String>,
}

const KINDS: [&str; 2] = ["chat", "agent"];
const STATUSES: [&str; 5] = ["working", "asking", "done", "failed", "stopped"];
const ACTIONS: [&str; 4] = ["stop", "approve", "deny", "message"];

/// Bounded, single-line, and screened: a text that looks like it holds a
/// credential is replaced.
fn clean(value: &str, limit: usize) -> String {
    let text = line(value, limit);
    if secret_screen::credential_in(&text).is_some() {
        LEFT_OUT.to_owned()
    } else {
        text
    }
}

/// Check and bound an item from Coder; `None` refuses it.
fn checked_item(mut item: Item) -> Option<Item> {
    if item.id.is_empty()
        || item.id.len() > 128
        || item.id.chars().any(char::is_control)
        || !KINDS.contains(&item.kind.as_str())
        || !STATUSES.contains(&item.status.as_str())
        || item
            .cost_usd
            .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
    {
        return None;
    }
    item.title = clean(&item.title, 120);
    item.engine = item.engine.map(|e| line(&e, 64)).filter(|e| !e.is_empty());
    item.session = item.session.filter(|s| coder_sync::valid_session(s));
    item.line = item.line.map(|l| clean(&l, 200)).filter(|l| !l.is_empty());
    item.question = item.question.and_then(|mut q| {
        q.id = line(&q.id, 64);
        let text: String = q.text.chars().take(2000).collect();
        q.text = if secret_screen::credential_in(&text).is_some() {
            LEFT_OUT.to_owned()
        } else {
            text
        };
        (!q.id.is_empty()).then_some(q)
    });
    Some(item)
}

/// One computer's last report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Board {
    pub items: Vec<Item>,
    pub updated_unix: u64,
    pub digest: String,
}

/// An action from the phone, waiting for Coder on its computer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Command {
    pub id: String,
    pub item: String,
    pub action: String,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Waiting {
    pub computer: String,
    pub command: Command,
    pub queued_unix: u64,
}

/// The account's running work: each computer's last report, and the
/// actions waiting for them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Agents {
    #[serde(default)]
    pub schema: String,
    #[serde(default)]
    pub boards: BTreeMap<String, Board>,
    #[serde(default)]
    pub commands: Vec<Waiting>,
    /// Ids of actions already handed out, newest last.
    #[serde(default)]
    pub taken: Vec<String>,
}

impl Agents {
    fn prune(&mut self, now: u64) {
        self.commands
            .retain(|waiting| now.saturating_sub(waiting.queued_unix) < COMMAND_TTL);
        while self.commands.len() > MAX_COMMANDS {
            self.commands.remove(0);
        }
        while self.boards.len() > MAX_BOARDS {
            let Some(oldest) = self
                .boards
                .iter()
                .min_by_key(|(_, board)| board.updated_unix)
                .map(|(name, _)| name.clone())
            else {
                break;
            };
            self.boards.remove(&oldest);
        }
        if self.taken.len() > MAX_TAKEN_IDS {
            self.taken.drain(..self.taken.len() - MAX_TAKEN_IDS);
        }
    }
}

/// Read the account's running work and change it with `change` (which
/// says whether it changed anything and what to answer), retrying on a
/// concurrent write.
pub(crate) async fn update_agents<T>(
    store: &Store,
    owner: &str,
    change: impl Fn(&mut Agents) -> (bool, T),
) -> Result<T, Error> {
    let key = Store::owner_key(owner, AGENTS_KEY)?;
    for _ in 0..4 {
        let (mut agents, generation) = match store.read_key(&key).await? {
            Some((bytes, generation)) => (
                serde_json::from_slice::<Agents>(&bytes)
                    .map_err(|_| Error::Corrupt("The running work is invalid."))?,
                Some(generation),
            ),
            None => (Agents::default(), None),
        };
        let (changed, result) = change(&mut agents);
        if !changed {
            return Ok(result);
        }
        agents.schema = AGENTS_SCHEMA.to_owned();
        agents.prune(now_unix());
        let bytes = serde_json::to_vec(&agents)
            .map_err(|_| Error::Invalid("The running work is invalid."))?;
        match store.write_key(&key, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(result),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

/// The account's running work as stored; none is empty.
pub(crate) async fn read_agents(store: &Store, owner: &str) -> Result<Agents, Error> {
    update_agents(store, owner, |agents| (false, agents.clone())).await
}

/// Coder on `computer` reports its items: keep them (when they changed, or
/// the last write is old) and hand out the actions waiting for it.
pub(crate) async fn report(
    store: &Store,
    owner: &str,
    computer: &str,
    items: Vec<Item>,
) -> Result<Vec<Command>, Error> {
    let digest = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&items).unwrap_or_default())
    );
    let computer = computer.to_owned();
    update_agents(store, owner, move |agents| {
        let now = now_unix();
        let mut changed = false;
        let board = agents.boards.entry(computer.clone()).or_default();
        if board.digest != digest || now.saturating_sub(board.updated_unix) >= REWRITE_EVERY {
            *board = Board {
                items: items.clone(),
                updated_unix: now,
                digest: digest.clone(),
            };
            changed = true;
        }
        let mut handed = Vec::new();
        agents.commands.retain(|waiting| {
            let fresh = now.saturating_sub(waiting.queued_unix) < COMMAND_TTL;
            if waiting.computer == computer && fresh {
                handed.push(waiting.command.clone());
                false
            } else {
                fresh
            }
        });
        if !handed.is_empty() {
            changed = true;
            agents.taken.extend(handed.iter().map(|c| c.id.clone()));
        }
        (changed, handed)
    })
    .await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    items: Vec<Value>,
}

async fn activity(
    State(app): State<App>,
    headers: HeaderMap,
    Path(name): Path<String>,
    body: Bytes,
) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let computer = line(&name, 64);
    let Some(items) = serde_json::from_slice::<Report>(&body)
        .ok()
        .filter(|report| report.items.len() <= MAX_ITEMS && !computer.is_empty())
        .and_then(|report| {
            report
                .items
                .into_iter()
                .map(|value| {
                    serde_json::from_value::<Item>(value)
                        .ok()
                        .and_then(checked_item)
                })
                .collect::<Option<Vec<_>>>()
        })
    else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {items} with at most 32 items.",
        );
    };
    let store = &app.config.chat_store;
    // A report is a check-in too: the computer is online.
    if let Err(error) = coder_sync::check_in(store, &owner, &computer).await {
        return stored(&error);
    }
    match report(store, &owner, &computer, items).await {
        Ok(commands) => answer(StatusCode::OK, json!({"commands": commands})),
        Err(error) => stored(&error),
    }
}

async fn agents(State(app): State<App>, headers: HeaderMap) -> Response {
    let account = match coder_sync::account(&app, &headers).await {
        Ok(account) => account,
        Err(response) => return response,
    };
    let owner = crate::chat_store::account_owner(&account);
    let store = &app.config.chat_store;
    let (agents, computers) = match (
        read_agents(store, &owner).await,
        store.computers(&owner).await,
    ) {
        (Ok(agents), Ok(computers)) => (agents, computers),
        (Err(error), _) | (_, Err(error)) => return stored(&error),
    };
    let boards = with_mac_jobs(
        agents.boards,
        crate::mac_jobs_actor::board_items(&app, &account).await,
    );
    let mut boards: Vec<(String, Board)> = boards.into_iter().collect();
    boards.sort_by_key(|(_, board)| std::cmp::Reverse(board.updated_unix));
    answer(
        StatusCode::OK,
        json!({"computers": boards
            .into_iter()
            .map(|(name, board)| json!({
                "online": online(&computers, &name),
                "name": name,
                "updated_unix": board.updated_unix,
                "items": board.items,
            }))
            .collect::<Vec<_>>()}),
    )
}

/// The boards with each Mac job (#11223) as an item on its Mac's board,
/// after what Coder reported there, so the phone and the web show a Mac's
/// jobs with its other work.
pub(crate) fn with_mac_jobs(
    mut boards: BTreeMap<String, Board>,
    jobs: Vec<(String, Item)>,
) -> BTreeMap<String, Board> {
    for (computer, item) in jobs {
        let board = boards.entry(computer).or_default();
        board.updated_unix = board
            .updated_unix
            .max(item.finished_unix.unwrap_or(item.started_unix));
        if board.items.len() < MAX_ITEMS * 2 {
            board.items.push(item);
        }
    }
    boards
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Action {
    request_id: String,
    computer: String,
    item: String,
    action: String,
    #[serde(default)]
    question: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

/// What became of an action from the phone.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Acted {
    Queued,
    Unknown,
    Offline,
}

/// Queue an action for an item Coder on `computer` reported; the same
/// command id is queued once.
pub(crate) async fn queue_action(
    store: &Store,
    owner: &str,
    computer: &str,
    command: Command,
) -> Result<Acted, Error> {
    let computers = store.computers(owner).await?;
    let computer = computer.to_owned();
    let is_online = online(&computers, &computer);
    update_agents(store, owner, move |agents| {
        if agents.taken.contains(&command.id)
            || agents.commands.iter().any(|w| w.command.id == command.id)
        {
            return (false, Acted::Queued);
        }
        let known = agents
            .boards
            .get(&computer)
            .is_some_and(|board| board.items.iter().any(|item| item.id == command.item));
        if !known {
            return (false, Acted::Unknown);
        }
        if !is_online {
            return (false, Acted::Offline);
        }
        agents.commands.push(Waiting {
            computer: computer.clone(),
            command: command.clone(),
            queued_unix: now_unix(),
        });
        (true, Acted::Queued)
    })
    .await
}

async fn action(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let account = match coder_sync::account(&app, &headers).await {
        Ok(account) => account,
        Err(response) => return response,
    };
    let owner = crate::chat_store::account_owner(&account);
    let Some(sent) = serde_json::from_slice::<Action>(&body).ok().filter(|sent| {
        valid_request(&sent.request_id)
            && ACTIONS.contains(&sent.action.as_str())
            && !sent.item.is_empty()
            && sent.item.len() <= 128
            && (sent.action != "message"
                || sent.text.as_deref().is_some_and(|t| !t.trim().is_empty()))
            && sent
                .text
                .as_deref()
                .is_none_or(|t| t.chars().count() <= MAX_REPLY_CHARS)
    }) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {request_id, computer, item, action} with action stop, approve, deny, or message.",
        );
    };
    if sent
        .text
        .as_deref()
        .is_some_and(|t| secret_screen::credential_in(t).is_some())
    {
        return refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "secret",
            "This looks like it holds a password or key, so it wasn't sent.",
        );
    }
    // A Mac job's item (#11223): Approve, Deny, and Stop reach the job
    // itself; the Mac takes the answer at its next report.
    if crate::mac_jobs::is_job(&sent.item) {
        return match crate::mac_jobs_actor::act(
            &app,
            &account,
            &sent.item,
            &sent.action,
            sent.question.as_deref(),
        )
        .await
        {
            Ok(Ok(())) => answer(StatusCode::ACCEPTED, json!({"queued": true})),
            Ok(Err((status, code, message))) => refused(status, code, message),
            Err(error) => stored(&error),
        };
    }
    let computer = line(&sent.computer, 64);
    let command = Command {
        id: sent.request_id,
        item: sent.item,
        action: sent.action,
        question: sent.question.map(|q| line(&q, 64)),
        text: sent.text.map(|t| t.trim().to_owned()),
    };
    match queue_action(&app.config.chat_store, &owner, &computer, command).await {
        Ok(Acted::Queued) => answer(StatusCode::ACCEPTED, json!({"queued": true})),
        Ok(Acted::Unknown) => refused(
            StatusCode::NOT_FOUND,
            "unknown",
            "That isn't running on this computer anymore.",
        ),
        Ok(Acted::Offline) => refused(
            StatusCode::CONFLICT,
            "offline",
            &format!("Coder on {computer} isn't online now."),
        ),
        Err(error) => stored(&error),
    }
}

#[cfg(test)]
mod tests;
