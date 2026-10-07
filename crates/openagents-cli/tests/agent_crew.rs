//! The selected crew commands against a real isolated native host and stores.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::process::Command;
use std::sync::Arc;

use coder::task::{agent, agent_host::Agents, remote::Inbox};
use coder_access::RelayPolicy;
use coder_host::config::{Config, Control, Iroh};
use coder_host::serve::keys::{FileKeySource, Keys};
use serde_json::{Value, json};

#[path = "../../coder-control/src/tests/relay.rs"]
#[allow(dead_code)]
mod relay;

#[test]
fn native_crew_creation_charter_verdict_restart_and_refusals() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let root = dir.path().join("host");
    let tasks_root = dir.path().join("tasks");
    let workspace = dir.path().join("workspace");
    let socket = dir.path().join("control/socket");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    assert!(
        Command::new("git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &home)
            .args(["init", "-q"])
            .arg(&workspace)
            .status()
            .unwrap()
            .success()
    );
    let alice = agent::Store::new(&root, "alice").unwrap();
    let old = alice.open(&workspace, 100).unwrap();
    let old = alice.ensure_key(old, 100).unwrap();
    let original_key = old.pubkey.clone();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(3)
        .enable_all()
        .build()
        .unwrap();
    let (running, relay_task, config) = runtime.block_on(async {
        let (url, relay_task, _) = relay::start().await;
        let mut config = Config::new(dir.path().join("access"), vec![url], 1);
        config.policy = RelayPolicy::LoopbackTest;
        config.iroh = Some(Iroh::loopback());
        config.keys = Some(Keys(Arc::new(FileKeySource::new(dir.path().join("keys")))));
        config.control = Some(Control {
            path: socket.clone(),
            root: root.clone(),
            autostart: None,
            tasks: tasks_root.clone(),
            uid: coder_host::control::own_uid(),
        });
        let tasks = Inbox::new(&tasks_root, BTreeMap::new()).with_agents(Agents::new(
            &root,
            &tasks_root,
            BTreeMap::new(),
        ));
        let running = coder_host::start(config.clone(), Arc::new(tasks))
            .await
            .unwrap();
        (running, relay_task, config)
    });
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_openagents"))
            .env_clear()
            .env("HOME", &home)
            .env("PATH", "/usr/bin:/bin")
            .current_dir(&home)
            .args(["--json", "agent"])
            .args(args)
            .arg("--root")
            .arg(&root)
            .arg("--control-socket")
            .arg(&socket)
            .output()
            .unwrap()
    };
    let ok = |args: &[&str]| -> Value {
        let output = run(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    };
    let paul = ok(&["new", "paul", "--workspace", workspace.to_str().unwrap()]);
    assert_eq!(paul["existed"], false);
    assert_eq!(
        ok(&["new", "paul", "--workspace", workspace.to_str().unwrap()])["existed"],
        true
    );
    let erin = ok(&[
        "new",
        "researcher",
        "--role",
        "sales-researcher",
        "--workspace",
        workspace.to_str().unwrap(),
    ]);
    assert_ne!(paul["pubkey"], erin["pubkey"]);
    assert_eq!(ok(&["show", "paul"])["record"]["job_role"], "sales-lead");
    assert_eq!(
        ok(&["show", "researcher"])["record"]["job_role"],
        "sales-researcher"
    );
    assert_eq!(
        ok(&["show", "alice"])["record"]["pubkey"],
        original_key.unwrap()
    );
    assert!(
        !run(&["new", "bad", "--role", "sales-administrator"])
            .status
            .success()
    );
    assert!(
        !run(&["ask", "paul", "Run a task.", "--mode", "task"])
            .status
            .success()
    );
    assert!(
        !run(&[
            "ask",
            "paul",
            "Read a workspace.",
            "--workspace",
            "workspace"
        ])
        .status
        .success()
    );
    let pause = ok(&["crew", "pause", "--cohort", "floor", "--all"]);
    assert_eq!(pause["state"], "complete");
    assert_eq!(
        pause["members"]["paul"]["pending_dispatch"]["enabled"],
        false
    );
    assert_eq!(ok(&["show", "paul"])["record"]["state"], "paused");
    assert_eq!(ok(&["show", "alice"])["record"]["state"], "active");
    assert!(
        !run(&["ask", "paul", "Remember a rejected note."])
            .status
            .success()
    );
    assert!(!run(&["resume", "paul"]).status.success());
    assert!(
        !run(&[
            "crew",
            "stop",
            "--cohort",
            "invalid",
            "--all",
            "--members",
            "paul"
        ])
        .status
        .success()
    );
    assert!(
        !run(&["crew", "stop", "--cohort", "invalid", "--members", "alice"])
            .status
            .success()
    );
    let digest = ok(&["crew", "status"])["digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let wrong = format!("sha256:{}", "0".repeat(64));
    assert!(
        !run(&[
            "crew",
            "resume",
            "--cohort",
            "floor",
            "--all",
            "--expected",
            &wrong
        ])
        .status
        .success()
    );
    ok(&[
        "crew",
        "resume",
        "--cohort",
        "floor",
        "--all",
        "--expected",
        &digest,
    ]);
    assert_eq!(ok(&["show", "paul"])["record"]["state"], "active");
    let stopped = ok(&[
        "crew",
        "stop",
        "--cohort",
        "subset",
        "--members",
        "paul,researcher",
    ]);
    assert_eq!(stopped["selected"], json!(["paul", "researcher"]));
    assert_eq!(ok(&["show", "researcher"])["record"]["state"], "stopped");
    let digest = ok(&["crew", "status"])["digest"]
        .as_str()
        .unwrap()
        .to_owned();
    ok(&[
        "crew",
        "resume",
        "--cohort",
        "subset",
        "--members",
        "paul,researcher",
        "--expected",
        &digest,
    ]);
    let input = dir.path().join("verdict.json");
    std::fs::write(&input, serde_json::to_vec(&json!({
        "id":"review-1", "subject":{"kind":"issue","reference":"github:issue/1","revision":1,"sha256":"a".repeat(64)},
        "evidence":[{"reference":"host:receipt/1","sha256":"b".repeat(64)}],
        "result":"needs_evidence","reason":"The owner records missing independent acceptance.",
        "question_set_sha256":"c".repeat(64)
    })).unwrap()).unwrap();
    let recorded = ok(&["verdict", "paul", "record", input.to_str().unwrap()]);
    assert_eq!(recorded["author"], paul["pubkey"]);
    assert_eq!(recorded["basis"], "owner_recorded_recommendation");
    assert_eq!(
        ok(&["verdict", "paul", "record", input.to_str().unwrap()]),
        recorded
    );
    let linked = dir.path().join("linked-input.json");
    std::os::unix::fs::symlink(&input, &linked).unwrap();
    assert!(
        !run(&["verdict", "paul", "record", linked.to_str().unwrap()])
            .status
            .success()
    );
    let mut malformed: Value = serde_json::from_slice(&std::fs::read(&input).unwrap()).unwrap();
    malformed["subject"]["revision"] = json!(0);
    std::fs::write(&input, serde_json::to_vec(&malformed).unwrap()).unwrap();
    assert!(
        !run(&["verdict", "paul", "record", input.to_str().unwrap()])
            .status
            .success()
    );
    ok(&[
        "charter",
        "paul",
        "--role",
        "sales-lead",
        "--expected",
        "1",
        "--drafting",
        "off",
        "--purpose",
        "Wait for owner review.",
    ]);
    assert!(
        !run(&["ask", "paul", "Draft a recommendation."])
            .status
            .success()
    );
    let final_stop = ok(&["crew", "stop", "--cohort", "floor", "--all"]);
    assert_eq!(final_stop["state"], "complete");
    runtime.block_on(running.shutdown());
    let restarted = runtime.block_on(async {
        let tasks = Inbox::new(&tasks_root, BTreeMap::new()).with_agents(Agents::new(
            &root,
            &tasks_root,
            BTreeMap::new(),
        ));
        coder_host::start(config, Arc::new(tasks)).await.unwrap()
    });
    assert_eq!(
        ok(&["verdict", "paul", "list"])["verdicts"],
        json!([recorded])
    );
    assert_eq!(
        ok(&["show", "paul"])["record"]["crew_charter"]["revision"],
        2
    );
    assert_eq!(ok(&["show", "paul"])["record"]["state"], "stopped");
    assert!(
        !run(&["ask", "paul", "Remember a rejected note after restart."])
            .status
            .success()
    );
    assert!(
        ok(&["crew", "status"])["history"]
            .as_object()
            .unwrap()
            .values()
            .any(|v| v == &final_stop)
    );
    assert_eq!(ok(&["show", "alice"])["record"]["state"], "active");
    assert!(!root.join("agents/paul/policy.json").exists());
    assert!(!root.join("agents/researcher/verdicts").exists());
    runtime.block_on(restarted.shutdown());
    relay_task.abort();
}
