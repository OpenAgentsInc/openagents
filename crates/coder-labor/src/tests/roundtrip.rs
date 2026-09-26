//! Real bounded processes over synthetic source, with separate buyer authority.
use super::*;
use coder::task::{self, RequestedConfiguration, TaskIntent, Workspace};
use std::path::{Path, PathBuf};

fn git(directory: &Path, args: &[&str]) -> String {
    let result = std::process::Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}
fn repository(root: &Path, name: &str, filename: &str, bytes: &[u8]) -> PathBuf {
    let repo = root.join(format!("{name}-repository"));
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    std::fs::write(repo.join(filename), bytes).unwrap();
    git(&repo, &["add", filename]);
    git(
        &repo,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "Synthetic source",
        ],
    );
    let work = root.join(format!("{name}-workspace"));
    git(
        &repo,
        &["worktree", "add", "--detach", "-q", work.to_str().unwrap()],
    );
    work.canonicalize().unwrap()
}
fn intent(workspace: &Path, title: &str) -> TaskIntent {
    TaskIntent {
        title: title.into(),
        prompt: title.into(),
        workspace: Workspace {
            path: workspace.to_string_lossy().into(),
            source_revision: Some(git(workspace, &["rev-parse", "HEAD"])),
        },
        configuration: RequestedConfiguration {
            adapter: "bounded-command".into(),
            model: None,
        },
    }
}
fn make_grant(intent: &TaskIntent, id: &str, program: &Path, args: Vec<String>) -> Value {
    json!({"schema":task::owner::GRANT_SCHEMA,"task_id":id,"intent_digest":nostr::contracts::digest_bytes(&serde_json::to_vec(intent).unwrap()),"expected_revision":1,"expected_source_snapshot":coder_boundary::Snapshot::observe(Path::new(&intent.workspace.path)).digest(),"program":program.canonicalize().unwrap(),"arguments":args,"write_workspace":id=="labor-request","wall_seconds":10,"stream_bytes":4096,"memory_bytes":268435456,"requirements":null})
}
async fn transmit(
    f: &Fixture,
    url: &str,
    buyer: &mut store::Store,
    provider: &mut store::Store,
    value: &Value,
    schema: &str,
    from_buyer: bool,
) -> Event {
    let (from, to) = if from_buyer {
        (&f.buyer, &f.provider)
    } else {
        (&f.provider, &f.buyer)
    };
    let event = sealed(value, schema, from, to, f.now);
    transport::publish(url, from, &event).await.unwrap();
    for (key, store) in [(&f.buyer, buyer), (&f.provider, provider)] {
        let received = transport::fetch(url, key, &event.id).await.unwrap();
        assert_eq!(
            store.receive(received, f.now, f.blobs.clone()).unwrap(),
            "applied"
        );
    }
    event
}

#[tokio::test]
async fn bounded_coding_order_runs_separate_buyer_check_and_accepts_over_relay() {
    let retained = std::env::var_os("CODER_LABOR_ACCEPTANCE_DIR").map(PathBuf::from);
    let root = if let Some(parent) = &retained {
        std::fs::create_dir_all(parent).unwrap();
        tempfile::Builder::new()
            .prefix("labor-")
            .tempdir_in(parent)
            .unwrap()
    } else {
        tempfile::tempdir().unwrap()
    };
    let expected = b"pub fn answer() -> u32 { 42 }\n";
    let workspace = repository(
        root.path(),
        "provider",
        "answer.rs",
        b"pub fn answer() -> u32 { 41 }\n",
    );
    let checker_workspace = repository(root.path(), "buyer-check", "expected.rs", expected);
    let task_intent = intent(
        &workspace,
        "Repair answer() to return 42; deliver the changed Rust file.",
    );
    let grant = make_grant(
        &task_intent,
        "labor-request",
        Path::new("/bin/sh"),
        vec![
            "-c".into(),
            "printf 'pub fn answer() -> u32 { 42 }\\n' > answer.rs".into(),
        ],
    );
    let input = json!({"v":"coder.free-labor.command.v1","requires":[],"intent":task_intent,"source_snapshot":grant["expected_source_snapshot"],"expected_output_digest":nostr::contracts::digest_bytes(expected)});
    let requirements = json!({"v":"coder.free-labor.requirements.v1","requires":[],"program_digest":nostr::contracts::digest_bytes(&std::fs::read(grant["program"].as_str().unwrap()).unwrap()),"arguments_digest":nostr::contracts::digest_bytes(&jcs(&grant["arguments"]).unwrap()),"write_workspace":true,"wall_seconds":10,"stream_bytes":4096,"memory_bytes":268435456});
    let mut f = fixture_with(Some((input, requirements)));
    let (url, relay, events) = relay::start().await;
    let buyer_path = root.path().join("buyer");
    let provider_path = root.path().join("provider");
    let tasks_path = root.path().join("provider-tasks");
    let mut buyer = store::Store::open(&buyer_path, f.setup.clone(), f.buyer).unwrap();
    let mut provider = store::Store::open(&provider_path, f.setup.clone(), f.provider).unwrap();
    for event in &f.events {
        let sender = if event.pubkey == public(&f.buyer) {
            &f.buyer
        } else {
            &f.provider
        };
        transport::publish(&url, sender, event).await.unwrap();
        for (key, store) in [(&f.buyer, &mut buyer), (&f.provider, &mut provider)] {
            let received = transport::fetch(&url, key, &event.id).await.unwrap();
            assert_eq!(
                store.receive(received, f.now, Blobs::default()).unwrap(),
                "applied"
            );
        }
    }
    let mut scratch = agreed(&f, f.provider);
    let execute = link(&mut f, &mut scratch);
    let linkage = scratch
        .records
        .resolve(scratch.records.link.as_ref().unwrap())
        .unwrap()
        .clone();
    transmit(
        &f,
        &url,
        &mut buyer,
        &mut provider,
        &linkage,
        records::LINK,
        true,
    )
    .await;
    let grant_bytes = serde_json::to_vec(&grant).unwrap();
    // Fault injection at the durable-intent-before-submit boundary. This is
    // an interrupted journal fixture, not a claim that an OS crash occurred.
    let pending_path = root.path().join("interrupted-provider");
    drop(store::Store::open(&pending_path, f.setup.clone(), f.provider).unwrap());
    let pending_tasks = root.path().join("interrupted-tasks");
    drop(task::Store::open(&pending_tasks).unwrap());
    let mut pending: Value =
        serde_json::from_slice(&std::fs::read(provider_path.join("labor.json")).unwrap()).unwrap();
    pending["dispatch"] = json!({"execute":execute,"grant_digest":nostr::contracts::digest_bytes(&grant_bytes),"task_id":"labor-request","task_directory":pending_tasks,"observation":null,"state":"unknown"});
    std::fs::write(
        pending_path.join("labor.json"),
        serde_json::to_vec(&pending).unwrap(),
    )
    .unwrap();
    let mut interrupted = store::Store::open(&pending_path, f.setup.clone(), f.provider).unwrap();
    assert_eq!(interrupted.reconcile().unwrap().state, "unknown");
    assert_eq!(
        interrupted
            .dispatch(execute.clone(), &grant_bytes, &pending_tasks, f.now)
            .await
            .unwrap()
            .state,
        "unknown"
    );
    assert!(
        task::Store::open(&pending_tasks)
            .unwrap()
            .show("labor-request")
            .is_err()
    );
    drop(interrupted);
    let result = provider
        .dispatch(execute.clone(), &grant_bytes, &tasks_path, f.now)
        .await
        .unwrap();
    assert_eq!(result.state, "finished");
    let task = result.observation.as_ref().unwrap();
    let produced =
        task::artifact::read(&tasks_path, "labor-request", Path::new("answer.rs")).unwrap();
    assert_eq!(produced, expected);
    let first_run = task.run.as_ref().unwrap().clone();
    assert_eq!(
        provider
            .dispatch(execute.clone(), &grant_bytes, &tasks_path, f.now)
            .await
            .unwrap()
            .observation
            .unwrap()
            .run
            .unwrap(),
        first_run
    );
    drop(provider);
    let mut provider = store::Store::open(&provider_path, f.setup.clone(), f.provider).unwrap();
    assert_eq!(provider.reconcile().unwrap().state, "finished");
    let output = put(
        &mut f.blobs,
        json!({"v":"openagents.free-labor.patch.v1","base":task_intent.workspace.source_revision,"source_snapshot":grant["expected_source_snapshot"],"candidate_snapshot":first_run.result.as_ref().unwrap().candidate_snapshot,"files":[{"path":"answer.rs","utf8":String::from_utf8(produced.clone()).unwrap()}]}),
        "openagents.free-labor.patch.v1",
    );
    let observation = put(
        &mut f.blobs,
        serde_json::to_value(&result).unwrap(),
        "openagents.free-labor.dispatch.v1",
    );
    let runs = run_evidence(&mut f, &provider.book, &output, &observation, "completed");
    let submission = json!({"v":records::SUBMISSION,"requires":[],"issuer":public(&f.provider),"order":order_value(&provider.book),"number":0,"previous":null,"rework":null,"executions":[provider.book.records.link],"deliverables":[{"id":"patch","content":output}],"run_evidence":runs,"limitations":observation});
    transmit(
        &f,
        &url,
        &mut buyer,
        &mut provider,
        &submission,
        records::SUBMISSION,
        false,
    )
    .await;
    f.now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let delivery = json!({"v":records::DELIVERY,"requires":[],"issuer":public(&f.buyer),"order":order_value(&buyer.book),"submission":buyer.book.records.submission,"received_at":f.now,"available":true});
    // The receiver supplies its observed receipt time, never the provider's timestamp.
    transmit(
        &f,
        &url,
        &mut buyer,
        &mut provider,
        &delivery,
        records::DELIVERY,
        true,
    )
    .await;

    // This checker workspace is never writable by the provider. The expected
    // bytes were chosen before dispatch; worker output enters as data only.
    assert_eq!(
        f.blobs.get(&f.setup.admission.input).unwrap()["expected_output_digest"],
        nostr::contracts::digest_bytes(
            &std::fs::read(checker_workspace.join("expected.rs")).unwrap()
        )
    );
    std::fs::write(checker_workspace.join("candidate.rs"), &produced).unwrap();
    let checker_intent = intent(
        &checker_workspace,
        "Compare retained candidate bytes with the frozen buyer acceptance fixture.",
    );
    let checker_grant = make_grant(
        &checker_intent,
        "buyer-check",
        Path::new("/usr/bin/cmp"),
        vec!["expected.rs".into(), "candidate.rs".into()],
    );
    let checker_directory = root.path().join("buyer-check-tasks");
    {
        let mut tasks = task::Store::open(&checker_directory).unwrap();
        tasks.apply(&serde_json::to_vec(&json!({"schema":task::COMMAND_SCHEMA,"command_id":"check-one","task_id":"buyer-check","expected_revision":null,"action":{"type":"submit","intent":checker_intent}})).unwrap()).unwrap();
    }
    let check = task::owner::execute(
        &checker_directory,
        &serde_json::to_vec(&checker_grant).unwrap(),
    )
    .await
    .unwrap();
    let checked = check.run.as_ref().unwrap().result.as_ref().unwrap();
    assert_eq!(checked.exit_code, Some(0));
    assert!(!checked.output_incomplete);
    let check_evidence = put(
        &mut f.blobs,
        serde_json::to_value(&check).unwrap(),
        "openagents.free-labor.check-observation.v1",
    );
    let criteria = json!([{"id":"result","verdict":"passed","evidence":[check_evidence]}]);
    let receipt = put(
        &mut f.blobs,
        json!({"v":"openagents.free-labor.checker.v1","requires":[],"submission":buyer.book.records.submission,"checker":f.setup.admission.checker,"lock":artifact_value(&buyer.book.policy().lock),"input":f.setup.admission.input,"criteria":criteria,"verdict":"passed","elapsed_ms":checked.elapsed_ms,"cost_usd":null,"evidence":[check_evidence],"limitations":observation}),
        "openagents.free-labor.checker.v1",
    );
    let verification = json!({"v":records::VERIFICATION,"requires":[],"issuer":public(&f.buyer),"order":order_value(&buyer.book),"submission":buyer.book.records.submission,"policy":artifact_value(&buyer.book.labor().acceptance_policy),"checker_receipts":[receipt],"criteria":criteria,"verdict":"passed","limitations":observation});
    transmit(
        &f,
        &url,
        &mut buyer,
        &mut provider,
        &verification,
        records::VERIFICATION,
        true,
    )
    .await;
    assert!(buyer.book.records.acceptance.is_none());
    let review = json!({"v":records::REVIEW,"requires":[],"issuer":public(&f.buyer),"order":order_value(&buyer.book),"submission":buyer.book.records.submission,"verification":buyer.book.records.verification,"decision":"accept","criteria":[],"reason":observation});
    transmit(
        &f,
        &url,
        &mut buyer,
        &mut provider,
        &review,
        records::REVIEW,
        true,
    )
    .await;
    let acceptance = json!({"v":records::ACCEPTANCE,"requires":[],"issuer":public(&f.buyer),"order":order_value(&buyer.book),"submission":buyer.book.records.submission,"verification":buyer.book.records.verification,"outcome":"accepted","basis":"buyer_acceptance","review":buyer.book.records.review,"resolution":null,"amount_due_msat":0,"supersedes":[],"evidence":[buyer.book.records.delivery]});
    let final_event = transmit(
        &f,
        &url,
        &mut buyer,
        &mut provider,
        &acceptance,
        records::ACCEPTANCE,
        true,
    )
    .await;
    assert_eq!(
        provider
            .receive(final_event.clone(), f.now, Blobs::default())
            .unwrap(),
        "duplicate"
    );
    let retained_events = events.lock().await.clone();
    let count = retained_events.len();
    std::fs::write(
        root.path().join("relay-events.json"),
        serde_json::to_vec_pretty(&retained_events).unwrap(),
    )
    .unwrap();
    relay.abort();
    drop(buyer);
    drop(provider);
    let buyer = store::Store::open(&buyer_path, f.setup.clone(), f.buyer).unwrap();
    let provider = store::Store::open(&provider_path, f.setup.clone(), f.provider).unwrap();
    assert_eq!(
        buyer.book.records.acceptance,
        provider.book.records.acceptance
    );
    assert!(buyer.book.records.acceptance.is_some());
    let report = json!({"schema":"openagents.free-labor-acceptance-fixture.v1","synthetic_source":true,"independent_operators":false,"production_relay":false,"encrypted_events":count,"provider_elapsed_ms":first_run.result.unwrap().elapsed_ms,"buyer_checker_elapsed_ms":checked.elapsed_ms,"inference_calls":0,"all_in_cost_usd":null,"cost_reason":"host CPU, storage, and energy are unmetered","free_order_price_msat":0,"payment":"not_applicable","acceptance_event":final_event.id,"source":"one synthetic Rust file","result":"accepted","duplicate_execution":"same retained run","restart":"both roles reconstructed acceptance after relay shutdown"});
    std::fs::write(
        root.path().join("receipt.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    if retained.is_some() {
        let path = root.keep();
        eprintln!("Retained synthetic labor evidence at {}", path.display());
    }
}
