//! Authenticated introduction management over canonical account custody.
use crate::{accounts, serve::ServeState};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{MethodRouter, get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use tenancy::accounts::referrals::{self, Capture, Error, Kind};

pub(crate) fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/join", get(join)),
        ("/v1/account/acquisition", get(source).post(capture)),
        ("/v1/account/referrers", post(create)),
        ("/v1/account/referrers/{referrer}", get(show)),
        (
            "/v1/account/referrers/{referrer}/link",
            post(link).delete(disable),
        ),
        ("/v1/account/referrers/{referrer}/migration", post(migrate)),
        (
            "/v1/account/referrers/{referrer}/migration/accept",
            post(accept),
        ),
    ]
}
pub(crate) fn refused(error: Error) -> Response {
    let status = match error {
        Error::Unauthorized => StatusCode::FORBIDDEN,
        Error::Invalid => StatusCode::BAD_REQUEST,
        Error::Conflict => StatusCode::CONFLICT,
        Error::Bound => StatusCode::TOO_MANY_REQUESTS,
        Error::Unavailable => StatusCode::NOT_FOUND,
        Error::Store(_) => StatusCode::SERVICE_UNAVAILABLE,
    };
    accounts::refused(status, error.code(), "The referral operation was refused.")
}
fn answer(value: impl serde::Serialize) -> Response {
    (
        StatusCode::OK,
        Json(json!({"v":"openagents.accounts.v1", "referral": value})),
    )
        .into_response()
}
use axum::response::IntoResponse;
async fn actor(
    state: &ServeState,
    headers: &HeaderMap,
) -> Result<(tenancy::Accounts, String), Response> {
    let principal = accounts::principal(state, headers)?;
    let account = accounts::member_account(&principal)?.to_owned();
    if let Some(expected) = headers.get("x-openagents-referral-account") {
        if expected.to_str().ok() != Some(account.as_str()) {
            return Err(refused(Error::Conflict));
        }
    }
    Ok((accounts::accounts_store(state)?, account))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Join {
    #[serde(rename = "ref")]
    token: String,
}
/// Public source lookup material grants no account access or payout right.
async fn join(Query(input): Query<Join>) -> Response {
    if !input.token.strip_prefix("rfr_").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        return refused(Error::Invalid);
    }
    answer(
        json!({"token":input.token,"consent_version":referrals::CONSENT,"consent_text":"Allow this introduction source to be recorded privately with your account. This records an introduction and grants no commission or payment right.","signup":"/v1/accounts","capture":"/v1/account/acquisition"}),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    kind: Kind,
    label: String,
}
async fn create(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let input: Create = match serde_json::from_value(body) {
        Ok(v) => v,
        Err(_) => return refused(Error::Invalid),
    };
    match store.create_referrer(&account, input.kind, &input.label) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
async fn show(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.referrer(&account, &id) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
async fn link(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.issue_referral_link(&account, &id) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
async fn disable(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.disable_referral_links(&account, &id) {
        Ok(()) => answer(json!({"disabled":true})),
        Err(e) => refused(e),
    }
}
async fn source(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.acquisition(&account) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
async fn capture(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let input: Capture = match serde_json::from_value(body) {
        Ok(v) => v,
        Err(_) => return refused(Error::Invalid),
    };
    match store.capture_acquisition(&account, &input) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Migration {
    account: String,
}
async fn migrate(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let input: Migration = match serde_json::from_value(body) {
        Ok(v) => v,
        Err(_) => return refused(Error::Invalid),
    };
    match store.offer_referrer_migration(&account, &id, &input.account) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
async fn accept(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (store, account) = match actor(&state, &headers).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match store.accept_referrer_migration(&account, &id) {
        Ok(v) => answer(v),
        Err(e) => refused(e),
    }
}
