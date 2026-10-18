//! The signed-in person's projects and the threads in them, for apps and
//! agents (#11471, `docs/api/projects.md`): `openagents projects`, Coder,
//! and any script holding the account's own sign-in.
//!
//! A project is a named home for chats (threads), instructions, files,
//! and zero or more connected GitHub repositories, one of them the primary
//! when there are any. The same repository may be in several projects and
//! names may repeat. The left panel groups chats under it, and Drive
//! sources attach to it
//! (`/projects/{id}/sources`). Every route takes the same credential as the
//! other `/v1` app routes (`Authorization: Bearer sess_…` from `openagents
//! login` or `coder login`, or a sign-in key `oa_sk_…`) and answers only
//! that account's own data; another account's project or thread answers
//! `404`, the same as one that doesn't exist. Reads only read.
//!
//! | Route | What |
//! | --- | --- |
//! | `GET /v1/projects` | `{data: [Project], next}`, oldest first (`?limit`, `?after`) |
//! | `POST /v1/projects` `{name, instructions?, repositories?, protection?}` | A new project (always new); `{repository}` alone is the older form, named after it: `201 Project` |
//! | `GET /v1/projects/{id}` | One `Project`; `{id}` may be `name:NAME` (`409 ambiguous_name` names the choices when several share it) |
//! | `PATCH /v1/projects/{id}` `{name?, instructions?, primary_repository?, protection?}` | Rename, set or clear (`null`) instructions, make one of its repositories the primary, or set the default for new files (`standard`, `only_me`; [`super::files`]) |
//! | `POST /v1/projects/{id}/repositories` `{repository, primary?}` | Add a connected repository (the first one is the primary): `Project` |
//! | `DELETE /v1/projects/{id}/repositories/{owner}/{name}` | Remove one; removing the primary makes the first one left the primary: `Project` |
//! | `DELETE /v1/projects/{id}` | `{id, object: "project.deleted"}`; `409 project_has_threads` while threads are in it, unless `?detach_threads=true` (the threads stay, outside any project) |
//! | `GET /v1/threads?project=ID\|name:NAME\|none` | `{data: [Thread], next}`, by last activity, oldest first (`?limit`, `?after`) |
//! | `PATCH /v1/threads/{id}` `{project}` | Move the thread into a project (its id or `name:NAME`), or out of any (`null`): `Thread`. Sync then leaves it where the person put it (#11541) |
//! | `GET /v1/threads/{id}/messages` | `{data: [Message], next}`, oldest first (`?limit`, `?after`) |
//!
//! `Project` is `{id, object: "project", name, instructions, repositories:
//! [{repository, primary, default_branch, private}], repository,
//! default_branch, private, created_at, updated_at, protection, counts:
//! {threads, files, documents, sources}}`; times are RFC 3339 UTC.
//! `repository`, `default_branch`, and `private` are the primary's (null,
//! null, false without one), kept for apps from before 2026-10-13 until
//! the next app release, and deprecated. Errors are
//! `{error: {code, message}}`. The account service keeps the projects; this
//! server reads them with the caller's own credential.

use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use oa_auth::repos::{Project, RepoError, Role as ProjectRole, Status};
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
        .route(
            "/v1/projects/{id}/repositories",
            axum::routing::post(add_repository),
        )
        .route(
            "/v1/projects/{id}/repositories/{owner}/{name}",
            axum::routing::delete(remove_repository),
        )
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
        "Log in again with openagents login, or use a login key.",
    )
}

/// The app API's answer for a failed account-service call.
pub(crate) fn failed(error: RepoCallError) -> Response {
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
                RepoError::NotAllowed => (StatusCode::FORBIDDEN, "project_role"),
                RepoError::OwnerStays => (StatusCode::CONFLICT, "owner_stays"),
                RepoError::InvitationGone => (StatusCode::NOT_FOUND, "invitation_not_found"),
                RepoError::ProjectFull => (StatusCode::CONFLICT, "project_full"),
                _ => (StatusCode::BAD_GATEWAY, "github_unavailable"),
            };
            refused(status, code, &error.to_string())
        }
    }
}

/// The caller's role, read from the project the account service answered
/// with (an account service from before #11585 answers only the caller's
/// own projects: the owner).
pub(crate) fn role_of(project: &Project) -> ProjectRole {
    project
        .sharing
        .as_ref()
        .map_or(ProjectRole::Owner, |s| s.role)
}

/// The account whose storage keeps the project's files, notes, and Drive
/// sources: the caller's own, unless the project was shared with them.
pub(crate) fn home_of<'a>(project: &'a Project, account: &'a str) -> &'a str {
    project
        .sharing
        .as_ref()
        .and_then(|s| s.home.as_deref())
        .unwrap_or(account)
}

/// Whether other people are in the project.
pub(crate) fn shared(project: &Project) -> bool {
    project.sharing.as_ref().is_some_and(|s| s.shared())
}

/// The answer when the caller's role doesn't allow an action.
pub(crate) fn role_refused() -> Response {
    refused(
        StatusCode::FORBIDDEN,
        "project_role",
        &RepoError::NotAllowed.to_string(),
    )
}

/// The caller's access to one project (#11585).
pub(crate) struct Access {
    pub(crate) account: String,
    /// The caller's own sign-in, for the account service.
    pub(crate) token: String,
    pub(crate) project: Project,
    pub(crate) role: ProjectRole,
}

impl Access {
    /// The chat-store owner whose folder keeps the project's files.
    pub(crate) fn home_owner(&self) -> String {
        account_owner(home_of(&self.project, &self.account))
    }

    /// The home account.
    pub(crate) fn home(&self) -> &str {
        home_of(&self.project, &self.account)
    }

    /// Editors and the owner add and change; viewers only read.
    pub(crate) fn edits(&self) -> Result<(), Response> {
        if self.role.edits() {
            Ok(())
        } else {
            Err(role_refused())
        }
    }

    /// The owner only.
    pub(crate) fn owns(&self) -> Result<(), Response> {
        if self.role.owns() {
            Ok(())
        } else {
            Err(role_refused())
        }
    }
}

/// The caller's access to the project `reference` names (`404` when they
/// aren't in it).
pub(crate) async fn access(
    app: &App,
    headers: &HeaderMap,
    reference: &str,
) -> Result<Access, Response> {
    let (service, token, account) = caller(app, headers).await?;
    let status = status(service, &token).await?;
    let project = named(&status.projects, reference)?.clone();
    let role = role_of(&project);
    Ok(Access {
        account,
        token,
        project,
        role,
    })
}

/// A chat someone else started in a shared project the app's caller
/// (`owner`) is in, with that project (#11586).
pub(crate) async fn shared_thread(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    id: &str,
) -> Option<(crate::chat_store::Loaded, Project)> {
    let (service, token, _) = caller(app, headers).await.ok()?;
    let status = service.github_status_as(&token).await.ok()?;
    super::shared_chats::find(app, &status.projects, owner, id)
        .await
        .ok()?
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
            several => Err(refused(
                StatusCode::CONFLICT,
                "ambiguous_name",
                &ambiguous(name, several),
            )),
        };
    }
    projects
        .iter()
        .find(|project| project.id == reference)
        .ok_or_else(not_yours)
}

/// The plain choice when several projects share `name`: "2 projects are
/// called finances: prj_… (AtlantisPleb/finances), prj_… (no repository).
/// Use its id."
pub(crate) fn ambiguous(name: &str, several: &[&Project]) -> String {
    let choices: Vec<String> = several
        .iter()
        .map(|p| format!("{} ({})", p.id, p.repository().unwrap_or("no repository")))
        .collect();
    format!(
        "{} projects are called {name}: {}. Use its id.",
        several.len(),
        choices.join(", ")
    )
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
        "instructions": project.instructions,
        "repositories": project.repositories.iter().map(|r| json!({
            "repository": r.repository,
            "primary": r.primary,
            "default_branch": r.default_branch,
            "private": r.private,
        })).collect::<Vec<_>>(),
        // Deprecated: the primary's, for apps from before 2026-10-13.
        "repository": project.repository(),
        "default_branch": project.primary().map(|r| r.default_branch.clone()),
        "private": project.primary().is_some_and(|r| r.private),
        "created_at": rfc3339(project.created_unix),
        "updated_at": rfc3339(updated),
        "protection": kept.protection.word(),
        "counts": {"threads": threads, "files": kept.files, "documents": kept.documents, "sources": sources},
        "role": role_of(project),
        "members": project.sharing.as_ref().map_or(1, |s| s.members),
        "shared": shared(project),
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

/// A shared project's chats, everyone's (#11586); `None` for a project
/// only the caller is in, whose chats are their own.
async fn shared_chats_of(
    app: &App,
    project: &Project,
    owner: &str,
    own: &[Conversation],
) -> Result<Option<Vec<Conversation>>, Response> {
    if !shared(project) {
        return Ok(None);
    }
    super::shared_chats::chats_in(app, project, owner, own)
        .await
        .map(Some)
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
    let owner = account_owner(&account);
    for p in &projects {
        let home = home_of(p, &account);
        let summary = match kept(&app, home, &p.id).await {
            Ok(summary) => summary,
            Err(response) => return response,
        };
        let theirs = match shared_chats_of(&app, p, &owner, &chats).await {
            Ok(theirs) => theirs,
            Err(response) => return response,
        };
        rows.push(object(
            p,
            theirs.as_deref().unwrap_or(&chats),
            source_count(&app, home, &p.id),
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
    name: Option<String>,
    #[serde(default)]
    instructions: Option<String>,
    #[serde(default)]
    repositories: Vec<String>,
    /// The older form: one repository, the project named after it.
    #[serde(default)]
    repository: Option<String>,
    #[serde(default)]
    protection: Option<String>,
}

/// `POST /v1/projects` `{name, instructions?, repositories?, protection?}`
/// (or the older `{repository}`): always a new project.
async fn add(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<axum::Json<AddBody>, JsonRejection>,
) -> Response {
    let (service, token, account) = match caller(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(axum::Json(body)) = body else {
        return invalid_new();
    };
    let mut repositories: Vec<String> = body
        .repository
        .iter()
        .chain(&body.repositories)
        .map(|r| r.trim().to_owned())
        .collect();
    repositories.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    let name = body
        .name
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty());
    if name.is_none() && repositories.is_empty() {
        return invalid_new();
    }
    let new = oa_auth::repos::NewProject {
        name,
        instructions: body.instructions,
        repositories,
    };
    let project = match service.create_project_as(&token, &new).await {
        Ok(project) => project,
        Err(error) => return failed(error),
    };
    if let Some(protection) = body.protection.as_deref()
        && let Err(response) = super::files::edit(&app, &account, &project.id, protection).await
    {
        return response;
    }
    let summary = match kept(&app, &account, &project.id).await {
        Ok(summary) => summary,
        Err(response) => return response,
    };
    answer(StatusCode::CREATED, object(&project, &[], 0, &summary))
}

fn invalid_new() -> Response {
    refused(
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "Send {\"name\": \"…\"} as JSON, with \"instructions\" and \"repositories\": [\"owner/name\"] if you like. A project needs no repository.",
    )
}

/// The project as the API shows it, after a change.
async fn shown(app: &App, account: &str, project: &Project) -> Response {
    let chats = match account_chats(app, account).await {
        Ok(chats) => chats,
        Err(response) => return response,
    };
    let home = home_of(project, account);
    let sources = source_count(app, home, &project.id);
    let summary = match kept(app, home, &project.id).await {
        Ok(summary) => summary,
        Err(response) => return response,
    };
    answer(StatusCode::OK, object(project, &chats, sources, &summary))
}

#[derive(Deserialize)]
struct RepositoryBody {
    repository: String,
    #[serde(default)]
    primary: bool,
}

/// `POST /v1/projects/{id}/repositories` `{repository, primary?}`.
async fn add_repository(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<axum::Json<RepositoryBody>, JsonRejection>,
) -> Response {
    let Ok(axum::Json(body)) = body else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send {\"repository\": \"owner/name\"} as JSON, with \"primary\": true to make it the primary.",
        );
    };
    change(
        &app,
        &headers,
        &id,
        oa_auth::repos::ProjectChange {
            add_repository: Some(body.repository.trim().to_owned()),
            primary: body.primary,
            ..Default::default()
        },
    )
    .await
}

/// `DELETE /v1/projects/{id}/repositories/{owner}/{name}`.
async fn remove_repository(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, owner, name)): Path<(String, String, String)>,
) -> Response {
    change(
        &app,
        &headers,
        &id,
        oa_auth::repos::ProjectChange {
            remove_repository: Some(format!("{owner}/{name}")),
            ..Default::default()
        },
    )
    .await
}

/// Apply `change` to the caller's project `reference` at the account
/// service and answer the Project.
async fn change(
    app: &App,
    headers: &HeaderMap,
    reference: &str,
    change: oa_auth::repos::ProjectChange,
) -> Response {
    let (service, token, account) = match caller(app, headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let status = match status(service, &token).await {
        Ok(status) => status,
        Err(response) => return response,
    };
    let project = match named(&status.projects, reference) {
        Ok(project) => project.id.clone(),
        Err(response) => return response,
    };
    for repository in [&change.remove_repository, &change.primary_repository]
        .into_iter()
        .flatten()
    {
        if status
            .projects
            .iter()
            .find(|p| p.id == project)
            .and_then(|p| p.holding(repository))
            .is_none()
        {
            return refused(
                StatusCode::NOT_FOUND,
                "not_found",
                &format!("{repository} isn't in this project."),
            );
        }
    }
    match service.edit_project_as(&token, &project, &change).await {
        Ok(changed) => shown(app, &account, &changed).await,
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
    let home = home_of(project, &account);
    let sources = source_count(&app, home, &project.id);
    let summary = match kept(&app, home, &project.id).await {
        Ok(summary) => summary,
        Err(response) => return response,
    };
    let theirs = match shared_chats_of(&app, project, &account_owner(&account), &chats).await {
        Ok(theirs) => theirs,
        Err(response) => return response,
    };
    answer(
        StatusCode::OK,
        object(
            project,
            theirs.as_deref().unwrap_or(&chats),
            sources,
            &summary,
        ),
    )
}

#[derive(Deserialize)]
struct EditBody {
    #[serde(default)]
    protection: Option<String>,
    #[serde(default)]
    name: Option<String>,
    /// A string sets them; `null` clears them.
    #[serde(default, deserialize_with = "present")]
    instructions: Option<Option<String>>,
    #[serde(default)]
    primary_repository: Option<String>,
    /// Repositories change through `/v1/projects/{id}/repositories`.
    #[serde(default)]
    repository: Option<Value>,
}

/// A field that may be `null` (`Some(None)`) as well as absent (`None`).
fn present<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(deserializer).map(Some)
}

/// `PATCH /v1/projects/{id}` `{name?, instructions?, primary_repository?,
/// protection?}`.
async fn edit(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<axum::Json<EditBody>, JsonRejection>,
) -> Response {
    let Ok(axum::Json(body)) = body else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send any of {\"name\", \"instructions\", \"primary_repository\", \"protection\"} as JSON.",
        );
    };
    if body.repository.is_some() {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Add a repository with POST /v1/projects/{id}/repositories, remove one with DELETE /v1/projects/{id}/repositories/{owner}/{name}, and pick the primary with {\"primary_repository\": \"owner/name\"}.",
        );
    }
    let (service, token, account) = match caller(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let status = match status(service, &token).await {
        Ok(status) => status,
        Err(response) => return response,
    };
    let project = match named(&status.projects, &id) {
        Ok(project) => project.clone(),
        Err(response) => return response,
    };
    if let Some(protection) = body.protection.as_deref() {
        if !role_of(&project).owns() {
            return role_refused();
        }
        if protection == "only_me" && shared(&project) {
            return refused(
                StatusCode::CONFLICT,
                "shared_project",
                "Other people are in this project, so its files can't open only on your devices. Remove them first.",
            );
        }
        if let Err(response) =
            super::files::edit(&app, home_of(&project, &account), &project.id, protection).await
        {
            return response;
        }
    }
    let edit = oa_auth::repos::ProjectChange {
        name: body.name.map(|n| n.trim().to_owned()),
        instructions: body.instructions,
        primary_repository: body.primary_repository.map(|r| r.trim().to_owned()),
        ..Default::default()
    };
    if edit == oa_auth::repos::ProjectChange::default() {
        if body.protection.is_none() {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Send any of {\"name\", \"instructions\", \"primary_repository\", \"protection\"} as JSON.",
            );
        }
        return shown(&app, &account, &project).await;
    }
    change(&app, &headers, &project.id, edit).await
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
    let (project, home) = match named(&status.projects, &id) {
        Ok(project) if !role_of(project).owns() => return role_refused(),
        Ok(project) => (project.id.clone(), home_of(project, &account).to_owned()),
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
    if let Err(error) = super::files::forget_project(&app, &home, &project).await {
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
            Ok(project) => Some(project.clone()),
            Err(response) => return response,
        }
    };
    let own: Vec<Conversation> = match account_chats(app, &account).await {
        Ok(chats) => chats,
        Err(response) => return response,
    };
    let owner = account_owner(&account);
    let (mut chats, names) = match &project {
        // Every member's chats in a shared project (#11586).
        Some(project) => match super::shared_chats::chats_in(app, project, &owner, &own).await {
            Ok(chats) => (
                chats,
                super::shared_chats::starters(app, headers, project).await,
            ),
            Err(error) => return stored(&error),
        },
        None => (
            own.into_iter()
                .filter(|chat| chat.project.is_none())
                .collect(),
            Default::default(),
        ),
    };
    chats.sort_by(|a, b| a.updated_unix.cmp(&b.updated_unix).then(a.id.cmp(&b.id)));
    let rows = chats
        .iter()
        .map(|chat| {
            let mut row = thread_object(chat);
            row["started_by"] = json!({
                "name": super::shared_chats::started_by(chat, &owner, &names),
                "you": chat.owner == owner,
            });
            row
        })
        .collect();
    match paged(rows, page) {
        Ok(body) => answer(StatusCode::OK, body),
        Err(response) => response,
    }
}

/// `PATCH /v1/threads/{id}` `{project?, pinned?, archived?}`: change the
/// caller's own chat. `project` (#11541) moves the thread into one of the
/// caller's projects (its id, or `name:NAME`), or out of any project
/// (`null`); the person's move, so sync no longer files the thread
/// ([`Conversation::filed`]). `pinned` and `archived` (#12026) pin or
/// archive it as the web's sidebar does, with the same pin order, archive
/// stamp, and computer cleanup. Answers the `Thread`.
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
    let Some(change) = body.ok().and_then(|body| ThreadChange::read(&body)) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send {\"project\": ID} or {\"project\": null} to file the chat, {\"pinned\": true or false} to pin it, or {\"archived\": true or false} to archive it.",
        );
    };
    let project = match change.project.clone() {
        None => None,
        Some(None) => Some(None),
        Some(Some(reference)) => {
            let status = match status(service, &token).await {
                Ok(status) => status,
                Err(response) => return response,
            };
            match named(&status.projects, &reference) {
                Ok(project) if !role_of(project).edits() => return role_refused(),
                Ok(project) => Some(Some(project.clone())),
                Err(response) => return response,
            }
        }
    };
    let target = project.clone().flatten();
    let project = project.map(|p| p.map(|p| p.id));
    let owner = account_owner(&account);
    let pin_stamp = match change.pinned {
        Some(true) => match crate::pages::chat::sidebar::pin_stamp(&app, &owner).await {
            Ok(stamp) => stamp,
            Err(error) => return stored(&error),
        },
        _ => 0,
    };
    let archive_stamp = now();
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
        let mut next = current.clone();
        let mut moved = false;
        if let Some(project) = &project {
            if next.project != *project {
                next.branch = None;
            }
            moved = next.project != *project || !next.filed;
            next.project.clone_from(project);
            next.filed = true;
        }
        let pinned = change
            .pinned
            .is_some_and(|pin| crate::pages::chat::sidebar::set_pinned(&mut next, pin, pin_stamp));
        let archived = change.archived.is_some_and(|archive| {
            crate::pages::chat::sidebar::set_archived(&mut next, archive, archive_stamp)
        });
        if !moved && !pinned && !archived {
            return answer(StatusCode::OK, thread_object(current));
        }
        next.revision += 1;
        match store.compare_and_swap(&loaded, &next).await {
            Ok(_) => {
                if moved {
                    crate::pages::chat::note_shared(&app, target.as_ref(), &owner, &next).await;
                }
                if archived && next.archived_unix.is_some() {
                    crate::pages::chat::sidebar::archived_cleanup(&app, &headers, &owner, &id)
                        .await;
                }
                return answer(StatusCode::OK, thread_object(&next));
            }
            Err(Error::Conflict) => continue,
            Err(error) => return stored(&error),
        }
    }
    stored(&Error::Conflict)
}

/// What a `PATCH /v1/threads/{id}` body asks for; `None` from [`Self::read`]
/// when it asks for nothing or names a field with the wrong type.
struct ThreadChange {
    /// `Some(None)` takes the chat out of any project.
    project: Option<Option<String>>,
    pinned: Option<bool>,
    archived: Option<bool>,
}

impl ThreadChange {
    fn read(body: &Value) -> Option<Self> {
        let body = body.as_object()?;
        let project = match body.get("project") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(Value::String(reference)) => Some(Some(reference.trim().to_owned())),
            Some(_) => return None,
        };
        let flag = |name: &str| match body.get(name) {
            None => Ok(None),
            Some(Value::Bool(value)) => Ok(Some(*value)),
            Some(_) => Err(()),
        };
        let pinned = flag("pinned").ok()?;
        let archived = flag("archived").ok()?;
        (project.is_some() || pinned.is_some() || archived.is_some()).then_some(Self {
            project,
            pinned,
            archived,
        })
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
        Ok(_) | Err(Error::Invalid(_)) => match shared_thread(&app, &headers, &owner, &id).await {
            Some((loaded, _)) => loaded.conversation,
            None => return refused(StatusCode::NOT_FOUND, "not_found", "No such chat."),
        },
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
                    Role::Thinking => "thinking",
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
