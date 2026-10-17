//! Optional NIP-98 app proofs supplement the account session.
use super::{Identity, StateData, auth::ACCOUNT_HEADER};
use axum::{
    Json,
    body::{Body, to_bytes},
    extract::{Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use coder_access::http_proof::{self, DEVICE_HEADER, HOST_HEADER, Intent};
use serde::Deserialize;
use std::sync::Arc;

pub const KEY_HEADER: &str = "x-coder-verified-device-key";
pub const EXPIRY_HEADER: &str = "x-coder-device-proof-expiry";
type Result<T> = std::result::Result<T, StatusCode>;
fn unauthorized<T>(_: T) -> StatusCode {
    StatusCode::UNAUTHORIZED
}
fn unavailable<T>(_: T) -> StatusCode {
    StatusCode::SERVICE_UNAVAILABLE
}
fn header<'a>(headers: &'a HeaderMap, name: &str) -> Result<&'a str> {
    if headers.get_all(name).iter().count() != 1 {
        return Err(StatusCode::UNAUTHORIZED);
    }
    headers
        .get(name)
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)
}
fn now() -> u64 {
    coder_access::unix_time().unwrap_or(0)
}

pub async fn admit(state: &StateData, who: &Identity, mut request: Request) -> Result<Request> {
    request.headers_mut().remove(KEY_HEADER);
    request.headers_mut().remove(EXPIRY_HEADER);
    if !request.headers().contains_key(DEVICE_HEADER)
        && !request.headers().contains_key(HOST_HEADER)
    {
        return Ok(request);
    }
    let origin = state
        .proof_origin
        .as_deref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let url = format!("{}{}", origin.trim_end_matches('/'), request.uri());
    let (mut parts, body) = request.into_parts();
    let body = to_bytes(body, 32 * 1024 * 1024)
        .await
        .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;
    let intent = Intent::new(&url, parts.method.as_str(), &body).map_err(unauthorized)?;
    let at = now();
    let device = http_proof::verify(header(&parts.headers, DEVICE_HEADER)?, &intent, at)
        .map_err(unauthorized)?;
    if http_proof::tag(&device, "account").map_err(unauthorized)? != who.account {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut db = state.database.connect().await.map_err(unavailable)?;
    let tx = db.transaction().await.map_err(unavailable)?;
    let registration =
        parts.uri.path().starts_with("/device/") && parts.uri.path().ends_with("/key");
    let mut events = vec![device.id.clone()];
    if registration {
        parts.headers.insert(
            KEY_HEADER,
            HeaderValue::from_str(&device.pubkey).map_err(unauthorized)?,
        );
    } else {
        let host = http_proof::verify(header(&parts.headers, HOST_HEADER)?, &intent, at)
            .map_err(unauthorized)?;
        let host_id = http_proof::tag(&host, "hostId").map_err(unauthorized)?;
        let expiry = http_proof::tag(&host, "expiration")
            .map_err(unauthorized)?
            .parse::<u64>()
            .map_err(unauthorized)?;
        if http_proof::tag(&host, "account").map_err(unauthorized)? != who.account
            || http_proof::tag(&host, "p").map_err(unauthorized)? != device.pubkey
            || http_proof::tag(&device, "host").map_err(unauthorized)? != host.id
            || expiry <= at
            || expiry > host.created_at.saturating_add(15)
            || host.created_at > at.saturating_add(2)
        {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let admitted = tx.query_opt("SELECT 1 FROM device_host_keys WHERE account=$1 AND device_id=$2 AND public_key=$3 AND NOT revoked", &[&who.account, &host_id, &host.pubkey]).await.map_err(unavailable)?.is_some();
        if !admitted {
            return Err(StatusCode::UNAUTHORIZED);
        }
        // A host can admit only its own transcript rooms and device endpoints.
        let segments: Vec<_> = parts.uri.path().split('/').collect();
        match segments.as_slice() {
            ["", "chat2", chat, ..] => {
                let owner = tx.query_opt("SELECT value->'fields'->>'deviceId' FROM registry_rows WHERE account=$1 AND kind='chats' AND id=$2 AND NOT deleted", &[&who.account, chat]).await.map_err(unavailable)?;
                if owner.and_then(|r| r.get::<_, Option<String>>(0)).as_deref() != Some(host_id) {
                    return Err(StatusCode::UNAUTHORIZED);
                }
            }
            ["", "device", id, ..] if *id == host_id => {}
            _ => return Err(StatusCode::UNAUTHORIZED),
        }
        parts.headers.insert(
            EXPIRY_HEADER,
            HeaderValue::from_str(&expiry.to_string()).map_err(unauthorized)?,
        );
        events.push(host.id);
    }
    tx.execute(
        "DELETE FROM device_proof_events WHERE expires_at < $1",
        &[&(at as i64)],
    )
    .await
    .map_err(unavailable)?;
    for event in events {
        let inserted = tx.execute("INSERT INTO device_proof_events(event_id,expires_at) VALUES($1,$2) ON CONFLICT DO NOTHING", &[&event, &((at + 120) as i64)]).await.map_err(unavailable)?;
        if inserted != 1 {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    tx.commit().await.map_err(unavailable)?;
    Ok(Request::from_parts(parts, Body::from(body)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Registration {
    public_key: String,
}
pub async fn register(
    State(state): State<Arc<StateData>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(value): Json<Registration>,
) -> Response {
    match register_inner(&state, &id, &headers, &value.public_key).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(status) => status.into_response(),
    }
}
async fn register_inner(
    state: &StateData,
    id: &str,
    headers: &HeaderMap,
    public: &str,
) -> Result<()> {
    let account = header(headers, ACCOUNT_HEADER)?;
    if header(headers, KEY_HEADER)? != public || id.is_empty() || id.len() > 256 {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut db = state.database.connect().await.map_err(unavailable)?;
    let tx = db.transaction().await.map_err(unavailable)?;
    tx.execute(
        "INSERT INTO devices(device_id,account) VALUES($1,$2) ON CONFLICT DO NOTHING",
        &[&id, &account],
    )
    .await
    .map_err(unavailable)?;
    let owner: String = tx
        .query_one(
            "SELECT account FROM devices WHERE device_id=$1 FOR UPDATE",
            &[&id],
        )
        .await
        .map_err(unavailable)?
        .get(0);
    if owner != account {
        return Err(StatusCode::UNAUTHORIZED);
    }
    tx.execute("INSERT INTO device_host_keys(device_id,account,public_key) VALUES($1,$2,$3) ON CONFLICT DO NOTHING", &[&id, &account, &public]).await.map_err(unavailable)?;
    let row = tx
        .query_one(
            "SELECT public_key,revoked FROM device_host_keys WHERE device_id=$1",
            &[&id],
        )
        .await
        .map_err(unavailable)?;
    if row.get::<_, String>(0) != public || row.get::<_, bool>(1) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    tx.commit().await.map_err(unavailable)
}
pub async fn revoke(
    State(state): State<Arc<StateData>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let result = async {
        let account = header(&headers, ACCOUNT_HEADER)?;
        let key = header(&headers, KEY_HEADER)?;
        let db = state.database.connect().await.map_err(unavailable)?;
        let count = db.execute("UPDATE device_host_keys SET revoked=true WHERE device_id=$1 AND account=$2 AND public_key=$3", &[&id, &account, &key]).await.map_err(unavailable)?;
        if count == 1 { Ok(()) } else { Err(StatusCode::UNAUTHORIZED) }
    }.await;
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(status) => status.into_response(),
    }
}
