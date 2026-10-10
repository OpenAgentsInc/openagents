//! Agent traces uploaded to the account (#11109): an ATIF trajectory a
//! person sends from Coder (`coder trace upload`) or any client, kept
//! private to their account unless they share it.
//!
//! | Route | What |
//! | --- | --- |
//! | `POST /v1/traces[?share=true]` (body: ATIF JSON) | Save a trace. `201 {trace}` when new, `200 {trace}` when the same trace was already saved; `400 invalid`, `413 too_large` or `full`, `422 secret` |
//! | `GET /v1/traces` | `{traces: [trace]}`, newest first |
//! | `GET /v1/traces/{id}` | The saved ATIF document |
//! | `DELETE /v1/traces/{id}` | `200 {deleted}` |
//! | `POST /v1/traces/{id}/share` `{shared}` | Share or stop sharing. `200 {trace}`, `404 unknown` |
//! | `GET /settings/traces`, `/settings/traces/{id}` | The list and the viewer, with Share and Delete |
//! | `GET /trace/{id}` | A shared trace, for anyone with the link; `404` otherwise |
//! | `POST /v1/traces/{id}/agents[?parent=AGENT]` (body: ATIF JSON) | Save an agent under the trace (#11178), below `parent` or the trace's own conversation. `201 {agent}` when new, `200` when already saved; `404 unknown` (no such trace or parent), `413 full` past [`MAX_AGENTS`] or [`MAX_TREE_BYTES`], and the trace's refusals |
//! | `GET /v1/traces/{id}/agents` | `{agents: [agent]}`, parents before children, each with its times, tokens and cost |
//! | `GET /v1/traces/{id}/agents/{agent}` | The agent's ATIF document |
//! | `GET /settings/traces/{id}/agents/{agent}`, `/trace/{id}/agents/{agent}` | An agent's page; public while its trace is shared |
//!
//! Every `/v1/traces` route also answers at its older path, `/api/traces`,
//! marked deprecated (#11158, [`crate::older_paths`]).
//!
//! A trace with agents is a whole orchestration: a main conversation and
//! every agent it (or its agents) started. Each agent is its own document
//! (at most [`MAX_BODY`]) under the trace, listed in one small record per
//! trace (`traces/{id}/tree.json`), so the account's trace list stays one
//! entry per trace. The trace's page draws the tree with a timeline; sharing
//! or deleting the trace shares or deletes its agents.
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

use std::collections::HashMap;

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
pub(crate) const API: &str = "/v1/traces";
/// The API's older path (#11158), still answered.
pub(crate) const OLDER_API: &str = "/api/traces";
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
    // The API at `/v1/traces` (#11158) and at its older path, `/api/traces`,
    // which answers the same and names its successor.
    let api = |base: &str, older: bool| {
        let mark = |route: axum::routing::MethodRouter<App>| {
            if older {
                crate::older_paths::deprecated(route, OLDER_API, API)
            } else {
                route
            }
        };
        Router::new()
            .route(base, mark(get(api_list).post(api_upload)))
            .route(
                &format!("{base}/{{id}}"),
                mark(get(api_read).delete(api_delete)),
            )
            .route(&format!("{base}/{{id}}/share"), mark(post(api_share)))
            .route(
                &format!("{base}/{{id}}/agents"),
                mark(get(api_agents).post(api_upload_agent)),
            )
            .route(
                &format!("{base}/{{id}}/agents/{{agent}}"),
                mark(get(api_read_agent)),
            )
    };
    Router::new()
        .merge(api(API, false))
        .merge(api(OLDER_API, true))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .route(PAGE, get(list_page))
        .route(&format!("{PAGE}/{{id}}"), get(trace_page))
        .route(&format!("{PAGE}/{{id}}/agents/{{agent}}"), get(agent_page))
        .route(
            &format!("{PUBLIC}/{{id}}/agents/{{agent}}"),
            get(public_agent_page),
        )
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
    /// How many agents are saved under it (#11178).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub agents: usize,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(n: &usize) -> bool {
    *n == 0
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
        agents: 0,
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
    delete_tree(store, owner, id).await?;
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
// Trees: a trace's agents (#11178)

/// An agent's numbers, read from its trajectory: when it ran, what it
/// spent, how much it did.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Stats {
    #[serde(default)]
    pub started_ms: Option<u64>,
    #[serde(default)]
    pub ended_ms: Option<u64>,
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub tool_calls: usize,
}

impl Stats {
    /// From a trajectory: its steps' times, and its totals (or the sum of
    /// its steps' metrics when it has none).
    pub(crate) fn of(document: &Value) -> Self {
        let steps = steps(document);
        let times: Vec<u64> = steps
            .iter()
            .filter_map(|step| step.get("timestamp").and_then(Value::as_str))
            .filter_map(atif::parse_iso)
            .collect();
        let total = |field: &str, metric: &str| {
            document
                .pointer(&format!("/final_metrics/{field}"))
                .and_then(Value::as_u64)
                .unwrap_or_else(|| {
                    steps
                        .iter()
                        .filter_map(|step| step.pointer(&format!("/metrics/{metric}")))
                        .filter_map(Value::as_u64)
                        .sum()
                })
        };
        let cost = document
            .pointer("/final_metrics/total_cost_usd")
            .and_then(Value::as_f64)
            .unwrap_or_else(|| {
                steps
                    .iter()
                    .filter_map(|step| step.pointer("/metrics/cost_usd"))
                    .filter_map(Value::as_f64)
                    .sum()
            });
        Self {
            started_ms: times.iter().min().copied(),
            ended_ms: times.iter().max().copied(),
            prompt_tokens: total("total_prompt_tokens", "prompt_tokens"),
            completion_tokens: total("total_completion_tokens", "completion_tokens"),
            cost_usd: if cost.is_finite() { cost.max(0.0) } else { 0.0 },
            tool_calls: steps
                .iter()
                .filter_map(|step| step.get("tool_calls").and_then(Value::as_array))
                .map(Vec::len)
                .sum(),
        }
    }

    fn duration_ms(&self) -> Option<u64> {
        Some(self.ended_ms?.saturating_sub(self.started_ms?))
    }
}

/// One agent in a trace's tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Agent {
    pub id: String,
    /// The agent that started it; `None` for the trace's own conversation.
    #[serde(default)]
    pub parent: Option<String>,
    pub title: String,
    pub agent: String,
    pub model: String,
    pub steps: usize,
    pub bytes: usize,
    pub digest: String,
    pub uploaded_unix: u64,
    /// The tool call in the parent that started it, when known.
    #[serde(default)]
    pub call: Option<String>,
    #[serde(default)]
    pub stats: Stats,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tree {
    schema: String,
    /// In upload order: every parent before its children.
    agents: Vec<Agent>,
}

const TREE_SCHEMA: &str = "openagents.web.trace-tree.v1";
/// The most agents one trace keeps.
pub(crate) const MAX_AGENTS: usize = 1000;
/// The most one trace and its agents may hold in all.
pub(crate) const MAX_TREE_BYTES: usize = 512 * 1024 * 1024;

fn tree_key(owner: &str, root: &str) -> Result<String, Error> {
    Store::owner_key(owner, &format!("traces/{root}/tree.json"))
}

fn agent_key(owner: &str, root: &str, id: &str) -> Result<String, Error> {
    Store::owner_key(owner, &format!("traces/{root}/agents/{id}.json"))
}

/// An agent's id: from the owner, the trace, and the agent's digest, so the
/// same agent under the same trace saves once.
pub(crate) fn agent_id(owner: &str, root: &str, digest: &str) -> String {
    trace_id(owner, &format!("agent\0{root}\0{digest}"))
}

async fn read_tree(
    store: &Store,
    owner: &str,
    root: &str,
) -> Result<(Tree, Option<String>), Error> {
    match store.read_key(&tree_key(owner, root)?).await? {
        Some((bytes, generation)) => {
            let tree: Tree = serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("The trace's agents are invalid."))?;
            if tree.schema != TREE_SCHEMA {
                return Err(Error::Corrupt("The trace's agents are invalid."));
            }
            Ok((tree, Some(generation)))
        }
        None => Ok((
            Tree {
                schema: TREE_SCHEMA.into(),
                agents: Vec::new(),
            },
            None,
        )),
    }
}

/// A trace's agents, parents before children; empty when it has none.
pub(crate) async fn agents(store: &Store, owner: &str, root: &str) -> Result<Vec<Agent>, Error> {
    if !valid_id(root) {
        return Ok(Vec::new());
    }
    Ok(read_tree(store, owner, root).await?.0.agents)
}

/// What became of an agent's upload.
#[derive(Debug, PartialEq)]
pub(crate) enum AgentUploaded {
    Saved {
        agent: Box<Agent>,
        existing: bool,
    },
    /// No such trace, or no such parent agent under it.
    Unknown(&'static str),
    /// The trace has [`MAX_AGENTS`] or [`MAX_TREE_BYTES`].
    Full,
    Refused(Uploaded),
}

/// Save an agent's trajectory under `root`, below `parent` (another of
/// its agents) or below the trace's own conversation.
pub(crate) async fn upload_agent(
    store: &Store,
    owner: &str,
    root: &str,
    parent: Option<&str>,
    body: &[u8],
) -> Result<AgentUploaded, Error> {
    if !valid_id(root) || parent.is_some_and(|parent| !valid_id(parent)) {
        return Ok(AgentUploaded::Unknown("No such trace."));
    }
    let (document, summary) = match check(body) {
        Ok(checked) => checked,
        Err(refused) => return Ok(AgentUploaded::Refused(refused)),
    };
    let (index, _) = read_index(store, owner).await?;
    let Some(trace) = index.traces.iter().find(|trace| trace.id == root) else {
        return Ok(AgentUploaded::Unknown("No such trace."));
    };
    let root_bytes = trace.bytes;
    let id = agent_id(owner, root, &summary.digest);
    let agent = Agent {
        id: id.clone(),
        parent: parent.map(str::to_owned),
        title: summary.title,
        agent: summary.agent,
        model: summary.model,
        steps: summary.steps,
        bytes: summary.bytes,
        digest: summary.digest,
        uploaded_unix: now_unix(),
        call: document
            .pointer("/extra/parent_tool_call_id")
            .and_then(Value::as_str)
            .map(|call| line(call, 120)),
        stats: Stats::of(&document),
    };
    let (tree, _) = read_tree(store, owner, root).await?;
    if let Some(existing) = tree.agents.iter().find(|agent| agent.id == id) {
        return Ok(AgentUploaded::Saved {
            agent: Box::new(existing.clone()),
            existing: true,
        });
    }
    if let Some(parent) = parent
        && !tree.agents.iter().any(|agent| agent.id == parent)
    {
        return Ok(AgentUploaded::Unknown("No such agent under this trace."));
    }
    let used = |tree: &Tree| root_bytes + tree.agents.iter().map(|a| a.bytes).sum::<usize>();
    if tree.agents.len() >= MAX_AGENTS || used(&tree) + agent.bytes > MAX_TREE_BYTES {
        return Ok(AgentUploaded::Full);
    }
    let bytes =
        serde_json::to_vec(&document).map_err(|_| Error::Invalid("The trace is invalid."))?;
    match store
        .write_key(&agent_key(owner, root, &id)?, bytes, None)
        .await
    {
        Ok(_) | Err(Error::Conflict) => {}
        Err(error) => return Err(error),
    }
    let key = tree_key(owner, root)?;
    for _ in 0..6 {
        let (mut tree, generation) = read_tree(store, owner, root).await?;
        if let Some(existing) = tree.agents.iter().find(|a| a.id == id) {
            return Ok(AgentUploaded::Saved {
                agent: Box::new(existing.clone()),
                existing: true,
            });
        }
        if tree.agents.len() >= MAX_AGENTS || used(&tree) + agent.bytes > MAX_TREE_BYTES {
            return Ok(AgentUploaded::Full);
        }
        tree.agents.push(agent.clone());
        let count = tree.agents.len();
        let bytes =
            serde_json::to_vec(&tree).map_err(|_| Error::Invalid("The trace is invalid."))?;
        match store.write_key(&key, bytes, generation.as_deref()).await {
            Ok(_) => {
                let root = root.to_owned();
                update_index(store, owner, move |index| {
                    match index.traces.iter_mut().find(|trace| trace.id == root) {
                        Some(trace) if trace.agents != count => {
                            trace.agents = count;
                            (true, ())
                        }
                        _ => (false, ()),
                    }
                })
                .await?;
                return Ok(AgentUploaded::Saved {
                    agent: Box::new(agent),
                    existing: false,
                });
            }
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

/// One agent of the owner's trace and its trajectory.
pub(crate) async fn load_agent(
    store: &Store,
    owner: &str,
    root: &str,
    id: &str,
) -> Result<Option<(Agent, Value)>, Error> {
    if !valid_id(root) || !valid_id(id) {
        return Ok(None);
    }
    let Some(agent) = agents(store, owner, root)
        .await?
        .into_iter()
        .find(|agent| agent.id == id)
    else {
        return Ok(None);
    };
    let Some((bytes, _)) = store.read_key(&agent_key(owner, root, id)?).await? else {
        return Ok(None);
    };
    let document =
        serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt("The trace is invalid."))?;
    Ok(Some((agent, document)))
}

/// Delete a trace's agents and their list.
async fn delete_tree(store: &Store, owner: &str, root: &str) -> Result<(), Error> {
    let (tree, generation) = read_tree(store, owner, root).await?;
    for agent in &tree.agents {
        let key = agent_key(owner, root, &agent.id)?;
        if let Some((_, generation)) = store.read_key(&key).await? {
            match store.delete_key(&key, &generation).await {
                Ok(_) | Err(Error::Conflict) => {}
                Err(error) => return Err(error),
            }
        }
    }
    if let Some(generation) = generation {
        match store.delete_key(&tree_key(owner, root)?, &generation).await {
            Ok(_) | Err(Error::Conflict) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// A shared trace's agent, for anyone with the trace's link.
pub(crate) async fn public_agent(
    store: &Store,
    root: &str,
    id: &str,
) -> Result<Option<(Summary, Agent, Value)>, Error> {
    if !valid_id(root) || !valid_id(id) {
        return Ok(None);
    }
    let Some(owner) = shared_owner(store, root).await? else {
        return Ok(None);
    };
    let Some((summary, _)) = load(store, &owner, root)
        .await?
        .filter(|(summary, _)| summary.shared)
    else {
        return Ok(None);
    };
    Ok(load_agent(store, &owner, root, id)
        .await?
        .map(|(agent, document)| (summary, agent, document)))
}

/// The owner of a shared trace, for its public agent pages.
async fn public_agents(store: &Store, root: &str) -> Result<Vec<Agent>, Error> {
    match shared_owner(store, root).await? {
        Some(owner) => agents(store, &owner, root).await,
        None => Ok(Vec::new()),
    }
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
        "agents": trace.agents,
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

fn wire_agent(root: &str, agent: &Agent, origin: &str) -> Value {
    json!({
        "id": agent.id,
        "parent": agent.parent,
        "title": agent.title,
        "agent": agent.agent,
        "model": agent.model,
        "steps": agent.steps,
        "bytes": agent.bytes,
        "uploaded_unix": agent.uploaded_unix,
        "started_ms": agent.stats.started_ms,
        "ended_ms": agent.stats.ended_ms,
        "duration_ms": agent.stats.duration_ms(),
        "prompt_tokens": agent.stats.prompt_tokens,
        "completion_tokens": agent.stats.completion_tokens,
        "cost_usd": agent.stats.cost_usd,
        "tool_calls": agent.stats.tool_calls,
        "url": format!("{origin}{PAGE}/{root}/agents/{}", agent.id),
    })
}

fn query_value<'a>(query: Option<&'a str>, name: &str) -> Option<&'a str> {
    query?.split('&').find_map(|part| {
        part.split_once('=')
            .filter(|(key, _)| *key == name)
            .map(|(_, value)| value)
    })
}

async fn api_upload_agent(
    State(app): State<App>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    if !json_request(&headers) {
        return refused(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "invalid",
            "Send the agent's trace as application/json.",
        );
    }
    let owner = match api_owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let origin = origin(&app, &headers);
    let parent = query_value(query.as_deref(), "parent").filter(|parent| !parent.is_empty());
    match upload_agent(&app.config.chat_store, &owner, &id, parent, &body).await {
        Ok(AgentUploaded::Saved { agent, existing }) => answer(
            if existing {
                StatusCode::OK
            } else {
                StatusCode::CREATED
            },
            json!({"agent": wire_agent(&id, &agent, &origin), "existing": existing}),
        ),
        Ok(AgentUploaded::Unknown(message)) => refused(StatusCode::NOT_FOUND, "unknown", message),
        Ok(AgentUploaded::Full) => refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "full",
            "This trace has no room for more agents.",
        ),
        Ok(AgentUploaded::Refused(Uploaded::Secret(_))) => refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "secret",
            "Part of this agent's trace looks like a password or key, so it wasn't saved. coder trace upload takes those out for you.",
        ),
        Ok(AgentUploaded::Refused(Uploaded::Invalid(message))) if message == "too_large" => {
            refused(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_large",
                "This agent's trace is larger than 8 MB.",
            )
        }
        Ok(AgentUploaded::Refused(Uploaded::Invalid(message))) => {
            refused(StatusCode::BAD_REQUEST, "invalid", &message)
        }
        Ok(AgentUploaded::Refused(_)) => refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "That trace can't be read.",
        ),
        Err(error) => stored(&error),
    }
}

async fn api_agents(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let owner = match api_owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let origin = origin(&app, &headers);
    match load(&app.config.chat_store, &owner, &id).await {
        Ok(Some(_)) => {}
        Ok(None) => return refused(StatusCode::NOT_FOUND, "unknown", "No such trace."),
        Err(error) => return stored(&error),
    }
    match agents(&app.config.chat_store, &owner, &id).await {
        Ok(agents) => answer(
            StatusCode::OK,
            json!({"agents": agents.iter().map(|a| wire_agent(&id, a, &origin)).collect::<Vec<_>>()}),
        ),
        Err(error) => stored(&error),
    }
}

async fn api_read_agent(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, agent)): Path<(String, String)>,
) -> Response {
    let owner = match api_owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match load_agent(&app.config.chat_store, &owner, &id, &agent).await {
        Ok(Some((_, document))) => answer(StatusCode::OK, document),
        Ok(None) => refused(StatusCode::NOT_FOUND, "unknown", "No such agent."),
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
                                (steps_label(trace.steps))
                                @if trace.agents == 1 { " · 1 agent" }
                                @else if trace.agents > 1 { " · " (trace.agents) " agents" }
                                " · uploaded " (day(trace.uploaded_unix))
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

/// The agents a trace's steps started, by the tool call that started
/// each: where its page is and its title.
type AgentLinks = HashMap<String, (String, String)>;

fn agent_links(agents: &[Agent], base: &str) -> AgentLinks {
    agents
        .iter()
        .filter_map(|agent| {
            Some((
                agent.call.clone()?,
                (format!("{base}/agents/{}", agent.id), agent.title.clone()),
            ))
        })
        .collect()
}

/// The trace's steps, drawn plainly.
fn steps_content(document: &Value, links: &AgentLinks) -> Markup {
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
                    @let started = call.get("tool_call_id").and_then(Value::as_str).and_then(|id| links.get(id));
                    p { strong { (line(name, 80)) } }
                    @if !arguments.is_empty() && arguments != "null" && arguments != "{}" {
                        pre { code { (secret_screen::head_and_tail(&arguments, MAX_SHOWN_RESULT)) } }
                    }
                    @if let Some((href, title)) = started {
                        p { "Started " a href=(href) { (title) } }
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

/// "45s", "19m", "3h 12m", "2d 4h".
pub(crate) fn duration_words(ms: u64) -> String {
    let seconds = ms / 1000;
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m", seconds / 60),
        3600..86_400 => format!("{}h {}m", seconds / 3600, seconds % 3600 / 60),
        _ => format!("{}d {}h", seconds / 86_400, seconds % 86_400 / 3600),
    }
}

/// "950", "219K", "43.8M".
pub(crate) fn tokens_words(n: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let x = n as f64;
    match n {
        0..1000 => n.to_string(),
        1000..1_000_000 => format!("{:.0}K", x / 1e3),
        1_000_000..100_000_000 => format!("{:.1}M", x / 1e6),
        _ => format!("{:.0}M", x / 1e6),
    }
}

/// "$0.04", "under $0.01", "$12.40".
pub(crate) fn dollars(x: f64) -> String {
    if x > 0.0 && x < 0.005 {
        "under $0.01".into()
    } else {
        format!("${x:.2}")
    }
}

/// An agent's numbers in a line: model, time, tokens, cost, tools.
fn stats_words(model: &str, steps: usize, stats: &Stats) -> String {
    let mut parts = Vec::new();
    if !model.is_empty() {
        parts.push(line(model, 60));
    }
    if let Some(ms) = stats.duration_ms() {
        parts.push(duration_words(ms));
    }
    parts.push(steps_label(steps));
    let tokens = stats.prompt_tokens + stats.completion_tokens;
    if tokens > 0 {
        parts.push(format!("{} tokens", tokens_words(tokens)));
    }
    if stats.cost_usd > 0.0 {
        parts.push(dollars(stats.cost_usd));
    }
    if stats.tool_calls > 0 {
        parts.push(if stats.tool_calls == 1 {
            "1 tool call".into()
        } else {
            format!("{} tool calls", stats.tool_calls)
        });
    }
    parts.join(" · ")
}

/// One row of the tree: the trace's own conversation or an agent.
struct TreeNode<'a> {
    title: &'a str,
    model: &'a str,
    steps: usize,
    stats: &'a Stats,
    href: String,
    current: bool,
    /// Open: the conversation, and every agent above the current one.
    open: bool,
    parent: usize,
    children: Vec<usize>,
}

/// The trace's agents as a tree with a timeline: the conversation, then
/// every agent under the one that started it, each with when it ran and
/// what it spent. `base` is the trace's page (signed in or public);
/// `current` marks the agent whose page this is.
fn tree_content(
    trace: &Summary,
    root_stats: &Stats,
    agents: &[Agent],
    base: &str,
    current: Option<&str>,
) -> Markup {
    let mut nodes = vec![TreeNode {
        title: &trace.title,
        model: &trace.model,
        steps: trace.steps,
        stats: root_stats,
        href: base.to_owned(),
        current: current.is_none(),
        open: true,
        parent: 0,
        children: Vec::new(),
    }];
    let mut at: HashMap<&str, usize> = HashMap::new();
    for agent in agents {
        let index = nodes.len();
        let parent = agent
            .parent
            .as_deref()
            .and_then(|parent| at.get(parent).copied())
            .unwrap_or(0);
        nodes.push(TreeNode {
            title: &agent.title,
            model: &agent.model,
            steps: agent.steps,
            stats: &agent.stats,
            href: format!("{base}/agents/{}", agent.id),
            current: current == Some(agent.id.as_str()),
            open: false,
            parent,
            children: Vec::new(),
        });
        nodes[parent].children.push(index);
        at.insert(&agent.id, index);
    }
    if let Some(mut index) = nodes.iter().position(|node| node.current) {
        while index != 0 {
            nodes[index].open = true;
            index = nodes[index].parent;
        }
    }
    let start = nodes.iter().filter_map(|n| n.stats.started_ms).min();
    let end = nodes.iter().filter_map(|n| n.stats.ended_ms).max();
    let span = start.zip(end).filter(|(s, e)| e > s);
    let tokens: u64 = nodes
        .iter()
        .map(|n| n.stats.prompt_tokens + n.stats.completion_tokens)
        .sum();
    let cost: f64 = nodes.iter().map(|n| n.stats.cost_usd).sum();
    let mut totals = vec![if agents.len() == 1 {
        "1 agent".to_owned()
    } else {
        format!("{} agents", agents.len())
    }];
    if let Some((s, e)) = span {
        totals.push(format!("{} from start to finish", duration_words(e - s)));
    }
    if tokens > 0 {
        totals.push(format!("{} tokens", tokens_words(tokens)));
    }
    if cost > 0.0 {
        totals.push(format!("about {} in all", dollars(cost)));
    }
    html! {
        section class="oa-trace-tree" aria-labelledby="trace-agents" {
            h2 #trace-agents { "Agents" }
            p class="oa-trace-totals" { (totals.join(" · ")) }
            (tree_node(&nodes, 0, span))
        }
    }
}

fn tree_row(node: &TreeNode<'_>, span: Option<(u64, u64)>) -> Markup {
    let bar = span.zip(node.stats.started_ms).map(|((s, e), started)| {
        let whole = (e - s) as f64;
        let ended = node.stats.ended_ms.unwrap_or(started).max(started);
        #[allow(clippy::cast_precision_loss)]
        let x = (started.saturating_sub(s)) as f64 / whole * 1000.0;
        #[allow(clippy::cast_precision_loss)]
        let width = ((ended - started) as f64 / whole * 1000.0).max(4.0);
        (x.min(996.0), width.min(1000.0 - x.min(996.0)))
    });
    html! {
        span class="oa-trace-row" {
            span class="oa-trace-row-text" {
                @if node.current {
                    strong class="oa-trace-row-title" aria-current="page" { (node.title) }
                } @else {
                    a class="oa-trace-row-title" href=(node.href) { (node.title) }
                }
                span class="oa-trace-row-meta" { (stats_words(node.model, node.steps, node.stats)) }
            }
            @if let Some((x, width)) = bar {
                svg class="oa-trace-bar" viewBox="0 0 1000 8" preserveAspectRatio="none" aria-hidden="true" {
                    rect x=(format!("{x:.1}")) y="0" width=(format!("{width:.1}")) height="8" rx="2" {}
                }
            }
        }
    }
}

fn tree_node(nodes: &[TreeNode<'_>], index: usize, span: Option<(u64, u64)>) -> Markup {
    let node = &nodes[index];
    html! {
        @if node.children.is_empty() {
            div class="oa-trace-node" { (tree_row(node, span)) }
        } @else {
            details class="oa-trace-node" open[node.open] {
                summary { (tree_row(node, span)) }
                div class="oa-trace-children" {
                    @for &child in &node.children {
                        (tree_node(nodes, child, span))
                    }
                }
            }
        }
    }
}

/// A trace's page for its owner: what it is, Share and Delete, its agents,
/// the steps.
fn trace_content(
    trace: &Summary,
    document: &Value,
    agents: &[Agent],
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
                p {
                    @if agents.is_empty() {
                        "Shared: anyone with this link can see it."
                    } @else {
                        "Shared: anyone with this link can see it and its agents."
                    }
                }
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
        @if !agents.is_empty() {
            (tree_content(trace, &Stats::of(document), agents, &base, None))
        }
        (MarkdownRoot::new(steps_content(document, &agent_links(agents, &base))))
    }
}

/// A shared trace's public page.
fn public_content(trace: &Summary, document: &Value, agents: &[Agent]) -> Markup {
    let base = format!("{PUBLIC}/{}", trace.id);
    html! {
        (MarkdownRoot::new(html! {
            h1 { (trace.title) }
            p { (about(trace)) }
        }))
        @if !agents.is_empty() {
            (tree_content(trace, &Stats::of(document), agents, &base, None))
        }
        (MarkdownRoot::new(steps_content(document, &agent_links(agents, &base))))
    }
}

/// An agent's page: where it sits in the tree, then its steps. `base` is
/// its trace's page, signed in or public.
fn agent_content(
    trace: &Summary,
    root_stats: &Stats,
    agents: &[Agent],
    agent: &Agent,
    document: &Value,
    base: &str,
) -> Markup {
    let parent = agent
        .parent
        .as_deref()
        .and_then(|parent| agents.iter().find(|a| a.id == parent));
    html! {
        p class="oa-trace-crumbs" {
            a href=(base) { (trace.title) }
            @if let Some(parent) = parent {
                " › " a href=(format!("{base}/agents/{}", parent.id)) { (parent.title) }
            }
        }
        (MarkdownRoot::new(html! {
            h1 { (agent.title) }
            p { (stats_words(&agent.model, agent.steps, &agent.stats)) }
        }))
        (tree_content(trace, root_stats, agents, base, Some(&agent.id)))
        (MarkdownRoot::new(steps_content(document, &agent_links(agents, base))))
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
    let agents = match agents(&app.config.chat_store, &owner, &id).await {
        Ok(agents) => agents,
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
        &agents,
        &share_url,
        (&tickets[0].0, &tickets[0].1),
        (&tickets[1].0, &tickets[1].1),
    );
    page(&headers, service, &viewer, &trace.title, &back, body)
}

async fn agent_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, agent_id)): Path<(String, String)>,
) -> Response {
    let base = format!("{PAGE}/{id}");
    let back = format!("{base}/agents/{agent_id}");
    let (service, viewer) = match viewer(&app, &headers, &back).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let store = &app.config.chat_store;
    let found = async {
        let Some((trace, root)) = load(store, &owner, &id).await? else {
            return Ok(None);
        };
        let Some((agent, document)) = load_agent(store, &owner, &id, &agent_id).await? else {
            return Ok(None);
        };
        Ok::<_, Error>(Some((
            trace,
            root,
            agent,
            document,
            agents(store, &owner, &id).await?,
        )))
    };
    let (trace, root, agent, document, agents) = match found.await {
        Ok(Some(found)) => found,
        Ok(None) => return problem(StatusCode::NOT_FOUND, "You have no trace at that address."),
        Err(error) => return unavailable_page(&error),
    };
    let body = agent_content(&trace, &Stats::of(&root), &agents, &agent, &document, &base);
    page(&headers, service, &viewer, &agent.title, &back, body)
}

fn public_response(headers: &HeaderMap, title: &str, path: &str, body: Markup) -> Response {
    let mut response = UiPage::new(title.to_owned())
        .path(path)
        .scriptless()
        .content(PageColumn::new(body))
        .respond(headers);
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

fn public_missing() -> Response {
    crate::layout::problem(
        StatusCode::NOT_FOUND,
        "Not found",
        "Nothing on this site has that address.",
        ("/", "Home"),
    )
}

fn public_unavailable(error: &Error) -> Response {
    eprintln!("openagents-web: traces: {error}");
    crate::layout::problem(
        StatusCode::SERVICE_UNAVAILABLE,
        "Trace",
        "This trace can't be read right now. Try again in a minute.",
        ("/", "Home"),
    )
}

async fn public_agent_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, agent_id)): Path<(String, String)>,
) -> Response {
    let store = &app.config.chat_store;
    let found = async {
        let Some((trace, agent, document)) = public_agent(store, &id, &agent_id).await? else {
            return Ok(None);
        };
        let Some((_, root)) = public(store, &id).await? else {
            return Ok(None);
        };
        Ok::<_, Error>(Some((
            trace,
            root,
            agent,
            document,
            public_agents(store, &id).await?,
        )))
    };
    match found.await {
        Ok(Some((trace, root, agent, document, agents))) => {
            let base = format!("{PUBLIC}/{id}");
            let body = agent_content(&trace, &Stats::of(&root), &agents, &agent, &document, &base);
            public_response(
                &headers,
                &agent.title,
                &format!("{base}/agents/{agent_id}"),
                body,
            )
        }
        Ok(None) => public_missing(),
        Err(error) => public_unavailable(&error),
    }
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
    let store = &app.config.chat_store;
    let found = async {
        let Some((trace, document)) = public(store, &id).await? else {
            return Ok(None);
        };
        Ok::<_, Error>(Some((trace, document, public_agents(store, &id).await?)))
    };
    match found.await {
        Ok(Some((trace, document, agents))) => public_response(
            &headers,
            &trace.title,
            &format!("{PUBLIC}/{id}"),
            public_content(&trace, &document, &agents),
        ),
        Ok(None) => public_missing(),
        Err(error) => public_unavailable(&error),
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
            &[],
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
            &[],
            "https://openagents.com/trace/x",
            ("a", "b"),
            ("c", "d"),
        )
        .into_string();
        assert!(
            shared.contains(">Stop sharing<") && shared.contains("https://openagents.com/trace/x")
        );
        let public = public_content(&summary, &document, &[]).into_string();
        crate::copy_guard::assert_plain("/trace/x", &public);
        assert!(!public.contains("Delete"));
    }

    fn agent_trace(title: &str, call: &str, start: &str, end: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "schema_version": "ATIF-v1.8",
            "session_id": title,
            "agent": {"name": "claude-code", "version": "2", "model_name": "claude-sonnet-5-5"},
            "steps": [
                {"step_id": 1, "source": "user", "message": format!("Do {title}"), "timestamp": start},
                {"step_id": 2, "source": "agent", "message": "Done.", "timestamp": end,
                 "metrics": {"prompt_tokens": 1000, "completion_tokens": 50, "cost_usd": 0.25},
                 "tool_calls": [{"tool_call_id": format!("{title}-c"), "function_name": "Bash", "arguments": {}}]}
            ],
            "extra": {"title": title, "parent_tool_call_id": call},
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn a_trace_keeps_a_tree_of_agents() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        let me = owner();
        let Uploaded::Saved { trace: root, .. } =
            upload(&store, &me, &trace("Fan out"), false).await.unwrap()
        else {
            panic!("not saved");
        };
        // Unknown trace, unknown parent, another account.
        let a = agent_trace("a", "c1", "2026-10-09T10:00:00Z", "2026-10-09T10:10:00Z");
        let unknown = trace_id(&me, "nope");
        assert!(matches!(
            upload_agent(&store, &me, &unknown, None, &a).await.unwrap(),
            AgentUploaded::Unknown(_)
        ));
        assert!(matches!(
            upload_agent(&store, &me, &root.id, Some(&unknown), &a)
                .await
                .unwrap(),
            AgentUploaded::Unknown(_)
        ));
        assert!(matches!(
            upload_agent(&store, &account_owner("acct_two"), &root.id, None, &a)
                .await
                .unwrap(),
            AgentUploaded::Unknown(_)
        ));
        let github = format!("ghp_{}", "Z9".repeat(18));
        assert!(matches!(
            upload_agent(&store, &me, &root.id, None, &trace(&github))
                .await
                .unwrap(),
            AgentUploaded::Refused(Uploaded::Secret(_))
        ));
        let mut ids = Vec::new();
        for (title, call) in [("a", "c1"), ("b", "c2"), ("c", "c3")] {
            let AgentUploaded::Saved { agent, existing } = upload_agent(
                &store,
                &me,
                &root.id,
                None,
                &agent_trace(title, call, "2026-10-09T10:00:00Z", "2026-10-09T10:10:00Z"),
            )
            .await
            .unwrap() else {
                panic!("not saved");
            };
            assert!(!existing);
            assert_eq!(agent.stats.duration_ms(), Some(600_000));
            assert_eq!(agent.stats.prompt_tokens, 1000);
            ids.push(agent.id);
        }
        // Again: saved once.
        let AgentUploaded::Saved { agent, existing } =
            upload_agent(&store, &me, &root.id, None, &a).await.unwrap()
        else {
            panic!("not saved");
        };
        assert!(existing && agent.id == ids[0]);
        // A nested agent under the first.
        let AgentUploaded::Saved { agent: deep, .. } = upload_agent(
            &store,
            &me,
            &root.id,
            Some(&ids[0]),
            &agent_trace(
                "deep",
                "a-c",
                "2026-10-09T10:02:00Z",
                "2026-10-09T10:03:00Z",
            ),
        )
        .await
        .unwrap() else {
            panic!("not saved");
        };
        let tree = agents(&store, &me, &root.id).await.unwrap();
        assert_eq!(tree.len(), 4);
        assert_eq!(tree[3].parent.as_deref(), Some(ids[0].as_str()));
        assert_eq!(list(&store, &me).await.unwrap()[0].agents, 4);
        assert!(
            load_agent(&store, &me, &root.id, &deep.id)
                .await
                .unwrap()
                .is_some()
        );
        // Private until the trace is shared; then each agent is too.
        assert!(
            public_agent(&store, &root.id, &deep.id)
                .await
                .unwrap()
                .is_none()
        );
        set_shared(&store, &me, &root.id, true).await.unwrap();
        assert!(
            public_agent(&store, &root.id, &deep.id)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(public_agents(&store, &root.id).await.unwrap().len(), 4);
        // Deleting the trace takes its agents with it.
        assert!(delete(&store, &me, &root.id).await.unwrap());
        assert!(agents(&store, &me, &root.id).await.unwrap().is_empty());
        assert!(
            public_agent(&store, &root.id, &deep.id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .read_key(&agent_key(&me, &root.id, &deep.id).unwrap())
                .await
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn the_tree_shows_each_agent_with_its_time_and_cost() {
        let (document, mut summary) = check(&trace("Fan out")).unwrap();
        summary.id = trace_id(&owner(), &summary.digest);
        let agent = |id: &str, parent: Option<&str>, call: &str, start: u64, end: u64| Agent {
            id: trace_id(&owner(), id),
            parent: parent.map(|p| trace_id(&owner(), p)),
            title: format!("Agent {id}"),
            agent: "claude-code".into(),
            model: "claude-sonnet-5-5".into(),
            steps: 4,
            bytes: 10,
            digest: id.into(),
            uploaded_unix: 1,
            call: Some(call.into()),
            stats: Stats {
                started_ms: Some(start),
                ended_ms: Some(end),
                prompt_tokens: 219_000,
                completion_tokens: 523,
                cost_usd: 1.2,
                tool_calls: 85,
            },
        };
        let agents = vec![
            agent("a", None, "c1", 0, 1_140_000),
            agent("b", None, "c2", 60_000, 600_000),
            agent("c", None, "c3", 120_000, 3_600_000),
            agent("d", Some("a"), "x", 300_000, 400_000),
        ];
        let base = format!("{PAGE}/{}", summary.id);
        let page = trace_content(
            &summary,
            &document,
            &agents,
            "https://openagents.com/trace/x",
            ("a", "b"),
            ("c", "d"),
        )
        .into_string();
        crate::copy_guard::assert_plain(&base, &page);
        assert!(
            page.contains("4 agents · 1h 0m from start to finish"),
            "{page}"
        );
        for agent in &agents {
            assert!(page.contains(&format!("{base}/agents/{}", agent.id)));
        }
        assert!(
            page.contains(
                "claude-sonnet-5-5 · 19m · 4 steps · 220K tokens · $1.20 · 85 tool calls"
            )
        );
        assert_eq!(page.matches("<rect").count(), 4, "one bar per timed agent");
        // The conversation's three agents sit directly under it, the fourth under the first.
        assert_eq!(page.matches("<details class=\"oa-trace-node\"").count(), 2);
        // The call that started an agent links to it.
        assert!(page.contains(&format!(
            "Started <a href=\"{base}/agents/{}\">Agent a</a>",
            agents[0].id
        )));
        let page = agent_content(
            &summary,
            &Stats::default(),
            &agents,
            &agents[3],
            &document,
            &format!("{PUBLIC}/{}", summary.id),
        )
        .into_string();
        crate::copy_guard::assert_plain("/trace/x/agents/y", &page);
        assert!(page.contains("aria-current=\"page\">Agent d<"));
        assert!(page.contains(">Agent a<"), "the parent is linked");
        assert_eq!(
            page.matches("<details class=\"oa-trace-node\" open")
                .count(),
            2
        );
        let public = public_content(&summary, &document, &agents).into_string();
        assert!(public.contains(&format!("{PUBLIC}/{}/agents/", summary.id)));
        assert!(!public.contains(PAGE));
        assert_eq!(duration_words(45_000), "45s");
        assert_eq!(duration_words(90_000_000), "1d 1h");
        assert_eq!(tokens_words(43_800_000), "43.8M");
        assert_eq!(dollars(0.001), "under $0.01");
    }
}
