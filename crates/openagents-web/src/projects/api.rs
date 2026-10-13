//! The signed-in person's projects and the threads in them, for apps and
//! agents (#11471, `docs/api/projects.md`): `openagents projects`, Coder,
//! and any script holding the account's own sign-in.
//!
//! A project is one connected GitHub repository; the left panel groups
//! chats (threads) under it, and Drive sources attach to it
//! (`/projects/{id}/sources`). Every route takes the same credential as the
//! other `/v1` app routes (`Authorization: Bearer sess_…` from `openagents
//! login` or `coder login`, or a sign-in key `oa_sk_…`) and answers only
//! that account's own data; another account's project or thread answers
//! `404`, the same as one that doesn't exist. Reads only read.
//!
//! | Route | What |
//! | --- | --- |
//! | `GET /v1/projects` | `{data: [Project], next}`, oldest first (`?limit`, `?after`) |
//! | `POST /v1/projects` `{repository}` | Add a project from a connected repository (`owner/name`): `201 Project` |
//! | `GET /v1/projects/{id}` | One `Project`; `{id}` may be `name:NAME` |
//! | `PATCH /v1/projects/{id}` `{protection}` | The default for new files: `standard` or `only_me` ([`super::files`]) |
//! | `DELETE /v1/projects/{id}` | `{id, object: "project.deleted"}`; `409 project_has_threads` while threads are in it, unless `?detach_threads=true` (the threads stay, outside any project) |
//! | `GET /v1/threads?project=ID\|name:NAME\|none` | `{data: [Thread], next}`, by last activity, oldest first (`?limit`, `?after`) |
//! | `PATCH /v1/threads/{id}` `{project}` | Move the thread into a project (its id or `name:NAME`), or out of any (`null`): `Thread`. Sync then leaves it where the person put it (#11541) |
//! | `GET /v1/threads/{id}/messages` | `{data: [Message], next}`, oldest first (`?limit`, `?after`) |
//!
//! `Project` is `{id, object: "project", name, instructions, repository,
//! default_branch, private, created_at, updated_at, counts: {threads,
//! files, documents, sources}}`; times are RFC 3339 UTC. Errors are
//! `{error: {code, message}}`. The account service keeps the projects; this
//! server reads them with the caller's own credential.

use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use oa_auth::repos::{Project, RepoError, Status};
use serde::Deserialize;
use serde_json::{Value, json};

use super::files::Summary;
use crate::App;
use crate::chat_store::{Conversation, Error, Role, account_owner};
use crate::cloud::session::github::RepoCallError;
use crate::cloud::session::{CloudSession, SessionError, session_token};
use crate::coder_sync::{answer, refused, stored};

/// The listing.
pub(crate) const API: &str = "/v1/projects";
/// The most rows one page lists.
const MAX_LIMIT: usize = 200;
/// Rows a page lists when `?limit` isn't given.
const DEFAULT_LIMIT: usize = 100;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(API, get(list).post(add))
        .route("/v1/projects/{id}", get(show).delete(remove).patch(edit))
}

/// Whether `path` is one of these routes, so the site answers it itself.
pub(crate) fn owns(path: &str) -> bool {
    path.strip_prefix(API)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// The caller: the account service, their own token, and their account
/// id; or the app API's refusal (`401 signed_out` when the token no
/// longer signs in).
pub(crate) async fn caller<'a>(
    app: &'a App,
    headers: &HeaderMap,
) -> Result<(&'a CloudSession, String, String), Response> {
    let Some(service) = app.config.cloud.as_deref() else {
        return Err(refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "This site doesn't offer accounts.",
        ));
    };
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| session_token(token))
        .ok_or_else(signed_out)?
        .to_owned();
    let account = crate::coder_sync::account(app, headers).await?;
    Ok((service, token, account))
}

fn signed_out() -> Response {
    refused(
        StatusCode::UNAUTHORIZED,
        "signed_out",
        "Sign in again with openagents login, or use a sign-in key.",
    )
}

/// The app API's answer for a failed account-service call.
fn failed(error: RepoCallError) -> Response {
    match error {
        RepoCallError::Session(SessionError::Unauthenticated | SessionError::InvalidRequest) => {
            signed_out()
        }
        RepoCallError::Session(_) => refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "Your projects couldn't be read right now. Try again in a minute.",
        ),
        RepoCallError::Repo(error) => {
            let (status, code) = match error {
                RepoError::Invalid => (StatusCode::BAD_REQUEST, "invalid_request"),
                RepoError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
                RepoError::Full => (StatusCode::CONFLICT, "full"),
                RepoError::NotConnected => (StatusCode::CONFLICT, "github_not_connected"),
                RepoError::Reconnect => (StatusCode::CONFLICT, "github_reconnect"),
                RepoError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
                RepoError::SsoRequired | RepoError::Forbidden | RepoError::OtherGithub => {
                    (StatusCode::FORBIDDEN, "github_refused")
                }
                _ => (StatusCode::BAD_GATEWAY, "github_unavailable"),
            };
            refused(status, code, &error.to_string())
        }
    }
}

fn not_yours() -> Response {
    refused(
        StatusCode::NOT_FOUND,
        "not_found",
        "That isn't one of your projects. See them with: openagents projects list",
    )
}

/// The caller's projects, read with their own credential.
pub(crate) async fn status(service: &CloudSession, token: &str) -> Result<Status, Response> {
    service.github_status_as(token).await.map_err(failed)
}

/// The project `reference` names among `projects`: its id, or
/// `name:NAME` (the exact name, when only one project has it).
pub(crate) fn named<'a>(projects: &'a [Project], reference: &str) -> Result<&'a Project, Response> {
    if let Some(name) = reference.strip_prefix("name:") {
        let found: Vec<&Project> = projects.iter().filter(|p| p.name == name).collect();
        return match found.as_slice() {
            [project] => Ok(project),
            [] => Err(not_yours()),
            _ => Err(refused(
                StatusCode::CONFLICT,
                "ambiguous_name",
                "More than one of your projects has that name. Use its id.",
            )),
        };
    }
    projects
        .iter()
        .find(|project| project.id == reference)
        .ok_or_else(not_yours)
}

/// A Unix time as RFC 3339 UTC, `2026-10-12T20:41:09Z`.
pub(crate) fn rfc3339(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64 + 719_468;
    let rest = seconds % 86_400;
    let era = days.div_euclid(146_097);
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

fn in_project<'a>(
    chats: &'a [Conversation],
    project: &'a str,
) -> impl Iterator<Item = &'a Conversation> {
    chats
        .iter()
        .filter(move |chat| chat.project.as_deref() == Some(project))
}

/// One project as the API shows it. `chats` are the account's chats;
/// `sources` how many Drive sources it has; `kept` its files and
/// documents ([`super::files`]).
pub(crate) fn object(
    project: &Project,
    chats: &[Conversation],
    sources: usize,
    kept: &Summary,
) -> Value {
    let threads = in_project(chats, &project.id).count();
    let updated = in_project(chats, &project.id)
        .map(|chat| chat.updated_unix)
        .max()
        .unwrap_or(project.created_unix)
        .max(project.created_unix);
    json!({
        "id": project.id,
        "object": "project",
        "name": project.name,
        "instructions": null,
        "repository": project.repository,
        "default_branch": project.default_branch,
        "private": project.private,
        "created_at": rfc3339(project.created_unix),
        "updated_at": rfc3339(updated),
        "protection": kept.protection.word(),
        "counts": {"threads": threads, "files": kept.files, "documents": kept.documents, "sources": sources},
    })
}

/// How many Drive sources the account's project has.
fn source_count(app: &App, account: &str, project: &str) -> usize {
    app.config
        .connections
        .as_deref()
        .and_then(|store| store.load(&account_owner(account)).ok())
        .map_or(0, |kept| kept.sources_of(project).len())
}

/// A project's files and documents kept here; none when they can't be read.
async fn kept(app: &App, account: &str, project: &str) -> Result<Summary, Response> {
    super::files::summary(app, account, project)
        .await
        .map_err(|error| stored(&error))
}

async fn account_chats(app: &App, account: &str) -> Result<Vec<Conversation>, Response> {
    app.config
        .chat_store
        .list(&account_owner(account))
        .await
        .map_err(|error| stored(&error))
}

#[derive(Deserialize, Default)]
pub(crate) struct Page {
    #[serde(default)]
    pub(crate) limit: Option<usize>,
    #[serde(default)]
    pub(crate) after: Option<String>,
}

impl Page {
    fn limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }
}

/// One page of `rows` (already in order, each with an `id`) after the
/// cursor, and the cursor for the next page.
pub(crate) fn paged(rows: Vec<Value>, page: &Page) -> Result<Value, Response> {
    let start = match page.after.as_deref().filter(|after| !after.is_empty()) {
        None => 0,
        Some(after) => match rows.iter().position(|row| row["id"] == after) {
            Some(at) => at + 1,
            None => {
                return Err(refused(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    "That `after` value isn't in this list. Start again without it.",
                ));
            }
        },
    };
    let limit = page.limit();
    let data: Vec<Value> = rows.iter().skip(start).take(limit).cloned().collect();
    let next = (start + data.len() < rows.len())
        .then(|| data.last().map(|row| row["id"].clone()))
        .flatten();
    Ok(json!({"data": data, "next": next}))
}

/// `GET /v1/projects`.
async fn list(State(app): State<App>, headers: HeaderMap, Query(page): Query<Page>) -> Response {
    let (service, token, account) = match caller(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let status = match status(service, &token).await {
        Ok(status) => status,
        Err(response) => return response,
    };
    let chats = match account_chats(&app, &account).await {
        Ok(chats) => chats,
        Err(response) => return response,
    };
    let mut projects = status.projects;
    projects.sort_by(|a, b| a.created_unix.cmp(&b.created_unix).then(a.id.cmp(&b.id)));
    let mut rows = Vec::with_capacity(projects.len());
    for p in &projects {
        let summary = match kept(&app, &account, &p.id).await {
            Ok(summary) => summary,
            Err(response) => return response,
        };
        rows.push(object(
            p,
            &chats,
            source_count(&app, &account, &p.id),
            &summary,
        ));
    }
    match paged(rows, &page) {
        Ok(body) => answer(StatusCode::OK, body),
        Err(response) => response,
    }
}

#[derive(Deserialize)]
struct AddBody {
    #[serde(default)]
    repository: Option<String>,
}

/// `POST /v1/projects` `{repository}`.
async fn add(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<axum::Json<AddBody>, JsonRejection>,
) -> Response {
    let (service, token, _) = match caller(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(axum::Json(AddBody {
        repository: Some(repository),
    })) = body
    else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "A project is one GitHub repository. Send {\"repository\": \"owner/name\"} as JSON.",
        );
    };
    match service.add_project_as(&token, repository.trim()).await {
        Ok(project) => answer(
            StatusCode::CREATED,
            object(&project, &[], 0, &Summary::default()),
        ),
        Err(error) => failed(error),
    }
}

/// `GET /v1/projects/{id}`.
async fn show(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (service, token, account) = match caller(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let status = match status(service, &token).await {
        Ok(status) => status,
        Err(response) => return response,
    };
    let project = match named(&status.projects, &id) {
        Ok(project) => project,
        Err(response) => return response,
    };
    let chats = match account_chats(&app, &account).await {
        Ok(chats) => chats,
        Err(response) => return response,
    };
    let sources = source_count(&app, &account, &project.id);
    let summary = match kept(&app, &account, &project.id).await {
        Ok(summary) => summary,
        Err(response) => return response,
    };
    answer(StatusCode::OK, object(project, &chats, sources, &summary))
}

/// `PATCH /v1/projects/{id}` `{"protection": "standard"|"only_me"}`.
async fn edit(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<axum::Json<super::files::EditBody>, JsonRejection>,
) -> Response {
    let (service, token, account) = match caller(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let status = match status(service, &token).await {
        Ok(status) => status,
        Err(response) => return response,
    };
    let project = match named(&status.projects, &id) {
        Ok(project) => project,
        Err(response) => return response,
    };
    let Ok(axum::Json(body)) = body else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send {\"protection\": \"standard\"} or {\"protection\": \"only_me\"} as JSON.",
        );
    };
    if let Err(response) = super::files::edit(&app, &account, &project.id, body).await {
        return response;
    }
    let chats = match account_chats(&app, &account).await {
        Ok(chats) => chats,
        Err(response) => return response,
    };
    let sources = source_count(&app, &account, &project.id);
    let summary = match kept(&app, &account, &project.id).await {
        Ok(summary) => summary,
        Err(response) => return response,
    };
    answer(StatusCode::OK, object(project, &chats, sources, &summary))
}

#[derive(Deserialize, Default)]
struct RemoveQuery {
    #[serde(default)]
    detach_threads: bool,
}

/// `DELETE /v1/projects/{id}[?detach_threads=true]`.
async fn remove(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<RemoveQuery>,
) -> Response {
    let (service, token, account) = match caller(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let status = match status(service, &token).await {
        Ok(status) => status,
        Err(response) => return response,
    };
    let project = match named(&status.projects, &id) {
        Ok(project) => project.id.clone(),
        Err(response) => return response,
    };
    let chats = match account_chats(&app, &account).await {
        Ok(chats) => chats,
        Err(response) => return response,
    };
    let attached: Vec<&Conversation> = in_project(&chats, &project).collect();
    if !attached.is_empty() && !query.detach_threads {
        return refused(
            StatusCode::CONFLICT,
            "project_has_threads",
            &format!(
                "This project has {} chat{}. Remove it with ?detach_threads=true to keep them outside any project.",
                attached.len(),
                if attached.len() == 1 { "" } else { "s" }
            ),
        );
    }
    if let Err(error) = service.remove_project_as(&token, &project).await {
        return failed(error);
    }
    if let Err(error) = super::files::forget_project(&app, &account, &project).await {
        return stored(&error);
    }
    let owner = account_owner(&account);
    for chat in attached {
        if let Err(error) = detach(&app, &owner, &chat.id).await {
            return stored(&error);
        }
    }
    answer(
        StatusCode::OK,
        json!({"id": project, "object": "project.deleted"}),
    )
}

/// Take the chat out of its project, again after a concurrent change.
async fn detach(app: &App, owner: &str, id: &str) -> Result<(), Error> {
    let store = &app.config.chat_store;
    for _ in 0..3 {
        let Some(loaded) = store.load(owner, id).await? else {
            return Ok(());
        };
        if loaded.conversation.project.is_none() {
            return Ok(());
        }
        let mut changed = loaded.conversation.clone();
        changed.project = None;
        changed.branch = None;
        changed.revision += 1;
        match store.compare_and_swap(&loaded, &changed).await {
            Ok(_) => return Ok(()),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

// -------------------------------------------------------------- threads

fn surface(chat: &Conversation) -> &'static str {
    match &chat.terminal {
        None => "web",
        Some(t) if crate::phone_api::phone_session(&t.session) => "phone",
        Some(_) => "terminal",
    }
}

/// One thread as the API shows it.
fn thread_object(chat: &Conversation) -> Value {
    json!({
        "id": chat.id,
        "object": "thread",
        "title": chat.title,
        "project": chat.project,
        "surface": surface(chat),
        "computer": chat.terminal.as_ref().map(|t| t.computer.clone()),
        "updated_at": rfc3339(chat.updated_unix),
        "messages": chat.messages.len(),
        "pinned": chat.pinned_unix.is_some(),
        "archived": chat.archived_unix.is_some(),
    })
}

/// `GET /v1/threads?project=…` (`crate::phone_api` hands the request
/// here when it names a project): the account's threads in one project
/// (or `none`), by last activity, oldest first.
pub(crate) async fn threads_in(
    app: &App,
    headers: &HeaderMap,
    reference: &str,
    page: &Page,
) -> Response {
    let (service, token, account) = match caller(app, headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let project = if reference == "none" {
        None
    } else {
        let status = match status(service, &token).await {
            Ok(status) => status,
            Err(response) => return response,
        };
        match named(&status.projects, reference) {
            Ok(project) => Some(project.id.clone()),
            Err(response) => return response,
        }
    };
    let mut chats: Vec<Conversation> = match account_chats(app, &account).await {
        Ok(chats) => chats,
        Err(response) => return response,
    };
    chats.retain(|chat| chat.project == project);
    chats.sort_by(|a, b| a.updated_unix.cmp(&b.updated_unix).then(a.id.cmp(&b.id)));
    let rows = chats.iter().map(thread_object).collect();
    match paged(rows, page) {
        Ok(body) => answer(StatusCode::OK, body),
        Err(response) => response,
    }
}

/// `PATCH /v1/threads/{id}` `{project}` (#11541): move the thread into one
/// of the caller's projects (its id, or `name:NAME`), or out of any
/// project (`null`). The person's move: sync no longer files the thread
/// ([`Conversation::filed`]). Answers the `Thread`.
pub(crate) async fn move_thread(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<axum::Json<Value>, JsonRejection>,
) -> Response {
    let (service, token, account) = match caller(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let reference = match body.as_ref().map(|body| body.get("project")) {
        Ok(Some(Value::Null)) => None,
        Ok(Some(Value::String(reference))) => Some(reference.trim().to_owned()),
        _ => {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Send {\"project\": ID} to move the chat into a project, or {\"project\": null} to take it out.",
            );
        }
    };
    let project = match reference {
        None => None,
        Some(reference) => {
            let status = match status(service, &token).await {
                Ok(status) => status,
                Err(response) => return response,
            };
            match named(&status.projects, &reference) {
                Ok(project) => Some(project.id.clone()),
                Err(response) => return response,
            }
        }
    };
    let owner = account_owner(&account);
    let store = &app.config.chat_store;
    for _ in 0..3 {
        let loaded = match store.load(&owner, &id).await {
            Ok(Some(loaded)) if !loaded.conversation.deleted() => loaded,
            Ok(_) | Err(Error::Invalid(_)) => {
                return refused(StatusCode::NOT_FOUND, "not_found", "No such chat.");
            }
            Err(error) => return stored(&error),
        };
        let current = &loaded.conversation;
        if current.project == project && current.filed {
            return answer(StatusCode::OK, thread_object(current));
        }
        let mut moved = current.clone();
        if moved.project != project {
            moved.branch = None;
        }
        moved.project.clone_from(&project);
        moved.filed = true;
        moved.revision += 1;
        match store.compare_and_swap(&loaded, &moved).await {
            Ok(_) => return answer(StatusCode::OK, thread_object(&moved)),
            Err(Error::Conflict) => continue,
            Err(error) => return stored(&error),
        }
    }
    stored(&Error::Conflict)
}

/// `GET /v1/threads/{id}/messages`: the thread's messages, oldest first.
pub(crate) async fn messages(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(page): Query<Page>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let chat = match app.config.chat_store.load(&owner, &id).await {
        Ok(Some(loaded)) if !loaded.conversation.deleted() => loaded.conversation,
        Ok(_) | Err(Error::Invalid(_)) => {
            return refused(StatusCode::NOT_FOUND, "not_found", "No such chat.");
        }
        Err(error) => return stored(&error),
    };
    let rows = chat
        .messages
        .iter()
        .enumerate()
        .map(|(at, message)| {
            json!({
                "id": format!("msg_{at}"),
                "object": "message",
                "thread": chat.id,
                "role": match message.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => "tool",
                },
                "content": [{"type": "text", "text": message.text}],
                "files": null,
                "generated_files": null,
                "artifacts": null,
            })
        })
        .collect();
    match paged(rows, &page) {
        Ok(body) => answer(StatusCode::OK, body),
        Err(response) => response,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn times_read_as_rfc3339() {
        assert_eq!(super::rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(super::rfc3339(1_791_504_069), "2026-10-09T00:01:09Z");
    }
}
