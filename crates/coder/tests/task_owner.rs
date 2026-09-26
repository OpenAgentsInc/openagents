//! Exercise owner independence and crash recovery through real CLI processes.
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_coder"))
        .env_clear()
        .env("HOME", root)
        .env("PATH", "/usr/bin:/bin")
        .arg("task")
        .args(args)
        .output()
        .unwrap()
}
fn success(output: Output) -> Value {
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}
fn setup(script: &str) -> (tempfile::TempDir, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    let worktree = root.path().join("checkout");
    std::fs::create_dir(&repo).unwrap();
    for args in [
        vec!["init", "-q"],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--allow-empty",
            "-qm",
            "Fixture",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success()
        );
    }
    assert!(
        Command::new("git")
            .args(["worktree", "add", "--detach", "-q"])
            .arg(&worktree)
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );
    let request = root.path().join("request.json");
    std::fs::write(&request, serde_json::to_vec(&json!({"schema":"openagents.coder.task-command.v1", "command_id":"submit-one", "task_id":"task-one", "expected_revision":null,
        "action":{"type":"submit", "intent":{"title":"Test task", "prompt":"Exercise one bounded task.", "workspace":{"path":worktree.canonicalize().unwrap(),"source_revision":null},
            "configuration":{"adapter":"bounded-command","model":null}}}})).unwrap()).unwrap();
    success(cli(
        root.path(),
        &["submit", "--file", request.to_str().unwrap()],
    ));
    let task = success(cli(root.path(), &["show", "task-one"]));
    let grant = root.path().join("grant.json");
    std::fs::write(&grant, serde_json::to_vec(&json!({"schema":"openagents.coder.task-execution-grant.v1", "task_id":"task-one", "intent_digest":task["intent_digest"], "expected_revision":1,
        "program":Path::new("/bin/sh").canonicalize().unwrap(), "arguments":["-c",script], "write_workspace":true,"wall_seconds":10,"stream_bytes":4096,"memory_bytes":268435456})).unwrap()).unwrap();
    (root, grant)
}
fn wait_for(root: &Path, predicate: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let task = success(cli(root, &["show", "task-one"]));
        if predicate(&task) {
            return task;
        }
        assert!(
            Instant::now() < deadline,
            "task never reached expected state: {task}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn detached_owner_finishes_after_the_starting_client_exits() {
    let (root, grant) = setup("sleep 0.3; printf one >> effects; printf complete");
    let launch = success(cli(
        root.path(),
        &["start", "--grant", grant.to_str().unwrap()],
    ));
    assert_eq!(launch["admission"], "pending");
    assert!(launch["diagnostic_path"].as_str().is_some());
    // The start command has exited; a fresh process observes its independent owner.
    let task = wait_for(root.path(), |task| task["execution"] == "finished");
    assert_eq!(task["checks"], "not_run");
    let view = success(cli(root.path(), &["view", "task-one", "--limit", "1"]));
    assert_eq!(view["evidence"]["state"], "sealed");
    assert_eq!(view["verification"], "not_run");
    assert_eq!(view["cost_usd"], Value::Null);
    let cursor = serde_json::to_string(&view["evidence"]["next"]).unwrap();
    let next = success(cli(
        root.path(),
        &["view", "task-one", "--limit", "200", "--cursor", &cursor],
    ));
    assert!(next["evidence"]["steps"].to_string().contains("complete"));
    assert_eq!(
        std::fs::read_to_string(root.path().join("checkout/effects")).unwrap(),
        "one"
    );
    assert!(
        !cli(root.path(), &["start", "--grant", grant.to_str().unwrap()])
            .status
            .success()
    );
}

#[test]
fn killing_owner_retains_unknown_and_does_not_repeat_a_persisted_effect() {
    let (root, grant) = setup("printf one >> effects; sleep 1; printf late >> effects");
    let launch = success(cli(
        root.path(),
        &["start", "--grant", grant.to_str().unwrap()],
    ));
    wait_for(root.path(), |task| task["run"]["process_id"].is_u64());
    let effect = root.path().join("checkout/effects");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !effect.exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let pid = launch["owner_process"].as_u64().unwrap().to_string();
    assert!(
        Command::new("/bin/kill")
            .args(["-KILL", &pid])
            .status()
            .unwrap()
            .success()
    );
    std::thread::sleep(Duration::from_millis(50));
    let recovered = success(cli(root.path(), &["recover", "task-one"]));
    assert_eq!(recovered["execution"], "unknown");
    assert_eq!(recovered["run"]["epoch"], 2);
    assert!(
        !cli(root.path(), &["start", "--grant", grant.to_str().unwrap()])
            .status
            .success()
    );
    std::thread::sleep(Duration::from_millis(1200));
    let bytes = std::fs::read_to_string(effect).unwrap();
    assert_eq!(bytes.matches("one").count(), 1);
    // A descendant may have survived SIGKILL. That is why recovery reports
    // unknown rather than claiming quiescence or restarting the command.
    assert_eq!(
        success(cli(root.path(), &["show", "task-one"]))["execution"],
        "unknown"
    );
}
