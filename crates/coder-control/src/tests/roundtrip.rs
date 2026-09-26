//! Real sockets, authenticated identities, a bounded child, and retained replay.
use super::*;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

#[test]
fn process_reopen_child() {
    let Some(path) = std::env::var_os("CODER_CONTROL_CHILD_INPUT") else {
        return;
    };
    let value: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let setup: Setup = serde_json::from_value(value["setup"].clone()).unwrap();
    let request: Event = serde_json::from_value(value["request"].clone()).unwrap();
    let input: Event = serde_json::from_value(value["input"].clone()).unwrap();
    let mut host = Host::open(
        std::path::Path::new(value["host_directory"].as_str().unwrap()),
        setup,
        SecretKey::from_byte_array([1; 32]).unwrap(),
    )
    .unwrap();
    let reply = host
        .handle(&request, &input, &Blobs::default(), NOW + 2)
        .unwrap();
    let output = std::env::var_os("CODER_CONTROL_CHILD_OUTPUT").unwrap();
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(output)
        .unwrap();
    file.write_all(&serde_json::to_vec(&reply).unwrap())
        .unwrap();
    file.sync_all().unwrap();
}

fn read_in_new_process(
    host_directory: &std::path::Path,
    setup: &Setup,
    body: &Value,
    secret: &SecretKey,
) -> Value {
    let input = client::envelope(body, secret, &setup.authority, NOW, setup.retain_until).unwrap();
    let reference = crate::reference(
        &nostr::contracts::jcs(body).unwrap(),
        "application/json",
        control::READ,
    );
    let request_id = client::random_id();
    let request = client::request(
        secret,
        &setup.authority,
        &setup.operations["read"],
        &input,
        &reference,
        NOW,
        NOW + 100,
        setup.retain_until,
        &request_id,
    )
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    let input_path = root.path().join("child-input.json");
    let output_path = root.path().join("child-output.json");
    let value =
        json!({"setup":setup,"host_directory":host_directory,"request":request,"input":input});
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&input_path)
        .unwrap();
    file.write_all(&serde_json::to_vec(&value).unwrap())
        .unwrap();
    file.sync_all().unwrap();
    drop(file);
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tests::roundtrip::process_reopen_child",
            "--nocapture",
        ])
        .env("CODER_CONTROL_CHILD_INPUT", &input_path)
        .env("CODER_CONTROL_CHILD_OUTPUT", &output_path)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "child fixture failed: {}",
        String::from_utf8_lossy(&child.stderr)
    );
    let reply: Reply = serde_json::from_slice(&std::fs::read(output_path).unwrap()).unwrap();
    let result = client::result(
        &reply.result,
        &request,
        &setup.authority,
        &request_id,
        secret,
    )
    .unwrap();
    let reference = &result["output"]["artifact"];
    reply
        .artifacts
        .iter()
        .find_map(|event| client::open_artifact(event, &setup.authority, reference, secret).ok())
        .expect("child returned the original signed finite-cut page")
}

async fn exchange(
    f: &mut Fixture,
    url: &str,
    role: &str,
    body: &Value,
    secret: &SecretKey,
    attachments: &Blobs,
) -> Value {
    let (input, input_ref) = f.input(body, secret);
    nostr_transport::artifacts::publish(url, secret, &input)
        .await
        .unwrap();
    let id = client::random_id();
    let request = client::request(
        secret,
        &f.setup.authority,
        &f.setup.operations[role],
        &input,
        &input_ref,
        NOW,
        NOW + 100,
        f.setup.retain_until,
        &id,
    )
    .unwrap();
    let mut receiver = transport::Receiver::connect(url, &f.authority)
        .await
        .unwrap();
    let authority = f.authority;
    let response = async {
        let received = receiver.receive().await.unwrap();
        assert_eq!(received.id, request.id);
        let opened = nostr::execution::open_request(
            &received,
            &f.setup.authority,
            &authority,
            NOW + 1,
            nostr::execution::Window::DEFAULT,
        )
        .unwrap();
        let nostr::execution::Body::Execute(execute) = opened.body else {
            panic!("execute")
        };
        let source = execute.input_artifact.unwrap().event.unwrap();
        let fetched = nostr_transport::artifacts::fetch(url, &authority, &source.id)
            .await
            .unwrap();
        let reply =
            if let Some(reference) = f.host.required_text(&received, &fetched, NOW + 1).unwrap() {
                let text = client::fetch_text(url, &authority, &reference)
                    .await
                    .unwrap();
                f.host
                    .handle_text(&received, &fetched, &text, NOW + 1)
                    .unwrap()
            } else {
                f.host
                    .handle(&received, &fetched, attachments, NOW + 1)
                    .unwrap()
            };
        for event in &reply.artifacts {
            nostr_transport::artifacts::publish(url, &authority, event)
                .await
                .unwrap();
        }
        receiver.reply(&received, &reply).await.unwrap();
    };
    let authority_key = client::pubkey(&authority);
    let exchange = transport::exchange(url, secret, &request, &authority_key);
    let (_, received) = tokio::join!(response, exchange);
    let result = client::result(
        &received.unwrap(),
        &request,
        &client::pubkey(&authority),
        &id,
        secret,
    )
    .unwrap();
    assert_eq!(result["outcome"], "completed");
    let reference = &result["output"]["artifact"];
    let event =
        nostr_transport::artifacts::fetch(url, secret, reference["event"]["id"].as_str().unwrap())
            .await
            .unwrap();
    let answer =
        client::open_artifact(&event, &client::pubkey(&authority), reference, secret).unwrap();
    for reference in result["artifacts"].as_array().unwrap() {
        let event = nostr_transport::artifacts::fetch(
            url,
            secret,
            reference["event"]["id"].as_str().unwrap(),
        )
        .await
        .unwrap();
        client::open_artifact(&event, &client::pubkey(&authority), reference, secret).unwrap();
    }
    answer
}

#[tokio::test]
async fn authenticated_relay_controls_real_task_and_replays_original_cut_after_restart() {
    let mut f = Fixture::new();
    let (url, server, events) = relay::start().await;
    let secret = f.client;
    let invitation = f
        .host
        .invite(
            &client::pubkey(&secret),
            &["observe".into(), "steer".into(), "cancel".into()],
            NOW,
            NOW + 60,
            NOW + 1000,
        )
        .unwrap();
    nostr_transport::artifacts::publish(&url, &f.authority, &invitation)
        .await
        .unwrap();
    let fetched = nostr_transport::artifacts::fetch(&url, &secret, &invitation.id)
        .await
        .unwrap();
    let opened = nostr::private_artifact::open(&fetched, &secret).unwrap();
    let body: Value = serde_json::from_slice(opened.inline_bytes().unwrap()).unwrap();
    let r = reference(
        opened.inline_bytes().unwrap(),
        "application/json",
        control::INVITATION,
    );
    let pairing = json!({"v":control::PAIRING,"requires":[],"invitation":r,"client":client::pubkey(&secret),"challenge":body["challenge"],"rights":["observe","steer","cancel"],"accepted":true});
    let access = exchange(&mut f, &url, "pair", &pairing, &secret, &Blobs::default()).await;
    let grant = access["access"].clone();
    let attachments = Blobs::default();
    let text = client::text(
        "Retain this authenticated correction before execution.",
        &secret,
        &f.setup.authority,
        NOW,
        f.setup.retain_until,
    )
    .unwrap();
    for event in [&text.declaration, &text.carrier] {
        nostr_transport::artifacts::publish(&url, &secret, event)
            .await
            .unwrap();
    }
    let message = text.reference;
    let correction = f.command(&grant, "steer", json!({"message":message,"replaces":[]}), 1);
    let answer = exchange(&mut f, &url, "command", &correction, &secret, &attachments).await;
    assert_eq!(answer["status"], "accepted");
    let task = coder::task::Store::open(&f.tasks)
        .unwrap()
        .show("synthetic")
        .unwrap();
    let execution = coder::task::owner::Grant {
        schema: coder::task::owner::GRANT_SCHEMA.into(),
        task_id: task.task_id,
        intent_digest: task.intent_digest,
        expected_revision: task.revision,
        program: std::path::Path::new("/bin/sh").canonicalize().unwrap(),
        arguments: vec![
            "-c".into(),
            "printf started; sleep 30; printf forbidden > late.txt".into(),
        ],
        write_workspace: true,
        wall_seconds: 35,
        stream_bytes: 4096,
        memory_bytes: 256 * 1024 * 1024,
        requirements: None,
        expected_source_snapshot: None,
        adapter_configuration: None,
    };
    let directory = f.tasks.clone();
    let running = tokio::spawn(async move {
        coder::task::owner::execute(&directory, &serde_json::to_vec(&execution).unwrap()).await
    });
    let started = tokio::time::Instant::now();
    loop {
        let task = coder::task::Store::open(&f.tasks)
            .unwrap()
            .show("synthetic")
            .unwrap();
        if task
            .run
            .as_ref()
            .is_some_and(|run| run.process_id.is_some())
        {
            break;
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let read = f.read(&grant);
    let before = exchange(&mut f, &url, "read", &read, &secret, &Blobs::default()).await;
    assert_eq!(before["coverage"], "partial");
    let revision = coder::task::Store::open(&f.tasks)
        .unwrap()
        .show("synthetic")
        .unwrap()
        .revision;
    let cancel = f.command(
        &grant,
        "cancel",
        json!({"reason":"Authenticated loopback cancellation"}),
        revision,
    );
    assert_eq!(
        exchange(&mut f, &url, "command", &cancel, &secret, &Blobs::default()).await["status"],
        "accepted"
    );
    let stopped = running.await.unwrap().unwrap();
    assert_eq!(stopped.execution, coder::task::Execution::Stopped);
    assert!(
        stopped
            .run
            .as_ref()
            .unwrap()
            .result
            .as_ref()
            .unwrap()
            .group_clear
    );
    assert!(
        !std::path::Path::new(&stopped.intent.workspace.path)
            .join("late.txt")
            .exists()
    );
    let mut history = f.read(&grant);
    history["view"] = json!("history");
    history["max_items"] = json!(1);
    let first = exchange(&mut f, &url, "read", &history, &secret, &Blobs::default()).await;
    assert!(first["next"].is_string());
    let cursor = first["next"].clone();
    history["request"] = json!(client::random_id());
    history["after"] = cursor;
    drop(f.host);
    let page = read_in_new_process(&f.host_directory, &f.setup, &history, &secret);
    f.host = Host::open(&f.host_directory, f.setup.clone(), f.authority).unwrap();
    assert_eq!(page["captured_at"], first["captured_at"]);
    assert!(!page["items"].as_array().unwrap().is_empty());
    if let Some(directory) = std::env::var_os("CODER_CONTROL_TEST_EVIDENCE") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let retained = events.lock().await.clone();
        std::fs::write(
            directory.join("signed-events.json"),
            serde_json::to_vec_pretty(&retained).unwrap(),
        )
        .unwrap();
        std::fs::write(directory.join("receipt.json"),serde_json::to_vec_pretty(&json!({"schema":"openagents.control-loopback-proof.v1","transport":"actual-authenticated-websocket-test-relay","principals":2,"paid_model_calls":0,"task_execution":stopped.execution,"task_verification":stopped.checks,"cost_usd":null,"cost_status":"unknown","process_group_clear":true,"late_write_absent":true,"correction_recorded":true,"finite_cut_survived_process_restart":true,"signed_event_count":retained.len(),"first":first,"next":page})).unwrap()).unwrap();
        let run = stopped.run.unwrap();
        std::fs::copy(
            f.tasks.join(run.admission.trace_file),
            directory.join("task.atif.jsonl"),
        )
        .unwrap();
    }
    server.abort();
}
