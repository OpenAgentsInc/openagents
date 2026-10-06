//! HTTP fixtures for the caller-scoped execution projection (#10701):
//! idempotent creation, cross-caller refusal, event gaps, narrow scopes,
//! and reattachment after a restart without a second run.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::*;

/// A scratch executor: starts are keyed by run ID, so a repeat returns
/// the same task.
#[derive(Default)]
struct Scratch {
    tasks: BTreeMap<String, String>,
    starts: usize,
    stops: Vec<String>,
    state: BTreeMap<String, TaskState>,
    pending: BTreeMap<String, Vec<Value>>,
    artifacts: BTreeMap<String, Vec<u8>>,
}

impl Executor for Scratch {
    fn start(&mut self, run: &str, _: &RunInput) -> Result<String, (String, String)> {
        let count = self.tasks.len();
        let task = self
            .tasks
            .entry(run.to_owned())
            .or_insert_with(|| format!("task-{count}"))
            .clone();
        self.starts += 1;
        self.state.entry(task.clone()).or_insert(TaskState::Running);
        Ok(task)
    }
    fn poll(&mut self, task: &str) -> (TaskState, Vec<Value>) {
        (
            self.state.get(task).copied().unwrap_or(TaskState::Unknown),
            self.pending.remove(task).unwrap_or_default(),
        )
    }
    fn stop(&mut self, task: &str) -> Result<(), (String, String)> {
        self.stops.push(task.to_owned());
        self.state
            .insert(task.to_owned(), TaskState::CancelRequested);
        Ok(())
    }
    fn artifact(&mut self, _: &str, digest: &str) -> Option<Vec<u8>> {
        self.artifacts.get(digest).cloned()
    }
}

const ALL: &[&str] = &["runs", "threads"];

fn body(prompt: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "computer": "studio", "workspace": "openagents",
        "prompt": prompt, "thread": "th_1",
    }))
    .unwrap()
}

fn call(
    api: &mut Api,
    executor: &mut Scratch,
    method: &str,
    path: &str,
    caller: &str,
    scopes: &[&str],
    key: Option<&str>,
    body: &[u8],
) -> Response {
    handle(
        api,
        executor,
        &Request {
            method,
            path,
            caller,
            scopes,
            idempotency_key: key,
            body,
        },
    )
}

fn get(api: &mut Api, executor: &mut Scratch, path: &str, caller: &str) -> Response {
    call(api, executor, "GET", path, caller, ALL, None, b"")
}

#[test]
fn the_same_key_and_request_reuses_and_a_different_request_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = Api::open(dir.path()).unwrap();
    let mut exec = Scratch::default();
    let first = call(
        &mut api,
        &mut exec,
        "POST",
        "/v1/runs",
        "alice",
        ALL,
        Some("k1"),
        &body("fix it"),
    );
    assert_eq!(first.status, 200, "{:?}", first.body);
    let again = call(
        &mut api,
        &mut exec,
        "POST",
        "/v1/runs",
        "alice",
        ALL,
        Some("k1"),
        &body("fix it"),
    );
    assert_eq!(again, first);
    assert_eq!(exec.starts, 1);
    let other = call(
        &mut api,
        &mut exec,
        "POST",
        "/v1/runs",
        "alice",
        ALL,
        Some("k1"),
        &body("other"),
    );
    assert_eq!(other.status, 409);
    assert_eq!(other.body["error"]["code"], "idempotency_conflict");
    assert_eq!(exec.starts, 1);
    // The same key from another caller is that caller's own run.
    let bob = call(
        &mut api,
        &mut exec,
        "POST",
        "/v1/runs",
        "bob",
        ALL,
        Some("k1"),
        &body("fix it"),
    );
    assert_eq!(bob.status, 200);
    assert_ne!(bob.body["id"], first.body["id"]);
    // The key survives a restart.
    let mut reopened = Api::open(dir.path()).unwrap();
    let after = call(
        &mut reopened,
        &mut exec,
        "POST",
        "/v1/runs",
        "alice",
        ALL,
        Some("k1"),
        &body("fix it"),
    );
    assert_eq!(after, first);
    let conflict = call(
        &mut reopened,
        &mut exec,
        "POST",
        "/v1/runs",
        "alice",
        ALL,
        Some("k1"),
        &body("x"),
    );
    assert_eq!(conflict.status, 409);
    assert_eq!(exec.starts, 2);
}

#[test]
fn another_callers_resources_answer_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = Api::open(dir.path()).unwrap();
    let mut exec = Scratch::default();
    let created = call(
        &mut api,
        &mut exec,
        "POST",
        "/v1/runs",
        "alice",
        ALL,
        None,
        &body("fix"),
    );
    let id = created.body["id"].as_str().unwrap().to_owned();
    let task = created.body["task"].as_str().unwrap().to_owned();
    let artifact = b"patch".to_vec();
    let digest = super::digest_hex(&artifact);
    exec.artifacts.insert(digest.clone(), artifact);
    exec.pending
        .insert(task.clone(), vec![json!({"artifact": digest})]);
    api.put_offer(OfferRecord {
        id: "offer_1".into(),
        caller: "alice".into(),
        body: json!({"action": "run_coder"}),
    })
    .unwrap();
    // Alice reads everything she owns.
    assert_eq!(
        get(&mut api, &mut exec, &format!("/v1/runs/{id}"), "alice").status,
        200
    );
    assert_eq!(
        get(
            &mut api,
            &mut exec,
            &format!("/v1/runs/{id}/artifacts/{digest}"),
            "alice"
        )
        .body["content"],
        "patch"
    );
    assert_eq!(
        get(&mut api, &mut exec, "/v1/threads/th_1", "alice").status,
        200
    );
    assert_eq!(
        get(&mut api, &mut exec, "/v1/offers/offer_1", "alice").status,
        200
    );
    // Mallory sees none of it, exactly as if it did not exist.
    for (path, code) in [
        (format!("/v1/runs/{id}"), "run_not_found"),
        (format!("/v1/runs/{id}/events"), "run_not_found"),
        (format!("/v1/runs/{id}/artifacts/{digest}"), "run_not_found"),
        ("/v1/threads/th_1".to_owned(), "thread_not_found"),
        ("/v1/offers/offer_1".to_owned(), "offer_not_found"),
    ] {
        let answer = get(&mut api, &mut exec, &path, "mallory");
        assert_eq!(answer.status, 404, "{path}");
        assert_eq!(answer.body["error"]["code"], code, "{path}");
    }
    let stop = call(
        &mut api,
        &mut exec,
        "POST",
        &format!("/v1/runs/{id}/stop"),
        "mallory",
        ALL,
        None,
        b"",
    );
    assert_eq!(stop.status, 404);
    assert!(exec.stops.is_empty());
}

#[test]
fn narrow_scopes_read_and_stop_but_do_not_create() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = Api::open(dir.path()).unwrap();
    let mut exec = Scratch::default();
    let denied = call(
        &mut api,
        &mut exec,
        "POST",
        "/v1/runs",
        "alice",
        &["runs:read"],
        None,
        &body("x"),
    );
    assert_eq!(denied.status, 403);
    assert_eq!(denied.body["error"]["code"], "scope_missing");
    let created = call(
        &mut api,
        &mut exec,
        "POST",
        "/v1/runs",
        "alice",
        ALL,
        None,
        &body("x"),
    );
    let id = created.body["id"].as_str().unwrap().to_owned();
    let path = format!("/v1/runs/{id}");
    assert_eq!(
        call(
            &mut api,
            &mut exec,
            "GET",
            &path,
            "alice",
            &["runs:read"],
            None,
            b""
        )
        .status,
        200
    );
    let stop_path = format!("/v1/runs/{id}/stop");
    assert_eq!(
        call(
            &mut api,
            &mut exec,
            "POST",
            &stop_path,
            "alice",
            &["runs:read"],
            None,
            b""
        )
        .status,
        403
    );
    let stopped = call(
        &mut api,
        &mut exec,
        "POST",
        &stop_path,
        "alice",
        &["runs:cancel"],
        None,
        b"",
    );
    assert_eq!(stopped.status, 200);
    // Requested, not acknowledged: the run still reports its own state.
    assert_eq!(stopped.body["stop_requested"], true);
    assert_eq!(stopped.body["status"], "cancel_requested");
    // Stopping again sends nothing new.
    call(
        &mut api,
        &mut exec,
        "POST",
        &stop_path,
        "alice",
        &["runs:cancel"],
        None,
        b"",
    );
    assert_eq!(exec.stops.len(), 1);
}

#[test]
fn events_carry_sequences_and_explicit_retention_gaps() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = Api::open(dir.path()).unwrap();
    let mut exec = Scratch::default();
    let created = call(
        &mut api,
        &mut exec,
        "POST",
        "/v1/runs",
        "alice",
        ALL,
        None,
        &body("x"),
    );
    let id = created.body["id"].as_str().unwrap().to_owned();
    let task = created.body["task"].as_str().unwrap().to_owned();
    let lines: Vec<Value> = (0..EVENTS_RETAINED + 10)
        .map(|n| json!({"line": n}))
        .collect();
    exec.pending.insert(task, lines);
    let page = get(
        &mut api,
        &mut exec,
        &format!("/v1/runs/{id}/events"),
        "alice",
    );
    assert_eq!(page.body["gap"], json!({"from": 0, "to": 9}));
    assert_eq!(page.body["data"][0]["seq"], 10);
    assert_eq!(page.body["data"].as_array().unwrap().len(), PAGE);
    assert_eq!(page.body["more"], true);
    let next = page.body["next_after"].as_u64().unwrap();
    let page = get(
        &mut api,
        &mut exec,
        &format!("/v1/runs/{id}/events?after={next}"),
        "alice",
    );
    assert_eq!(page.body["gap"], Value::Null);
    assert_eq!(page.body["data"][0]["seq"], next + 1);
}

#[test]
fn a_restart_reattaches_to_the_original_task_and_never_runs_twice() {
    let dir = tempfile::tempdir().unwrap();
    let mut exec = Scratch::default();
    let id;
    let task;
    {
        let mut api = Api::open(dir.path()).unwrap();
        let created = call(
            &mut api,
            &mut exec,
            "POST",
            "/v1/runs",
            "alice",
            ALL,
            Some("k9"),
            &body("x"),
        );
        id = created.body["id"].as_str().unwrap().to_owned();
        task = created.body["task"].as_str().unwrap().to_owned();
        // The client's stream closes here; nothing is cancelled.
    }
    assert!(exec.stops.is_empty());
    let mut api = Api::open(dir.path()).unwrap();
    assert_eq!(api.reattach(&mut exec), [id.clone()]);
    assert_eq!(exec.starts, 1, "a started run is followed, never restarted");
    let view = get(&mut api, &mut exec, &format!("/v1/runs/{id}"), "alice");
    assert_eq!(view.body["task"], task);
    // A creation persisted before the executor answered starts again under
    // the same run ID, which the executor deduplicates.
    let mut kept: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join(format!("runs/{id}.json"))).unwrap())
            .unwrap();
    kept["task"] = Value::Null;
    std::fs::write(
        dir.path().join(format!("runs/{id}.json")),
        serde_json::to_vec(&kept).unwrap(),
    )
    .unwrap();
    let mut api = Api::open(dir.path()).unwrap();
    api.reattach(&mut exec);
    assert_eq!(exec.tasks.len(), 1);
    let view = get(&mut api, &mut exec, &format!("/v1/runs/{id}"), "alice");
    assert_eq!(view.body["task"], task);
}
