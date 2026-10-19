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
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
pub const ACCOUNT_HEADER: &str = "x-coder-verified-account";
/// How long a confirmed token keeps working while the account service is down.
pub const OUTAGE_GRACE: Duration = Duration::from_secs(5 * 60);
const CONFIRMED_LIMIT: usize = 10_000;
static CACHED_ADMISSIONS: AtomicU64 = AtomicU64::new(0);
/// Admissions served from the confirmed-token cache since the process started.
pub fn cached_admissions() -> u64 {
    CACHED_ADMISSIONS.load(Ordering::Relaxed)
}
#[derive(Clone)]
pub struct Accounts {
    client: Client,
    grace: Duration,
    /// Token digest to the last successful confirmation; never the token.
    confirmed: Arc<Mutex<HashMap<[u8; 32], (Instant, Identity)>>>,
}
impl Accounts {
    pub fn new(deployment: Deployment) -> Result<Self, Error> {
        Self::with_grace(deployment, OUTAGE_GRACE)
    }
    pub fn with_grace(deployment: Deployment, grace: Duration) -> Result<Self, Error> {
        Ok(Self {
            client: Client::with_timeout(deployment, Duration::from_secs(5))?,
            grace,
            confirmed: Default::default(),
        })
    }
    pub async fn verify(&self, token: &str) -> Result<Identity, StatusCode> {
        let key: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        match self.confirm(token).await {
            Ok(who) => {
                let mut cache = self.confirmed.lock().unwrap();
                let now = Instant::now();
                if cache.len() >= CONFIRMED_LIMIT && !cache.contains_key(&key) {
                    cache.retain(|_, (at, _)| now.duration_since(*at) < self.grace);
                    if cache.len() >= CONFIRMED_LIMIT {
                        cache.clear();
                    }
                }
                cache.insert(key, (now, who.clone()));
                Ok(who)
            }
            Err(StatusCode::UNAUTHORIZED) => {
                self.confirmed.lock().unwrap().remove(&key);
                Err(StatusCode::UNAUTHORIZED)
            }
            Err(status) => {
                let mut cache = self.confirmed.lock().unwrap();
                match cache.get(&key) {
                    Some((at, who)) if at.elapsed() < self.grace => {
                        CACHED_ADMISSIONS.fetch_add(1, Ordering::Relaxed);
                        tracing::debug!(account=%who.account, "account service unreachable; admitted a recently confirmed token");
                        Ok(who.clone())
                    }
                    Some(_) => {
                        cache.remove(&key);
                        Err(status)
                    }
                    None => Err(status),
                }
            }
        }
    }
    async fn confirm(&self, token: &str) -> Result<Identity, StatusCode> {
        let session = self
            .client
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
    request.headers_mut().remove(super::proof::KEY_HEADER);
    request.headers_mut().remove(super::proof::EXPIRY_HEADER);
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
        return refuse(request, StatusCode::UNAUTHORIZED).await;
    };
    let Some(accounts) = &state.accounts else {
        return refuse(request, StatusCode::SERVICE_UNAVAILABLE).await;
    };
    match accounts.verify(&token).await {
        Ok(who) => {
            let Ok(value) = HeaderValue::from_str(&who.account) else {
                return refuse(request, StatusCode::SERVICE_UNAVAILABLE).await;
            };
            request.headers_mut().insert(ACCOUNT_HEADER, value);
            match super::proof::admit(&state, &who, request).await {
                Ok(request) => next.run(request).await,
                Err(status) => denied(status),
            }
        }
        Err(status) => refuse(request, status).await,
    }
}
/// Refuse after reading the upload, so the connection stays reusable.
async fn refuse(request: Request, status: StatusCode) -> Response {
    super::drain(request.into_body()).await;
    denied(status)
}
fn denied(status: StatusCode) -> Response {
    (status,Json(serde_json::json!({"error":if status==StatusCode::UNAUTHORIZED{"unauthenticated"}else{"unavailable"}}))).into_response()
}
#[derive(Clone)]
pub struct SocketLease {
    accounts: Accounts,
    token: String,
    account: String,
    proof_expiry: Option<u64>,
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
        proof_expiry: headers
            .get(super::proof::EXPIRY_HEADER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok()),
    })
}
/// Close revoked, expired, or unverifiable live sockets; callers reconnect after an outage.
pub async fn socket_ended(lease: Option<SocketLease>) -> u16 {
    let Some(lease) = lease else {
        return std::future::pending().await;
    };
    let proof_expiry = lease.proof_expiry;
    let expires = async move {
        match proof_expiry {
            Some(expiry) => {
                tokio::time::sleep(Duration::from_secs(
                    expiry.saturating_sub(coder_access::unix_time().unwrap_or(u64::MAX)),
                ))
                .await
            }
            None => std::future::pending::<()>().await,
        }
    };
    let account_checks = async move {
        loop {
            tokio::time::sleep(Duration::from_secs(15)).await;
            match lease.accounts.verify(&lease.token).await {
                Ok(who) if who.account == lease.account => {}
                Ok(_) | Err(StatusCode::UNAUTHORIZED) => return 4401,
                Err(_) => return 1013,
            }
        }
    };
    // A slow account check must not extend a proved socket's admission.
    tokio::select! {
        code = account_checks => code,
        _ = expires => 4401,
    }
}
