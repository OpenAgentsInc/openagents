//! Who is in a project, and inviting people into it (#11585,
//! `docs/api/projects.md` "Members and invitations"). The account service
//! keeps the members and the invitations and checks every role; these
//! routes find the project by its id or `name:NAME`, add what only this
//! server knows (a project whose files open only on the owner's devices
//! can't be shared), and answer in the API's shapes.
//!
//! | Route | Who | What |
//! | --- | --- | --- |
//! | `GET /v1/projects/{id}/members` | Any member | `{data: [Member], role, invitations?}`; the owner also sees open invitations |
//! | `PATCH /v1/projects/{id}/members/{account}` `{role}` | Owner | `editor` or `viewer`; `owner` gives the project to that member (you become an editor) |
//! | `DELETE /v1/projects/{id}/members/{account}` | Owner, or the member themselves | Removes them; it takes effect on their next request |
//! | `POST /v1/projects/{id}/invitations` `{role?, expires_in?}` | Owner | `201 {code, url, role, expires_at}`: a single-use link, shown once; `editor` by default, for 7 days (`expires_in` seconds, 10 minutes to 30 days) |
//! | `POST /v1/projects/{id}/leave` | Any member but the owner | `{id, object: "project.left"}` |
//! | `POST /v1/projects/invitations/preview` `{code}` | Anyone signed in | `{project: {id, name}, role, invited_by, expires_at, member}` without using it |
//! | `POST /v1/projects/invitations/accept` `{code}` | Anyone signed in | Joins: the `Project`, with your role |
//!
//! `Member` is `{account, object: "project.member", name, login, role,
//! joined_at, you}`.

use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use reqwest::Method;
use serde_json::{Value, json};

use super::api::{Access, access, caller, failed, object, rfc3339};
use super::files::{Protection, Summary};
use crate::App;
use crate::coder_sync::{answer, refused};

/// Where an invitation link opens on the website.
pub(crate) const JOIN: &str = "/projects/join";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/projects/{id}/members", get(members))
        .route(
            "/v1/projects/{id}/members/{member}",
            axum::routing::patch(set_role).delete(remove_member),
        )
        .route("/v1/projects/{id}/invitations", post(invite))
        .route("/v1/projects/{id}/leave", post(leave))
        .route("/v1/projects/{id}/access", get(repository_access))
        .route("/v1/projects/invitations/preview", post(preview))
        .route("/v1/projects/invitations/accept", post(accept))
}

/// What the invite action says for a project whose files open only on its
/// owner's devices (#11588).
pub(crate) const ONLY_ME_REFUSAL: &str =
    "Files in this project open only on your devices. Change it to standard to share it.";

/// The caller's access to `reference` and their token.
async fn reach(
    app: &App,
    headers: &HeaderMap,
    reference: &str,
) -> Result<(String, Access), Response> {
    let found = access(app, headers, reference).await?;
    Ok((found.token.clone(), found))
}

async fn call(
    app: &App,
    token: &str,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, Response> {
    let service = app.config.cloud.as_deref().ok_or_else(|| {
        refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "This site doesn't offer accounts.",
        )
    })?;
    service
        .project_call_as(token, method, path, body)
        .await
        .map_err(failed)
}

/// One member as the API shows them.
fn member(row: &Value) -> Value {
    json!({
        "account": row["account"],
        "object": "project.member",
        "name": row["name"],
        "login": row["login"],
        "role": row["role"],
        "joined_at": rfc3339(row["joined_unix"].as_u64().unwrap_or(0)),
        "you": row["you"],
    })
}

/// The account service's members answer in the API's shape.
fn listed(body: &Value) -> Value {
    let mut out = json!({
        "data": body["members"].as_array().map(|rows| rows.iter().map(member).collect::<Vec<_>>()).unwrap_or_default(),
        "next": null,
        "role": body["role"],
    });
    if let Some(open) = body["invitations"].as_array() {
        out["invitations"] = open
            .iter()
            .map(|i| {
                json!({
                    "id": i["id"],
                    "role": i["role"],
                    "expires_at": rfc3339(i["expires_unix"].as_u64().unwrap_or(0)),
                })
            })
            .collect::<Vec<_>>()
            .into();
    }
    out
}

/// `GET /v1/projects/{id}/members`.
async fn members(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (token, found) = match reach(&app, &headers, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let path = format!("/v1/projects/{}/members", found.project.id);
    match call(&app, &token, Method::GET, &path, None).await {
        Ok(body) => answer(StatusCode::OK, listed(&body)),
        Err(response) => response,
    }
}

fn bad_member(member: &str) -> Option<Response> {
    let fine = !member.is_empty()
        && member.len() <= 128
        && member
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'));
    (!fine).then(|| {
        refused(
            StatusCode::NOT_FOUND,
            "not_found",
            "That person isn't in this project. See who is with: openagents projects members PROJECT",
        )
    })
}

/// The project after a membership change, as the API shows it.
async fn project_after(app: &App, found: &Access, body: &Value) -> Response {
    let project = match serde_json::from_value::<oa_auth::repos::Project>(body["project"].clone()) {
        Ok(project) => project,
        Err(_) => found.project.clone(),
    };
    let home = super::api::home_of(&project, &found.account).to_owned();
    let summary = super::files::summary(app, &home, &project.id)
        .await
        .unwrap_or_default();
    answer(StatusCode::OK, object(&project, &[], 0, &summary))
}

/// `PATCH /v1/projects/{id}/members/{account}` `{role}`.
async fn set_role(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, member)): Path<(String, String)>,
    body: Result<axum::Json<Value>, JsonRejection>,
) -> Response {
    let role = body
        .ok()
        .and_then(|axum::Json(body)| body["role"].as_str().map(str::to_owned))
        .and_then(|role| oa_auth::repos::Role::from_word(&role));
    let Some(role) = role else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send {\"role\": \"editor\"}, {\"role\": \"viewer\"}, or {\"role\": \"owner\"} to give them the project.",
        );
    };
    if let Some(response) = bad_member(&member) {
        return response;
    }
    let (token, found) = match reach(&app, &headers, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let path = format!("/v1/projects/{}/members/{member}", found.project.id);
    match call(
        &app,
        &token,
        Method::PATCH,
        &path,
        Some(json!({"role": role.word()})),
    )
    .await
    {
        Ok(body) => project_after(&app, &found, &body).await,
        Err(response) => response,
    }
}

/// `DELETE /v1/projects/{id}/members/{account}`.
async fn remove_member(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, member)): Path<(String, String)>,
) -> Response {
    if let Some(response) = bad_member(&member) {
        return response;
    }
    let (token, found) = match reach(&app, &headers, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if member == found.account {
        return left(&app, &token, &found).await;
    }
    let path = format!("/v1/projects/{}/members/{member}", found.project.id);
    match call(&app, &token, Method::DELETE, &path, None).await {
        Ok(_) => answer(
            StatusCode::OK,
            json!({"account": member, "object": "project.member.removed", "project": found.project.id}),
        ),
        Err(response) => response,
    }
}

async fn left(app: &App, token: &str, found: &Access) -> Response {
    let path = format!("/v1/projects/{}/leave", found.project.id);
    match call(app, token, Method::POST, &path, None).await {
        Ok(_) => answer(
            StatusCode::OK,
            json!({"id": found.project.id, "object": "project.left"}),
        ),
        Err(response) => response,
    }
}

/// `GET /v1/projects/{id}/access` (#11587): whether your own GitHub
/// access reaches each of the project's repositories:
/// `{data: [{repository, access, message?}]}`.
async fn repository_access(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (token, found) = match reach(&app, &headers, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let path = format!("/v1/projects/{}/access", found.project.id);
    match call(&app, &token, Method::GET, &path, None).await {
        Ok(body) => answer(
            StatusCode::OK,
            json!({"data": body["repositories"].as_array().cloned().unwrap_or_default().iter().map(|row| json!({
                "repository": row["repository"],
                "access": row["access"],
                "message": row["message"],
            })).collect::<Vec<_>>()}),
        ),
        Err(response) => response,
    }
}

/// Why a member's work in a shared project's primary repository doesn't
/// start (#11587): `None` when their own GitHub access reaches it, or the
/// project is theirs, or it has no repository.
pub(crate) async fn primary_refusal(
    app: &App,
    headers: &HeaderMap,
    project: &oa_auth::repos::Project,
) -> Option<String> {
    let primary = project.primary()?;
    // A project only its owner is in works as it always has.
    if super::api::role_of(project).owns() && !super::api::shared(project) {
        return None;
    }
    let token = super::shared_chats::token(headers)?;
    let path = format!("/v1/projects/{}/access", project.id);
    let body = call(app, &token, Method::GET, &path, None).await.ok()?;
    let row = body["repositories"]
        .as_array()?
        .iter()
        .find(|row| row["repository"] == primary.repository.as_str())?;
    (row["access"] != true).then(|| {
        row["message"]
            .as_str()
            .unwrap_or("You don't have access to this repository on GitHub.")
            .to_owned()
    })
}

/// `POST /v1/projects/{id}/leave`.
async fn leave(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (token, found) = match reach(&app, &headers, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    left(&app, &token, &found).await
}

/// `POST /v1/projects/{id}/invitations` `{role?, expires_in?}`.
async fn invite(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<axum::Json<Value>, JsonRejection>,
) -> Response {
    let body = body.map(|axum::Json(body)| body).unwrap_or(json!({}));
    let role = match body.get("role") {
        None | Some(Value::Null) => Some("editor"),
        Some(Value::String(role)) if role == "editor" || role == "viewer" => Some(role.as_str()),
        Some(_) => None,
    };
    let seconds = match body.get("expires_in") {
        None | Some(Value::Null) => Some(0),
        Some(value) => value.as_u64(),
    };
    let (Some(role), Some(seconds)) = (role, seconds) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send {\"role\": \"editor\"} or {\"role\": \"viewer\"}, and \"expires_in\" in seconds if you like (7 days when left out).",
        );
    };
    let github = match body.get("github") {
        None | Some(Value::Null) => None,
        Some(Value::String(login)) => Some(login.clone()),
        Some(_) => {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "\"github\" is the GitHub username to invite.",
            );
        }
    };
    let (token, found) = match reach(&app, &headers, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if let Err(response) = found.owns() {
        return response;
    }
    match super::files::protection(
        &app.config.chat_store,
        &found.home_owner(),
        &found.project.id,
    )
    .await
    {
        Ok(Protection::OnlyMe) => {
            return refused(StatusCode::CONFLICT, "only_me_project", ONLY_ME_REFUSAL);
        }
        Ok(Protection::Standard) => {}
        Err(error) => return crate::coder_sync::stored(&error),
    }
    // Invited by GitHub username: it waits on their Projects page.
    let person = match github {
        Some(login) => match super::pending::github_person(&app, &headers, &login).await {
            Ok(person) => Some(person),
            Err(problem) => {
                return refused(StatusCode::BAD_REQUEST, "unknown_github_user", &problem);
            }
        },
        None => None,
    };
    let path = format!("/v1/projects/{}/invitations", found.project.id);
    let made = match call(
        &app,
        &token,
        Method::POST,
        &path,
        Some(json!({"role": role, "seconds": seconds})),
    )
    .await
    {
        Ok(made) => made,
        Err(response) => return response,
    };
    let Some(code) = made["code"].as_str() else {
        return failed(crate::cloud::session::github::RepoCallError::Session(
            crate::cloud::session::SessionError::Unavailable,
        ));
    };
    let expires_unix = made["invitation"]["expires_unix"].as_u64().unwrap_or(0);
    let invitee = match person {
        Some((github, login)) => {
            // Who invited, as the invitation's own page names them.
            let from = call(
                &app,
                &token,
                Method::POST,
                "/v1/projects/invitations/preview",
                Some(json!({"code": code})),
            )
            .await
            .ok()
            .and_then(|preview| preview["invited_by"].as_str().map(str::to_owned))
            .unwrap_or_else(|| "Someone".to_owned());
            let waiting = super::pending::Pending {
                code: code.to_owned(),
                project: found.project.id.clone(),
                project_name: found.project.name.clone(),
                from,
                role: made["invitation"]["role"]
                    .as_str()
                    .unwrap_or(role)
                    .to_owned(),
                expires_unix,
            };
            if let Err(error) =
                super::pending::record(&app.config.chat_store, github, &waiting).await
            {
                eprintln!("openagents-web: project invitation for {login}: {error}");
            }
            json!({"github": login})
        }
        None => Value::Null,
    };
    let origin = crate::traces::origin(&app, &headers);
    answer(
        StatusCode::CREATED,
        json!({
            "object": "project.invitation",
            "invitee": invitee,
            "id": made["invitation"]["id"],
            "project": found.project.id,
            "role": made["invitation"]["role"],
            "code": code,
            "url": format!("{origin}{JOIN}/{code}"),
            "expires_at": rfc3339(made["invitation"]["expires_unix"].as_u64().unwrap_or(0)),
        }),
    )
}

/// The code in `{code}`, or a full link to it.
pub(crate) fn code_in(text: &str) -> Option<String> {
    let text = text.trim();
    let code = text.rsplit('/').next().unwrap_or(text);
    let code = code.split(['?', '#']).next().unwrap_or(code);
    (code.starts_with(oa_auth::repos::members::INVITATION_PREFIX) && code.len() <= 200)
        .then(|| code.to_owned())
}

fn code_of(body: Result<axum::Json<Value>, JsonRejection>) -> Result<String, Response> {
    body.ok()
        .and_then(|axum::Json(body)| body["code"].as_str().and_then(code_in))
        .ok_or_else(|| {
            refused(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Send {\"code\": \"…\"} with the invitation link or its code.",
            )
        })
}

/// `POST /v1/projects/invitations/preview` `{code}`.
async fn preview(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<axum::Json<Value>, JsonRejection>,
) -> Response {
    let code = match code_of(body) {
        Ok(code) => code,
        Err(response) => return response,
    };
    let (_, token, _) = match caller(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    match call(
        &app,
        &token,
        Method::POST,
        "/v1/projects/invitations/preview",
        Some(json!({"code": code})),
    )
    .await
    {
        Ok(body) => answer(
            StatusCode::OK,
            json!({
                "object": "project.invitation",
                "project": body["project"],
                "role": body["role"],
                "invited_by": body["invited_by"],
                "expires_at": rfc3339(body["expires_unix"].as_u64().unwrap_or(0)),
                "member": body["member"],
            }),
        ),
        Err(response) => response,
    }
}

/// `POST /v1/projects/invitations/accept` `{code}`.
async fn accept(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<axum::Json<Value>, JsonRejection>,
) -> Response {
    let code = match code_of(body) {
        Ok(code) => code,
        Err(response) => return response,
    };
    let (_, token, account) = match caller(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let body = match call(
        &app,
        &token,
        Method::POST,
        "/v1/projects/invitations/accept",
        Some(json!({"code": code})),
    )
    .await
    {
        Ok(body) => body,
        Err(response) => return response,
    };
    let Ok(project) = serde_json::from_value::<oa_auth::repos::Project>(body["project"].clone())
    else {
        return failed(crate::cloud::session::github::RepoCallError::Session(
            crate::cloud::session::SessionError::Unavailable,
        ));
    };
    let home = super::api::home_of(&project, &account).to_owned();
    // The chats already in the project move in with it (#11586).
    let home_owner = crate::chat_store::account_owner(&home);
    if let Err(error) =
        super::shared_chats::take_in(&app, &project.id, &home_owner, &home_owner).await
    {
        eprintln!("openagents-web: shared project chats: {error}");
    }
    let summary: Summary = super::files::summary(&app, &home, &project.id)
        .await
        .unwrap_or_default();
    answer(StatusCode::OK, object(&project, &[], 0, &summary))
}
