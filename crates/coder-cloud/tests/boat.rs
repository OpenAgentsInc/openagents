mod support;
use coder_cloud::{
    Mode, Placement, Record, Spec, State, Store, boat_backend::Boat, drive, runtime::Credentials,
};
use serde_json::{Value, json};
use std::{sync::atomic::AtomicBool, time::Duration};
use support::Reply;
fn reply(value: Value) -> Reply {
    Reply::new(200, &[], &serde_json::to_vec(&value).unwrap())
}
fn sandbox(state: &str, stop: Value) -> Value {
    json!({"id":"bx_test","name":"fixture","state":state,"desktopAvailable":false,"snapshotAvailable":false,"hydrated":true,"stop":stop})
}
fn info(state: &str, stop: Value) -> Reply {
    reply(json!({"ok":true,"type":"sandbox.info","sandbox":sandbox(state,stop)}))
}
#[tokio::test]
async fn integrated_agent_keeps_exact_selection_redacts_output_and_confirms_stop() {
    let stop = json!({"id":"stop_1","status":"completed","requestedAt":null,"lastAttemptAt":null,"error":null,"endedAt":null});
    let sequence = vec![
        reply(
            json!({"ok":true,"type":"sandbox.created","status":"ready","sandbox":sandbox("ready",Value::Null)}),
        ),
        info("ready", Value::Null),
        reply(
            json!({"ok":true,"type":"command.result","success":true,"exitCode":0,"stdout":"","stderr":"","timedOut":false}),
        ),
        reply(
            json!({"ok":true,"type":"file.written","id":"bx_test","path":"task","size":4,"success":true,"encoding":"utf8"}),
        ),
        reply(
            json!({"ok":true,"type":"file.written","id":"bx_test","path":"env","size":4,"success":true,"encoding":"utf8"}),
        ),
        reply(
            json!({"ok":true,"type":"command.result","success":true,"exitCode":0,"stdout":"","stderr":"","timedOut":false}),
        ),
        reply(
            json!({"ok":true,"type":"prompt.queued","id":"bx_test","promptId":"p1","conversationId":"c1","status":"queued","provider":"codex","promptRun":{"id":"p1","promptId":"p1","sandboxId":"bx_test","status":"queued","done":false}}),
        ),
        reply(
            json!({"ok":true,"type":"events.list","id":"bx_test","events":[{"id":"e1","timestamp":1,"type":"response","data":{"model":"chosen-model","content":"answer with test-secret"}}],"pageInfo":{"hasMore":false,"limit":100,"nextCursor":"after-e1"}}),
        ),
        reply(
            json!({"ok":true,"type":"prompt.run","id":"bx_test","promptRun":{"id":"p1","promptId":"p1","sandboxId":"bx_test","status":"finished","done":true,"model":"chosen-model"}}),
        ),
        info("ready", Value::Null),
        reply(
            json!({"ok":true,"type":"sandbox.stopping","id":"bx_test","status":"archiving","sandbox":sandbox("archiving",stop.clone())}),
        ),
        info("archived", stop),
        reply(
            json!({"ok":true,"type":"sandbox.usage","sandboxId":"bx_test","sandboxType":"small","billingMultiplier":0.5,"since":"2026-10-07","until":"2026-10-07","seconds":10,"dollars":0.0001,"secondsPerDollar":100000,"running":false}),
        ),
    ];
    let (client, job) = support::serve_sequence(sequence, |b| b).await;
    let backend = Boat {
        client,
        credentials: Credentials::from_names(&["TEST_API_KEY".into()], |_| {
            Some("test-secret".into())
        })
        .unwrap(),
    };
    let root = tempfile::tempdir().unwrap();
    let store = Store::under(root.path());
    let lease = store.lease("j1").unwrap();
    let mut record = Record::new(
        "j1",
        Spec {
            placement: Placement::Boat,
            mode: Mode::Integrated,
            agent: "codex".into(),
            task: "test".into(),
            model: Some("chosen-model".into()),
            reasoning: Some("medium".into()),
            cwd: "fixture".into(),
            timeout_seconds: 600,
            size: "small".into(),
            template: None,
            credential_names: vec!["TEST_API_KEY".into()],
        },
    )
    .unwrap();
    drive(
        &backend,
        &lease,
        &mut record,
        &AtomicBool::new(false),
        Duration::from_millis(1),
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(record.state, State::Completed);
    assert!(record.cleanup_complete);
    assert_eq!(
        record.result.as_ref().unwrap()["reply"],
        "answer with [redacted]"
    );
    assert!(
        !serde_json::to_string(&store.read("j1").unwrap())
            .unwrap()
            .contains("test-secret")
    );
    let seen = job.await.unwrap();
    assert_eq!(seen[0].headers["idempotency-key"], "oa-coder-j1");
    let create: Value = serde_json::from_slice(&seen[0].body).unwrap();
    assert_eq!(create["noEnv"], true);
    assert_eq!(create["env"]["TEST_API_KEY"], "test-secret");
    let prompt: Value = serde_json::from_slice(&seen[6].body).unwrap();
    assert_eq!(prompt["provider"], "codex");
    assert_eq!(prompt["model"], "chosen-model");
    assert_eq!(prompt["reasoningEffort"], "medium");
    assert_eq!(seen[11].method, "GET");
}

// The launch script runs on Linux boat hosts: it needs `flock(1)` and `/proc`.
#[cfg(target_os = "linux")]
#[test]
fn headless_launch_runs_once_and_retains_ndjson_with_the_selected_model() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    let root = tempfile::tempdir().unwrap();
    let id = format!("launch_{}_{}", std::process::id(), coder_cloud::now_ms());
    let mut record = Record::new(
        &id,
        Spec {
            placement: Placement::Boat,
            mode: Mode::Coder,
            agent: "codex".into(),
            task: "Read the fixture".into(),
            model: Some("fixture-model".into()),
            reasoning: Some("high".into()),
            cwd: root.path().into(),
            timeout_seconds: 60,
            size: "small".into(),
            template: Some("fixture-template".into()),
            credential_names: vec![],
        },
    )
    .unwrap();
    let dir = root.path().join("job");
    std::fs::create_dir_all(&dir).unwrap();
    let binary = root.path().join("openagents");
    std::fs::write(&binary, "#!/bin/sh\ncase \"$*\" in *--help*) echo 'delegate AGENT';; *) printf '%s\\n' '{\"event\":\"delta\",\"text\":\"fixture\"}'; printf '{\"reply\":\"fixture\",\"model\":\"%s\",\"reasoning\":\"%s\"}\\n' \"$CODER_CODEX_MODEL\" \"$CODER_CODEX_REASONING\";; esac\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(dir.join("task"), &record.spec.task).unwrap();
    let call = |script: &str| {
        Command::new("sh")
            .args(["-c", script])
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", root.path())
            .env("OA_CODER_CLOUD_BINARY", &binary)
            .output()
            .unwrap()
    };
    let prepared = call(&coder_cloud::runtime::prepare_script(
        &record,
        dir.to_str().unwrap(),
    ));
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let alternate = root.path().join("alternate");
    std::fs::write(&alternate, "#!/bin/sh\nexit 9\n").unwrap();
    std::fs::set_permissions(&alternate, std::fs::Permissions::from_mode(0o700)).unwrap();
    let repeated = Command::new("sh")
        .args([
            "-c",
            &coder_cloud::runtime::prepare_script(&record, dir.to_str().unwrap()),
        ])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", root.path())
        .env("OA_CODER_CLOUD_BINARY", &alternate)
        .output()
        .unwrap();
    assert!(repeated.status.success());
    assert_eq!(
        std::fs::read_to_string(dir.join("binary")).unwrap(),
        binary.to_string_lossy()
    );
    let script = coder_cloud::runtime::launch_script(&record, dir.to_str().unwrap());
    assert!(call(&script).status.success());
    assert!(call(&script).status.success());
    let observed = call(&coder_cloud::runtime::poll_script(dir.to_str().unwrap(), 0));
    let parsed =
        coder_cloud::runtime::parse_poll(&record, std::str::from_utf8(&observed.stdout).unwrap())
            .unwrap();
    assert_eq!(parsed.events.len(), 2);
    let result = parsed.end.unwrap().unwrap();
    assert_eq!(result["model"], "fixture-model");
    assert_eq!(result["reasoning"], "high");
    record.cursor = parsed.cursor;
    record.events = parsed.events;
    let observed = call(&coder_cloud::runtime::poll_script(
        dir.to_str().unwrap(),
        record.cursor.as_deref().unwrap().parse().unwrap(),
    ));
    let parsed =
        coder_cloud::runtime::parse_poll(&record, std::str::from_utf8(&observed.stdout).unwrap())
            .unwrap();
    assert!(parsed.events.is_empty());
    assert_eq!(parsed.end.unwrap().unwrap(), result);
    std::fs::remove_dir_all(format!("/tmp/oa-coder-{id}")).unwrap();
}
