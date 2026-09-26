use super::*;

fn fixture() -> (tempfile::TempDir, tempfile::TempDir, Grant) {
    let root = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let repository = workspace.path().join("repo");
    let checkout = workspace.path().join("checkout");
    std::fs::create_dir(&repository).unwrap();
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
            std::process::Command::new("git")
                .args(args)
                .current_dir(&repository)
                .status()
                .unwrap()
                .success()
        );
    }
    assert!(
        std::process::Command::new("git")
            .args(["worktree", "add", "--detach", "-q"])
            .arg(&checkout)
            .current_dir(&repository)
            .status()
            .unwrap()
            .success()
    );
    let mut store = Store::open(&root.path().join("store")).unwrap();
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: "submit-one".into(),
        task_id: "task-one".into(),
        expected_revision: None,
        action: Action::Submit {
            intent: TaskIntent {
                title: "Fixture".into(),
                prompt: "Write one output.".into(),
                workspace: Workspace {
                    path: checkout.canonicalize().unwrap().display().to_string(),
                    source_revision: None,
                },
                configuration: RequestedConfiguration {
                    adapter: "bounded-command".into(),
                    model: None,
                },
            },
        },
    };
    store.apply(&serde_json::to_vec(&command).unwrap()).unwrap();
    let task = store.show("task-one").unwrap();
    let grant = Grant {
        adapter_configuration: None,
        schema: GRANT_SCHEMA.into(),
        task_id: task.task_id,
        intent_digest: task.intent_digest,
        expected_revision: 1,
        program: Path::new("/bin/sh").canonicalize().unwrap(),
        arguments: vec![
            "-c".into(),
            "printf output > result.txt; printf transcript".into(),
        ],
        write_workspace: true,
        wall_seconds: 5,
        stream_bytes: 4096,
        memory_bytes: 256 * 1024 * 1024,
        requirements: None,
        expected_source_snapshot: None,
    };
    (root, workspace, grant)
}

#[tokio::test]
async fn executes_once_with_retained_intent_trace_and_unknown_cost() {
    let (root, _workspace, grant) = fixture();
    let dir = root.path().join("store");
    let bytes = serde_json::to_vec(&grant).unwrap();
    let task = execute(&dir, &bytes).await.unwrap();
    assert_eq!(task.execution, Execution::Finished);
    assert_eq!(task.checks, Checks::NotRun);
    let run = task.run.as_ref().unwrap();
    assert!(run.effect_id.is_some());
    assert_eq!(run.result.as_ref().unwrap().cost_status, "unknown");
    assert_eq!(run.result.as_ref().unwrap().exit_code, Some(0));
    let trace = atif::log::read_whole(&dir.join(&run.admission.trace_file)).unwrap();
    assert!(trace.document().to_string().contains("transcript"));
    assert!(execute(&dir, &bytes).await.is_err());
    assert_eq!(Store::open(&dir).unwrap().show("task-one").unwrap(), task);
    assert_eq!(
        artifact::read(&dir, "task-one", Path::new("result.txt")).unwrap(),
        b"output"
    );
    std::fs::remove_file(Path::new(&task.intent.workspace.path).join("result.txt")).unwrap();
    assert_eq!(
        artifact::read(&dir, "task-one", Path::new("result.txt")).unwrap(),
        b"output"
    );
    assert!(artifact::read(&dir, "task-one", Path::new("../outside")).is_err());
}

#[tokio::test]
async fn cancellation_acknowledges_before_process_cleanup_and_keeps_original_receipt() {
    let (root, _workspace, mut grant) = fixture();
    grant.arguments[1] = "printf started; sleep 20; printf late > late.txt".into();
    let dir = root.path().join("store");
    let run_dir = dir.clone();
    let handle =
        tokio::spawn(async move { execute(&run_dir, &serde_json::to_vec(&grant).unwrap()).await });
    let deadline = Instant::now() + Duration::from_secs(10);
    let receipt = loop {
        assert!(Instant::now() < deadline, "owner did not start");
        let mut store = Store::open(&dir).unwrap();
        let task = store.show("task-one").unwrap();
        if task.run.as_ref().is_some_and(|run| run.effect_id.is_some()) {
            let command = Command {
                schema: COMMAND_SCHEMA.into(),
                command_id: "cancel-one".into(),
                task_id: task.task_id,
                expected_revision: Some(task.revision),
                action: Action::Cancel {
                    reason: "Stop the fixture.".into(),
                },
            };
            let bytes = serde_json::to_vec(&command).unwrap();
            let receipt = store.apply(&bytes).unwrap();
            assert_eq!(receipt.status, Status::CancelRequested);
            assert_eq!(receipt.execution, Execution::Running);
            assert_eq!(store.apply(&bytes).unwrap(), receipt);
            break receipt;
        }
        drop(store);
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    let task = handle.await.unwrap().unwrap();
    assert_eq!(task.execution, Execution::Stopped);
    assert!(
        task.run
            .as_ref()
            .unwrap()
            .result
            .as_ref()
            .unwrap()
            .group_clear
    );
    assert!(task.revision > receipt.revision);
    assert!(
        !Path::new(&task.intent.workspace.path)
            .join("late.txt")
            .exists()
    );
}

#[tokio::test]
async fn a_live_owner_refuses_competitors_and_recovery_never_reexecutes() {
    let (root, _workspace, mut grant) = fixture();
    grant.arguments[1] = "sleep 20".into();
    let dir = root.path().join("store");
    let run_dir = dir.clone();
    let bytes = serde_json::to_vec(&grant).unwrap();
    let run_bytes = bytes.clone();
    let handle = tokio::spawn(async move { execute(&run_dir, &run_bytes).await });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "owner did not start");
        let task = Store::open(&dir).unwrap().show("task-one").unwrap();
        if task.run.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(matches!(recover(&dir, "task-one"), Err(Error::Busy)));
    assert!(matches!(execute(&dir, &bytes).await, Err(Error::Busy)));
    handle.abort();
    let _ = handle.await;
    let task = recover(&dir, "task-one").unwrap();
    assert_eq!(task.execution, Execution::Unknown);
    assert_eq!(task.run.as_ref().unwrap().epoch, 2);
    assert!(execute(&dir, &bytes).await.is_err());
    assert_eq!(recover(&dir, "task-one").unwrap(), task);
    tokio::time::sleep(Duration::from_millis(500)).await;
}

#[tokio::test]
async fn read_boundary_and_source_pin_are_enforced() {
    let (root, _workspace, mut grant) = fixture();
    let dir = root.path().join("store");
    let secret = root.path().join("outside.txt");
    std::fs::write(&secret, "private fixture").unwrap();
    grant.write_workspace = false;
    grant.arguments[1] = format!("cat '{}'; printf forbidden > result.txt", secret.display());
    let task = execute(&dir, &serde_json::to_vec(&grant).unwrap())
        .await
        .unwrap();
    assert_eq!(task.execution, Execution::Failed);
    assert!(
        !Path::new(&task.intent.workspace.path)
            .join("result.txt")
            .exists()
    );
    let text = std::fs::read_to_string(dir.join(&task.run.unwrap().admission.trace_file)).unwrap();
    assert!(!text.contains("private fixture"));
}

#[test]
fn grants_refuse_unknown_fields_and_unbounded_execution() {
    let (_root, _workspace, grant) = fixture();
    let mut value = serde_json::to_value(grant).unwrap();
    value["wall_seconds"] = json!(0);
    assert!(Grant::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    value["wall_seconds"] = json!(5);
    value["network"] = json!(true);
    assert!(Grant::parse(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[tokio::test]
async fn failure_at_every_effect_barrier_never_replays_uncertain_work() {
    for point in [
        "after_admission",
        "after_intent",
        "after_dispatch",
        "before_result",
        "after_result",
    ] {
        let (root, _workspace, grant) = fixture();
        let dir = root.path().join("store");
        let bytes = serde_json::to_vec(&grant).unwrap();
        OWNER_FAULT.with(|fault| fault.set(Some(point)));
        assert!(execute(&dir, &bytes).await.is_err(), "{point}");
        let recovered = recover(&dir, "task-one").unwrap();
        assert_eq!(
            recovered.execution,
            if point == "after_result" {
                Execution::Finished
            } else {
                Execution::Unknown
            },
            "{point}"
        );
        assert!(execute(&dir, &bytes).await.is_err(), "{point}");
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

#[tokio::test]
async fn changed_grant_and_source_are_refused_before_admission() {
    let (root, _workspace, mut grant) = fixture();
    let dir = root.path().join("store");
    grant.intent_digest = format!("sha256:{}", "0".repeat(64));
    assert!(matches!(
        execute(&dir, &serde_json::to_vec(&grant).unwrap()).await,
        Err(Error::RevisionMismatch)
    ));
    assert!(
        Store::open(&dir)
            .unwrap()
            .show("task-one")
            .unwrap()
            .run
            .is_none()
    );
}

#[test]
fn legacy_inbox_reads_without_rewrite_and_migrates_on_first_mutation() {
    let (root, _workspace, _) = fixture();
    let dir = root.path().join("store");
    let path = dir.join(STORE_FILE);
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["schema"] = json!("openagents.coder.task-store.v1");
    value.as_object_mut().unwrap().remove("host_events");
    let bytes = serde_json::to_vec(&value).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let mut store = Store::open(&dir).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: "cancel-legacy".into(),
        task_id: "task-one".into(),
        expected_revision: Some(1),
        action: Action::Cancel {
            reason: "Stop.".into(),
        },
    };
    store.apply(&serde_json::to_vec(&command).unwrap()).unwrap();
    drop(store);
    assert_eq!(
        Store::open(&dir).unwrap().show("task-one").unwrap().status,
        Status::Cancelled
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap()).unwrap()["schema"],
        STORE_SCHEMA
    );
}

fn requirements(host: &Path, command: &str) -> checks::Requirements {
    use crate::capability::{self, Entry, Source};
    use std::os::unix::fs::PermissionsExt;
    let program = host.join("check-suite.sh");
    let script = format!(
        "#!/bin/sh\nset -eu\nsuite=$(shasum -a 256 \"$0\" | cut -d ' ' -f 1)\nemit() {{ printf '{{\"schema\":\"openagents.verification.v1\",\"suite_digest\":\"sha256:%s\",\"input_digest\":\"%s\",\"verdict\":\"%s\"}}' \"$suite\" \"$2\" \"$1\"; }}\n{command}\n"
    );
    std::fs::write(&program, &script).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let program = program.canonicalize().unwrap();
    let suite_digest = nostr::contracts::digest_bytes(script.as_bytes());
    let manifest = host.join("check.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(&capability::executor_document(
            "task-check-fixture",
            program.to_str().unwrap(),
            vec![program.display().to_string(), "--version".into()],
            json!({"name":"Task check fixture","invoke":[program],"isolation":["directory"]}),
        ))
        .unwrap(),
    )
    .unwrap();
    let entry = Entry::load(&manifest, Source::Operator).unwrap();
    checks::Requirements {
        schema: checks::REQUIREMENTS_SCHEMA.into(),
        version: 1,
        requirements: vec![checks::Requirement {
            id: "output".into(),
            statement: "Produce the requested output.".into(),
            checks: vec!["content".into()],
        }],
        plan: json!({"schema":"openagents.verification.v1","input_digest":checks::CANDIDATE,"seconds":5,"allow_unrestricted_reads":true,"allow_network":true,"checks":[{"id":"content","manifest":manifest,"manifest_digest":entry.digest,"arguments":[checks::CANDIDATE],"seconds":3,"output_bytes":4096,"acceptance":{"kind":"suite","suite_digest":suite_digest,"input_digest":checks::CANDIDATE}}]}),
        instruction_targets: vec![PathBuf::from("nested/result.txt")],
        source_exclusions: vec!["benchmark-official-outcomes".into()],
    }
}

#[tokio::test]
async fn independent_checks_reject_false_green_missing_and_stale_evidence() {
    use crate::capability::Trust;
    let passed = r#"test "$(cat result.txt)" = output || exit 2; emit passed "$1""#;
    for (command, expected) in [
        (passed, Checks::Passed),
        ("test \"$(cat result.txt)\" = other", Checks::Failed),
        ("printf done", Checks::Unavailable),
        ("emit passed stale", Checks::Unavailable),
    ] {
        let (root, _workspace, mut grant) = fixture();
        grant.requirements = Some(requirements(root.path(), command));
        let dir = root.path().join("store");
        let task = execute(&dir, &serde_json::to_vec(&grant).unwrap())
            .await
            .unwrap();
        assert_eq!(task.execution, Execution::Finished);
        assert_eq!(task.checks, Checks::NotRun);
        let checked = check(&dir, "task-one", &Trust::everything()).await.unwrap();
        assert_eq!(checked.execution, Execution::Finished);
        assert_eq!(checked.checks, expected);
        assert_eq!(
            Store::open(&dir).unwrap().show("task-one").unwrap(),
            checked
        );
        assert!(checked.run.as_ref().unwrap().check_report.is_some());
        assert!(check(&dir, "task-one", &Trust::everything()).await.is_err());
    }
}

#[tokio::test]
async fn scoped_context_and_corrections_survive_replay_without_relabeling_effects() {
    let (root, _workspace, mut grant) = fixture();
    let dir = root.path().join("store");
    let initial = Store::open(&dir).unwrap().show("task-one").unwrap();
    let workspace = Path::new(&initial.intent.workspace.path);
    std::fs::create_dir(workspace.join("nested")).unwrap();
    std::fs::write(workspace.join("AGENTS.md"), "Root instructions.").unwrap();
    std::fs::write(workspace.join("nested/AGENTS.md"), "Nested instructions.").unwrap();
    grant.requirements = Some(requirements(root.path(), "true"));
    let task = execute(&dir, &serde_json::to_vec(&grant).unwrap())
        .await
        .unwrap();
    let run = task.run.clone().unwrap();
    assert_eq!(run.admission.context.instructions.len(), 2);
    assert_eq!(
        run.admission.context.instructions[0].path,
        Path::new("AGENTS.md")
    );
    assert_eq!(
        run.admission.context.instructions[1].scope,
        Path::new("nested")
    );
    let correction = serde_json::to_vec(&Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: "correct-one".into(),
        task_id: task.task_id.clone(),
        expected_revision: Some(task.revision),
        action: Action::Correct {
            prompt: "Use the corrected requirements.".into(),
            reason: "The requested output changed.".into(),
        },
    })
    .unwrap();
    let receipt = Store::open(&dir).unwrap().apply(&correction).unwrap();
    assert_eq!(receipt.checks, Checks::Disputed);
    let corrected = Store::open(&dir).unwrap().show("task-one").unwrap();
    assert_eq!(corrected.run.as_ref().unwrap(), &run);
    assert_eq!(corrected.intent, initial.intent);
    assert_eq!(
        corrected.effective_prompt(),
        "Use the corrected requirements."
    );
    assert!(
        check(&dir, "task-one", &crate::capability::Trust::everything())
            .await
            .is_err()
    );
    assert_eq!(
        Store::open(&dir).unwrap().apply(&correction).unwrap(),
        receipt
    );
}

#[tokio::test]
async fn changed_candidate_or_missing_checker_is_retained_as_unavailable() {
    for change in [true, false] {
        let (root, _workspace, mut grant) = fixture();
        let dir = root.path().join("store");
        grant.requirements = Some(requirements(root.path(), "true"));
        let task = execute(&dir, &serde_json::to_vec(&grant).unwrap())
            .await
            .unwrap();
        if change {
            std::fs::write(
                Path::new(&task.intent.workspace.path).join("result.txt"),
                "changed",
            )
            .unwrap();
        } else {
            std::fs::remove_file(root.path().join("check.json")).unwrap();
        }
        let checked = check(&dir, "task-one", &crate::capability::Trust::everything())
            .await
            .unwrap();
        assert_eq!(checked.checks, Checks::Unavailable);
        assert!(checked.run.unwrap().check_report.unwrap().reason.is_some());
    }
}

#[tokio::test]
async fn declared_source_snapshot_mismatch_refuses_before_admission() {
    let (root, _workspace, mut grant) = fixture();
    grant.expected_source_snapshot = Some("0".repeat(64));
    let dir = root.path().join("store");
    assert!(
        execute(&dir, &serde_json::to_vec(&grant).unwrap())
            .await
            .is_err()
    );
    assert!(
        Store::open(&dir)
            .unwrap()
            .show("task-one")
            .unwrap()
            .run
            .is_none()
    );
}

#[tokio::test]
async fn identical_candidate_manifests_are_reused_for_distinct_tasks() {
    let (root, _workspace, mut grant) = fixture();
    let dir = root.path().join("store");
    grant.arguments = vec!["-c".into(), "printf first".into()];
    let first = execute(&dir, &serde_json::to_vec(&grant).unwrap())
        .await
        .unwrap();
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: "submit-two".into(),
        task_id: "task-two".into(),
        expected_revision: None,
        action: Action::Submit {
            intent: first.intent.clone(),
        },
    };
    Store::open(&dir)
        .unwrap()
        .apply(&serde_json::to_vec(&command).unwrap())
        .unwrap();
    let second = Store::open(&dir).unwrap().show("task-two").unwrap();
    grant.task_id = second.task_id;
    grant.intent_digest = second.intent_digest;
    let second = execute(&dir, &serde_json::to_vec(&grant).unwrap())
        .await
        .unwrap();
    assert_eq!(second.execution, Execution::Finished);
    assert_eq!(
        first.run.unwrap().result.unwrap().artifact_digest,
        second.run.unwrap().result.unwrap().artifact_digest
    );
}

#[tokio::test]
async fn cancellation_before_result_commit_cannot_be_reduced_as_finished() {
    let (root, _workspace, grant) = fixture();
    let dir = root.path().join("store");
    OWNER_FAULT.with(|fault| fault.set(Some("before_result")));
    assert!(
        execute(&dir, &serde_json::to_vec(&grant).unwrap())
            .await
            .is_err()
    );
    let task = Store::open(&dir).unwrap().show("task-one").unwrap();
    let path = dir.join(&task.run.as_ref().unwrap().admission.trace_file);
    let recording = atif::log::read_whole(&path).unwrap();
    let mut result: ResultRecord = serde_json::from_value(
        recording
            .steps
            .iter()
            .find_map(|step| step.extensions.get("result"))
            .unwrap()
            .clone(),
    )
    .unwrap();
    assert_eq!(result.exit_code, Some(0));
    assert!(!result.stop_requested);
    result.trace_digest = digest_bytes(&std::fs::read(&path).unwrap());
    let cancel = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: "cancel-before-seal".into(),
        task_id: task.task_id.clone(),
        expected_revision: Some(task.revision),
        action: Action::Cancel {
            reason: "Stop before sealing.".into(),
        },
    };
    let mut store = Store::open(&dir).unwrap();
    store.apply(&serde_json::to_vec(&cancel).unwrap()).unwrap();
    let owner = Owner::acquire(&store, "task-one").unwrap();
    let task = store
        .record(
            &owner,
            Event::Result {
                result: result.clone(),
            },
            1,
        )
        .unwrap();
    assert_eq!(task.execution, Execution::Stopped);
    assert_eq!(task.run.as_ref().unwrap().result.as_ref(), Some(&result));
    drop(store);
    assert_eq!(Store::open(&dir).unwrap().show("task-one").unwrap(), task);
}
