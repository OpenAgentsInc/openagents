//! Account-private owner notes use the existing phone grants and delivery signer.
use super::{
    StateData,
    chat::{Failure, identity},
    push, registry,
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use openagents::owner_notifications::{NotifyOwner, OwnerNote, OwnerNotification};
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashMap, sync::Arc};

type Result<T> = std::result::Result<T, Failure>;
fn bad() -> Failure {
    Failure(StatusCode::BAD_REQUEST, "invalid_request")
}
fn account(state: &StateData, headers: &HeaderMap) -> Result<String> {
    Ok(identity(state, headers, &HashMap::new(), "notify")?.account)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    enabled: bool,
}

pub async fn grant(
    State(state): State<Arc<StateData>>,
    Path(agent): Path<String>,
    headers: HeaderMap,
    Json(grant): Json<Grant>,
) -> Result<Json<serde_json::Value>> {
    let account = account(&state, &headers)?;
    if !openagents::agents::valid_id(&agent) {
        return Err(bad());
    }
    let db = state.database.connect().await?;
    db.execute("INSERT INTO owner_notification_grants(account,agent,enabled) VALUES($1,$2,$3) ON CONFLICT(account,agent) DO UPDATE SET enabled=$3", &[&account, &agent, &grant.enabled]).await?;
    Ok(Json(json!({"ok":true})))
}

/// Save before delivery. The same request resumes unfinished deliveries after a restart.
pub async fn notify(
    State(state): State<Arc<StateData>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match send(&state, &headers, &body).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            let limited = e.0 == StatusCode::TOO_MANY_REQUESTS;
            let mut response = e.into_response();
            if limited {
                response
                    .headers_mut()
                    .insert("retry-after", "3600".parse().unwrap());
            }
            response
        }
    }
}
async fn send(state: &StateData, headers: &HeaderMap, body: &[u8]) -> Result<serde_json::Value> {
    let account = account(state, headers)?;
    if body.len() > 8192 {
        return Err(bad());
    }
    let request: NotifyOwner = serde_json::from_slice(body).map_err(|_| bad())?;
    request.note.validate().map_err(|_| bad())?;
    if !openagents::agents::valid_id(&request.request_id) {
        return Err(bad());
    }
    // Explicit test runs neither store notes nor call a gateway, even if configured.
    if push::is_test_run(headers) {
        return Ok(json!({"ok":true,"test":true}));
    }
    let sender = state
        .push
        .as_ref()
        .ok_or(Failure(StatusCode::SERVICE_UNAVAILABLE, "push_unavailable"))?;
    let at = push_gateway::unix_now() as i64;
    let mut db = state.database.connect().await?;
    let tx = db.transaction().await?;
    // Serialize this account's admissions, including the hourly limit and request IDs.
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,12040))",
        &[&account],
    )
    .await?;
    let permitted = tx.query_opt("SELECT enabled FROM owner_notification_grants WHERE account=$1 AND agent=$2 FOR UPDATE", &[&account, &request.note.agent]).await?.is_some_and(|r| r.get::<_,bool>(0));
    if !permitted {
        return Err(Failure(StatusCode::FORBIDDEN, "notification_not_granted"));
    }
    let note = serde_json::to_value(&request.note).map_err(|_| bad())?;
    let note_text = note.to_string();
    let previous = tx.query_opt("SELECT note::text,created_at FROM owner_notifications WHERE account=$1 AND request_id=$2", &[&account, &request.request_id]).await?;
    let created_at = if let Some(row) = previous {
        if serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(0))
            .ok()
            .as_ref()
            != Some(&note)
        {
            return Err(Failure(StatusCode::CONFLICT, "request_changed"));
        }
        row.get::<_, i64>(1)
    } else {
        let count: i64 = tx.query_one("SELECT count(*) FROM owner_notifications WHERE account=$1 AND agent=$2 AND created_at>$3", &[&account, &request.note.agent, &(at-3600)]).await?.get(0);
        if count >= 6 {
            return Err(Failure(StatusCode::TOO_MANY_REQUESTS, "rate_limited"));
        }
        let chat = tx.query_opt("SELECT 1 FROM registry_rows WHERE account=$1 AND kind='chats' AND id=$2 AND NOT deleted", &[&account, &request.note.chat]).await?;
        if chat.is_none() {
            return Err(Failure(StatusCode::NOT_FOUND, "chat_not_found"));
        }
        tx.execute("INSERT INTO owner_notifications(account,request_id,agent,note,created_at) VALUES($1,$2,$3,$4::text::jsonb,$5)", &[&account, &request.request_id, &request.note.agent, &note_text, &at]).await?;
        at
    };
    tx.execute(
        "DELETE FROM owner_notifications WHERE account=$1 AND created_at<$2",
        &[&account, &(at - 7 * 86400)],
    )
    .await?;
    tx.commit().await?;
    // Old replays remain idempotent without issuing a newly expiring wake.
    if at - created_at >= push::WAKE_SECONDS as i64 {
        return Ok(json!({"ok":true}));
    }
    let targets = registry::grants(state, &account).await?;
    // Publish the phone's note before the platform can wake it.
    for target in &targets {
        if target.expires_at > at as u64 && target.prefs.input {
            db.execute("INSERT INTO owner_notification_deliveries(account,request_id,device) VALUES($1,$2,$3) ON CONFLICT DO NOTHING", &[&account, &request.request_id, &target.device]).await?;
        }
    }
    // Concurrent retries share one delivery transaction. Only one caller sends
    // each phone's wake; a restart releases this lock and keeps the receipt.
    let deliveries = db.transaction().await?;
    let delivery_key = format!("{account}/{}", request.request_id);
    deliveries
        .query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,12041))",
            &[&delivery_key],
        )
        .await?;
    let mut failed = false;
    for target in targets {
        if target.expires_at <= at as u64 || !target.prefs.input {
            continue;
        }
        let delivered: bool = deliveries.query_one("SELECT delivered FROM owner_notification_deliveries WHERE account=$1 AND request_id=$2 AND device=$3", &[&account, &request.request_id, &target.device]).await?.get(0);
        if delivered {
            continue;
        }
        let marker = push::Note {
            chat_id: request.note.chat.clone(),
            category: super::notifications::Category::Input,
            marker: format!("attention:{}:{}", request.note.agent, request.request_id),
        };
        let wake = push::Wake {
            device: target.device.clone(),
            transport: target.transport,
            grant: target.grant.clone(),
            request_id: push::request_id(&account, &target.device, &marker),
            chat_id: request.note.chat.clone(),
            category: marker.category,
        };
        match sender
            .send_attention(
                &account,
                &wake,
                target
                    .expires_at
                    .min(created_at as u64 + push::WAKE_SECONDS),
            )
            .await
        {
            push::Outcome::Failed(_) => {
                failed = true;
                continue;
            }
            push::Outcome::Gone(_) => {
                deliveries.execute("DELETE FROM registry_push_grants WHERE account=$1 AND device=$2 AND endpoint_grant=$3", &[&account,&target.device,&target.grant]).await?;
            }
            push::Outcome::Accepted => {}
        }
        deliveries.execute("UPDATE owner_notification_deliveries SET delivered=TRUE WHERE account=$1 AND request_id=$2 AND device=$3", &[&account, &request.request_id, &target.device]).await?;
    }
    deliveries.commit().await?;
    if failed {
        return Err(Failure(StatusCode::SERVICE_UNAVAILABLE, "delivery_failed"));
    }
    Ok(json!({"ok":true}))
}

/// Read the newest unread note for this phone. Taps acknowledge only the fetched note.
pub async fn latest(
    State(state): State<Arc<StateData>>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>> {
    let account = account(&state, &headers)?;
    let device = q
        .get("device")
        .filter(|d| !d.is_empty() && d.len() <= 128)
        .ok_or_else(bad)?;
    let db = state.database.connect().await?;
    let row = db.query_opt("SELECT n.request_id,n.note::text FROM owner_notifications n JOIN owner_notification_deliveries d USING(account,request_id) WHERE n.account=$1 AND d.device=$2 AND NOT d.read AND n.created_at>$3 ORDER BY n.sequence DESC LIMIT 1", &[&account, device, &(push_gateway::unix_now() as i64-86400)]).await?;
    let notification = row
        .map(|r| -> Result<OwnerNotification> {
            let note: OwnerNote =
                serde_json::from_str(&r.get::<_, String>(1)).map_err(|_| bad())?;
            Ok(OwnerNotification {
                request_id: r.get(0),
                deep_link: note.deep_link(),
                note,
            })
        })
        .transpose()?;
    Ok(Json(json!({"notification":notification})))
}

pub async fn read(
    State(state): State<Arc<StateData>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>> {
    let account = account(&state, &headers)?;
    if !openagents::agents::valid_id(&id) {
        return Err(bad());
    }
    let device = q
        .get("device")
        .filter(|d| !d.is_empty() && d.len() <= 128)
        .ok_or_else(bad)?;
    let db = state.database.connect().await?;
    db.execute("UPDATE owner_notification_deliveries SET read=TRUE WHERE account=$1 AND request_id=$2 AND device=$3", &[&account, &id, device]).await?;
    Ok(Json(json!({"ok":true})))
}
