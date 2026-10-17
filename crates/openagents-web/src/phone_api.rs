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
//! | `POST /v1/threads` `{request_id, text, files?, project?}` | Start a chat answered by OpenAgents (`201 {id, answering}`); `project` (its id or `name:NAME`, #11542) starts it inside one of the person's projects, as starting a chat in a project on the web does (`404 not_found` when it isn't theirs) |
//! | `POST /v1/threads/{id}/messages` `{request_id, text, files?}` | Reply. A terminal chat queues it for Coder on its computer (`202 {queued}`); a web chat is answered here (`202 {answering}`), with the files added below (`files`: their ids, up to four); a phone chat refuses (`409 phone`) |
//! | `POST /v1/threads/{id}/files?name=` (the file's bytes) | Add an image, PDF, or text file to a web chat for the next reply (`201 {id, name, kind, size, url}`; #11174, [`crate::chat_files`]) |
//! | `GET`/`DELETE /v1/threads/{id}/files/{file}` | A file of the chat; remove one not sent yet (`204`) |
//! | `DELETE /v1/threads/{id}` | Delete the chat for good, as the web's Delete does (`204`; `404 not_found`; `409 busy` while an answer is being written, `409 changed`) (#11601) |
//! | `GET /v1/computers` | `{computers: [Computer]}`: the account's own computers, then (for accounts cloud computers are open to, #11624) its cloud computers with `kind: "cloud"`, `id`, `chat`, `state` |
//! | `DELETE /v1/computers/{name}` | Remove the computer from the account: its check-ins, sync choice, waiting replies, and running work (`204`; `404 not_found`). Its synced chats stay; delete them one by one (#11601) |
//! | `GET`/`PUT /v1/computers/{name}/sync` `{choice}` | The computer's (or phone's) sync choice, as `/coder/sync` |
//! | `POST /v1/computers/{name}/activity` `{items: [Item]}` | Coder's running work on the computer; answers `{commands: [Command]}`, each handed out once. With `?kind=landing` (#11250), the landing queue's entries on the integrator computer (kind `landing`): they replace only that computer's landing items, keep its other items, and take no commands |
//! | `GET /v1/agents` | `{computers: [{name, online, updated_unix, items}]}` |
//! | `POST /v1/agents/actions` `{request_id, computer, item, action, question?, text?}` | Stop, approve, deny, or message an item, or take a paused item's choice (`your_key`, `openagents`, only when its `choices` list it): `202 {queued}`. On the `Environments` board (a Claude Code run on the person's environments, #11307) Stop and Message reach the run itself: `202 {queued, message}`, the message waiting for the run's next turn |
//!
//! A request with `X-OpenAgents-Test: 1` is a test run
//! ([`crate::test_runs`]): the chat it starts never lists and is deleted
//! within the hour, and its computer's sync choice and running work are
//! never recorded.
//!
//! A phone chat is a synced chat ([`crate::chat_store::Terminal`]) whose
//! session starts with [`PHONE_PREFIX`], uploaded through
//! `PUT /coder/sessions/{session}` with the phone's name as the computer.
//! The running work lives in one small object per account beside its
//! chats ([`AGENTS_KEY`]), never in the chat or computer records.

use std::collections::BTreeMap;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::App;
use crate::chat_store::{Computers, Conversation, Error, Store, SyncChoice, now_unix};
use crate::coder_sync::{self, Queued, answer, line, online, refused, stored};
use crate::pages::chat::{AppSent, follow_from_app, line_two, start_from_app};

/// The session prefix of a chat synced from the phone.
pub(crate) const PHONE_PREFIX: &str = "phone-";
/// The most chats one `GET /v1/threads` page lists (`?offset=N&limit=M`,
/// `limit` at most this).
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
        .merge(crate::chat_api::routes())
        .merge(crate::cloud_host::routes())
        .merge(crate::continuations::routes())
        .route("/v1/threads", get(threads).post(start))
        .route(
            "/v1/threads/{id}",
            get(thread)
                .patch(crate::projects::api::move_thread)
                .delete(delete_thread),
        )
        .route(
            "/v1/threads/{id}/messages",
            get(crate::projects::api::messages).post(reply),
        )
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
        .route("/v1/computer-usage", get(crate::computers_api::usage::read))
        .route(
            "/v1/computers",
            get(computers_route)
                .post(crate::computers_api::create)
                .layer(axum::extract::DefaultBodyLimit::max(16 * 1024)),
        )
        .route(
            "/v1/computers/{name}",
            get(crate::computers_api::get).delete(remove_computer),
        )
        .route(
            "/v1/computers/{name}/stop",
            post(crate::computers_api::stop),
        )
        .route(
            "/v1/computers/{name}/wake",
            post(crate::computers_api::wake),
        )
        .route(
            "/v1/computers/{name}/files",
            get(crate::computers_api::files::transfer).put(crate::computers_api::files::transfer),
        )
        .route(
            "/v1/computers/{name}/commands",
            post(crate::computers_api::commands::create),
        )
        .route(
            "/v1/computers/{name}/commands/{command}",
            get(crate::computers_api::commands::read),
        )
        .route(
            "/v1/computers/{name}/commands/{command}/output",
            get(crate::computers_api::commands::output),
        )
        .route(
            "/v1/computers/{name}/commands/{command}/cancel",
            post(crate::computers_api::commands::cancel),
        )
        .route("/v1/computers/{name}/sync", get(sync_get).put(sync_put))
        .route("/v1/computers/{name}/activity", post(activity))
        .route("/v1/agents", get(agents))
        .route("/v1/agents/actions", post(action))
}

/// Whether `path` is one of these routes (`crate::upstream::owned`).
pub(crate) fn owns(path: &str) -> bool {
    if path == "/v1/computer-usage" {
        return true;
    }
    // The gateway's agent sign-up routes (#11630) share the prefix.
    if path.starts_with("/v1/agents/") && path != "/v1/agents/actions" {
        return false;
    }
    ["/v1/threads", "/v1/computers", "/v1/agents", "/v1/history"]
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
        "request_id": chat.requests.last().map(|r| &r.id),
        "outcome": chat.requests.last().map(|r| r.outcome),
        "title": chat.title,
        "line": line_two(chat, true),
        "surface": surface,
        "computer": terminal.map(|t| t.computer.clone()),
        "session": terminal.map(|t| t.session.clone()),
        "updated_unix": chat.updated_unix,
        "working": chat.working(),
        "online": online_now,
        "can_reply": can_reply && chat.links.continued_by.is_none(),
        "home": crate::continuations::home(chat),
        "continues": chat.links.continues,
        "continued_by": chat.links.continued_by,
        "pinned": chat.pinned_unix.is_some(),
        // The project it is filed under (#11541), for the apps to name.
        "project": chat.project,
        // How many messages it holds, as the projects API counts them
        // (`projects::api::thread_object`), so `chat threads --account`
        // agrees with `projects show` (#11543).
        "messages": chat.messages.len(),
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

/// The first page of chats `GET /v1/threads` lists, in order.
pub(crate) async fn listed(store: &Store, owner: &str) -> Result<(Vec<Value>, Computers), Error> {
    let (rows, computers, _) = listed_page(store, owner, 0, SHOWN_THREADS).await?;
    Ok((rows, computers))
}

/// One page of the account's chats, newest and pinned first, and how many
/// chats there are in all.
pub(crate) async fn listed_page(
    store: &Store,
    owner: &str,
    offset: usize,
    limit: usize,
) -> Result<(Vec<Value>, Computers, usize), Error> {
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
    let total = chats.len();
    let rows = chats
        .iter()
        .skip(offset)
        .take(limit.clamp(1, SHOWN_THREADS))
        .map(|chat| row(chat, &computers))
        .collect();
    Ok((rows, computers, total))
}

#[derive(Deserialize, Default)]
pub(crate) struct ThreadsQuery {
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    limit: Option<usize>,
    /// A project's threads in the API's list shape (#11471,
    /// [`crate::projects::api::threads_in`]): its id, `name:NAME`, or
    /// `none`.
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    after: Option<String>,
}

async fn threads(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<ThreadsQuery>,
) -> Response {
    if let Some(project) = &query.project {
        let page = crate::projects::api::Page {
            limit: query.limit,
            after: query.after.clone(),
        };
        return crate::projects::api::threads_in(&app, &headers, project, &page).await;
    }
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let limit = query.limit.unwrap_or(SHOWN_THREADS);
    match listed_page(&app.config.chat_store, &owner, query.offset, limit).await {
        Ok((rows, computers, total)) => {
            let next = query.offset + rows.len();
            answer(
                StatusCode::OK,
                json!({
                    "threads": rows,
                    "computers": computer_rows(&computers),
                    "total": total,
                    "more": next < total,
                    "next_offset": if next < total { Some(next) } else { None },
                }),
            )
        }
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
        // A chat someone else started in a shared project (#11586).
        Ok(_) | Err(Error::Invalid(_)) => {
            match crate::projects::api::shared_thread(&app, &headers, &owner, &id).await {
                Some((loaded, _)) => loaded.conversation,
                None => return not_found(),
            }
        }
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
    /// Starting a chat only: the project it starts in, its id or
    /// `name:NAME` (#11542).
    #[serde(default)]
    project: Option<String>,
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
        .filter(|sent| valid_request(&sent.request_id) && sent.project.is_none())
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
        Ok(_) | Err(Error::Invalid(_)) => {
            // A reply in a chat someone else started in a shared project
            // (#11586): editors and the owner, words only, under the
            // sender's own allowance.
            let Some((loaded, project)) =
                crate::projects::api::shared_thread(&app, &headers, &owner, &id).await
            else {
                return not_found();
            };
            if !crate::projects::api::role_of(&project).edits() {
                return refused(
                    StatusCode::FORBIDDEN,
                    "project_role",
                    crate::pages::chat::VIEWER_REPLY,
                );
            }
            if loaded.conversation.terminal.is_some() || !sent.files.is_empty() {
                return refused(
                    StatusCode::BAD_REQUEST,
                    "words_only",
                    crate::pages::chat::MEMBER_WORDS_ONLY,
                );
            }
            if secret_screen::credential_in(text).is_some() {
                return refused(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "secret",
                    "This looks like it holds a password or key, so it wasn't sent.",
                );
            }
            let starter = loaded.conversation.owner.clone();
            let cad_plan = crate::cad_gate::plan(&app, &headers).await;
            return sent_answer(
                follow_from_app(
                    &app,
                    &starter,
                    Some(&owner),
                    &id,
                    &sent.request_id,
                    text,
                    Vec::new(),
                    cad_plan,
                )
                .await,
            );
        }
        Err(error) => return stored(&error),
    };
    if chat.links.continued_by.is_some() {
        return refused(
            StatusCode::CONFLICT,
            "continued",
            "This chat continues elsewhere. Open its latest part to reply.",
        );
    }
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
    let cad_plan = crate::cad_gate::plan(&app, &headers).await;
    sent_answer(
        follow_from_app(
            &app,
            &owner,
            None,
            &id,
            &sent.request_id,
            text,
            files,
            cad_plan,
        )
        .await,
    )
}

/// The answer to a reply an app sent.
fn sent_answer(sent: AppSent) -> Response {
    match sent {
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

/// `POST /v1/threads {request_id, text, files}` (#11349): start a chat
/// answered by OpenAgents, as the website's composer does, so an app's
/// first message can carry photos and files. The request id names the
/// chat; files go under it first (`POST /v1/threads/{request_id}/files`).
/// `project` (its id or `name:NAME`) starts the chat inside one of the
/// person's projects (#11542), its answer reading the project's
/// repository as the web's does. Answers `201 {id, answering: true}`, or
/// `200` when the same send already made the chat.
async fn start(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
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
    if secret_screen::credential_in(text).is_some() {
        return refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "secret",
            "This looks like it holds a password or key, so it wasn't sent.",
        );
    }
    let project = match sent.project.as_deref() {
        None => None,
        Some(reference) => match project_of(&app, &headers, reference).await {
            Ok(found) => Some(found),
            Err(response) => return response,
        },
    };
    let store = &app.config.chat_store;
    let id = sent.request_id.clone();
    let existed = matches!(store.load(&owner, &id).await, Ok(Some(_)));
    // A test run's chat never lists, and goes within the hour (#11601).
    crate::test_runs::note_if_marked(store, &headers, &owner, &id).await;
    let files = match crate::chat_files::take(store, &owner, &id, &sent.files.join(",")).await {
        Ok(files) => files,
        Err(message) => return refused(StatusCode::BAD_REQUEST, "files", message),
    };
    let cad_plan = crate::cad_gate::plan(&app, &headers).await;
    match start_from_app(&app, &owner, &id, text.to_owned(), files, project, cad_plan).await {
        AppSent::Answering => answer(
            if existed {
                StatusCode::OK
            } else {
                StatusCode::CREATED
            },
            json!({"id": id, "answering": true}),
        ),
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

/// The caller's project `reference` names (its id or `name:NAME`), and
/// their GitHub token for reading its repository when they have one.
async fn project_of(
    app: &App,
    headers: &HeaderMap,
    reference: &str,
) -> Result<(oa_auth::repos::Project, Option<String>), Response> {
    let (service, token, _) = crate::projects::api::caller(app, headers).await?;
    let status = crate::projects::api::status(service, &token).await?;
    let project = crate::projects::api::named(&status.projects, reference)?.clone();
    if !crate::projects::api::role_of(&project).edits() {
        return Err(crate::projects::api::role_refused());
    }
    let github = service.github_token_as(&token).await.ok();
    Ok((project, github))
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

/// `DELETE /v1/threads/{id}` (#11601): the web's Delete, for the apps and
/// the `openagents chats delete` command.
async fn delete_thread(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    if !crate::chat_store::valid_id(&id) {
        return not_found();
    }
    let record = match app.config.chat_store.load(&owner, &id).await {
        Ok(Some(loaded)) if !loaded.conversation.deleted() => loaded,
        Ok(_) | Err(Error::Invalid(_)) => return not_found(),
        Err(error) => return stored(&error),
    };
    use crate::pages::chat::Removed;
    match crate::pages::chat::remove_owned(&app, &owner, &record).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(Removed::Busy) => refused(
            StatusCode::CONFLICT,
            "busy",
            "Wait for the answer to finish, then delete this chat.",
        ),
        Err(Removed::Changed) => refused(
            StatusCode::CONFLICT,
            "changed",
            "This chat just changed. Try again.",
        ),
        Err(Removed::Unavailable) => refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "Try again later.",
        ),
    }
}

/// Remove computer `name` from the account (#11601): its check-ins, sync
/// choice, waiting replies, and running work. Its synced chats stay.
/// Returns whether it was there.
pub(crate) async fn remove_computer_named(
    store: &Store,
    owner: &str,
    name: &str,
) -> Result<bool, Error> {
    let name = line(name, 64);
    let gone = name.clone();
    let mut found = false;
    let before = store.computers(owner).await?;
    if before.seen.contains_key(&name)
        || before.sync.contains_key(&name)
        || before.waiting.values().any(|on| *on == name)
    {
        found = true;
        store
            .update_computers(owner, move |computers| {
                let had =
                    computers.seen.remove(&gone).is_some() | computers.sync.remove(&gone).is_some();
                let waiting = computers.waiting.len();
                computers.waiting.retain(|_, on| *on != gone);
                had || computers.waiting.len() != waiting
            })
            .await?;
    }
    let gone = name.clone();
    let boards = update_agents(store, owner, move |agents| {
        let had = agents.boards.remove(&gone).is_some();
        let commands = agents.commands.len();
        agents.commands.retain(|waiting| waiting.computer != gone);
        let changed = had || agents.commands.len() != commands;
        (changed, had)
    })
    .await?;
    Ok(found || boards)
}

async fn remove_computer(
    State(app): State<App>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Query(query): Query<RemoveComputer>,
) -> Response {
    if let Some(kind) = query.kind.as_deref() {
        if kind != "cloud" {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid",
                "Use kind=cloud for a cloud computer.",
            );
        }
        return crate::computers_api::delete(State(app), headers, Path(name)).await;
    }
    let identity = match crate::computers_api::identity(&app, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    match remove_computer_named(&app.config.chat_store, &identity.owner, &name).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        // Preserve account detach for any existing enrolled-machine name,
        // including a name that happens to have the cloud ID format.
        Ok(false) if crate::computers_api::cloud_id(&name) => {
            crate::computers_api::delete(State(app), headers, Path(name)).await
        }
        Ok(false) => refused(StatusCode::NOT_FOUND, "not_found", "No such computer."),
        Err(error) => stored(&error),
    }
}

#[derive(Deserialize)]
struct RemoveComputer {
    kind: Option<String>,
}

async fn computers_route(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<crate::computers_api::ListQuery>,
) -> Response {
    if let Err(response) = crate::computers_api::validate_page(&query) {
        return response;
    }
    let identity = match crate::computers_api::identity(&app, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    if query.kind.as_deref() == Some("cloud") {
        if !identity.permitted {
            return refused(
                StatusCode::FORBIDDEN,
                "invite_only",
                "Cloud computers aren't available for this account yet.",
            );
        }
        let Some(client) = crate::computers::client() else {
            return refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "Cloud computers aren't available right now.",
            );
        };
        return match crate::computers_api::list_cloud(client, &identity, &query).await {
            Ok(value) => answer(StatusCode::OK, value),
            Err(response) => response,
        };
    }
    let computers = match app.config.chat_store.computers(&identity.owner).await {
        Ok(computers) => computers,
        Err(error) => return stored(&error),
    };
    let mut rows = computer_rows(&computers);
    // The account's cloud computers, one list with its own (#11624), for
    // the accounts cloud computers are open to.
    let mut next_cursor = Value::Null;
    if identity.permitted
        && let Some(client) = crate::computers::client()
        && let Ok(cloud) = crate::computers_api::list_cloud(client, &identity, &query).await
    {
        rows.extend(cloud["computers"].as_array().into_iter().flatten().cloned());
        next_cursor = cloud["next_cursor"].clone();
    }
    answer(
        StatusCode::OK,
        json!({"computers": rows, "next_cursor": next_cursor}),
    )
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
    // A test run's choice is answered, never recorded (#11601).
    if crate::test_runs::marked(&headers) {
        return answer(StatusCode::OK, json!({"choice": picked.as_str()}));
    }
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
    /// While a usage limit pauses it: what the person may do now, from
    /// [`CHOICES`] only (#11295).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
}

/// The pause choices an item may offer, in the order they show:
/// Continue on your key, then Continue with OpenAgents (#11295).
pub(crate) const YOUR_KEY: &str = "your_key";
pub(crate) const OPENAGENTS: &str = "openagents";
pub(crate) const CHOICES: [&str; 2] = [YOUR_KEY, OPENAGENTS];

/// Only the known choices, once each, in their order.
pub(crate) fn clean_choices(choices: &[String]) -> Vec<String> {
    CHOICES
        .iter()
        .filter(|choice| choices.iter().any(|c| c == *choice))
        .map(|choice| (*choice).to_owned())
        .collect()
}

/// `landing`: a landing queue entry the integrator reports (#11250).
const KINDS: [&str; 3] = ["chat", "agent", LANDING];
/// The kind of a landing queue entry's item.
pub(crate) const LANDING: &str = "landing";
const STATUSES: [&str; 5] = ["working", "asking", "done", "failed", "stopped"];
const ACTIONS: [&str; 6] = ["stop", "approve", "deny", "message", YOUR_KEY, OPENAGENTS];

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
    item.choices = clean_choices(&item.choices);
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
    report_lane(store, owner, computer, items, false).await
}

/// [`report`] for one lane of the computer's list: with `landing`, the
/// landing queue's entries (#11250), which replace only its landing items
/// and take no commands; otherwise Coder's own items, which keep the
/// landing items another process on the same computer reported.
pub(crate) async fn report_lane(
    store: &Store,
    owner: &str,
    computer: &str,
    items: Vec<Item>,
    landing: bool,
) -> Result<Vec<Command>, Error> {
    let computer = computer.to_owned();
    update_agents(store, owner, move |agents| {
        let now = now_unix();
        let mut changed = false;
        let board = agents.boards.entry(computer.clone()).or_default();
        let kept = board
            .items
            .iter()
            .filter(|item| (item.kind == LANDING) != landing)
            .cloned();
        let items: Vec<Item> = if landing {
            kept.chain(items.iter().cloned()).collect()
        } else {
            items.iter().cloned().chain(kept).collect()
        };
        let digest = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&items).unwrap_or_default())
        );
        if board.digest != digest || now.saturating_sub(board.updated_unix) >= REWRITE_EVERY {
            *board = Board {
                items,
                updated_unix: now,
                digest,
            };
            changed = true;
        }
        if landing {
            return (changed, Vec::new());
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

#[derive(Deserialize)]
struct Lane {
    #[serde(default)]
    kind: Option<String>,
}

async fn activity(
    State(app): State<App>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Query(lane): Query<Lane>,
    body: Bytes,
) -> Response {
    let landing = match lane.kind.as_deref() {
        None => false,
        Some(LANDING) => true,
        Some(_) => {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid",
                "The only list kind is landing.",
            );
        }
    };
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
                        // A lane reports only its own kind.
                        .filter(|item| (item.kind == LANDING) == landing)
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
    // A test run's computer is never recorded (#11601).
    if crate::test_runs::marked(&headers) {
        return answer(StatusCode::OK, json!({"commands": []}));
    }
    // A report is a check-in too: the computer is online.
    if let Err(error) = coder_sync::check_in(store, &owner, &computer).await {
        return stored(&error);
    }
    match report_lane(store, &owner, &computer, items, landing).await {
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
    let mut jobs = crate::mac_jobs_actor::board_items(&app, &account).await;
    jobs.extend(crate::work_runs::board_items(store, &owner).await);
    // The person's Claude Code runs on their environments (#11307), paused
    // chat runs with the choices this person has (#11295).
    if app.config.environments.get().is_some() {
        let viewer = app_viewer(&app, &headers).await;
        jobs.extend(
            crate::environments::board::rows(&app, &account, viewer.as_ref())
                .await
                .into_iter()
                .map(|item| (PAUSED_BOARD.to_owned(), item)),
        );
    }
    let boards = with_mac_jobs(agents.boards, jobs);
    let mut boards: Vec<(String, Board)> = boards.into_iter().collect();
    boards.sort_by_key(|(_, board)| std::cmp::Reverse(board.updated_unix));
    answer(
        StatusCode::OK,
        json!({"computers": boards
            .into_iter()
            .map(|(name, board)| json!({
                "online": name == PAUSED_BOARD || online(&computers, &name),
                "name": name,
                "updated_unix": board.updated_unix,
                "items": board.items,
            }))
            .collect::<Vec<_>>()}),
    )
}

/// The board the phone shows the person's environment runs on (#11307;
/// paused ones with their choices, #11295), beside the account's computers.
pub(crate) const PAUSED_BOARD: &str = "Environments";

/// The phone's own signed-in view, from its bearer token.
async fn app_viewer(app: &App, headers: &HeaderMap) -> Option<crate::cloud::session::Viewer> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))?;
    app.config.cloud.as_deref()?.app_viewer(token).await.ok()
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
    /// A pause choice the item doesn't offer now.
    NotOffered,
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
        let Some(item) = agents
            .boards
            .get(&computer)
            .and_then(|board| board.items.iter().find(|item| item.id == command.item))
        else {
            return (false, Acted::Unknown);
        };
        if CHOICES.contains(&command.action.as_str())
            && !item.choices.iter().any(|choice| *choice == command.action)
        {
            return (false, Acted::NotOffered);
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

/// Whether `item` on `computer` is a landing queue entry (#11250).
pub(crate) async fn landing_item(
    store: &Store,
    owner: &str,
    computer: &str,
    item: &str,
) -> Result<bool, Error> {
    Ok(read_agents(store, owner)
        .await?
        .boards
        .get(computer)
        .is_some_and(|board| {
            board
                .items
                .iter()
                .any(|seen| seen.id == item && seen.kind == LANDING)
        }))
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
            "Send {request_id, computer, item, action} with action stop, approve, deny, message, your_key, or openagents.",
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
    // A paused environment run's choice (#11295): the person's own key is
    // released into the run only, never to the phone.
    if sent.computer == PAUSED_BOARD {
        if !CHOICES.contains(&sent.action.as_str()) {
            return environment_action(&app, &headers, &account, &owner, &sent).await;
        }
        let Some(viewer) = app_viewer(&app, &headers).await else {
            return refused(
                StatusCode::UNAUTHORIZED,
                "signed_out",
                "Log in again to choose.",
            );
        };
        return match crate::pages::chat::choose_paused(
            &app,
            &viewer,
            &owner,
            &sent.item,
            &sent.action,
        )
        .await
        {
            Ok(said) => answer(
                StatusCode::ACCEPTED,
                json!({"queued": true, "message": said}),
            ),
            Err(said) => refused(StatusCode::CONFLICT, "not_offered", &said),
        };
    }
    // A Mac job's item (#11223): Approve, Deny, and Stop reach the job
    // itself; the Mac takes the answer at its next report.
    // A work run's item (#11258): Stop reaches the run.
    if crate::work_runs::is_run(&sent.item) {
        let store = &app.config.chat_store;
        return match crate::work_runs::act(store, &owner, &sent.item, &sent.action).await {
            Ok(Ok(())) => answer(StatusCode::ACCEPTED, json!({"queued": true})),
            Ok(Err((status, code, message))) => refused(status, code, message),
            Err(error) => stored(&error),
        };
    }
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
    match landing_item(&app.config.chat_store, &owner, &computer, &sent.item).await {
        Ok(true) => {
            return refused(
                StatusCode::CONFLICT,
                "landing",
                &format!(
                    "A landing queue entry can't be stopped or messaged from here. To take it out of the queue, run `openagents land withdraw {}` on the computer that sent it.",
                    sent.item
                ),
            );
        }
        Ok(false) => {}
        Err(error) => return stored(&error),
    }
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
        Ok(Acted::NotOffered) => refused(
            StatusCode::CONFLICT,
            "not_offered",
            "That choice isn't open now.",
        ),
        Err(error) => stored(&error),
    }
}

/// Stop or Message on a Claude Code run of the person's environments
/// (#11307): it reaches the run itself. A message waits for the run's next
/// turn, or starts a follow-up when the run has just ended.
async fn environment_action(
    app: &App,
    headers: &HeaderMap,
    account: &str,
    owner: &str,
    sent: &Action,
) -> Response {
    use crate::environments::board;
    let Some(studio) = app.config.environments.get() else {
        return refused(
            StatusCode::NOT_FOUND,
            "unknown",
            "That isn't running anymore.",
        );
    };
    let Some((env, run)) = board::resolve(app, &studio, account, owner, &sent.item).await else {
        return refused(
            StatusCode::NOT_FOUND,
            "unknown",
            "That isn't running anymore.",
        );
    };
    match sent.action.as_str() {
        "stop" => match studio.stop_claude(&env, &run) {
            Ok(()) => answer(StatusCode::ACCEPTED, json!({"queued": true})),
            Err(_) => refused(
                StatusCode::NOT_FOUND,
                "unknown",
                "That isn't running anymore.",
            ),
        },
        "message" => {
            let Some(viewer) = app_viewer(app, headers).await else {
                return refused(
                    StatusCode::UNAUTHORIZED,
                    "signed_out",
                    "Log in again to send it a message.",
                );
            };
            let text = sent.text.as_deref().unwrap_or_default().trim();
            match board::message(
                app,
                &studio,
                board::Asker::App(&viewer, headers),
                &env,
                &run,
                text,
            )
            .await
            {
                Ok(messaged) => answer(
                    StatusCode::ACCEPTED,
                    json!({"queued": true, "message": messaged.words()}),
                ),
                Err(said) => refused(StatusCode::CONFLICT, "not_sent", &said),
            }
        }
        _ => refused(
            StatusCode::CONFLICT,
            "unsupported",
            "A Claude Code run takes Stop or Message.",
        ),
    }
}

#[cfg(test)]
mod tests;
