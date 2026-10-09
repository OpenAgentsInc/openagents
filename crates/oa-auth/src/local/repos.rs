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
        .route("/v1/account/github/app/grant", post(app_grant))
        .route("/v1/account/github/app/refresh", post(app_refresh))
        .route("/v1/account/github/broker", post(broker_ticket))
        .route(repos::broker::PATH, post(git_credential))
        .with_state(service)
}

async fn run(state: &LocalService, headers: &HeaderMap, call: Call) -> Response {
    let (_, account) = match caller(state, headers) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let (status, mut body) = repos::answer_with(
        &state.0.dir,
        Some(&state.0.github),
        state.0.app.as_ref(),
        &account,
        call,
    )
    .await;
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

#[derive(serde::Deserialize)]
struct RepositoryPage {
    page: Option<u32>,
}

async fn repositories(
    State(state): State<LocalService>,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<RepositoryPage>,
) -> Response {
    run(
        &state,
        &headers,
        Call::Repositories(query.page.unwrap_or(1)),
    )
    .await
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

async fn app_grant(
    State(state): State<LocalService>,
    headers: HeaderMap,
    request: Result<axum::Json<CodeRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    match request {
        Ok(axum::Json(request)) => run(&state, &headers, Call::AppGrant(request)).await,
        Err(_) => invalid(),
    }
}

async fn app_refresh(State(state): State<LocalService>, headers: HeaderMap) -> Response {
    run(&state, &headers, Call::AppRefresh).await
}

async fn broker_ticket(
    State(state): State<LocalService>,
    headers: HeaderMap,
    body: Result<axum::Json<Value>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Some((repository, seconds)) = body.ok().and_then(|axum::Json(body)| {
        Some((
            body["repository"].as_str()?.to_string(),
            body["seconds"].as_u64().unwrap_or(0),
        ))
    }) else {
        return invalid();
    };
    run(
        &state,
        &headers,
        Call::BrokerTicket {
            repository,
            seconds,
        },
    )
    .await
}

/// The credential broker: no session, the ticket in the form body.
async fn git_credential(State(state): State<LocalService>, body: axum::body::Bytes) -> Response {
    let (status, text) = repos::broker::credential(&state.0.dir, state.0.app.as_ref(), &body).await;
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
