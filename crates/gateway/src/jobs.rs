//! Durable classification jobs: accepted manifests, a per-job item
//! ledger, resumable status, and authorized result export — the
//! storage and execution half of `docs/decision-models/service/durable-jobs.md`.
//!
//! A job is a `POST /v1/classify` request the gateway accepts, persists,
//! and executes outside the submitting connection's lifetime. The
//! manifest and status land on disk before the acknowledgement; every
//! item's result appends to the job's ledger as it lands; a restart
//! reconciles what it finds rather than claiming work it cannot prove.
//! Execution re-enters the same admission sequence the synchronous
//! route runs — `classify_run` — under the job's own request identity.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, watch};

use receipts::execution::{Outcome, digest_request};
use tenancy::Registry;

use crate::serve::{
    Caller, Cancellation, Context, ItemResult, Naming, PhaseAuth, ReceiptContext, ServeState,
    Verdict, authenticate, classify_run, now_utc, parse_classify, validate_classify, write_receipt,
};

/// The schema tag every job document carries.
pub const SCHEMA: &str = "openagents.job.v1";

/// The schema tag a results export carries.
const RESULTS_SCHEMA: &str = "openagents.job-results.v1";

/// The schema tag a notification event carries.
const EVENT_SCHEMA: &str = "openagents.job-event.v1";

/// The idempotency index — one line per accepted key.
const INDEX: &str = "index.jsonl";

/// The durable cancellation marker — its presence is the intent, so a
/// cancel never rewrites a status the runner may be writing.
const CANCEL_MARKER: &str = "cancelled";

/// The most items one export page returns.
const PAGE_MAX: u64 = 1_000;

/// The default page size when `limit` is absent.
const PAGE_DEFAULT: u64 = 100;

/// The most delivery attempts one event earns.
const DELIVERY_MAX: u32 = 4;

/// The backoff between delivery attempts, indexed by attempt number.
const DELIVERY_BACKOFF_MS: [u64; 3] = [250, 1_000, 4_000];

type HmacSha256 = Hmac<Sha256>;

/// The states a job's status document reports.
const QUEUED: &str = "queued";
const RUNNING: &str = "running";
const CANCELLING: &str = "cancelling";
const COMPLETED: &str = "completed";
const CANCELLED: &str = "cancelled";
const FAILED: &str = "failed";

// ---------- routes ----------

/// `POST /v1/jobs`: accept a durable classification job. The request is
/// validated against the same checks the synchronous route runs, the
/// manifest and status are persisted, and only then is the job
/// acknowledged — the idempotency key binds tenant and content, so a
/// replay names the same job and a changed payload conflicts.
pub(crate) async fn submit(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Ok(envelope) = serde_json::from_slice::<Value>(&body) else {
        return job_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The request body isn't valid JSON.",
            None,
        );
    };
    if envelope.get("v").and_then(Value::as_str) != Some(SCHEMA) {
        return job_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_request",
            "Set `v` to `openagents.job.v1`.",
            None,
        );
    }
    if envelope.get("kind").and_then(Value::as_str) != Some("classify") {
        return job_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported_kind",
            "Set `kind` to `classify`. It's the only job kind.",
            None,
        );
    }
    let Some(request_v) = envelope.get("request").filter(|value| value.is_object()) else {
        return job_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_request",
            "Add a `request` object that holds the classify request.",
            None,
        );
    };
    let notify = match envelope.get("notify") {
        None | Some(Value::Null) => None,
        Some(notify) => match validate_notify(notify) {
            Ok(notify) => Some(notify),
            Err(message) => {
                return job_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_notify",
                    &message,
                    None,
                );
            }
        },
    };
    let (registry, caller) = match authenticate(&state, &headers) {
        Ok(parts) => parts,
        Err((status, code, message)) => {
            return job_error(status, code, &message, None);
        }
    };
    // The submission runs the synchronous route's parse and validation —
    // a request it would refuse is refused here identically, and
    // nothing is persisted.
    let request_bytes = Bytes::from(serde_json::to_vec(request_v).unwrap_or_default());
    let mut ctx: Context = Box::new(ReceiptContext {
        tenant_ref: caller.tenant.as_ref().map(|_| caller.key.clone()),
        ..ReceiptContext::default()
    });
    let idem = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("req_{}", hex(&secp256k1::rand::random::<[u8; 16]>())));
    let request = match parse_classify(&request_bytes, ctx.clone()) {
        Ok(request) => request,
        Err(verdict) => return refusal_response(verdict, &idem),
    };
    if let Err(verdict) = validate_classify(&state, &registry, &caller, &request, &mut ctx) {
        return refusal_response(verdict, &idem);
    }

    // The content digest binds the idempotency key to what it named —
    // a replay with identical content finds the same job; a changed
    // payload conflicts.
    let digest = digest_request(&json!({
        "request": request_v,
        "notify": notify.as_ref().map(|notify| notify.public()).unwrap_or(Value::Null),
    }));
    let scope = caller.key.clone();
    let job = job_id(&scope, &idem, &digest);
    match index_lookup(&state.dir, &scope, &idem) {
        Some(entry) if entry.digest == digest => {
            // The identical replay: the job's current status, not a
            // second execution. A deleted job is gone — the index
            // remembers the submission, the record does not.
            match load_status(&state.dir, &entry.job) {
                Some(status) => {
                    return Json(view_status(&job_dir(&state.dir, &entry.job), status))
                        .into_response();
                }
                None => {
                    return job_error(
                        StatusCode::GONE,
                        "job_deleted",
                        "This job was deleted, so the service doesn't run it again. Use a new `Idempotency-Key` to submit it again.",
                        Some(&entry.job),
                    );
                }
            }
        }
        Some(_) => {
            return job_error(
                StatusCode::CONFLICT,
                "idempotency_conflict",
                "This `Idempotency-Key` was already used for a different request. Use a new key.",
                None,
            );
        }
        None => {}
    }

    sweep(&state);
    let submitted = now_utc();
    let manifest = json!({
        "v": SCHEMA,
        "job": job,
        "kind": "classify",
        "tenant": caller.tenant,
        "key": caller.key,
        "workspace": caller.workspace,
        "scopes": caller.scopes,
        "idem": idem,
        "digest": digest,
        "submitted_at": submitted,
        "model": request.model,
        "request": request_v,
        "request_digest": digest_request(request_v),
        "notify": notify.as_ref().map(|notify| notify.stored()),
    });
    let status = json!({
        "v": SCHEMA,
        "job": job,
        "kind": "classify",
        "status": QUEUED,
        "model": request.model,
        "capacity": request.capacity,
        "counts": counts_json(&Counts::queued(request.inputs.len() as u64)),
        "submitted_at": submitted,
        "started_at": Value::Null,
        "finished_at": Value::Null,
        "receipt": Value::Null,
        "cause": Value::Null,
    });
    let dir = job_dir(&state.dir, &job);
    if let Err(message) = persist_new(&dir, &manifest, &status) {
        return job_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "job_unavailable",
            &message,
            None,
        );
    }
    // The index entry completes acceptance — written after the manifest
    // so a crash leaves an orphaned dir recovery removes, never an
    // index pointing at nothing.
    if let Err(message) = index_append(&state.dir, &scope, &idem, &job, &digest) {
        let _ = fs::remove_dir_all(&dir);
        return job_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "job_unavailable",
            &message,
            None,
        );
    }
    tokio::spawn(run(state.clone(), job.clone()));
    let mut response = status;
    if let Some(notify) = &notify
        && notify.generated
    {
        response["secret"] = json!(notify.secret);
    }
    (StatusCode::ACCEPTED, Json(response)).into_response()
}

/// `GET /v1/jobs/{id}`: the job's current status document — the same
/// shape the submission returned, live.
pub(crate) async fn status(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let Some((_manifest, status)) = load_owned(&state, &headers, &id) else {
        return not_found(&id);
    };
    Json(view_status(&job_dir(&state.dir, &id), status)).into_response()
}

/// `POST /v1/jobs/{id}/cancel`: record the cancel durably — a marker
/// beside the status, never a rewrite of it, so a terminal write racing
/// the cancel is not clobbered — then signal the runner. A terminal job
/// returns its status — cancellation is idempotent, never an error.
pub(crate) async fn cancel(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let Some((_manifest, status)) = load_owned(&state, &headers, &id) else {
        return not_found(&id);
    };
    if terminal(&status) {
        return Json(status).into_response();
    }
    // The durable record leads the signal: a crash between them leaves
    // the marker on disk, which recovery resolves to `cancelled`.
    if let Err(trouble) = fs::write(job_dir(&state.dir, &id).join(CANCEL_MARKER), b"") {
        return job_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "job_unavailable",
            &trouble.to_string(),
            Some(&id),
        );
    }
    if let Some(sender) = state.job_cancels.lock().await.get(&id) {
        let _ = sender.send(true);
    }
    let mut status = status;
    status["status"] = json!(CANCELLING);
    (StatusCode::ACCEPTED, Json(status)).into_response()
}

/// `GET /v1/jobs/{id}/results`: the finished items in completion
/// order — each line of the item ledger with its index, attempt id,
/// dispatch flag, and the per-input result the pipeline produced.
pub(crate) async fn results(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let Some((_manifest, status)) = load_owned(&state, &headers, &id) else {
        return not_found(&id);
    };
    let status = view_status(&job_dir(&state.dir, &id), status);
    let limit = params
        .get("limit")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|limit| *limit > 0)
        .unwrap_or(PAGE_DEFAULT)
        .min(PAGE_MAX);
    let offset = match params.get("cursor") {
        None => 0,
        Some(cursor) => match parse_cursor(cursor) {
            Some((issued, offset)) => {
                if now_ms().saturating_sub(issued) > state.config.job_cursor_ttl_ms {
                    return job_error(
                        StatusCode::GONE,
                        "cursor_expired",
                        "This cursor has expired. Read the results again from the start.",
                        Some(&id),
                    );
                }
                offset
            }
            None => {
                return job_error(
                    StatusCode::BAD_REQUEST,
                    "bad_cursor",
                    "This cursor isn't valid. Use the `next_cursor` value from the previous page.",
                    Some(&id),
                );
            }
        },
    };
    let items = read_items(&job_dir(&state.dir, &id));
    let total = items.len() as u64;
    let page: Vec<&Value> = items
        .iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();
    let next = offset + page.len() as u64;
    // A cursor is issued while more items exist or the job still runs —
    // `terminal` is how the caller knows the last page is the last.
    let next_cursor = if next < total || !terminal(&status) {
        json!(format!("{}:{next}", now_ms()))
    } else {
        Value::Null
    };
    Json(json!({
        "v": RESULTS_SCHEMA,
        "job": id,
        "items": page,
        "next_cursor": next_cursor,
        "terminal": status["status"],
        "counts": status["counts"],
    }))
    .into_response()
}

/// `DELETE /v1/jobs/{id}`: remove a terminal job's record — manifest,
/// status, items, events, and deliveries. The index entry stays: a late
/// replay learns the record is gone rather than running again.
pub(crate) async fn remove(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let Some((_manifest, status)) = load_owned(&state, &headers, &id) else {
        return not_found(&id);
    };
    if !terminal(&status) {
        return job_error(
            StatusCode::CONFLICT,
            "job_running",
            "You can't delete a running job. Cancel it first.",
            Some(&id),
        );
    }
    if let Err(trouble) = fs::remove_dir_all(job_dir(&state.dir, &id)) {
        return job_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "job_unavailable",
            &trouble.to_string(),
            Some(&id),
        );
    }
    Json(json!({"deleted": true, "job": id})).into_response()
}

/// `POST /v1/jobs/{id}/notify/rotate`: replace the notification secret.
/// The new secret answers once; events after the rotation sign under it
/// and the old one stops working.
pub(crate) async fn rotate_notify(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let Some((mut manifest, _status)) = load_owned(&state, &headers, &id) else {
        return not_found(&id);
    };
    if manifest["notify"].is_null() {
        return job_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "notify_not_configured",
            "This job has no `notify` URL, so there are no notifications to send.",
            Some(&id),
        );
    }
    let secret = mint_secret();
    manifest["notify"]["secret"] = json!(secret);
    manifest["notify"]["rotated"] = json!(manifest["notify"]["rotated"].as_u64().unwrap_or(0) + 1);
    if let Err(message) =
        write_json_atomic(&job_dir(&state.dir, &id).join("manifest.json"), &manifest)
    {
        return job_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "job_unavailable",
            &message,
            Some(&id),
        );
    }
    Json(json!({"job": id, "secret": secret})).into_response()
}

// ---------- the runner ----------

/// One job's execution: re-validate the persisted request — bindings
/// may have moved since submission — then run the same admission
/// sequence the synchronous route runs, under the job's own request
/// identity, writing each item's result to the ledger as it lands.
/// Nothing about the run borrows the submitting connection.
async fn run(state: Arc<ServeState>, id: String) {
    let dir = job_dir(&state.dir, &id);
    let Some(manifest) = read_json(&dir.join("manifest.json")) else {
        return;
    };
    let Some(mut status) = read_json(&dir.join("status.json")) else {
        return;
    };
    // The cancel sender registers before the run starts so a cancel
    // landing between submission and dispatch is not lost.
    let (sender, receiver) = watch::channel(false);
    state
        .job_cancels
        .lock()
        .await
        .insert(id.clone(), sender.clone());
    let cancellation = Cancellation(receiver);
    if cancel_requested(&dir) || status["status"].as_str() == Some(CANCELLING) {
        let _ = sender.send(true);
    }

    let started = Instant::now();
    status["status"] = json!(RUNNING);
    status["started_at"] = json!(now_utc());
    let _ = write_status(&state.dir, &id, &status);

    let caller = Caller {
        tenant: manifest["tenant"].as_str().map(str::to_string),
        key: manifest["key"].as_str().unwrap_or_default().to_string(),
        workspace: manifest["workspace"].as_str().map(str::to_string),
        scopes: serde_json::from_value(manifest["scopes"].clone()).unwrap_or_default(),
    };
    let ctx: Context = Box::new(ReceiptContext {
        tenant_ref: caller.tenant.as_ref().map(|_| caller.key.clone()),
        ..ReceiptContext::default()
    });
    let request_v = manifest["request"].clone();
    let request_bytes = Bytes::from(serde_json::to_vec(&request_v).unwrap_or_default());
    let request_digest = manifest["request_digest"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let naming = Naming {
        request: &id,
        attempt: 1,
        attempt_id: format!("{id}-1-{}", state.mint()),
        request_digest: &request_digest,
    };

    // The sink drains item results into the job's ledger — each line
    // durable before the next item is reported.
    let (sink, receiver) = mpsc::unbounded_channel::<ItemResult>();
    let writer = tokio::spawn(ledger_writer(dir.clone(), receiver));

    let mut ctx_for_run = ctx;
    let verdict = match Registry::open(&state.dir) {
        Ok(registry) => match parse_classify(&request_bytes, ctx_for_run.clone()) {
            Ok(request) => {
                match validate_classify(&state, &registry, &caller, &request, &mut ctx_for_run) {
                    Ok(validated) => {
                        classify_run(
                            &state,
                            &registry,
                            &caller,
                            &request,
                            validated,
                            &request_bytes,
                            &naming,
                            &cancellation,
                            ctx_for_run,
                            started,
                            &PhaseAuth::Stored,
                            Some(sink),
                        )
                        .await
                    }
                    Err(verdict) => verdict,
                }
            }
            Err(verdict) => verdict,
        },
        Err(trouble) => Verdict::Refused {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "registry_unavailable",
            message: trouble.to_string(),
            outcome: Outcome::Unavailable,
            ctx: ctx_for_run,
        },
    };
    let _ = writer.await;

    // Terminal resolution: a cancel request wins over whatever the
    // admission sequence resolved — the caller asked for the end it
    // got. Otherwise a forwarded run completed and a refused one failed.
    let cancelled = cancellation.stopped() || cancel_requested(&dir);
    let forwarded = matches!(verdict, Verdict::Forwarded { .. });
    let (terminal_state, outcome, cause, ctx) = match verdict {
        Verdict::Forwarded {
            body,
            outcome,
            cause,
            ctx,
            ..
        } => {
            write_result(&dir, &body);
            let state = if cancelled { CANCELLED } else { COMPLETED };
            let cause = if cancelled {
                Some("cancelled".to_string())
            } else {
                cause
            };
            (state, outcome, cause, ctx)
        }
        Verdict::Refused {
            code, outcome, ctx, ..
        } => {
            let state = if cancelled { CANCELLED } else { FAILED };
            (state, outcome, Some(code.to_string()), ctx)
        }
    };
    // A forwarded run reconciles its missing lines as `unknown` — a
    // task that never reported may have dispatched. A refused run
    // dispatched nothing; its missing inputs are `unattempted`.
    let expected = manifest["request"]["inputs"].as_array().map_or(0, Vec::len) as u64;
    let counts = recount(&dir, expected, forwarded);
    let receipt = write_receipt(&state, &naming, outcome, cause.as_deref(), started, &ctx).await;

    let mut status = read_json(&dir.join("status.json")).unwrap_or(status);
    status["status"] = json!(terminal_state);
    status["finished_at"] = json!(now_utc());
    status["counts"] = counts_json(&counts);
    status["receipt"] = json!(receipt);
    status["cause"] = json!(cause);
    let _ = write_status(&state.dir, &id, &status);
    state.job_cancels.lock().await.remove(&id);

    if !manifest["notify"].is_null() {
        enqueue_event(&dir, &id, &format!("job.{terminal_state}"), &counts);
        tokio::spawn(deliver(state.clone(), id));
    }
}

/// Append each finished item to the job's ledger as it lands — the
/// record a restart reconciles, flushed before the next result is
/// accepted.
async fn ledger_writer(dir: PathBuf, mut receiver: mpsc::UnboundedReceiver<ItemResult>) {
    let path = dir.join("items.jsonl");
    let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    while let Some(result) = receiver.recv().await {
        let line = json!({
            "index": result.index,
            "attempt_id": result.attempt_id,
            "dispatched": result.dispatched,
            "queue_ms": result.queue_ms,
            "usage": result.usage,
            "item": result.item,
        });
        if writeln!(file, "{line}").is_err() || file.sync_data().is_err() {
            return;
        }
    }
}

/// The assembled response the run produced — the job's full result
/// document, kept beside the per-item ledger for the record.
fn write_result(dir: &Path, body: &Bytes) {
    let _ = fs::write(dir.join("result.json"), body);
}

// ---------- recovery ----------

/// Startup reconciliation: remove orphaned job dirs a crashed
/// submission left behind, and end every interrupted run honestly — a
/// missing item outcome is `unknown`, never guessed. Queued jobs stay
/// queued: nothing dispatched, nothing is ambiguous, and `resume`
/// re-spawns their runners once the runtime is up.
pub(crate) fn recover(dir: &Path, retention_ms: u64) {
    let root = dir.join("jobs");
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest = path.join("manifest.json");
        let status_path = path.join("status.json");
        if !manifest.exists() || !status_path.exists() {
            // An accepted job always has both; a dir without them was a
            // submission that died before its index write — never
            // acknowledged, safe to remove.
            let _ = fs::remove_dir_all(&path);
            continue;
        }
        let Some(status) = read_json(&status_path) else {
            continue;
        };
        if let Some(RUNNING | CANCELLING) = status["status"].as_str() {
            // The run died with the process: recorded outcomes
            // stand, unrecorded inputs are `unknown`, and the job
            // ends as it actually ended.
            let cancelling =
                status["status"].as_str() == Some(CANCELLING) || cancel_requested(&path);
            let expected = status["counts"]["expected"].as_u64().unwrap_or(0);
            let mut status = status;
            status["counts"] = counts_json(&recount(&path, expected, true));
            status["finished_at"] = json!(now_utc());
            if cancelling {
                status["status"] = json!(CANCELLED);
                status["cause"] = json!("cancelled");
            } else {
                status["status"] = json!(FAILED);
                status["cause"] = json!("gateway_restart");
            }
            let _ = write_status_at(&status_path, &status);
        }
    }
    sweep_at(dir, retention_ms);
}

/// Re-spawn the runner for every job recovery left `queued`, and
/// re-enter every undelivered notification in the delivery queue —
/// called once the router is built, inside the runtime.
pub(crate) fn resume(state: &Arc<ServeState>) {
    let root = state.dir.join("jobs");
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(status) = read_json(&path.join("status.json")) else {
            continue;
        };
        if status["status"].as_str() == Some(QUEUED)
            && let Some(id) = status["job"].as_str()
        {
            tokio::spawn(run(state.clone(), id.to_string()));
        }
        let Some(manifest) = read_json(&path.join("manifest.json")) else {
            continue;
        };
        if manifest["notify"].is_null() {
            continue;
        }
        let delivered = delivered_events(&path);
        let pending = read_lines(&path.join("events.jsonl"))
            .iter()
            .filter_map(|line| line.get("id").and_then(Value::as_str))
            .any(|event| !delivered.contains(event));
        if pending && let Some(id) = manifest["job"].as_str() {
            tokio::spawn(deliver(state.clone(), id.to_string()));
        }
    }
}

// ---------- webhooks ----------

/// The notification a terminal job emits — one event per terminal
/// transition, recorded before it is delivered.
fn enqueue_event(dir: &Path, job: &str, event_type: &str, counts: &Counts) {
    let seq = read_lines(&dir.join("events.jsonl")).len() + 1;
    let event = json!({
        "v": EVENT_SCHEMA,
        "id": format!("{job}-{seq}"),
        "type": event_type,
        "job": job,
        "at": now_utc(),
        "counts": counts_json(counts),
    });
    append_line(&dir.join("events.jsonl"), &event);
}

/// Deliver a job's undelivered events to its declared destination —
/// signed, bounded, and recorded attempt by attempt. A `2xx` completes
/// the event; anything else retries inside `DELIVERY_MAX`, and the
/// receiver deduplicates on `x-openagents-event`.
async fn deliver(state: Arc<ServeState>, job: String) {
    let dir = job_dir(&state.dir, &job);
    let Some(manifest) = read_json(&dir.join("manifest.json")) else {
        return;
    };
    let (Some(url), Some(secret)) = (
        manifest["notify"]["url"].as_str(),
        manifest["notify"]["secret"].as_str(),
    ) else {
        return;
    };
    for event in read_lines(&dir.join("events.jsonl")) {
        let Some(event_id) = event.get("id").and_then(Value::as_str) else {
            continue;
        };
        let (mut attempts, mut done) = delivery_state(&dir, event_id);
        let body = serde_json::to_vec(&event).unwrap_or_default();
        let signature = sign(secret, event_id, &body);
        while !done && attempts < DELIVERY_MAX {
            attempts += 1;
            let response = state
                .client
                .post(url)
                .header("x-openagents-event", event_id)
                .header("x-openagents-signature", format!("sha256={signature}"))
                .header("content-type", "application/json")
                .body(body.clone())
                .send()
                .await;
            let (outcome, detail) = match &response {
                Ok(response) if response.status().is_success() => {
                    done = true;
                    ("delivered", response.status().as_u16().to_string())
                }
                Ok(response) => ("failed", response.status().as_u16().to_string()),
                Err(trouble) => ("failed", trouble.to_string()),
            };
            append_line(
                &dir.join("deliveries.jsonl"),
                &json!({
                    "event": event_id,
                    "attempt": attempts,
                    "at": now_utc(),
                    "outcome": outcome,
                    "detail": detail,
                }),
            );
            if !done && attempts < DELIVERY_MAX {
                let backoff = DELIVERY_BACKOFF_MS
                    .get(attempts as usize - 1)
                    .copied()
                    .unwrap_or(4_000);
                tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
            }
        }
    }
}

/// What the delivery log says about one event: attempts made and
/// whether a `2xx` landed.
fn delivery_state(dir: &Path, event: &str) -> (u32, bool) {
    let mut attempts = 0;
    let mut done = false;
    for line in read_lines(&dir.join("deliveries.jsonl")) {
        if line["event"].as_str() == Some(event) {
            attempts += 1;
            done |= line["outcome"].as_str() == Some("delivered");
        }
    }
    (attempts, done)
}

/// The event ids that already delivered — a restart does not resend.
fn delivered_events(dir: &Path) -> std::collections::BTreeSet<String> {
    read_lines(&dir.join("deliveries.jsonl"))
        .iter()
        .filter(|line| line["outcome"].as_str() == Some("delivered"))
        .filter_map(|line| line["event"].as_str().map(str::to_string))
        .collect()
}

/// `sha256=` HMAC over the event id and body — what the receiver
/// verifies against the secret it holds.
fn sign(secret: &str, event_id: &str, body: &[u8]) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret.as_bytes())
        .unwrap_or_else(|_| unreachable!("hmac accepts any key length"));
    mac.update(event_id.as_bytes());
    mac.update(b".");
    mac.update(body);
    hex(&mac.finalize().into_bytes())
}

/// The notify declaration, validated: `https`, or `http` to a loopback
/// address, and never a URL carrying credentials.
struct Notify {
    url: String,
    secret: String,
    /// The secret was minted here — the submission response returns it
    /// once; a caller-supplied secret is never echoed.
    generated: bool,
}

impl Notify {
    /// The notify shape the content digest covers — url only, so a
    /// secret change under the same key is still the same submission.
    fn public(&self) -> Value {
        json!({"url": self.url})
    }

    /// The stored shape — url, secret, and rotation count.
    fn stored(&self) -> Value {
        json!({"url": self.url, "secret": self.secret, "rotated": 0})
    }
}

fn validate_notify(notify: &Value) -> Result<Notify, String> {
    let url = notify["url"]
        .as_str()
        .ok_or_else(|| "The `notify` object needs a `url`.".to_string())?;
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| format!("The `notify` URL `{url}` isn't a valid URL."))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("The `notify` URL can't include a user name or password.".to_string());
    }
    match parsed.scheme() {
        "https" => {}
        "http" => {
            let loopback = parsed
                .host_str()
                .map(|host| {
                    host.eq_ignore_ascii_case("localhost")
                        || host
                            .parse::<std::net::IpAddr>()
                            .map(|ip| ip.is_loopback())
                            .unwrap_or(false)
                })
                .unwrap_or(false);
            if !loopback {
                return Err("An `http` `notify` URL must point to this machine, such as `127.0.0.1`. Use `https` for other hosts.".to_string());
            }
        }
        scheme => {
            return Err(format!(
                "The `notify` URL must use `https`, not `{scheme}`."
            ));
        }
    }
    let provided = notify["secret"].as_str().map(str::to_string);
    Ok(Notify {
        url: url.to_string(),
        generated: provided.is_none(),
        secret: provided.unwrap_or_else(mint_secret),
    })
}

// ---------- counts ----------

/// The outcome tallies every status document reports.
#[derive(Clone, Default)]
struct Counts {
    expected: u64,
    attempted: u64,
    answered: u64,
    refused: u64,
    unavailable: u64,
    unattempted: u64,
    unknown: u64,
}

impl Counts {
    fn queued(expected: u64) -> Self {
        Self {
            expected,
            ..Self::default()
        }
    }
}

fn counts_json(counts: &Counts) -> Value {
    json!({
        "expected": counts.expected,
        "attempted": counts.attempted,
        "answered": counts.answered,
        "refused": counts.refused,
        "unavailable": counts.unavailable,
        "unattempted": counts.unattempted,
        "unknown": counts.unknown,
    })
}

/// Tally the item ledger: recorded outcomes count under their own
/// name, dispatched items count attempted, and inputs with no record
/// are `unattempted` for a refused run or `unknown` for one that may
/// have dispatched — the distinction the recovery contract draws.
fn recount(dir: &Path, expected: u64, missing_is_unknown: bool) -> Counts {
    let mut counts = Counts::queued(expected);
    let lines = read_lines(&dir.join("items.jsonl"));
    for line in &lines {
        if line["dispatched"].as_bool() == Some(true) {
            counts.attempted += 1;
        }
        match line["item"]["outcome"].as_str() {
            Some("answered") => counts.answered += 1,
            Some("refused") => counts.refused += 1,
            Some("unavailable") => counts.unavailable += 1,
            _ => counts.unattempted += 1,
        }
    }
    let missing = expected.saturating_sub(lines.len() as u64);
    if missing_is_unknown {
        counts.unknown += missing;
    } else {
        counts.unattempted += missing;
    }
    counts
}

// ---------- storage ----------

fn job_dir(root: &Path, id: &str) -> PathBuf {
    root.join("jobs").join(id)
}

/// The job id — derived from the credential scope, the idempotency
/// key, and the content digest, so the identical replay names the same
/// job without a lookup.
fn job_id(scope: &str, key: &str, digest: &str) -> String {
    let material = format!("{scope}\0{key}\0{digest}");
    format!("job_{}", &hex(&Sha256::digest(material.as_bytes()))[..32])
}

fn mint_secret() -> String {
    hex(&secp256k1::rand::random::<[u8; 32]>())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// A durable JSON write — staged, flushed, then renamed over the old
/// file so a crash never leaves half a status.
fn write_json_atomic(path: &Path, value: &Value) -> Result<(), String> {
    let staged = path.with_extension("json.tmp");
    let write = || -> Result<(), std::io::Error> {
        let mut file = fs::File::create(&staged)?;
        file.write_all(&serde_json::to_vec_pretty(value).unwrap_or_default())?;
        file.sync_all()?;
        fs::rename(&staged, path)?;
        Ok(())
    };
    write().map_err(|trouble| trouble.to_string())
}

fn append_line(path: &Path, value: &Value) {
    let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let line = serde_json::to_string(value).unwrap_or_default();
    let _ = writeln!(file, "{line}");
    let _ = file.sync_data();
}

fn read_lines(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn read_items(dir: &Path) -> Vec<Value> {
    read_lines(&dir.join("items.jsonl"))
}

fn load_status(root: &Path, id: &str) -> Option<Value> {
    read_json(&job_dir(root, id).join("status.json"))
}

/// Write the job's status document — one atomic file, the record every
/// route reads.
fn write_status(root: &Path, id: &str, status: &Value) -> Result<(), String> {
    write_json_atomic(&job_dir(root, id).join("status.json"), status)
}

fn write_status_at(path: &Path, status: &Value) -> Result<(), String> {
    write_json_atomic(path, status)
}

/// The manifest and status write a submission performs before its
/// index entry — both present or the job was never accepted.
fn persist_new(dir: &Path, manifest: &Value, status: &Value) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|trouble| trouble.to_string())?;
    write_json_atomic(&dir.join("manifest.json"), manifest)?;
    write_json_atomic(&dir.join("status.json"), status)
}

struct IndexEntry {
    digest: String,
    job: String,
}

fn index_append(
    root: &Path,
    scope: &str,
    key: &str,
    job: &str,
    digest: &str,
) -> Result<(), String> {
    let path = root.join("jobs").join(INDEX);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|trouble| trouble.to_string())?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|trouble| trouble.to_string())?;
    let line = json!({
        "scope": scope,
        "key": key,
        "job": job,
        "digest": digest,
        "at": now_utc(),
    });
    writeln!(file, "{line}").map_err(|trouble| trouble.to_string())?;
    file.sync_data().map_err(|trouble| trouble.to_string())
}

/// The latest index entry for this scope and key, when one exists.
fn index_lookup(root: &Path, scope: &str, key: &str) -> Option<IndexEntry> {
    read_lines(&root.join("jobs").join(INDEX))
        .iter()
        .rfind(|line| line["scope"].as_str() == Some(scope) && line["key"].as_str() == Some(key))
        .map(|line| IndexEntry {
            digest: line["digest"].as_str().unwrap_or_default().to_string(),
            job: line["job"].as_str().unwrap_or_default().to_string(),
        })
}

/// Authenticate the caller, then find the job only if the caller's
/// credential owns it — another tenant's job answers `job_not_found`,
/// so existence is not disclosed across tenants.
fn load_owned(state: &ServeState, headers: &HeaderMap, id: &str) -> Option<(Value, Value)> {
    let (_registry, caller) = authenticate(state, headers).ok()?;
    let dir = job_dir(&state.dir, id);
    let manifest = read_json(&dir.join("manifest.json"))?;
    let status = read_json(&dir.join("status.json"))?;
    let owner = manifest["key"].as_str().unwrap_or_default();
    let tenant = manifest["tenant"].as_str();
    if owner != caller.key || tenant != caller.tenant.as_deref() {
        return None;
    }
    Some((manifest, status))
}

fn terminal(status: &Value) -> bool {
    matches!(
        status["status"].as_str(),
        Some(COMPLETED | CANCELLED | FAILED)
    )
}

/// Whether cancellation was durably requested — the marker survives a
/// restart, and the runner checks it alongside its live signal.
fn cancel_requested(dir: &Path) -> bool {
    dir.join(CANCEL_MARKER).exists()
}

/// The status a caller sees: the stored document, with `cancelling`
/// overlaid when the marker exists and the job has not resolved.
fn view_status(dir: &Path, mut status: Value) -> Value {
    if !terminal(&status) && cancel_requested(dir) {
        status["status"] = json!(CANCELLING);
    }
    status
}

/// The retention sweep at startup or submission: terminal jobs whose
/// status outlived `job_retention_ms` lose their record.
fn sweep(state: &ServeState) {
    sweep_at(&state.dir, state.config.job_retention_ms);
}

fn sweep_at(dir: &Path, retention_ms: u64) {
    let root = dir.join("jobs");
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_millis(retention_ms))
        .unwrap_or(std::time::UNIX_EPOCH);
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let status = path.join("status.json");
        let Some(doc) = read_json(&status) else {
            continue;
        };
        if !terminal(&doc) {
            continue;
        }
        let aged = fs::metadata(&status)
            .and_then(|meta| meta.modified())
            .map(|modified| modified < cutoff)
            .unwrap_or(false);
        if aged {
            let _ = fs::remove_dir_all(&path);
        }
    }
}

// ---------- responses ----------

/// The caller's manifest — a job another credential owns does not
/// exist as far as this caller knows.
fn not_found(id: &str) -> Response {
    job_error(
        StatusCode::NOT_FOUND,
        "job_not_found",
        "No job with that ID exists for your API key.",
        Some(id),
    )
}

/// The typed refusal a rejected submission returns — the same shape
/// the synchronous route's errors carry.
fn refusal_response(verdict: Verdict, request: &str) -> Response {
    match verdict {
        Verdict::Refused {
            status,
            code,
            message,
            ..
        } => (
            status,
            Json(json!({
                "error": {"code": code, "message": message,
                          "request": request, "attempt": 1},
            })),
        )
            .into_response(),
        Verdict::Forwarded { status, body, .. } => (status, body).into_response(),
    }
}

fn job_error(status: StatusCode, code: &str, message: &str, job: Option<&str>) -> Response {
    let mut error = json!({"code": code, "message": message});
    if let Some(job) = job {
        error["job"] = json!(job);
    }
    (status, Json(json!({"error": error}))).into_response()
}

/// The wall clock in milliseconds — the cursor's own clock, finer
/// than `unix_now`'s seconds.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// A results cursor — `issued_ms:offset`, expired by
/// `job_cursor_ttl_ms`, and deliberately unsigned: the route's tenant
/// check is the authorization, and a tampered cursor only moves the
/// caller's own offset.
fn parse_cursor(cursor: &str) -> Option<(u64, u64)> {
    let (issued, offset) = cursor.split_once(':')?;
    Some((issued.parse().ok()?, offset.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("jobs")).unwrap();
        dir
    }

    fn write_job(root: &Path, id: &str, manifest: Value, status: Value) {
        let dir = root.join("jobs").join(id);
        fs::create_dir_all(&dir).unwrap();
        write_json_atomic(&dir.join("manifest.json"), &manifest).unwrap();
        write_json_atomic(&dir.join("status.json"), &status).unwrap();
    }

    fn manifest(id: &str) -> Value {
        json!({
            "v": SCHEMA,
            "job": id,
            "kind": "classify",
            "key": "k1",
            "tenant": "acme",
            "request": {"v": "openagents.classify.v1", "inputs": [{"id": "a"}, {"id": "b"}, {"id": "c"}]},
            "request_digest": "sha256:x",
            "notify": Value::Null,
        })
    }

    fn status(id: &str, state: &str) -> Value {
        json!({
            "v": SCHEMA,
            "job": id,
            "status": state,
            "counts": counts_json(&Counts::queued(3)),
        })
    }

    #[test]
    fn a_running_job_reconciles_to_failed_with_unknown_items() {
        let dir = fixture();
        write_job(
            dir.path(),
            "job_a",
            manifest("job_a"),
            status("job_a", RUNNING),
        );
        let items = dir.path().join("jobs/job_a/items.jsonl");
        append_line(
            &items,
            &json!({"index": 0, "dispatched": true, "attempt_id": "a",
                    "item": {"input": "a", "outcome": "answered"}}),
        );
        recover(dir.path(), u64::MAX);
        let status = read_json(&dir.path().join("jobs/job_a/status.json")).unwrap();
        assert_eq!(status["status"], FAILED);
        assert_eq!(status["cause"], "gateway_restart");
        assert_eq!(status["counts"]["answered"], 1);
        assert_eq!(status["counts"]["attempted"], 1);
        assert_eq!(status["counts"]["unknown"], 2);
        assert_eq!(status["counts"]["unattempted"], 0);
    }

    #[test]
    fn a_cancelling_job_reconciles_to_cancelled() {
        let dir = fixture();
        write_job(
            dir.path(),
            "job_b",
            manifest("job_b"),
            status("job_b", CANCELLING),
        );
        recover(dir.path(), u64::MAX);
        let status = read_json(&dir.path().join("jobs/job_b/status.json")).unwrap();
        assert_eq!(status["status"], CANCELLED);
        assert_eq!(status["counts"]["unknown"], 3);
    }

    #[test]
    fn a_queued_job_survives_recovery_for_resume() {
        let dir = fixture();
        write_job(
            dir.path(),
            "job_c",
            manifest("job_c"),
            status("job_c", QUEUED),
        );
        recover(dir.path(), u64::MAX);
        let status = read_json(&dir.path().join("jobs/job_c/status.json")).unwrap();
        assert_eq!(status["status"], QUEUED);
    }

    #[test]
    fn an_orphaned_dir_is_removed() {
        let dir = fixture();
        let orphan = dir.path().join("jobs/job_d");
        fs::create_dir_all(&orphan).unwrap();
        fs::write(orphan.join("status.json"), "{}").unwrap();
        recover(dir.path(), u64::MAX);
        assert!(!orphan.exists());
    }

    #[test]
    fn a_completed_job_is_left_alone() {
        let dir = fixture();
        write_job(
            dir.path(),
            "job_e",
            manifest("job_e"),
            status("job_e", COMPLETED),
        );
        recover(dir.path(), u64::MAX);
        let status = read_json(&dir.path().join("jobs/job_e/status.json")).unwrap();
        assert_eq!(status["status"], COMPLETED);
    }

    #[test]
    fn identical_content_digests_match() {
        let one = digest_request(&json!({"request": {"a": 1}, "notify": null}));
        let two = digest_request(&json!({"request": {"a": 1}, "notify": null}));
        assert_eq!(one, two);
        assert_eq!(job_id("k1", "idem", &one), job_id("k1", "idem", &two));
    }

    #[test]
    fn cursors_parse_and_reject() {
        assert_eq!(parse_cursor("123:4"), Some((123, 4)));
        assert_eq!(parse_cursor("nope"), None);
        assert_eq!(parse_cursor("1:x"), None);
    }

    #[test]
    fn notify_urls_are_checked() {
        assert!(validate_notify(&json!({"url": "https://caller.example/hook"})).is_ok());
        assert!(validate_notify(&json!({"url": "http://127.0.0.1:9/hook"})).is_ok());
        assert!(validate_notify(&json!({"url": "http://localhost/hook"})).is_ok());
        assert!(validate_notify(&json!({"url": "http://example.com/hook"})).is_err());
        assert!(validate_notify(&json!({"url": "ftp://caller.example/hook"})).is_err());
        assert!(validate_notify(&json!({"url": "https://u:p@caller.example/hook"})).is_err());
        assert!(validate_notify(&json!({"url": "not a url"})).is_err());
    }

    #[test]
    fn events_sign_under_their_secret() {
        let signature = sign("secret", "job-1", b"{}");
        assert_eq!(signature.len(), 64);
        assert_eq!(signature, sign("secret", "job-1", b"{}"));
        assert_ne!(signature, sign("secret", "job-1", b"{ }"));
        assert_ne!(signature, sign("other", "job-1", b"{}"));
    }
}
