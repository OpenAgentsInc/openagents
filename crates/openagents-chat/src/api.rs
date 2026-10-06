//! The caller-scoped agent execution projection over HTTP (#10701).
//!
//! A transport-free handler for the run, thread, offer, event, and artifact
//! resources of the API design (`docs/api/2026-10-02-openagents-api.md`,
//! section 7): `POST /v1/runs`, `GET /v1/runs/{id}`,
//! `GET /v1/runs/{id}/events?after=N`, `GET /v1/runs/{id}/artifacts/{digest}`,
//! `POST /v1/runs/{id}/stop`, `GET /v1/threads/{id}`, and
//! `GET /v1/offers/{id}`. The HTTP server authenticates the bearer key and
//! hands [`handle`] the caller and its scopes; this module never executes
//! work itself and adds nothing to the decision gateway. Execution goes
//! through an [`Executor`], the existing task owner or host.
//!
//! - Every resource belongs to one caller. Another caller's thread, run,
//!   offer, event page, or artifact answers `404` as if it did not exist.
//! - `runs` covers create, read, and stop; `runs:read` and `runs:cancel`
//!   are the narrow read and stop capabilities.
//! - `Idempotency-Key` on a `POST`: the same key with the same request
//!   returns the first response; with a different request it is `409
//!   idempotency_conflict`. Creation is persisted before the executor is
//!   asked, and the executor's start is keyed by the run ID, so a resend is
//!   never a second run.
//! - Events carry sequence numbers. A page that starts before the retained
//!   window says so with an explicit `gap`.
//! - [`Api::open`] reloads the store after a restart, and [`Api::reattach`]
//!   asks the executor about the original task; a run whose start was
//!   persisted but not acknowledged is started again under the same run
//!   ID, which the executor deduplicates. A closed stream cancels nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The most events a run retains; older ones become a gap.
pub const EVENTS_RETAINED: usize = 256;
/// The most events one page returns.
pub const PAGE: usize = 100;
/// The longest request body read.
pub const BODY_MAX: usize = 64 * 1024;

/// One HTTP request after authentication.
#[derive(Clone, Debug)]
pub struct Request<'a> {
    pub method: &'a str,
    /// Path and query, such as `/v1/runs/run_1/events?after=3`.
    pub path: &'a str,
    /// The authenticated caller (the key's account).
    pub caller: &'a str,
    pub scopes: &'a [&'a str],
    pub idempotency_key: Option<&'a str>,
    pub body: &'a [u8],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub status: u16,
    pub body: Value,
}

fn error(status: u16, kind: &str, code: &str, message: &str) -> Response {
    Response {
        status,
        body: json!({"error": {"type": kind, "code": code, "message": message}}),
    }
}

fn ok(body: Value) -> Response {
    Response { status: 200, body }
}

/// What the executor reports about a task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Running,
    Completed,
    Failed,
    CancelRequested,
    Cancelled,
    Unknown,
}

/// The run's work, as the caller asked for it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunInput {
    pub computer: String,
    pub workspace: String,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
}

/// The existing task owner or host, as this projection reaches it.
pub trait Executor {
    /// Starts the run's work. Calling it again with the same `run` returns
    /// the same task and starts nothing.
    ///
    /// # Errors
    ///
    /// `(code, message)` when the executor refuses or cannot be reached.
    fn start(&mut self, run: &str, input: &RunInput) -> Result<String, (String, String)>;
    /// The task's state now, and any new event lines since the last call.
    fn poll(&mut self, task: &str) -> (TaskState, Vec<Value>);
    /// Requests the task's stop. Acknowledgment arrives through `poll`.
    ///
    /// # Errors
    ///
    /// `(code, message)` when refused.
    fn stop(&mut self, task: &str) -> Result<(), (String, String)>;
    /// The retained artifact with `digest`, when the task kept it.
    fn artifact(&mut self, task: &str, digest: &str) -> Option<Vec<u8>>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    seq: u64,
    data: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Run {
    id: String,
    caller: String,
    input: RunInput,
    #[serde(default)]
    task: Option<String>,
    state: TaskState,
    #[serde(default)]
    stop_requested: bool,
    #[serde(default)]
    events: Vec<Event>,
    #[serde(default)]
    next_seq: u64,
    #[serde(default)]
    artifacts: Vec<String>,
    #[serde(default)]
    start_error: Option<(String, String)>,
}

/// An offer the router made this caller, read here; confirming it is the
/// router's operation, not this projection's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfferRecord {
    pub id: String,
    pub caller: String,
    pub body: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Idempotent {
    request: String,
    status: u16,
    body: Value,
}

/// The durable, caller-scoped store.
pub struct Api {
    dir: PathBuf,
    runs: BTreeMap<String, Run>,
    offers: BTreeMap<String, OfferRecord>,
    keys: BTreeMap<(String, String), Idempotent>,
    counter: u64,
}

fn safe(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn write_atomic(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec(value).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(tmp, path)
}

fn digest_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_dir<T: for<'de> Deserialize<'de>>(dir: &Path) -> Vec<T> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Some(value) = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        {
            out.push(value);
        }
    }
    out
}

impl Api {
    /// Opens the store under `dir`, reloading every run, offer, and
    /// idempotency record a previous process kept.
    ///
    /// # Errors
    ///
    /// When the directories cannot be created.
    pub fn open(dir: &Path) -> std::io::Result<Self> {
        for sub in ["runs", "offers", "keys"] {
            std::fs::create_dir_all(dir.join(sub))?;
        }
        let runs: BTreeMap<String, Run> = read_dir::<Run>(&dir.join("runs"))
            .into_iter()
            .map(|run| (run.id.clone(), run))
            .collect();
        let offers = read_dir::<OfferRecord>(&dir.join("offers"))
            .into_iter()
            .map(|offer| (offer.id.clone(), offer))
            .collect();
        let keys = read_dir::<(String, String, Idempotent)>(&dir.join("keys"))
            .into_iter()
            .map(|(caller, key, record)| ((caller, key), record))
            .collect();
        Ok(Self {
            dir: dir.to_owned(),
            counter: runs.len() as u64,
            runs,
            offers,
            keys,
        })
    }

    fn save_run(&self, run: &Run) -> std::io::Result<()> {
        write_atomic(&self.dir.join("runs").join(format!("{}.json", run.id)), run)
    }

    /// Keeps an offer the router made `caller`.
    ///
    /// # Errors
    ///
    /// An unsafe ID or a failed write.
    pub fn put_offer(&mut self, offer: OfferRecord) -> std::io::Result<()> {
        if !safe(&offer.id) {
            return Err(std::io::Error::other("unsafe offer id"));
        }
        write_atomic(
            &self.dir.join("offers").join(format!("{}.json", offer.id)),
            &offer,
        )?;
        self.offers.insert(offer.id.clone(), offer);
        Ok(())
    }

    /// After a restart: every run follows its original task. A run whose
    /// creation was persisted but whose start was not acknowledged is
    /// started again under its own run ID, which the executor
    /// deduplicates. Returns the runs reattached.
    pub fn reattach(&mut self, executor: &mut dyn Executor) -> Vec<String> {
        let ids: Vec<String> = self.runs.keys().cloned().collect();
        let mut out = Vec::new();
        for id in ids {
            let mut run = self.runs[&id].clone();
            if run.task.is_none() && run.start_error.is_none() {
                match executor.start(&run.id, &run.input) {
                    Ok(task) => run.task = Some(task),
                    Err(error) => run.start_error = Some(error),
                }
            }
            if run.task.is_some() {
                Self::refresh(&mut run, executor);
                out.push(id.clone());
            }
            let _ = self.save_run(&run);
            self.runs.insert(id, run);
        }
        out
    }

    fn refresh(run: &mut Run, executor: &mut dyn Executor) {
        let Some(task) = run.task.clone() else {
            return;
        };
        let (state, events) = executor.poll(&task);
        run.state = state;
        for data in events {
            if let Some(digest) = data["artifact"].as_str()
                && !run.artifacts.iter().any(|kept| kept == digest)
            {
                run.artifacts.push(digest.to_owned());
            }
            run.events.push(Event {
                seq: run.next_seq,
                data,
            });
            run.next_seq += 1;
        }
        if run.events.len() > EVENTS_RETAINED {
            let drop = run.events.len() - EVENTS_RETAINED;
            run.events.drain(..drop);
        }
    }

    fn view(run: &Run) -> Value {
        json!({
            "id": run.id,
            "object": "run",
            "thread": run.input.thread,
            "computer": run.input.computer,
            "task": run.task,
            "status": run.state,
            "stop_requested": run.stop_requested,
            "artifacts": run.artifacts,
            "error": run.start_error.as_ref().map(|(code, message)| json!({"code": code, "message": message})),
        })
    }

    fn owned(&self, caller: &str, id: &str) -> Option<&Run> {
        self.runs.get(id).filter(|run| run.caller == caller)
    }
}

fn allowed(scopes: &[&str], need: &str) -> bool {
    scopes.contains(&need)
        || (need.starts_with("runs:") && scopes.contains(&"runs"))
        || (need == "threads" && scopes.contains(&"runs"))
}

fn scope_missing(need: &str) -> Response {
    error(
        403,
        "permission",
        "scope_missing",
        &format!("This key lacks the {need} scope."),
    )
}

/// Answers one authenticated request.
pub fn handle(api: &mut Api, executor: &mut dyn Executor, request: &Request<'_>) -> Response {
    let (path, query) = request.path.split_once('?').unwrap_or((request.path, ""));
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    match (request.method, parts.as_slice()) {
        ("POST", ["v1", "runs"]) => create(api, executor, request),
        ("GET", ["v1", "runs", id]) => {
            if !allowed(request.scopes, "runs:read") {
                return scope_missing("runs:read");
            }
            read_run(api, executor, request.caller, id, |run| ok(Api::view(run)))
        }
        ("GET", ["v1", "runs", id, "events"]) => {
            if !allowed(request.scopes, "runs:read") {
                return scope_missing("runs:read");
            }
            let after: Option<u64> = query
                .split('&')
                .find_map(|pair| pair.strip_prefix("after="))
                .and_then(|value| value.parse().ok());
            read_run(api, executor, request.caller, id, |run| events(run, after))
        }
        ("GET", ["v1", "runs", id, "artifacts", digest]) => {
            if !allowed(request.scopes, "runs:read") {
                return scope_missing("runs:read");
            }
            let Some(run) = api.owned(request.caller, id) else {
                return run_not_found(id);
            };
            let (Some(task), true) = (
                run.task.clone(),
                run.artifacts.iter().any(|kept| kept == digest),
            ) else {
                return error(404, "not_found", "artifact_not_found", "No such artifact.");
            };
            match executor.artifact(&task, digest) {
                Some(bytes) if digest_hex(&bytes) == *digest => ok(json!({
                    "digest": digest, "bytes": bytes.len(),
                    "content": String::from_utf8_lossy(&bytes),
                })),
                Some(_) => error(
                    409,
                    "conflict",
                    "artifact_changed",
                    "The retained bytes no longer match their digest.",
                ),
                None => error(404, "not_found", "artifact_not_found", "No such artifact."),
            }
        }
        ("POST", ["v1", "runs", id, "stop"]) => stop(api, executor, request, id),
        ("GET", ["v1", "threads", id]) => {
            if !allowed(request.scopes, "threads") {
                return scope_missing("threads");
            }
            let runs: Vec<Value> = api
                .runs
                .values()
                .filter(|run| run.caller == request.caller)
                .filter(|run| run.input.thread.as_deref() == Some(*id))
                .map(Api::view)
                .collect();
            if runs.is_empty() {
                return error(404, "not_found", "thread_not_found", "No such thread.");
            }
            ok(json!({"id": id, "object": "thread", "runs": runs}))
        }
        ("GET", ["v1", "offers", id]) => {
            if !allowed(request.scopes, "runs:read") {
                return scope_missing("runs:read");
            }
            match api.offers.get(*id).filter(|o| o.caller == request.caller) {
                Some(offer) => ok(json!({"id": offer.id, "object": "offer", "offer": offer.body})),
                None => error(404, "not_found", "offer_not_found", "No such offer."),
            }
        }
        _ => error(404, "not_found", "route_not_found", "No such resource."),
    }
}

fn run_not_found(_id: &str) -> Response {
    error(404, "not_found", "run_not_found", "No such run.")
}

fn read_run(
    api: &mut Api,
    executor: &mut dyn Executor,
    caller: &str,
    id: &str,
    answer: impl Fn(&Run) -> Response,
) -> Response {
    let Some(mut run) = api.owned(caller, id).cloned() else {
        return run_not_found(id);
    };
    Api::refresh(&mut run, executor);
    let _ = api.save_run(&run);
    let response = answer(&run);
    api.runs.insert(run.id.clone(), run);
    response
}

fn events(run: &Run, after: Option<u64>) -> Response {
    let first = run.events.first().map_or(run.next_seq, |event| event.seq);
    let from = after.map_or(0, |after| after + 1);
    let page: Vec<Value> = run
        .events
        .iter()
        .filter(|event| event.seq >= from)
        .take(PAGE)
        .map(|event| json!({"seq": event.seq, "data": event.data}))
        .collect();
    let gap = (from < first).then(|| json!({"from": from, "to": first - 1}));
    let last = page.last().and_then(|event| event["seq"].as_u64());
    ok(json!({
        "object": "list",
        "data": page,
        "gap": gap,
        "next_after": last.or(after),
        "more": last.is_some_and(|last| last + 1 < run.next_seq),
    }))
}

fn create(api: &mut Api, executor: &mut dyn Executor, request: &Request<'_>) -> Response {
    if !allowed(request.scopes, "runs") {
        return scope_missing("runs");
    }
    if request.body.len() > BODY_MAX {
        return error(
            400,
            "invalid_request",
            "invalid_json",
            "The body is too large.",
        );
    }
    let Ok(input) = serde_json::from_slice::<RunInput>(request.body) else {
        return error(
            400,
            "invalid_request",
            "invalid_json",
            "The body is not a run request.",
        );
    };
    if input.thread.as_deref().is_some_and(|thread| !safe(thread)) {
        return error(400, "invalid_request", "missing_param", "Invalid thread.");
    }
    let digest = digest_hex(request.body);
    let key = request.idempotency_key.map(str::to_owned);
    if let Some(key) = &key {
        if !safe(key) {
            return error(
                400,
                "invalid_request",
                "client_request_id_invalid",
                "Invalid Idempotency-Key.",
            );
        }
        if let Some(kept) = api.keys.get(&(request.caller.to_owned(), key.clone())) {
            if kept.request != digest {
                return error(
                    409,
                    "conflict",
                    "idempotency_conflict",
                    "This Idempotency-Key was used with another request.",
                );
            }
            return Response {
                status: kept.status,
                body: kept.body.clone(),
            };
        }
    }
    // A key that two callers share stays two runs: the record is keyed by
    // caller and key, and the run ID is derived from both.
    api.counter += 1;
    let id = match &key {
        Some(key) => format!(
            "run_{}",
            &digest_hex(format!("{}\n{key}", request.caller).as_bytes())[..24]
        ),
        None => format!(
            "run_{}",
            &digest_hex(format!("{}\n{}\n{digest}", request.caller, api.counter).as_bytes())[..24]
        ),
    };
    let mut run = Run {
        id: id.clone(),
        caller: request.caller.to_owned(),
        input,
        task: None,
        state: TaskState::Queued,
        stop_requested: false,
        events: Vec::new(),
        next_seq: 0,
        artifacts: Vec::new(),
        start_error: None,
    };
    // Persist creation, and the key, before the executor is asked.
    if api.save_run(&run).is_err() {
        return error(
            500,
            "server_error",
            "internal_error",
            "The run was not kept.",
        );
    }
    let pending = Idempotent {
        request: digest.clone(),
        status: 200,
        body: Api::view(&run),
    };
    if let Some(key) = &key {
        let _ = write_atomic(
            &api.dir.join("keys").join(format!(
                "{}.json",
                &digest_hex(format!("{}\n{key}", request.caller).as_bytes())[..32]
            )),
            &(request.caller, key, &pending),
        );
    }
    match executor.start(&run.id, &run.input) {
        Ok(task) => run.task = Some(task),
        Err(failure) => run.start_error = Some(failure),
    }
    let _ = api.save_run(&run);
    let response = match &run.start_error {
        Some((code, message)) if code == "computer_not_granted" => {
            error(403, "permission", "computer_not_granted", message)
        }
        Some((code, message)) if code == "computer_offline" => {
            error(503, "unavailable", "computer_offline", message)
        }
        _ => ok(Api::view(&run)),
    };
    if let Some(key) = key {
        let record = Idempotent {
            request: digest,
            status: response.status,
            body: response.body.clone(),
        };
        let _ = write_atomic(
            &api.dir.join("keys").join(format!(
                "{}.json",
                &digest_hex(format!("{}\n{key}", request.caller).as_bytes())[..32]
            )),
            &(request.caller, &key, &record),
        );
        api.keys.insert((request.caller.to_owned(), key), record);
    }
    api.runs.insert(id, run);
    response
}

fn stop(api: &mut Api, executor: &mut dyn Executor, request: &Request<'_>, id: &str) -> Response {
    if !allowed(request.scopes, "runs:cancel") {
        return scope_missing("runs:cancel");
    }
    let Some(mut run) = api.owned(request.caller, id).cloned() else {
        return run_not_found(id);
    };
    Api::refresh(&mut run, executor);
    if matches!(
        run.state,
        TaskState::Completed | TaskState::Failed | TaskState::Cancelled
    ) {
        return error(409, "conflict", "run_already_stopped", "The run has ended.");
    }
    let Some(task) = run.task.clone() else {
        return error(
            409,
            "conflict",
            "run_not_started",
            "The run has no task yet.",
        );
    };
    if !run.stop_requested {
        if let Err((code, message)) = executor.stop(&task) {
            return error(503, "unavailable", &code, &message);
        }
        run.stop_requested = true;
        Api::refresh(&mut run, executor);
    }
    let _ = api.save_run(&run);
    let body = Api::view(&run);
    api.runs.insert(run.id.clone(), run);
    ok(body)
}

#[cfg(test)]
mod tests;
