//! Exercise the durable inbox through independent CLI processes and real files.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

const SUBMIT: &[u8] = include_bytes!("../../../docs/coder/fixtures/tasks/submit.json");
const CANCEL: &[u8] = include_bytes!("../../../docs/coder/fixtures/tasks/cancel.json");

fn binary(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_coder"));
    command
        .env_clear()
        .env("HOME", root)
        .env("PATH", "/usr/bin:/bin")
        .env("CODER_DOOR_URL", "http://127.0.0.1:1/never-call")
        .env("CODER_DELEGATE", "always")
        .arg("task");
    command
}

fn read(root: &Path, arguments: &[&str]) -> Output {
    binary(root).args(arguments).output().unwrap()
}

fn apply(root: &Path, operation: &str, bytes: &[u8]) -> Output {
    let mut child = binary(root)
        .args([operation, "--file", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    child.wait_with_output().unwrap()
}

fn success(output: Output) -> Value {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn a_request_and_cancel_survive_restart_and_retries_keep_the_original_receipts() {
    let root = tempfile::tempdir().unwrap();
    let receipt = success(apply(root.path(), "submit", SUBMIT));
    assert_eq!(receipt["revision"], 1);
    let state = root.path().join(".openagents/tasks/tasks.json");
    let original_bytes = std::fs::read(&state).unwrap();
    assert_eq!(success(apply(root.path(), "submit", SUBMIT)), receipt);
    assert_eq!(std::fs::read(&state).unwrap(), original_bytes);
    let queued = success(read(root.path(), &["show", "example-task-1"]));
    assert_eq!(queued["revision"], 1);
    assert_eq!(queued["status"], "queued");
    assert_eq!(queued["execution"], "not_started");
    assert_eq!(
        success(read(root.path(), &["list"]))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(std::fs::read(&state).unwrap(), original_bytes);

    let cancelled = success(apply(root.path(), "cancel", CANCEL));
    assert_eq!(cancelled["revision"], 2);
    let final_bytes = std::fs::read(&state).unwrap();
    assert_eq!(success(apply(root.path(), "cancel", CANCEL)), cancelled);
    assert_eq!(success(apply(root.path(), "submit", SUBMIT)), receipt);
    assert_eq!(std::fs::read(&state).unwrap(), final_bytes);
    let current = success(read(root.path(), &["show", "example-task-1"]));
    assert_eq!(current["status"], "cancelled");
    assert_eq!(current["execution"], "not_started");
    assert_eq!(current["checks"], "not_run");
    assert!(!root.path().join(".openagents/traces").exists());
}

#[test]
fn simultaneous_processes_return_one_submission_receipt() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("request.json");
    std::fs::write(&file, SUBMIT).unwrap();
    let mut children = Vec::new();
    for _ in 0..8 {
        children.push(
            binary(root.path())
                .args(["submit", "--file"])
                .arg(&file)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    let receipts: Vec<_> = children
        .into_iter()
        .map(|child| success(child.wait_with_output().unwrap()))
        .collect();
    assert!(receipts.iter().all(|receipt| receipt == &receipts[0]));
    assert_eq!(
        success(read(root.path(), &["list"]))
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn conflicting_retries_and_stale_cancels_do_not_change_state() {
    let root = tempfile::tempdir().unwrap();
    success(apply(root.path(), "submit", SUBMIT));
    let state = root.path().join(".openagents/tasks/tasks.json");
    let before = std::fs::read(&state).unwrap();
    let mut changed: Value = serde_json::from_slice(SUBMIT).unwrap();
    changed["action"]["intent"]["prompt"] = json!("Different request");
    assert_eq!(
        apply(
            root.path(),
            "submit",
            &serde_json::to_vec(&changed).unwrap()
        )
        .status
        .code(),
        Some(1)
    );
    // Exact bytes matter, even when the parsed request is semantically identical.
    let mut whitespace = SUBMIT.to_vec();
    whitespace.push(b' ');
    assert_eq!(
        apply(root.path(), "submit", &whitespace).status.code(),
        Some(1)
    );
    let mut stale: Value = serde_json::from_slice(CANCEL).unwrap();
    stale["expected_revision"] = json!(0);
    assert_eq!(
        apply(root.path(), "cancel", &serde_json::to_vec(&stale).unwrap())
            .status
            .code(),
        Some(1)
    );
    assert_eq!(std::fs::read(&state).unwrap(), before);
    success(apply(root.path(), "cancel", CANCEL));
    let after = std::fs::read(&state).unwrap();
    stale["expected_revision"] = json!(1);
    stale["command_id"] = json!("new-cancel");
    assert_eq!(
        apply(root.path(), "cancel", &serde_json::to_vec(&stale).unwrap())
            .status
            .code(),
        Some(1)
    );
    assert_eq!(std::fs::read(&state).unwrap(), after);
}

#[test]
fn concurrent_cancellations_require_one_current_revision() {
    let root = tempfile::tempdir().unwrap();
    success(apply(root.path(), "submit", SUBMIT));
    let mut children = Vec::new();
    for index in 0..2 {
        let mut request: Value = serde_json::from_slice(CANCEL).unwrap();
        request["command_id"] = json!(format!("cancel-{index}"));
        let file = root.path().join(format!("cancel-{index}.json"));
        std::fs::write(&file, serde_json::to_vec(&request).unwrap()).unwrap();
        children.push(
            binary(root.path())
                .args(["cancel", "--file"])
                .arg(file)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    let mut codes: Vec<_> = children
        .into_iter()
        .map(|child| child.wait_with_output().unwrap().status.code().unwrap())
        .collect();
    codes.sort();
    assert_eq!(codes, [0, 1]);
    assert_eq!(
        success(read(root.path(), &["show", "example-task-1"]))["revision"],
        2
    );
}

#[test]
fn malformed_and_misdirected_input_never_initializes_a_store() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(apply(root.path(), "submit", CANCEL).status.code(), Some(1));
    assert_eq!(apply(root.path(), "cancel", SUBMIT).status.code(), Some(1));
    assert_eq!(
        apply(root.path(), "submit", b"{broken").status.code(),
        Some(1)
    );
    let duplicate = String::from_utf8(SUBMIT.to_vec()).unwrap().replacen(
        "\"task_id\":",
        "\"task_id\": \"duplicate\", \"task_id\":",
        1,
    );
    assert_eq!(
        apply(root.path(), "submit", duplicate.as_bytes())
            .status
            .code(),
        Some(1)
    );
    let oversized = vec![b' '; coder::task::MAX_COMMAND_BYTES + 1];
    assert_eq!(
        apply(root.path(), "submit", &oversized).status.code(),
        Some(1)
    );
    assert!(!root.path().join(".openagents/tasks").exists());
    assert_eq!(
        read(root.path(), &["run", "example-task-1"]).status.code(),
        Some(64)
    );
}

#[test]
fn corruption_is_preserved_and_never_replaced_by_an_empty_store() {
    let root = tempfile::tempdir().unwrap();
    success(apply(root.path(), "submit", SUBMIT));
    let state = root.path().join(".openagents/tasks/tasks.json");
    std::fs::write(&state, b"{truncated").unwrap();
    assert_eq!(read(root.path(), &["list"]).status.code(), Some(1));
    assert_eq!(apply(root.path(), "submit", SUBMIT).status.code(), Some(1));
    assert_eq!(std::fs::read(&state).unwrap(), b"{truncated");
}

#[cfg(unix)]
#[test]
fn persisted_files_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    success(apply(root.path(), "submit", SUBMIT));
    let store = root.path().join(".openagents/tasks");
    assert_eq!(
        std::fs::metadata(&store).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for name in ["tasks.json", "tasks.lock"] {
        assert_eq!(
            std::fs::metadata(store.join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
