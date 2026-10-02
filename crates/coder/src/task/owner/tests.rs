use super::*;

pub(super) fn fixture() -> (tempfile::TempDir, tempfile::TempDir, Grant) {
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
                images: Vec::new(),
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
async fn landed_v1_context_admission_replays_without_inventing_lineage() {
    let (root, _workspace, grant) = fixture();
    let directory = root.path().join("store");
    let queued = Store::open(&directory).unwrap().show("task-one").unwrap();
    let finished = execute(&directory, &serde_json::to_vec(&grant).unwrap())
        .await
        .unwrap();
    let mut admission = finished.run.unwrap().admission;
    admission.context.schema = "openagents.coder.task-context.v1".into();
    admission.context.digest = atif::digest(&json!({
        "schema":admission.context.schema,"task_revision":admission.context.task_revision,
        "prompt":admission.context.prompt,"instructions":admission.context.instructions,
        "suites":admission.context.suites,
    }));
    let record = Record {
        sequence: 1,
        task_id: queued.task_id.clone(),
        epoch: 1,
        event: Event::Admitted {
            admission: Box::new(admission),
        },
    };
    let bytes = serde_json::to_vec(&record).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("\"lineage\""));
    let recorded: Record = serde_json::from_slice(&bytes).unwrap();
    let mut tasks = BTreeMap::from([(queued.task_id.clone(), queued)]);
    transition(&recorded, &mut tasks).unwrap();
    let context = &tasks["task-one"].run.as_ref().unwrap().admission.context;
    assert_eq!(context.schema, "openagents.coder.task-context.v1");
    assert!(context.knowledge.is_empty());
    assert!(context.lineage.checks.is_empty());
    assert_eq!(serde_json::to_vec(&recorded).unwrap(), bytes);
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
async fn a_steer_is_consumed_when_the_next_turn_starts_and_recorded_as_its_own_step() {
    let (root, _workspace, mut grant) = fixture();
    let dir = root.path().join("store");
    let correction = serde_json::to_vec(&Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: "steer-one".into(),
        task_id: "task-one".into(),
        expected_revision: Some(1),
        action: Action::Correct {
            prompt: "Write the corrected output.".into(),
            reason: "Steered before the turn started".into(),
        },
    })
    .unwrap();
    Store::open(&dir).unwrap().apply(&correction).unwrap();
    let queued = Store::open(&dir).unwrap().show("task-one").unwrap();
    // Accepted is not consumed: the ledger still holds the steer.
    assert_eq!(queued.unconsumed_steers().len(), 1);
    grant.expected_revision = queued.revision;
    let task = execute(&dir, &serde_json::to_vec(&grant).unwrap())
        .await
        .unwrap();
    let run = task.run.as_ref().unwrap();
    assert_eq!(run.admission.context.task_revision, 2);
    assert_eq!(run.admission.context.prompt, "Write the corrected output.");
    assert!(task.unconsumed_steers().is_empty());
    let recording = atif::log::read_whole(&dir.join(&run.admission.trace_file)).unwrap();
    let document = recording.document();
    let steps = document["steps"].as_array().unwrap();
    let consumed: Vec<_> = steps
        .iter()
        .filter_map(|step| step["extra"]["steer_consumed"].as_object())
        .collect();
    assert_eq!(consumed.len(), 1, "{document}");
    assert_eq!(consumed[0]["revision"], 2);
    assert_eq!(consumed[0]["acknowledgment"], "next_turn_start");
    assert_eq!(consumed[0]["adapter"], "bounded-command");
    // The step sits after the user's instructions and before admission.
    let position = |needle: &str| {
        steps
            .iter()
            .position(|step| {
                step["message"]
                    .as_str()
                    .is_some_and(|m| m.starts_with(needle))
            })
            .unwrap()
    };
    assert!(position("Write the corrected output.") < position("Steer consumed"));
    assert!(position("Steer consumed") < position("Execution admitted"));
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
    // A live owner's run is never ended under it.
    assert_eq!(Store::open(&dir).unwrap().settle("task-one").unwrap(), None);
    handle.abort();
    let _ = handle.await;
    // Once nothing of the run is left, recovery ends it (#10124); while
    // its process group lingers it is unknown, and a later recovery ends it.
    let task = settled(&dir, "task-one").await;
    assert_eq!(task.execution, Execution::Failed);
    assert!(execute(&dir, &bytes).await.is_err());
    assert_eq!(recovered(&dir, "task-one"), task);
    tokio::time::sleep(Duration::from_millis(500)).await;
}

/// Wait out a moment's holder of a lock: a process another test forks
/// shares every lock this process has open until it execs, so a lock just
/// released can still read as held for a moment (#10230).
fn waiting<T>(mut attempt: impl FnMut() -> Result<T, Error>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match attempt() {
            Ok(value) => return value,
            Err(Error::Busy) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("{error:?}"),
        }
    }
}

/// [`Owner::acquire`] once no other owner holds `id`.
fn acquired(store: &Store, id: &str) -> Owner {
    waiting(|| Owner::acquire(store, id))
}

/// [`recover`] once no other owner holds `id`.
fn recovered(dir: &Path, id: &str) -> Task {
    waiting(|| recover(dir, id))
}

/// [`execute`] once no other owner holds the grant's task.
async fn executed(dir: &Path, bytes: &[u8]) -> Task {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match execute(dir, bytes).await {
            Err(Error::Busy) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            result => return result.unwrap_or_else(|error| panic!("{error:?}")),
        }
    }
}

/// [`check`] task-one once no other owner holds it.
async fn run_checks(dir: &Path) -> Task {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match check(dir, "task-one", &crate::capability::Trust::everything()).await {
            Err(Error::Busy) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            result => return result.unwrap_or_else(|error| panic!("{error:?}")),
        }
    }
}

/// Recover `id` until its run is ended, as a host's sweeps would, and check
/// it ended as one whose owner process is gone.
pub(super) async fn settled(dir: &Path, id: &str) -> Task {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let task = match recover(dir, id) {
            Ok(task) => task,
            // A process another test forked holds every open lock until it
            // execs (#10230): wait on the condition, not on luck.
            Err(Error::Busy) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(20)).await;
                continue;
            }
            Err(error) => panic!("{error:?}"),
        };
        if task.status == Status::Finished {
            let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
            assert_eq!(result.ending, OWNER_ENDED);
            assert!(result.group_clear && result.exit_code.is_none());
            return task;
        }
        assert_eq!(task.status, Status::Unknown);
        assert!(Instant::now() < deadline, "the run was never ended");
        // The runtime reaps the test's own children meanwhile.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// A second task, `task-two`, on `task-one`'s workspace, and its grant.
fn second_task(dir: &Path, grant: &Grant) -> Grant {
    let mut store = Store::open(dir).unwrap();
    let first = store.show("task-one").unwrap();
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: "submit-two".into(),
        task_id: "task-two".into(),
        expected_revision: None,
        action: Action::Submit {
            intent: first.intent.clone(),
        },
    };
    store.apply(&serde_json::to_vec(&command).unwrap()).unwrap();
    let task = store.show("task-two").unwrap();
    Grant {
        task_id: task.task_id,
        intent_digest: task.intent_digest,
        expected_revision: task.revision,
        ..grant.clone()
    }
}

#[tokio::test]
async fn a_run_whose_owner_died_is_ended_and_the_next_task_in_its_project_runs() {
    // Owners that died at each point a run can be left running: after
    // admission (no process yet), and after the command ran with its
    // process recorded and gone (#10124).
    for point in ["after_admission", "before_result"] {
        let (root, _workspace, grant) = fixture();
        let dir = root.path().join("store");
        OWNER_FAULT.with(|fault| fault.set(Some(point)));
        assert!(
            execute(&dir, &serde_json::to_vec(&grant).unwrap())
                .await
                .is_err()
        );
        let stuck = Store::open(&dir).unwrap().show("task-one").unwrap();
        assert_eq!(stuck.status, Status::Running, "{point}");
        assert_eq!(
            stuck.run.as_ref().unwrap().process_id.is_some(),
            point == "before_result"
        );
        let events_before = events(&dir, "task-one");
        // The next task in the same project starts and finishes.
        let next = second_task(&dir, &grant);
        let task = execute(&dir, &serde_json::to_vec(&next).unwrap())
            .await
            .unwrap();
        assert_eq!(task.execution, Execution::Finished, "{point}");
        // The dead run was ended, not erased: its admission, effects, and
        // trace stay, and only a result was added.
        let store = Store::open(&dir).unwrap();
        let ended = store.show("task-one").unwrap();
        assert_eq!(ended.status, Status::Finished, "{point}");
        assert_eq!(ended.execution, Execution::Failed, "{point}");
        let run = ended.run.as_ref().unwrap();
        assert_eq!(run.admission, stuck.run.as_ref().unwrap().admission);
        assert_eq!(run.result.as_ref().unwrap().ending, OWNER_ENDED);
        assert_eq!(
            dir.join(&run.admission.trace_file).exists(),
            point == "before_result"
        );
        assert!(events(&dir, "task-one") > events_before);
        // A reopened store replays the same history.
        drop(store);
        assert_eq!(Store::open(&dir).unwrap().show("task-one").unwrap(), ended);
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

#[tokio::test]
async fn a_live_run_still_holds_its_project() {
    let (root, _workspace, mut grant) = fixture();
    let dir = root.path().join("store");
    // A live owner.
    grant.arguments[1] = "sleep 20".into();
    let run_dir = dir.clone();
    let bytes = serde_json::to_vec(&grant).unwrap();
    let handle = tokio::spawn(async move { execute(&run_dir, &bytes).await });
    let deadline = Instant::now() + Duration::from_secs(10);
    while Store::open(&dir)
        .unwrap()
        .show("task-one")
        .unwrap()
        .run
        .is_none()
    {
        assert!(Instant::now() < deadline, "owner did not start");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let next = second_task(&dir, &grant);
    let next_bytes = serde_json::to_vec(&next).unwrap();
    assert!(matches!(
        execute(&dir, &next_bytes).await,
        Err(Error::WorkspaceBusy)
    ));
    let first = Store::open(&dir).unwrap().show("task-one").unwrap();
    assert_eq!(first.status, Status::Running);
    handle.abort();
    let _ = handle.await;
    tokio::time::sleep(Duration::from_millis(300)).await;
}

#[tokio::test]
async fn a_dead_owners_live_process_still_holds_its_project_and_is_never_killed() {
    let (root, _workspace, grant) = fixture();
    let dir = root.path().join("store");
    // The owner died after recording its effect intent, and a process the
    // run recorded still runs.
    OWNER_FAULT.with(|fault| fault.set(Some("after_intent")));
    assert!(
        execute(&dir, &serde_json::to_vec(&grant).unwrap())
            .await
            .is_err()
    );
    let mut child = {
        let mut command = std::process::Command::new("/bin/sleep");
        command.arg("30");
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        command.spawn().unwrap()
    };
    {
        let mut store = Store::open(&dir).unwrap();
        let owner = acquired(&store, "task-one");
        store
            .record(
                &owner,
                Event::Spawned {
                    process_id: child.id(),
                },
                1,
            )
            .unwrap();
    }
    let next = second_task(&dir, &grant);
    let next_bytes = serde_json::to_vec(&next).unwrap();
    assert!(matches!(
        execute(&dir, &next_bytes).await,
        Err(Error::WorkspaceBusy)
    ));
    assert_eq!(Store::open(&dir).unwrap().settle("task-one").unwrap(), None);
    // Recovery leaves it unknown while the process runs, and kills nothing.
    assert_eq!(recovered(&dir, "task-one").status, Status::Unknown);
    assert!(
        child.try_wait().unwrap().is_none(),
        "the process was killed"
    );
    // Once it has ended, the next start ends the unknown run and runs.
    child.kill().unwrap();
    child.wait().unwrap();
    let task = executed(&dir, &next_bytes).await;
    assert_eq!(task.execution, Execution::Finished);
    let ended = Store::open(&dir).unwrap().show("task-one").unwrap();
    assert_eq!(ended.status, Status::Finished);
    assert_eq!(ended.execution, Execution::Failed);
    let run = ended.run.as_ref().unwrap();
    assert_eq!(run.epoch, 2);
    assert_eq!(
        run.recovery_reason.as_deref(),
        Some("owner_lost_effects_not_replayed")
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
}

#[tokio::test]
async fn a_stuck_task_stops_from_any_device_and_says_why() {
    use super::super::commands::{self, Kind, Outcome, Request, Sender, State};
    use coder_host::Tasks as _;
    let (root, _workspace, grant) = fixture();
    let dir = root.path().join("store");
    OWNER_FAULT.with(|fault| fault.set(Some("after_admission")));
    assert!(
        execute(&dir, &serde_json::to_vec(&grant).unwrap())
            .await
            .is_err()
    );
    // Left unknown, as `coder task recover` used to leave it.
    {
        let mut store = Store::open(&dir).unwrap();
        let owner = acquired(&store, "task-one");
        store.record(&owner, Event::OwnerLost, 2).unwrap();
    }
    let stuck = Store::open(&dir).unwrap().show("task-one").unwrap();
    assert_eq!(stuck.status, Status::Unknown);
    // A phone's, desktop's, or terminal's Stop (`task.command` interrupt).
    let request = Request {
        command: "c".repeat(64),
        task: "task-one".into(),
        kind: Kind::Interrupt,
        based_on: stuck.revision,
        text: "Stopped from a phone.".into(),
        emulate: false,
        issued_at: super::super::autostart::unix_now(),
    };
    let sender = Sender {
        device: "phone".into(),
        grant: Some("g".repeat(64)),
        epoch: Some(1),
    };
    let (recorded, _) = commands::record(
        &dir,
        &sender,
        &request,
        &super::super::adapter::STEERING,
        &|_| true,
        super::super::autostart::unix_now(),
    )
    .unwrap();
    assert!(matches!(
        recorded.state,
        State::Done(Outcome::Applied { .. })
    ));
    let task = recorded.task.unwrap();
    assert_eq!(task.status, Status::Finished);
    assert_eq!(task.execution, Execution::Failed);
    // Every surface reads why, in plain words.
    let inbox = super::super::remote::Inbox::new(&dir, Default::default());
    let note = inbox.note("task-one").unwrap();
    assert_eq!(note, coder_host::Note::OwnerEnded);
    assert_eq!(note.headline(), OWNER_ENDED_TEXT);
    assert!(
        inbox.current().iter().any(|task| task.task == "task-one"
            && task.phase == nostr::activity_summary::Phase::Cancelled)
    );
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
        // Recovery reruns nothing; an owner that died before its result
        // leaves a run that is ended once nothing of it is left (#10124).
        let recovered = if point == "after_result" {
            recovered(&dir, "task-one")
        } else {
            settled(&dir, "task-one").await
        };
        assert_eq!(
            recovered.execution,
            if point == "after_result" {
                Execution::Finished
            } else {
                Execution::Failed
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

/// How many owner events `id`'s file in the store at `dir` holds.
fn events(dir: &Path, id: &str) -> usize {
    read_task_file(&dir.join(TASK_DIR).join(format!("{id}.json")), id)
        .unwrap()
        .unwrap()
        .host_events
        .len()
}

/// Plant, in a new store directory `to`, the single legacy document a
/// store before #10231 would hold for `from`'s only task, `task-one`.
fn plant_legacy(from: &Path, to: &Path, schema: &str) -> Vec<u8> {
    let file = read_task_file(&from.join(TASK_DIR).join("task-one.json"), "task-one")
        .unwrap()
        .unwrap();
    let mut host_events = file.host_events;
    if schema != LEGACY_STORE_SCHEMA {
        host_events.clear();
    }
    let document = Document {
        schema: schema.into(),
        sequence: (file.commands.len() + host_events.len()) as u64,
        tasks: BTreeMap::from([("task-one".to_owned(), file.task)]),
        commands: file.commands,
        host_events,
    };
    make_private_directory(to).unwrap();
    drop(private_open(&to.join(LOCK_FILE), true, true).unwrap());
    let bytes = serde_json::to_vec(&document).unwrap();
    let mut legacy = private_open(&to.join(LEGACY_STORE_FILE), true, true).unwrap();
    legacy.write_all(&bytes).unwrap();
    bytes
}

#[test]
fn a_v1_inbox_migrates_and_takes_the_next_command() {
    let (root, _workspace, _) = fixture();
    let dir = root.path().join("legacy");
    let bytes = plant_legacy(&root.path().join("store"), &dir, LEGACY_V1_SCHEMA);
    let mut store = Store::open(&dir).unwrap();
    assert_eq!(std::fs::read(dir.join(LEGACY_BACKUP_FILE)).unwrap(), bytes);
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
}

#[tokio::test]
async fn a_v2_store_with_a_finished_run_migrates_with_its_owner_events() {
    let (root, _workspace, grant) = fixture();
    let store = root.path().join("store");
    let ran = execute(&store, &serde_json::to_vec(&grant).unwrap())
        .await
        .unwrap();
    assert_eq!(ran.execution, Execution::Finished);
    let dir = root.path().join("legacy");
    plant_legacy(&store, &dir, LEGACY_STORE_SCHEMA);
    let migrated = Store::open(&dir).unwrap();
    assert_eq!(migrated.show("task-one").unwrap(), ran);
    assert_eq!(events(&dir, "task-one"), events(&store, "task-one"));
    assert!(events(&dir, "task-one") > 0);
}

pub(super) fn requirements(host: &Path, command: &str) -> checks::Requirements {
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
        task_sources: vec!["fixture-request".into()],
        check_lineage: vec![checks::CheckLineage {
            check: "content".into(),
            sources: vec!["operator-fixture-specification".into()],
        }],
        knowledge: Vec::new(),
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
        let checked = run_checks(&dir).await;
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
        let checked = run_checks(&dir).await;
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
    let owner = acquired(&store, "task-one");
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

/// Hold task-one's write lock on another thread for `hold`, as another
/// process's slow write would.
fn hold_store(store: &Path, hold: Duration) -> std::thread::JoinHandle<()> {
    let store = store.to_path_buf();
    let (held, taken) = std::sync::mpsc::channel();
    let holder = std::thread::spawn(move || {
        let _held = Store::open(&store).unwrap().lock_task("task-one").unwrap();
        held.send(()).unwrap();
        std::thread::sleep(hold);
    });
    taken.recv().unwrap();
    holder
}

#[tokio::test]
async fn the_owner_process_waits_out_a_store_busy_past_the_lock_wait() {
    let (root, _workspace, grant) = fixture();
    let dir = root.path().join("store");
    let bytes = serde_json::to_vec(&grant).unwrap();
    // Another process's slow disk sync holds the store past the five
    // seconds a device-facing open waits.
    let holder = hold_store(&dir, LOCK_WAIT + Duration::from_secs(2));
    let cancel = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: "cancel-busy".into(),
        task_id: "task-one".into(),
        expected_revision: Some(1),
        action: Action::Cancel {
            reason: "Stop.".into(),
        },
    };
    assert!(matches!(
        Store::open_waiting(&dir, Duration::from_millis(100))
            .unwrap()
            .apply(&serde_json::to_vec(&cancel).unwrap()),
        Err(Error::Busy)
    ));
    // The task's own process waits it out and runs the task, where it
    // used to fail at admission.
    let task = execute(&dir, &bytes).await.unwrap();
    holder.join().unwrap();
    assert_eq!(task.execution, Execution::Finished);
}

/// A run's cost is recorded by part, priced only when every part is known,
/// and never a stand-in zero (#10161).
#[test]
fn a_result_records_its_cost_by_part() {
    let cost = Cost::from_usd(Some(0.9), Some(0.04));
    assert_eq!(cost.engine_microusd, Some(900_000));
    assert_eq!(cost.total_microusd(), Some(940_000));
    assert_eq!(cost.status(), "priced");
    let partial = Cost::from_usd(None, Some(0.04));
    assert_eq!(partial.status(), "partial");
    assert_eq!(partial.total_microusd(), None);
    assert_eq!(Cost::default().status(), "unknown");
    assert_eq!(Cost::ZERO.plus(cost), cost);
    assert_eq!(cost.plus(partial).status(), "partial");
    assert_eq!(Cost::from_usd(Some(f64::NAN), None).engine_microusd, None);

    let mut result = ResultRecord {
        ending: "model_finished".into(),
        exit_code: Some(0),
        stop_requested: false,
        group_clear: true,
        elapsed_ms: 1,
        trace_digest: "0".repeat(64),
        candidate_snapshot: None,
        artifact_file: None,
        artifact_digest: None,
        output_incomplete: false,
        cost_status: "unknown".into(),
        cost_microusd: None,
        engine_microusd: None,
        jev_microusd: None,
        payer: None,
        payer_keys: Vec::new(),
    };
    // A record written before costs were recorded reads as unknown.
    let old = serde_json::to_value(&result).unwrap();
    assert!(old.get("cost_microusd").is_none());
    let read: ResultRecord = serde_json::from_value(old).unwrap();
    assert!(read.cost_consistent());
    result.priced(cost);
    assert_eq!(result.cost_status, "priced");
    assert_eq!(result.cost_microusd, Some(940_000));
    assert!(result.cost_consistent());
    result.cost_microusd = Some(1);
    assert!(
        !result.cost_consistent(),
        "a total that disagrees is refused"
    );
}

/// BYOK (#10176): a run record names who paid and, under `theirs`, each
/// key by provider and fingerprint, never the key; a record written before
/// the payer was kept still reads.
#[test]
fn a_run_record_names_its_payer_by_fingerprint_only() {
    let mut result: ResultRecord = serde_json::from_value(json!({
        "ending": "model_finished",
        "exit_code": 0,
        "stop_requested": false,
        "group_clear": true,
        "elapsed_ms": 1,
        "trace_digest": "0".repeat(64),
        "candidate_snapshot": null,
        "artifact_file": null,
        "artifact_digest": null,
        "output_incomplete": false,
        "cost_status": "unknown",
    }))
    .unwrap();
    assert_eq!(result.payer, None);
    result.paid_by(&model_access::Access::ours());
    assert_eq!(result.payer, Some(model_access::Paid::Ours));
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["payer"], "ours");
    assert!(value.get("payer_keys").is_none());
    let mut keys = model_access::Keys::none();
    keys.insert(
        model_access::Provider::OpenRouter,
        model_access::ApiKey::new("sk-or-v1-secret".to_owned()),
    );
    result.paid_by(&model_access::Access::theirs(keys));
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["payer"], "theirs");
    assert_eq!(value["payer_keys"][0]["provider"], "openrouter");
    assert_eq!(
        value["payer_keys"][0]["fingerprint"],
        model_access::fingerprint("sk-or-v1-secret")
    );
    assert!(!value.to_string().contains("sk-or-v1-secret"));
    let read: ResultRecord = serde_json::from_value(value).unwrap();
    assert_eq!(read, result);
}
