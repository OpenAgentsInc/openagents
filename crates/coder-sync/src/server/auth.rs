//! OpenAgents app sessions are checked against the account service on every request.
use super::{Identity, StateData, config::AuthMode};
use axum::{
    Json,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use openagents::{Client, Deployment, Error};
use std::{collections::HashMap, sync::Arc, time::Duration};
pub const ACCOUNT_HEADER: &str = "x-coder-verified-account";
#[derive(Clone)]
pub struct Accounts(Client);
impl Accounts {
    pub fn new(deployment: Deployment) -> Result<Self, Error> {
        Client::with_timeout(deployment, Duration::from_secs(5)).map(Self)
    }
    pub async fn verify(&self, token: &str) -> Result<Identity, StatusCode> {
        let session = self
            .0
            .authenticate(token)
            .await
            .map_err(|error| match error {
                Error::InvalidCredential
                | Error::Unauthorized
                | Error::Forbidden
                | Error::ExpiredSession
                | Error::WrongAccount => StatusCode::UNAUTHORIZED,
                _ => StatusCode::SERVICE_UNAVAILABLE,
            })?;
        let id = &session.account().id;
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        Ok(Identity {
            account: id.clone(),
            workspace: None,
        })
    }
}
pub fn bearer(headers: &HeaderMap, query: &HashMap<String, String>) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, token)| token.trim().to_owned())
        .or_else(|| query.get("token").cloned())
}
pub async fn guard(
    State(state): State<Arc<StateData>>,
    mut request: Request,
    next: Next,
) -> Response {
    request.headers_mut().remove(ACCOUNT_HEADER);
    if request.uri().path() == "/health" || state.auth != AuthMode::OpenAgents {
        return next.run(request).await;
    }
    let query: HashMap<String, String> = request
        .uri()
        .query()
        .and_then(|q| reqwest::Url::parse(&format!("http://localhost/?{q}")).ok())
        .map(|url| {
            url.query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect()
        })
        .unwrap_or_default();
    let Some(token) = bearer(request.headers(), &query).filter(|v| v.len() <= 4096) else {
        return denied(StatusCode::UNAUTHORIZED);
    };
    let Some(accounts) = &state.accounts else {
        return denied(StatusCode::SERVICE_UNAVAILABLE);
    };
    match accounts.verify(&token).await {
        Ok(who) => {
            let Ok(value) = HeaderValue::from_str(&who.account) else {
                return denied(StatusCode::SERVICE_UNAVAILABLE);
            };
            request.headers_mut().insert(ACCOUNT_HEADER, value);
            next.run(request).await
        }
        Err(status) => denied(status),
    }
}
fn denied(status: StatusCode) -> Response {
    (status,Json(serde_json::json!({"error":if status==StatusCode::UNAUTHORIZED{"unauthenticated"}else{"unavailable"}}))).into_response()
}
#[derive(Clone)]
pub struct SocketLease {
    accounts: Accounts,
    token: String,
    account: String,
}
pub fn socket_lease(
    state: &StateData,
    headers: &HeaderMap,
    query: &HashMap<String, String>,
    who: &Identity,
) -> Option<SocketLease> {
    if state.auth != AuthMode::OpenAgents {
        return None;
    }
    Some(SocketLease {
        accounts: state.accounts.clone()?,
        token: bearer(headers, query)?,
        account: who.account.clone(),
    })
}
/// Close revoked, expired, or unverifiable live sockets; callers reconnect after an outage.
pub async fn socket_ended(lease: Option<SocketLease>) -> u16 {
    let Some(lease) = lease else {
        return std::future::pending().await;
    };
    loop {
        tokio::time::sleep(Duration::from_secs(15)).await;
        match lease.accounts.verify(&lease.token).await {
            Ok(who) if who.account == lease.account => {}
            Ok(_) | Err(StatusCode::UNAUTHORIZED) => return 4401,
            Err(_) => return 1013,
        }
    }
}
