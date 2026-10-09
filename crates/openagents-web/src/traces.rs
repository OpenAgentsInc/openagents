//! Agent traces uploaded to the account (#11109): an ATIF trajectory a
//! person sends from Coder (`coder trace upload`) or any client, kept
//! private to their account unless they share it.
//!
//! | Route | What |
//! | --- | --- |
//! | `POST /api/traces[?share=true]` (body: ATIF JSON) | Save a trace. `201 {trace}` when new, `200 {trace}` when the same trace was already saved; `400 invalid`, `413 too_large` or `full`, `422 secret` |
//! | `GET /api/traces` | `{traces: [trace]}`, newest first |
//! | `GET /api/traces/{id}` | The saved ATIF document |
//! | `DELETE /api/traces/{id}` | `200 {deleted}` |
//! | `POST /api/traces/{id}/share` `{shared}` | Share or stop sharing. `200 {trace}`, `404 unknown` |
//! | `GET /settings/traces`, `/settings/traces/{id}` | The list and the viewer, with Share and Delete |
//! | `GET /trace/{id}` | A shared trace, for anyone with the link; `404` otherwise |
//!
//! The API takes the account's browser session or an app token
//! (`Authorization: Bearer sess_…`, from `coder login`), the same as the
//! Coder sync API ([`crate::coder_sync`]). A browser `POST` must be JSON,
//! which another site can't send without asking first. `{trace}` is
//! `{id, title, agent, model, steps, bytes, uploaded_unix, shared, url,
//! share_url}`.
//!
//! A trace is the uploaded document, checked as ATIF ([`atif::validate`])
//! and refused when any string in it looks like a credential
//! ([`secret_screen::credential_in_document`]); Coder redacts before
//! sending. Its id comes from the account and the document's digest, so
//! sending the same trace twice saves it once. Each account keeps at most
//! [`MAX_TRACES`]: the documents and one list per account live beside the
//! account's chats ([`Store::owner_key`]), and a shared trace has one more
//! small record naming its owner, so `/trace/{id}` can find it.

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::FormRejection;
use axum::extract::{DefaultBodyLimit, Form, Path, RawQuery, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, PreEscaped, html};
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::App;
use crate::chat_store::{Error, Store, account_owner, now_unix};
use crate::cloud::byo::fresh_request;
use crate::cloud::protect;
use crate::cloud::session::{SessionError, Viewer};
use crate::settings::{page, viewer};
use crate::ui_page::{UiPage, action_link};

/// The API.
pub(crate) const API: &str = "/api/traces";
/// The signed-in list; a trace's page is under it.
pub(crate) const PAGE: &str = "/settings/traces";
/// Where a shared trace is public.
pub(crate) const PUBLIC: &str = "/trace";
/// The largest trace a request may send.
pub(crate) const MAX_BODY: usize = 8 * 1024 * 1024;
/// The most traces one account keeps.
pub(crate) const MAX_TRACES: usize = 100;
/// The most steps a page draws.
const MAX_SHOWN_STEPS: usize = 400;
/// The longest tool result a page draws.
const MAX_SHOWN_RESULT: usize = 4000;

const INDEX: &str = "traces/index.json";
const INDEX_SCHEMA: &str = "openagents.web.traces.v1";
const SHARE_SCOPE: &str = "trace-share";
const DELETE_SCOPE: &str = "trace-delete";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(API, get(api_list).post(api_upload))
        .route(&format!("{API}/{{id}}"), get(api_read).delete(api_delete))
        .route(&format!("{API}/{{id}}/share"), post(api_share))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .route(PAGE, get(list_page))
        .route(&format!("{PAGE}/{{id}}"), get(trace_page))
        .route(&format!("{PAGE}/{{id}}/share"), post(share_form))
        .route(&format!("{PAGE}/{{id}}/delete"), post(delete_form))
        .route(&format!("{PUBLIC}/{{id}}"), get(public_page))
}

/// One saved trace, as the list keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Summary {
    pub id: String,
    pub title: String,
    pub agent: String,
    pub model: String,
    pub steps: usize,
    pub bytes: usize,
    pub digest: String,
    pub uploaded_unix: u64,
    pub shared: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    schema: String,
    /// Newest first.
    traces: Vec<Summary>,
}

/// Who owns a shared trace, so its public page can find it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedRecord {
    owner: String,
}

/// What became of an upload.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Uploaded {
    Saved {
        trace: Summary,
        existing: bool,
    },
    /// The account has [`MAX_TRACES`].
    Full,
    /// A string looks like a credential, by rule.
    Secret(&'static str),
    Invalid(String),
}

/// A trace id: a version 4 UUID shape from the owner and the document's
/// digest, so the same document saves once and two accounts never share
/// an id.
pub(crate) fn trace_id(owner: &str, digest: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"openagents.web.trace.v1\0");
    hash.update(owner.as_bytes());
    hash.update(b"\0");
    hash.update(digest.as_bytes());
    let digest = hash.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

fn document_key(owner: &str, id: &str) -> Result<String, Error> {
    Store::owner_key(owner, &format!("traces/{id}.json"))
}

fn shared_key(id: &str) -> String {
    format!("shared-traces/{id}.json")
}

/// One line of plain text, at most `limit` characters.
fn line(value: &str, limit: usize) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

/// A step's message as text: a string, or its content parts' text.
fn message_text(value: &Value) -> String {
    value.as_str().map(str::to_owned).unwrap_or_else(|| {
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

fn steps(document: &Value) -> &[Value] {
    document
        .get("steps")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// The checked document and its summary (no id or time yet).
pub(crate) fn check(body: &[u8]) -> Result<(Value, Summary), Uploaded> {
    if body.len() > MAX_BODY {
        return Err(Uploaded::Invalid("too_large".into()));
    }
    let Ok(mut document) = serde_json::from_slice::<Value>(body) else {
        return Err(Uploaded::Invalid(
            "Send the trace as ATIF JSON, such as the file coder export writes.".into(),
        ));
    };
    if !document.is_object() || !document.get("steps").is_some_and(Value::is_array) {
        return Err(Uploaded::Invalid(
            "That isn't an ATIF trace: it has no steps.".into(),
        ));
    }
    if let Err(error) = atif::upgrade(&mut document) {
        return Err(Uploaded::Invalid(format!(
            "That trace can't be read: {error}."
        )));
    }
    if let Some(error) = atif::validate(&document).into_iter().next() {
        return Err(Uploaded::Invalid(format!(
            "That trace can't be read: {error}."
        )));
    }
    if let Some(rule) = secret_screen::credential_in_document(&document) {
        return Err(Uploaded::Secret(rule));
    }
    let title = document
        .pointer("/extra/title")
        .or_else(|| document.get("title"))
        .and_then(Value::as_str)
        .map(|title| line(title, 120))
        .filter(|title| !title.is_empty())
        .or_else(|| {
            steps(&document)
                .iter()
                .filter(|step| step.get("source").and_then(Value::as_str) == Some("user"))
                .map(|step| {
                    line(
                        &message_text(step.get("message").unwrap_or(&Value::Null)),
                        120,
                    )
                })
                .find(|text| !text.is_empty())
        })
        .unwrap_or_else(|| "Agent trace".to_owned());
    let field = |pointer: &str| {
        document
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(|value| line(value, 80))
            .unwrap_or_default()
    };
    let summary = Summary {
        id: String::new(),
        title,
        agent: field("/agent/name"),
        model: field("/agent/model_name"),
        steps: steps(&document).len(),
        bytes: body.len(),
        digest: atif::digest(&document),
        uploaded_unix: 0,
        shared: false,
    };
    Ok((document, summary))
}

async fn read_index(store: &Store, owner: &str) -> Result<(Index, Option<String>), Error> {
    let key = Store::owner_key(owner, INDEX)?;
    match store.read_key(&key).await? {
        Some((bytes, generation)) => {
            let index: Index = serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("The trace list is invalid."))?;
            if index.schema != INDEX_SCHEMA {
                return Err(Error::Corrupt("The trace list is invalid."));
            }
            Ok((index, Some(generation)))
        }
        None => Ok((
            Index {
                schema: INDEX_SCHEMA.into(),
                traces: Vec::new(),
            },
            None,
        )),
    }
}

/// Change the account's trace list with `change`, which says whether it
/// changed anything; returns what `change` returned.
async fn update_index<T>(
    store: &Store,
    owner: &str,
    mut change: impl FnMut(&mut Index) -> (bool, T),
) -> Result<T, Error> {
    let key = Store::owner_key(owner, INDEX)?;
    for _ in 0..6 {
        let (mut index, generation) = read_index(store, owner).await?;
        let (changed, out) = change(&mut index);
        if !changed {
            return Ok(out);
        }
        let bytes =
            serde_json::to_vec(&index).map_err(|_| Error::Invalid("The trace list is invalid."))?;
        match store.write_key(&key, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(out),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

/// Save a trace for `owner`, shared at once when `share`.
pub(crate) async fn upload(
    store: &Store,
    owner: &str,
    body: &[u8],
    share: bool,
) -> Result<Uploaded, Error> {
    let (document, mut summary) = match check(body) {
        Ok(checked) => checked,
        Err(refused) => return Ok(refused),
    };
    let id = trace_id(owner, &summary.digest);
    summary.id = id.clone();
    summary.uploaded_unix = now_unix();
    let (index, _) = read_index(store, owner).await?;
    if let Some(existing) = index.traces.iter().find(|trace| trace.id == id) {
        let trace = if share && !existing.shared {
            set_shared(store, owner, &id, true)
                .await?
                .unwrap_or_else(|| existing.clone())
        } else {
            existing.clone()
        };
        return Ok(Uploaded::Saved {
            trace,
            existing: true,
        });
    }
    if index.traces.len() >= MAX_TRACES {
        return Ok(Uploaded::Full);
    }
    let bytes =
        serde_json::to_vec(&document).map_err(|_| Error::Invalid("The trace is invalid."))?;
    let key = document_key(owner, &id)?;
    match store.write_key(&key, bytes, None).await {
        // The same document is there already (an earlier upload that
        // didn't reach the list).
        Ok(_) | Err(Error::Conflict) => {}
        Err(error) => return Err(error),
    }
    let entry = summary.clone();
    let added = update_index(store, owner, move |index| {
        if index.traces.iter().any(|trace| trace.id == entry.id) {
            return (false, Some(true));
        }
        if index.traces.len() >= MAX_TRACES {
            return (false, None);
        }
        index.traces.insert(0, entry.clone());
        (true, Some(false))
    })
    .await?;
    let Some(existing) = added else {
        if let Some((_, generation)) = store.read_key(&key).await? {
            let _ = store.delete_key(&key, &generation).await;
        }
        return Ok(Uploaded::Full);
    };
    let trace = if share {
        set_shared(store, owner, &id, true)
            .await?
            .unwrap_or(summary)
    } else {
        summary
    };
    Ok(Uploaded::Saved { trace, existing })
}

/// The account's traces, newest first.
pub(crate) async fn list(store: &Store, owner: &str) -> Result<Vec<Summary>, Error> {
    Ok(read_index(store, owner).await?.0.traces)
}

/// One of the account's traces and its document.
pub(crate) async fn load(
    store: &Store,
    owner: &str,
    id: &str,
) -> Result<Option<(Summary, Value)>, Error> {
    if !valid_id(id) {
        return Ok(None);
    }
    let (index, _) = read_index(store, owner).await?;
    let Some(summary) = index.traces.into_iter().find(|trace| trace.id == id) else {
        return Ok(None);
    };
    let Some((bytes, _)) = store.read_key(&document_key(owner, id)?).await? else {
        return Ok(None);
    };
    let document =
        serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt("The trace is invalid."))?;
    Ok(Some((summary, document)))
}

/// Share a trace, or stop sharing it. `None` when the account has no such
/// trace.
pub(crate) async fn set_shared(
    store: &Store,
    owner: &str,
    id: &str,
    shared: bool,
) -> Result<Option<Summary>, Error> {
    if !valid_id(id) {
        return Ok(None);
    }
    let key = shared_key(id);
    if shared {
        let (index, _) = read_index(store, owner).await?;
        if !index.traces.iter().any(|trace| trace.id == id) {
            return Ok(None);
        }
        let record = serde_json::to_vec(&SharedRecord {
            owner: owner.to_owned(),
        })
        .map_err(|_| Error::Invalid("The share is invalid."))?;
        match store.write_key(&key, record, None).await {
            Ok(_) => {}
            Err(Error::Conflict) => {
                // Shared already, by this owner (ids differ per owner).
                if !shared_owner(store, id).await?.is_some_and(|o| o == owner) {
                    return Err(Error::Conflict);
                }
            }
            Err(error) => return Err(error),
        }
    }
    let id_owned = id.to_owned();
    let updated = update_index(store, owner, move |index| {
        match index.traces.iter_mut().find(|trace| trace.id == id_owned) {
            Some(trace) if trace.shared != shared => {
                trace.shared = shared;
                (true, Some(trace.clone()))
            }
            Some(trace) => (false, Some(trace.clone())),
            None => (false, None),
        }
    })
    .await?;
    if !shared {
        remove_share(store, owner, id).await?;
    }
    Ok(updated)
}

async fn shared_owner(store: &Store, id: &str) -> Result<Option<String>, Error> {
    let Some((bytes, _)) = store.read_key(&shared_key(id)).await? else {
        return Ok(None);
    };
    let record: SharedRecord =
        serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt("The share is invalid."))?;
    Ok(Some(record.owner))
}

async fn remove_share(store: &Store, owner: &str, id: &str) -> Result<(), Error> {
    let key = shared_key(id);
    if let Some((bytes, generation)) = store.read_key(&key).await?
        && serde_json::from_slice::<SharedRecord>(&bytes).is_ok_and(|record| record.owner == owner)
    {
        match store.delete_key(&key, &generation).await {
            Ok(_) | Err(Error::Conflict) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Delete one of the account's traces. False when there was none.
pub(crate) async fn delete(store: &Store, owner: &str, id: &str) -> Result<bool, Error> {
    if !valid_id(id) {
        return Ok(false);
    }
    let id_owned = id.to_owned();
    let removed = update_index(store, owner, move |index| {
        let before = index.traces.len();
        index.traces.retain(|trace| trace.id != id_owned);
        let removed = index.traces.len() != before;
        (removed, removed)
    })
    .await?;
    remove_share(store, owner, id).await?;
    let key = document_key(owner, id)?;
    if let Some((_, generation)) = store.read_key(&key).await? {
        store.delete_key(&key, &generation).await?;
    }
    Ok(removed)
}

/// A shared trace, for anyone with its link.
pub(crate) async fn public(store: &Store, id: &str) -> Result<Option<(Summary, Value)>, Error> {
    if !valid_id(id) {
        return Ok(None);
    }
    let Some(owner) = shared_owner(store, id).await? else {
        return Ok(None);
    };
    Ok(load(store, &owner, id)
        .await?
        .filter(|(summary, _)| summary.shared))
}

// ---------------------------------------------------------------------
// The API

fn answer(status: StatusCode, body: Value) -> Response {
    let mut response = protect((status, axum::Json(body)).into_response());
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

fn refused(status: StatusCode, code: &str, message: &str) -> Response {
    answer(status, json!({"error": {"code": code, "message": message}}))
}

fn stored(error: &Error) -> Response {
    eprintln!("openagents-web: traces: {error}");
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "Try again later.",
    )
}

/// The site's address, for the links an answer carries.
fn origin(app: &App, headers: &HeaderMap) -> String {
    if let Some(service) = app.config.cloud.as_deref() {
        return service.origin().trim_end_matches('/').to_owned();
    }
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("openagents.com");
    let scheme = if host.starts_with("127.0.0.1:") || host.starts_with("localhost:") {
        "http"
    } else {
        "https"
    };
    format!("{scheme}://{host}")
}

fn wire(trace: &Summary, origin: &str) -> Value {
    json!({
        "id": trace.id,
        "title": trace.title,
        "agent": trace.agent,
        "model": trace.model,
        "steps": trace.steps,
        "bytes": trace.bytes,
        "uploaded_unix": trace.uploaded_unix,
        "shared": trace.shared,
        "url": format!("{origin}{PAGE}/{}", trace.id),
        "share_url": trace.shared.then(|| format!("{origin}{PUBLIC}/{}", trace.id)),
    })
}

/// The account a request speaks for: its app token, or else its browser
/// session.
async fn api_owner(app: &App, headers: &HeaderMap) -> Result<String, Response> {
    if headers.contains_key(header::AUTHORIZATION) {
        return crate::coder_sync::owner(app, headers).await;
    }
    let Some(service) = app.config.cloud.as_deref() else {
        return Err(refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "This site doesn't offer accounts.",
        ));
    };
    match service.authenticate(headers).await {
        Ok(viewer) => Ok(account_owner(&viewer.account_id)),
        Err(SessionError::Unauthenticated | SessionError::InvalidRequest) => Err(refused(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Sign in first, or sign Coder in with coder login.",
        )),
        Err(_) => Err(refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "Try again later.",
        )),
    }
}

/// A browser may only change things with a JSON request, which another
/// site can't send without asking first.
fn json_request(headers: &HeaderMap) -> bool {
    headers.contains_key(header::AUTHORIZATION)
        || headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
            })
}

fn wants_share(query: Option<&str>) -> bool {
    query.is_some_and(|query| {
        query
            .split('&')
            .any(|part| matches!(part, "share=true" | "share=1"))
    })
}

async fn api_upload(
    State(app): State<App>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !json_request(&headers) {
        return refused(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "invalid",
            "Send the trace as application/json.",
        );
    }
    let owner = match api_owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let origin = origin(&app, &headers);
    match upload(
        &app.config.chat_store,
        &owner,
        &body,
        wants_share(query.as_deref()),
    )
    .await
    {
        Ok(Uploaded::Saved { trace, existing }) => answer(
            if existing {
                StatusCode::OK
            } else {
                StatusCode::CREATED
            },
            json!({"trace": wire(&trace, &origin), "existing": existing}),
        ),
        Ok(Uploaded::Full) => refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "full",
            "Your account has no room for more traces. Delete some in Settings on openagents.com.",
        ),
        Ok(Uploaded::Secret(_)) => refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "secret",
            "Part of this trace looks like a password or key, so it wasn't saved. coder trace upload takes those out for you.",
        ),
        Ok(Uploaded::Invalid(message)) if message == "too_large" => refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "too_large",
            "This trace is larger than 8 MB.",
        ),
        Ok(Uploaded::Invalid(message)) => refused(StatusCode::BAD_REQUEST, "invalid", &message),
        Err(error) => stored(&error),
    }
}

async fn api_list(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = match api_owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let origin = origin(&app, &headers);
    match list(&app.config.chat_store, &owner).await {
        Ok(traces) => answer(
            StatusCode::OK,
            json!({"traces": traces.iter().map(|t| wire(t, &origin)).collect::<Vec<_>>()}),
        ),
        Err(error) => stored(&error),
    }
}

async fn api_read(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let owner = match api_owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match load(&app.config.chat_store, &owner, &id).await {
        Ok(Some((_, document))) => answer(StatusCode::OK, document),
        Ok(None) => refused(StatusCode::NOT_FOUND, "unknown", "No such trace."),
        Err(error) => stored(&error),
    }
}

async fn api_delete(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let owner = match api_owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match delete(&app.config.chat_store, &owner, &id).await {
        Ok(deleted) => answer(StatusCode::OK, json!({"deleted": deleted})),
        Err(error) => stored(&error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShareBody {
    shared: bool,
}

async fn api_share(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    if !json_request(&headers) {
        return refused(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "invalid",
            "Send {shared} as application/json.",
        );
    }
    let owner = match api_owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let Ok(sent) = serde_json::from_slice::<ShareBody>(&body) else {
        return refused(StatusCode::BAD_REQUEST, "invalid", "Send {shared}.");
    };
    let origin = origin(&app, &headers);
    match set_shared(&app.config.chat_store, &owner, &id, sent.shared).await {
        Ok(Some(trace)) => answer(StatusCode::OK, json!({"trace": wire(&trace, &origin)})),
        Ok(None) => refused(StatusCode::NOT_FOUND, "unknown", "No such trace."),
        Err(error) => stored(&error),
    }
}

// ---------------------------------------------------------------------
// The pages

fn day(unix: u64) -> String {
    atif::iso(unix.saturating_mul(1000))[..10].to_owned()
}

fn steps_label(count: usize) -> String {
    if count == 1 {
        "1 step".into()
    } else {
        format!("{count} steps")
    }
}

/// The list page's content.
fn list_content(traces: &[Summary]) -> Markup {
    html! {
        p { (action_link("Settings", crate::settings::PAGE)) }
        (MarkdownRoot::new(html! {
            h1 { "Traces" }
            p {
                "Traces are agent runs you uploaded. Only you can see them unless you share one."
            }
            p {
                "Upload your latest Coder chat with " code { "coder trace upload --last" }
                ". See " a href="/docs/traces" { "Traces" } " in the docs."
            }
        }))
        @if traces.is_empty() {
            p { "You have no traces yet." }
        } @else {
            section class="oa-settings-group" aria-labelledby="traces-list" {
                h2 #traces-list { "Your traces" }
                @for trace in traces {
                    div class="oa-settings-row" {
                        div class="oa-settings-text" {
                            span class="oa-settings-label" { (trace.title) }
                            span class="oa-settings-hint" {
                                (steps_label(trace.steps)) " · uploaded " (day(trace.uploaded_unix))
                                @if trace.shared { " · Shared" } @else { " · Private" }
                            }
                        }
                        div class="oa-settings-control" {
                            (action_link("Open", &format!("{PAGE}/{}", trace.id)))
                        }
                    }
                }
            }
        }
    }
}

/// The trace's steps, drawn plainly.
fn steps_content(document: &Value) -> Markup {
    let all = steps(document);
    let shown = &all[..all.len().min(MAX_SHOWN_STEPS)];
    html! {
        @for step in shown {
            @let source = step.get("source").and_then(Value::as_str).unwrap_or("agent");
            @let text = message_text(step.get("message").unwrap_or(&Value::Null));
            @let calls = step.get("tool_calls").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
            @let results = step.pointer("/observation/results").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
            section class="oa-trace-step" {
                h3 {
                    @match source {
                        "user" => { "You" }
                        "system" => { "System" }
                        _ => {
                            "Agent"
                            @if let Some(model) = step.get("model_name").and_then(Value::as_str) {
                                " · " (line(model, 80))
                            }
                        }
                    }
                }
                @if !text.trim().is_empty() {
                    (PreEscaped(crate::markdown::render(&text)))
                }
                @for call in calls {
                    @let name = call.get("function_name").and_then(Value::as_str).unwrap_or("tool");
                    @let arguments = call.get("arguments").map(|a| a.as_str().map_or_else(|| a.to_string(), str::to_owned)).unwrap_or_default();
                    p { strong { (line(name, 80)) } }
                    @if !arguments.is_empty() && arguments != "null" && arguments != "{}" {
                        pre { code { (secret_screen::head_and_tail(&arguments, MAX_SHOWN_RESULT)) } }
                    }
                }
                @for result in results {
                    @let content = message_text(result.get("content").unwrap_or(&Value::Null));
                    @if !content.trim().is_empty() {
                        details {
                            summary { "Result" }
                            pre { code { (secret_screen::head_and_tail(&content, MAX_SHOWN_RESULT)) } }
                        }
                    }
                }
            }
        }
        @if all.len() > shown.len() {
            p { "The first " (shown.len()) " of " (all.len()) " steps are shown." }
        }
    }
}

fn about(trace: &Summary) -> String {
    let mut parts = vec![
        steps_label(trace.steps),
        format!("uploaded {}", day(trace.uploaded_unix)),
    ];
    if !trace.agent.is_empty() {
        parts.push(trace.agent.clone());
    }
    if !trace.model.is_empty() {
        parts.push(trace.model.clone());
    }
    parts.join(" · ")
}

/// A trace's page for its owner: what it is, Share and Delete, the steps.
fn trace_content(
    trace: &Summary,
    document: &Value,
    share_url: &str,
    share: (&str, &str),
    delete: (&str, &str),
) -> Markup {
    let base = format!("{PAGE}/{}", trace.id);
    html! {
        p { (action_link("Traces", PAGE)) }
        (MarkdownRoot::new(html! {
            h1 { (trace.title) }
            p { (about(trace)) }
            @if trace.shared {
                p { "Shared: anyone with this link can see it." }
                p { a href=(share_url) { (share_url) } }
            } @else {
                p { "Private: only you can see it." }
            }
        }))
        div class="oa-settings-row" {
            div class="oa-settings-control" {
                form method="post" action=(format!("{base}/share")) {
                    input type="hidden" name="csrf" value=(share.0);
                    input type="hidden" name="request" value=(share.1);
                    input type="hidden" name="shared" value=(if trace.shared { "false" } else { "true" });
                    (Button::new(if trace.shared { "Stop sharing" } else { "Share" })
                        .kind(ButtonType::Submit)
                        .variant(ButtonVariant::Soft)
                        .color(Color::Secondary))
                }
                form method="post" action=(format!("{base}/delete")) {
                    input type="hidden" name="csrf" value=(delete.0);
                    input type="hidden" name="request" value=(delete.1);
                    (Button::new("Delete")
                        .kind(ButtonType::Submit)
                        .variant(ButtonVariant::Soft)
                        .color(Color::Secondary))
                }
            }
        }
        (MarkdownRoot::new(steps_content(document)))
    }
}

/// A shared trace's public page.
fn public_content(trace: &Summary, document: &Value) -> Markup {
    html! {
        (MarkdownRoot::new(html! {
            h1 { (trace.title) }
            p { (about(trace)) }
        }))
        (MarkdownRoot::new(steps_content(document)))
    }
}

fn problem(status: StatusCode, text: &str) -> Response {
    protect(crate::layout::problem(
        status,
        "Traces",
        text,
        (PAGE, "Traces"),
    ))
}

fn unavailable_page(error: &Error) -> Response {
    eprintln!("openagents-web: traces: {error}");
    problem(
        StatusCode::SERVICE_UNAVAILABLE,
        "Your traces can't be read right now. Try again in a minute.",
    )
}

fn target(viewer: &Viewer, id: &str, request: &str) -> String {
    format!("{}:{id}:{request}", viewer.account_id)
}

async fn list_page(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    match list(&app.config.chat_store, &owner).await {
        Ok(traces) => page(
            &headers,
            service,
            &viewer,
            "Traces",
            PAGE,
            list_content(&traces),
        ),
        Err(error) => unavailable_page(&error),
    }
}

async fn trace_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let back = format!("{PAGE}/{id}");
    let (service, viewer) = match viewer(&app, &headers, &back).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let (trace, document) = match load(&app.config.chat_store, &owner, &id).await {
        Ok(Some(found)) => found,
        Ok(None) => return problem(StatusCode::NOT_FOUND, "You have no trace at that address."),
        Err(error) => return unavailable_page(&error),
    };
    let mut tickets = Vec::new();
    for scope in [SHARE_SCOPE, DELETE_SCOPE] {
        let request = fresh_request();
        match service.csrf(&headers, &viewer, scope, &target(&viewer, &id, &request)) {
            Ok(csrf) => tickets.push((csrf, request)),
            Err(error) => return crate::cloud::refused(error),
        }
    }
    let share_url = format!("{}{PUBLIC}/{id}", origin(&app, &headers));
    let body = trace_content(
        &trace,
        &document,
        &share_url,
        (&tickets[0].0, &tickets[0].1),
        (&tickets[1].0, &tickets[1].1),
    );
    page(&headers, service, &viewer, &trace.title, &back, body)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShareForm {
    csrf: String,
    request: String,
    shared: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteForm {
    csrf: String,
    request: String,
}

async fn share_form(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<ShareForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let back = format!("{PAGE}/{id}");
    let (service, viewer) = match viewer(&app, &headers, &back).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        SHARE_SCOPE,
        &target(&viewer, &id, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    let owner = account_owner(&viewer.account_id);
    match set_shared(&app.config.chat_store, &owner, &id, form.shared == "true").await {
        Ok(Some(_)) => protect(Redirect::to(&back).into_response()),
        Ok(None) => problem(StatusCode::NOT_FOUND, "You have no trace at that address."),
        Err(error) => unavailable_page(&error),
    }
}

async fn delete_form(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<DeleteForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let back = format!("{PAGE}/{id}");
    let (service, viewer) = match viewer(&app, &headers, &back).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        DELETE_SCOPE,
        &target(&viewer, &id, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    let owner = account_owner(&viewer.account_id);
    match delete(&app.config.chat_store, &owner, &id).await {
        Ok(_) => protect(Redirect::to(PAGE).into_response()),
        Err(error) => unavailable_page(&error),
    }
}

async fn public_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    match public(&app.config.chat_store, &id).await {
        Ok(Some((trace, document))) => {
            let mut response = UiPage::new(trace.title.clone())
                .path(&format!("{PUBLIC}/{id}"))
                .scriptless()
                .content(PageColumn::new(public_content(&trace, &document)))
                .respond(&headers);
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                axum::http::HeaderValue::from_static("no-store"),
            );
            response.headers_mut().insert(
                header::REFERRER_POLICY,
                axum::http::HeaderValue::from_static("no-referrer"),
            );
            response
        }
        Ok(None) => crate::layout::problem(
            StatusCode::NOT_FOUND,
            "Not found",
            "Nothing on this site has that address.",
            ("/", "Home"),
        ),
        Err(error) => {
            eprintln!("openagents-web: traces: {error}");
            crate::layout::problem(
                StatusCode::SERVICE_UNAVAILABLE,
                "Trace",
                "This trace can't be read right now. Try again in a minute.",
                ("/", "Home"),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> String {
        account_owner("acct_one")
    }

    fn trace(text: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "schema_version": "ATIF-v1.7",
            "session_id": "s1",
            "agent": {"name": "openagents-coder", "version": "1", "model_name": "gpt-test"},
            "steps": [
                {"step_id": 1, "source": "user", "message": text},
                {"step_id": 2, "source": "agent", "message": "Done. **Fixed** it.",
                 "model_name": "gpt-test",
                 "tool_calls": [{"tool_call_id": "c1", "function_name": "shell", "arguments": {"command": "cargo test"}}],
                 "observation": {"results": [{"source_call_id": "c1", "content": "test result: ok"}]}}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn ids_are_per_account_and_content() {
        let one = trace_id(&owner(), "abc");
        assert_eq!(one, trace_id(&owner(), "abc"));
        assert_ne!(one, trace_id(&account_owner("acct_two"), "abc"));
        assert_ne!(one, trace_id(&owner(), "abd"));
        assert!(valid_id(&one));
        assert!(!valid_id("../x"));
    }

    #[test]
    fn uploads_are_checked() {
        assert!(matches!(check(b"not json"), Err(Uploaded::Invalid(_))));
        assert!(matches!(check(b"{}"), Err(Uploaded::Invalid(_))));
        assert!(matches!(
            check(br#"{"schema_version":"ATIF-v9","steps":[]}"#),
            Err(Uploaded::Invalid(_))
        ));
        assert!(matches!(
            check(br#"{"schema_version":"ATIF-v1.8","steps":[{"message":7}]}"#),
            Err(Uploaded::Invalid(_))
        ));
        let github = format!("ghp_{}", "Z9".repeat(18));
        assert_eq!(
            check(&trace(&format!("my token is {github}"))).unwrap_err(),
            Uploaded::Secret("github-token")
        );
        let (document, summary) = check(&trace("Fix the build")).unwrap();
        assert_eq!(document["schema_version"], atif::SCHEMA_VERSION);
        assert_eq!(summary.title, "Fix the build");
        assert_eq!(summary.steps, 2);
        assert_eq!(summary.agent, "openagents-coder");
        assert_eq!(summary.model, "gpt-test");
        // What redaction leaves is fine.
        assert!(
            check(&trace(
                "CLAUDE_CODE_OAUTH_TOKEN=[redacted:claude-oauth-env]"
            ))
            .is_ok()
        );
    }

    #[tokio::test]
    async fn upload_list_share_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        let me = owner();
        let Uploaded::Saved {
            trace: saved,
            existing,
        } = upload(&store, &me, &trace("Fix the build"), false)
            .await
            .unwrap()
        else {
            panic!("not saved");
        };
        assert!(!existing && !saved.shared);
        // The same trace again saves nothing new.
        let Uploaded::Saved {
            trace: again,
            existing,
        } = upload(&store, &me, &trace("Fix the build"), false)
            .await
            .unwrap()
        else {
            panic!("not saved");
        };
        assert!(existing);
        assert_eq!(again.id, saved.id);
        assert_eq!(list(&store, &me).await.unwrap().len(), 1);
        // Traces never show as chats.
        assert!(store.list(&me).await.unwrap().is_empty());
        // Another account can't see it.
        let other = account_owner("acct_two");
        assert!(load(&store, &other, &saved.id).await.unwrap().is_none());
        assert!(list(&store, &other).await.unwrap().is_empty());
        assert!(
            set_shared(&store, &other, &saved.id, true)
                .await
                .unwrap()
                .is_none()
        );
        assert!(!delete(&store, &other, &saved.id).await.unwrap());
        // Private until shared; revocable.
        assert!(public(&store, &saved.id).await.unwrap().is_none());
        let shared = set_shared(&store, &me, &saved.id, true)
            .await
            .unwrap()
            .unwrap();
        assert!(shared.shared);
        let (_, document) = public(&store, &saved.id).await.unwrap().unwrap();
        assert_eq!(steps(&document).len(), 2);
        set_shared(&store, &me, &saved.id, false)
            .await
            .unwrap()
            .unwrap();
        assert!(public(&store, &saved.id).await.unwrap().is_none());
        // Share on upload.
        let Uploaded::Saved { trace: second, .. } =
            upload(&store, &me, &trace("Second"), true).await.unwrap()
        else {
            panic!("not saved");
        };
        assert!(second.shared);
        assert!(public(&store, &second.id).await.unwrap().is_some());
        let listed = list(&store, &me).await.unwrap();
        assert_eq!(listed[0].id, second.id, "newest first");
        // Delete takes the share with it.
        assert!(delete(&store, &me, &second.id).await.unwrap());
        assert!(public(&store, &second.id).await.unwrap().is_none());
        assert!(load(&store, &me, &second.id).await.unwrap().is_none());
        assert_eq!(list(&store, &me).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn an_account_keeps_a_bounded_number() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        let me = owner();
        for n in 0..MAX_TRACES {
            assert!(matches!(
                upload(&store, &me, &trace(&format!("trace {n}")), false)
                    .await
                    .unwrap(),
                Uploaded::Saved { .. }
            ));
        }
        assert_eq!(
            upload(&store, &me, &trace("one more"), false)
                .await
                .unwrap(),
            Uploaded::Full
        );
    }

    #[test]
    fn the_pages_are_plain_and_escape_the_trace() {
        let (document, mut summary) = check(&trace("Fix <b>the</b> build")).unwrap();
        summary.id = trace_id(&owner(), &summary.digest);
        summary.uploaded_unix = 1_791_504_000;
        let list = list_content(std::slice::from_ref(&summary)).into_string();
        crate::copy_guard::assert_plain(PAGE, &list);
        assert!(list.contains("Fix &lt;b&gt;the&lt;/b&gt; build"));
        assert!(list.contains(&format!("{PAGE}/{}", summary.id)));
        assert!(list.contains("Private"));
        let empty = list_content(&[]).into_string();
        crate::copy_guard::assert_plain(PAGE, &empty);
        assert!(empty.contains("coder trace upload --last"));
        let page = trace_content(
            &summary,
            &document,
            "https://openagents.com/trace/x",
            ("a", "b"),
            ("c", "d"),
        )
        .into_string();
        crate::copy_guard::assert_plain(&format!("{PAGE}/x"), &page);
        assert!(page.contains(">Share<") && page.contains(">Delete<"));
        assert!(page.contains("cargo test") && page.contains("test result: ok"));
        assert!(page.contains("<strong>Fixed</strong>"));
        assert!(!page.contains("<b>the</b>"));
        summary.shared = true;
        let shared = trace_content(
            &summary,
            &document,
            "https://openagents.com/trace/x",
            ("a", "b"),
            ("c", "d"),
        )
        .into_string();
        assert!(
            shared.contains(">Stop sharing<") && shared.contains("https://openagents.com/trace/x")
        );
        let public = public_content(&summary, &document).into_string();
        crate::copy_guard::assert_plain("/trace/x", &public);
        assert!(!public.contains("Delete"));
    }
}
