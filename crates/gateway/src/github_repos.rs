//! Connected GitHub repositories and projects on the account surface
//! (docs/auth/github.md, "Repository access"). The session-bearer routes
//! answer through [`oa_auth::repos::answer`], the same code the local
//! fixture's account service runs; the token stays encrypted in
//! `github-access/` beside `accounts.json` and is never logged.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, delete, get, post};
use oa_auth::repos::Call;
use serde_json::Value;

use crate::accounts::{answered, member_account, principal, refused};
use crate::serve::ServeState;

pub(crate) fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/v1/account/github", get(status)),
        ("/v1/account/github/grant", post(grant).delete(disconnect)),
        ("/v1/account/github/repositories", get(repositories)),
        ("/v1/account/github/token", post(token)),
        ("/v1/account/projects", post(add)),
        ("/v1/account/projects/{id}", delete(remove)),
        ("/v1/account/github/app/grant", post(app_grant)),
        ("/v1/account/github/app/refresh", post(app_refresh)),
        ("/v1/account/github/broker", post(broker_ticket)),
        (oa_auth::repos::broker::PATH, post(git_credential)),
    ]
}

/// The configured GitHub App, read from its private files on use; its
/// installation tokens stay in the process's shared cache. `None` when
/// this service has no GitHub App or it can't be read.
fn app(state: &ServeState) -> Option<oa_auth::AppClient> {
    let config = state.config.accounts.as_ref()?.github_app.as_ref()?;
    oa_auth::AppCredentials::load(
        &config.credentials,
        &config.redirect_url,
        oa_auth::Endpoints::default(),
    )
    .and_then(oa_auth::AppClient::new)
    .ok()
}

/// The configured GitHub OAuth client, read from its private file on use;
/// `None` when this service has no GitHub App or it can't be read.
fn github(state: &ServeState) -> Option<oa_auth::Github> {
    let config = state.config.accounts.as_ref()?.github.as_ref()?;
    oa_auth::GithubCredentials::load(
        &config.credentials,
        &config.redirect_url,
        oa_auth::Endpoints::default(),
    )
    .and_then(oa_auth::Github::new)
    .ok()
}

async fn run(state: &ServeState, headers: &HeaderMap, call: Call) -> Response {
    let principal = match principal(state, headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let account = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let github = github(state);
    let app = app(state);
    let (status, body) =
        oa_auth::repos::answer_with(&state.dir, github.as_ref(), app.as_ref(), &account, call)
            .await;
    answered(
        StatusCode::from_u16(status).unwrap_or(StatusCode::SERVICE_UNAVAILABLE),
        body,
    )
}

fn invalid() -> Response {
    refused(
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "That didn't work. Try again.",
    )
}

async fn status(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    run(&state, &headers, Call::Status).await
}

async fn grant(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    match serde_json::from_value::<oa_auth::service::CodeRequest>(body) {
        Ok(request) => run(&state, &headers, Call::Grant(request)).await,
        Err(_) => invalid(),
    }
}

async fn disconnect(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    run(&state, &headers, Call::Disconnect).await
}

#[derive(serde::Deserialize)]
struct RepositoryPage {
    page: Option<u32>,
    /// The previous answer's `next` (the shared cursor, #11156).
    after: Option<String>,
}

impl RepositoryPage {
    fn page(&self) -> u32 {
        self.after
            .as_deref()
            .and_then(|after| after.parse().ok())
            .or(self.page)
            .unwrap_or(1)
    }
}

async fn repositories(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<RepositoryPage>,
) -> Response {
    run(&state, &headers, Call::Repositories(query.page())).await
}

async fn token(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    run(&state, &headers, Call::Token).await
}

async fn add(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    match body["repository"].as_str() {
        Some(repository) => run(&state, &headers, Call::AddProject(repository.into())).await,
        None => invalid(),
    }
}

async fn remove(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    run(&state, &headers, Call::RemoveProject(id)).await
}

async fn app_grant(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    match serde_json::from_value::<oa_auth::service::CodeRequest>(body) {
        Ok(request) => run(&state, &headers, Call::AppGrant(request)).await,
        Err(_) => invalid(),
    }
}

async fn app_refresh(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    run(&state, &headers, Call::AppRefresh).await
}

async fn broker_ticket(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    match body["repository"].as_str() {
        Some(repository) => {
            run(
                &state,
                &headers,
                Call::BrokerTicket {
                    repository: repository.into(),
                    seconds: body["seconds"].as_u64().unwrap_or(0),
                },
            )
            .await
        }
        None => invalid(),
    }
}

/// The credential broker: no session; the ticket is in the form body.
/// The answer is Git's credential format, never logged.
async fn git_credential(State(state): State<Arc<ServeState>>, body: axum::body::Bytes) -> Response {
    let app = app(&state);
    let (status, text) = oa_auth::repos::broker::credential(&state.dir, app.as_ref(), &body).await;
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::SERVICE_UNAVAILABLE),
        [
            (
                axum::http::header::CONTENT_TYPE,
                "text/plain; charset=utf-8",
            ),
            (axum::http::header::CACHE_CONTROL, "no-store"),
        ],
        text,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_routes_match_the_local_account_service() {
        let paths: Vec<&str> = super::routes().into_iter().map(|(path, _)| path).collect();
        assert_eq!(
            paths,
            [
                "/v1/account/github",
                "/v1/account/github/grant",
                "/v1/account/github/repositories",
                "/v1/account/github/token",
                "/v1/account/projects",
                "/v1/account/projects/{id}",
                "/v1/account/github/app/grant",
                "/v1/account/github/app/refresh",
                "/v1/account/github/broker",
                "/v1/github/git-credential",
            ]
        );
    }
}
