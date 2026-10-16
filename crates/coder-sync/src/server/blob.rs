//! Account-scoped tool blobs, published only after their immutable object exists.
use super::{
    StateData,
    chat::{Failure, identity},
    rooms::Room,
};
use axum::{
    Json,
    body::{Bytes, to_bytes},
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc};
use tokio_postgres::Transaction;
const LIMIT: usize = 1024 * 1024;
type Result<T> = std::result::Result<T, Failure>;
fn unavailable<T>(_: T) -> Failure {
    Failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable")
}
fn valid(part: &str) -> bool {
    !part.is_empty()
        && part.len() <= 200
        && part
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:#~-".contains(&b))
}
async fn owner(tx: &Transaction<'_>, chat: &str, account: &str, create: bool) -> Result<()> {
    if create {
        tx.execute(
            "INSERT INTO chats(chat_id,account) VALUES($1,$2) ON CONFLICT DO NOTHING",
            &[&chat, &account],
        )
        .await?;
    }
    let row = tx
        .query_opt(
            "SELECT account FROM chats WHERE chat_id=$1 FOR UPDATE",
            &[&chat],
        )
        .await?
        .ok_or(Failure(StatusCode::NOT_FOUND, "not_found"))?;
    if row.get::<_, String>(0) != account {
        return Err(Failure(StatusCode::FORBIDDEN, "forbidden"));
    }
    Ok(())
}
pub async fn put(
    State(state): State<Arc<StateData>>,
    Path((chat, part)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
    request: Request,
) -> Result<Json<Value>> {
    let who = identity(&state, request.headers(), &q, &chat)?;
    if !valid(&part) {
        return Err(Failure(StatusCode::BAD_REQUEST, "bad_part_id"));
    }
    let content_type = request
        .headers()
        .get("content-type")
        .map(|v| v.to_str().map(str::to_owned))
        .transpose()
        .map_err(|_| Failure(StatusCode::BAD_REQUEST, "bad_content_type"))?
        .unwrap_or_else(|| "text/plain; charset=utf-8".into());
    let body = to_bytes(request.into_body(), LIMIT)
        .await
        .map_err(|_| Failure(StatusCode::PAYLOAD_TOO_LARGE, "too_large"))?;
    state
        .rooms
        .run(
            Room::Chat(chat.clone()),
            publish(state.clone(), chat, part, who.account, body, content_type),
        )
        .await
        .map_err(|_| Failure(StatusCode::SERVICE_UNAVAILABLE, "busy"))?
}
async fn publish(
    state: Arc<StateData>,
    chat: String,
    part: String,
    account: String,
    body: Bytes,
    content_type: String,
) -> Result<Json<Value>> {
    let mut db = state.database.connect().await?;
    let tx = db.transaction().await?;
    owner(&tx, &chat, &account, true).await?;
    let prefix = account
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let version = uuid::Uuid::new_v4();
    let object = format!("blob/{prefix}/{version}");
    let staged = format!("orphan/{prefix}/{version}");
    // A committed intent covers a crash after copying but before SQL publication.
    let intents = state.database.connect().await?;
    intents
        .execute("INSERT INTO blob_gc(object) VALUES($1)", &[&object])
        .await?;
    state
        .bucket
        .put(&staged, body.clone(), &content_type)
        .await
        .map_err(unavailable)?;
    state
        .bucket
        .copy(&staged, &object)
        .await
        .map_err(unavailable)?;
    let etag = format!("\"{:x}\"", Sha256::digest(&body));
    let key = format!("{chat}/{part}");
    let size = body.len() as i64;
    tx.execute("INSERT INTO blobs(account,id,chat_id,object,size,content_type,etag) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(account,id) DO UPDATE SET object=EXCLUDED.object,size=EXCLUDED.size,content_type=EXCLUDED.content_type,etag=EXCLUDED.etag,created_at=now()",&[&account,&key,&chat,&object,&size,&content_type,&etag]).await?;
    tx.execute("DELETE FROM blob_gc WHERE object=$1", &[&object])
        .await?;
    tx.commit().await?;
    // The lifecycle rule also removes this stage if cleanup or the process fails.
    let _ = state.bucket.delete(&staged).await;
    Ok(Json(json!({"ok":true,"bytes":size})))
}

fn text<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}
/// One byte range, including closed, open, and suffix forms.
fn range(value: &str, total: usize) -> Option<(usize, usize)> {
    let value = value.strip_prefix("bytes=")?;
    let (a, b) = value.split_once('-')?;
    if total == 0 {
        return None;
    }
    let number = |s: &str| {
        if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
            s.parse::<usize>().ok()
        } else {
            None
        }
    };
    if a.is_empty() {
        let n = number(b)?;
        if n == 0 {
            return None;
        }
        return Some((total.saturating_sub(n), total - 1));
    }
    let start = number(a)?;
    let end = if b.is_empty() {
        total - 1
    } else {
        number(b)?.min(total - 1)
    };
    (start < total && start <= end).then_some((start, end))
}
pub async fn read(
    State(state): State<Arc<StateData>>,
    Path((chat, part)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response> {
    let who = identity(&state, &headers, &q, &chat)?;
    if !valid(&part) {
        return Err(Failure(StatusCode::BAD_REQUEST, "bad_part_id"));
    }
    let s = state.clone();
    let id = chat.clone();
    let (key, content_type, etag) = state
        .rooms
        .run(Room::Chat(chat), async move {
            let mut db = s.database.connect().await?;
            let tx = db.transaction().await?;
            owner(&tx, &id, &who.account, false).await?;
            let key = format!("{id}/{part}");
            let row = tx
                .query_opt(
                    "SELECT object,content_type,etag FROM blobs WHERE account=$1 AND id=$2",
                    &[&who.account, &key],
                )
                .await?
                .ok_or(Failure(StatusCode::NOT_FOUND, "not_found"))?;
            let result = (
                row.get::<_, String>(0),
                row.get::<_, String>(1),
                row.get::<_, String>(2),
            );
            tx.commit().await?;
            Ok::<_, Failure>(result)
        })
        .await
        .map_err(|_| Failure(StatusCode::SERVICE_UNAVAILABLE, "busy"))??;
    let mut response = if text(&headers, "if-none-match").is_some_and(|v| {
        v.split(',').any(|v| {
            let v = v.trim();
            v == "*" || v.strip_prefix("W/").unwrap_or(v) == etag
        })
    }) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        let object = state
            .bucket
            .get(&key)
            .await
            .map_err(unavailable)?
            .ok_or(Failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"))?;
        let total = object.bytes.len();
        let requested = text(&headers, "range")
            .filter(|_| text(&headers, "if-range").is_none_or(|v| v == etag));
        let mut r = if let Some(requested) = requested {
            if let Some((start, end)) = range(requested, total) {
                let mut r =
                    (StatusCode::PARTIAL_CONTENT, object.bytes.slice(start..=end)).into_response();
                r.headers_mut().insert(
                    "content-range",
                    format!("bytes {start}-{end}/{total}").parse().unwrap(),
                );
                r.headers_mut().insert(
                    "content-length",
                    (end - start + 1).to_string().parse().unwrap(),
                );
                r
            } else {
                let mut r = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
                r.headers_mut()
                    .insert("content-range", format!("bytes */{total}").parse().unwrap());
                r
            }
        } else {
            let mut r = object.bytes.into_response();
            r.headers_mut()
                .insert("content-length", total.to_string().parse().unwrap());
            r
        };
        r.headers_mut()
            .insert("accept-ranges", "bytes".parse().unwrap());
        r
    };
    let h = response.headers_mut();
    h.insert("content-type", content_type.parse().map_err(unavailable)?);
    h.insert("etag", etag.parse().map_err(unavailable)?);
    h.insert("cache-control", "private, max-age=300".parse().unwrap());
    Ok(response)
}
/// Retired mirrors are acknowledged so older clients can drain their outbox.
pub async fn retired_attachment(
    State(state): State<Arc<StateData>>,
    Path(path): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let id = path.split('/').next().unwrap_or("");
    identity(&state, &headers, &q, id)?;
    Ok(Json(json!({"ok":true})))
}
/// Delete only expired upload intents or retired versions, never a live object.
pub async fn collect_orphans(state: &StateData) -> Result<usize> {
    let mut db = state.database.connect().await?;
    let tx = db.transaction().await?;
    let rows=tx.query("SELECT object FROM blob_gc WHERE queued_at<now()-interval '7 days' AND NOT EXISTS(SELECT 1 FROM blobs WHERE blobs.object=blob_gc.object) ORDER BY queued_at LIMIT 32 FOR UPDATE SKIP LOCKED",&[]).await?;
    for row in &rows {
        let key: String = row.get(0);
        state.bucket.delete(&key).await.map_err(unavailable)?;
        tx.execute("DELETE FROM blob_gc WHERE object=$1", &[&key])
            .await?;
    }
    tx.commit().await?;
    Ok(rows.len())
}
pub fn start_cleanup(state: StateData) {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            timer.tick().await;
            loop {
                match collect_orphans(&state).await {
                    Ok(32) => tokio::task::yield_now().await,
                    Ok(_) => break,
                    Err(_) => {
                        tracing::warn!("Could not clean up expired uploads");
                        break;
                    }
                }
            }
        }
    });
}
