//! The local account service's repository and project routes, over
//! [`crate::repos::answer`] like the gateway's.

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use serde_json::{Value, json};

use super::{LocalService, caller, refused};
use crate::repos::{self, Call};
use crate::service::CodeRequest;

pub(super) fn router(service: LocalService) -> Router {
    Router::new()
        .route("/v1/account/github", get(status))
        .route("/v1/account/github/grant", post(grant).delete(disconnect))
        .route("/v1/account/github/repositories", get(repositories))
        .route("/v1/account/github/token", post(token))
        .route("/v1/account/projects", post(add))
        .route("/v1/account/projects/{id}", delete(remove))
        .with_state(service)
}

async fn run(state: &LocalService, headers: &HeaderMap, call: Call) -> Response {
    let (_, account) = match caller(state, headers) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let (status, mut body) =
        repos::answer(&state.0.dir, Some(&state.0.github), &account, call).await;
    body["v"] = json!("openagents.accounts.v1");
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::SERVICE_UNAVAILABLE),
        axum::Json(body),
    )
        .into_response()
}

fn invalid() -> Response {
    refused(
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "That didn't work. Try again.",
    )
}

async fn status(State(state): State<LocalService>, headers: HeaderMap) -> Response {
    run(&state, &headers, Call::Status).await
}

async fn grant(
    State(state): State<LocalService>,
    headers: HeaderMap,
    request: Result<axum::Json<CodeRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    match request {
        Ok(axum::Json(request)) => run(&state, &headers, Call::Grant(request)).await,
        Err(_) => invalid(),
    }
}

async fn disconnect(State(state): State<LocalService>, headers: HeaderMap) -> Response {
    run(&state, &headers, Call::Disconnect).await
}

async fn repositories(State(state): State<LocalService>, headers: HeaderMap) -> Response {
    run(&state, &headers, Call::Repositories).await
}

async fn token(State(state): State<LocalService>, headers: HeaderMap) -> Response {
    run(&state, &headers, Call::Token).await
}

async fn add(
    State(state): State<LocalService>,
    headers: HeaderMap,
    body: Result<axum::Json<Value>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Some(repository) = body
        .ok()
        .and_then(|axum::Json(body)| body["repository"].as_str().map(str::to_string))
    else {
        return invalid();
    };
    run(&state, &headers, Call::AddProject(repository)).await
}

async fn remove(
    State(state): State<LocalService>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
) -> Response {
    run(&state, &headers, Call::RemoveProject(id)).await
}
