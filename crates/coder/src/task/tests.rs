use super::*;

fn private_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    dir
}

/// An absolute workspace path on this platform.
const EXAMPLE_WORKSPACE: &str = if cfg!(windows) {
    r"C:\example\workspace"
} else {
    "/example/workspace"
};

fn submit(command_id: &str, task_id: &str) -> Vec<u8> {
    serde_json::to_vec(&Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: command_id.into(),
        task_id: task_id.into(),
        expected_revision: None,
        action: Action::Submit {
            intent: TaskIntent {
                title: "Repair a test".into(),
                prompt: "Inspect the failing test and propose a fix.".into(),
                workspace: Workspace {
                    path: EXAMPLE_WORKSPACE.into(),
                    source_revision: None,
                },
                configuration: RequestedConfiguration {
                    adapter: "microluna".into(),
                    model: Some("example/model".into()),
                },
                images: Vec::new(),
            },
        },
    })
    .unwrap()
}

fn cancel(command_id: &str, task_id: &str, revision: u64) -> Vec<u8> {
    serde_json::to_vec(&Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: command_id.into(),
        task_id: task_id.into(),
        expected_revision: Some(revision),
        action: Action::Cancel {
            reason: "No longer needed.".into(),
        },
    })
    .unwrap()
}

/// The validated file of `id` in the store at `dir`.
fn task_file(dir: &Path, id: &str) -> TaskFile {
    read_task_file(&dir.join(TASK_DIR).join(format!("{id}.json")), id)
        .unwrap()
        .unwrap()
}

/// A legacy v2 document holding `commands` applied in order.
fn legacy_document(commands: &[Vec<u8>]) -> Document {
    let mut document = Document {
        schema: LEGACY_STORE_SCHEMA.into(),
        sequence: 0,
        tasks: BTreeMap::new(),
        commands: Vec::new(),
        host_events: Vec::new(),
    };
    for bytes in commands {
        let command = parse_command(bytes).unwrap();
        document.sequence += 1;
        let receipt = transition(
            &command,
            &digest_bytes(bytes),
            document.sequence,
            &mut document.tasks,
        )
        .unwrap();
        document.commands.push(Accepted {
            request: String::from_utf8(bytes.clone()).unwrap(),
            receipt,
        });
    }
    document
}

/// Lay out a store from before #10231 at `dir`: its stable lock and its
/// single document.
fn write_legacy(dir: &Path, document: &Document) -> Vec<u8> {
    drop(private_open(&dir.join(LOCK_FILE), true, true).unwrap());
    let bytes = serde_json::to_vec(document).unwrap();
    let mut file = private_open(&dir.join(LEGACY_STORE_FILE), true, true).unwrap();
    file.write_all(&bytes).unwrap();
    bytes
}

#[test]
fn intent_receipts_and_cancellation_survive_reopening_without_execution() {
    let dir = private_dir();
    let bytes = submit("command-one", "task-one");
    let initial;
    {
        let mut store = Store::open(dir.path()).unwrap();
        initial = store.apply(&bytes).unwrap();
        assert_eq!(initial.sequence, 1);
        assert_eq!(initial.revision, 1);
        assert_eq!(initial.status, Status::Queued);
        assert_eq!(initial.execution, Execution::NotStarted);
        assert_eq!(initial.checks, Checks::NotRun);
    }
    {
        let mut store = Store::open(dir.path()).unwrap();
        let cancelled = store.apply(&cancel("command-two", "task-one", 1)).unwrap();
        assert_eq!(cancelled.revision, 2);
        assert_eq!(cancelled.sequence, 2);
        assert_eq!(cancelled.status, Status::Cancelled);
        assert_eq!(
            store.apply(&bytes).unwrap(),
            initial,
            "retry returns the original receipt, not the latest state"
        );
        assert_eq!(store.list().unwrap().len(), 1);
    }
    let store = Store::open(dir.path()).unwrap();
    let task = store.show("task-one").unwrap();
    assert_eq!(task.status, Status::Cancelled);
    assert_eq!(task.execution, Execution::NotStarted);
    assert_eq!(task.checks, Checks::NotRun);
    assert_eq!(
        task.cancellation_reason.as_deref(),
        Some("No longer needed.")
    );
}

#[test]
fn identity_conflicts_span_tasks_actions_and_byte_encodings() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    let bytes = submit("command-one", "task-one");
    store.apply(&bytes).unwrap();
    for conflicting in [
        submit("command-one", "task-two"),
        cancel("command-one", "task-one", 1),
        [bytes.clone(), b"\n".to_vec()].concat(),
    ] {
        assert!(matches!(store.apply(&conflicting), Err(Error::Conflict)));
    }
    assert_eq!(task_file(dir.path(), "task-one").commands.len(), 1);
    assert!(!dir.path().join(TASK_DIR).join("task-two.json").exists());
    assert_eq!(store.show("task-one").unwrap().status, Status::Queued);
}

#[test]
fn revisions_and_terminal_cancellation_fail_closed() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    store.apply(&submit("create", "task-one")).unwrap();
    assert!(matches!(
        store.apply(&cancel("cancel-old", "task-one", 0)),
        Err(Error::RevisionMismatch)
    ));
    assert!(matches!(
        store.apply(&cancel("cancel-missing", "missing", 1)),
        Err(Error::NotFound)
    ));
    assert!(matches!(
        store.apply(&submit("create-again", "task-one")),
        Err(Error::InvalidTransition)
    ));
    let bytes = cancel("cancel-now", "task-one", 1);
    let receipt = store.apply(&bytes).unwrap();
    assert_eq!(store.apply(&bytes).unwrap(), receipt);
    assert!(matches!(
        store.apply(&cancel("cancel-again", "task-one", 2)),
        Err(Error::InvalidTransition)
    ));
    assert_eq!(task_file(dir.path(), "task-one").commands.len(), 2);
}

#[test]
fn closed_commands_reject_duplicate_unknown_and_oversized_fields() {
    let bytes = submit("create", "task-one");
    let valid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for altered in [
        {
            let mut value = valid.clone();
            value["schema"] = "future".into();
            value
        },
        {
            let mut value = valid.clone();
            value["command_id"] = "task-one".into();
            value
        },
        {
            let mut value = valid.clone();
            value["action"]["intent"]["credentials"] = "forbidden".into();
            value
        },
        {
            let mut value = valid.clone();
            value["action"]["intent"]["configuration"]["endpoint"] = "forbidden".into();
            value
        },
        {
            let mut value = valid.clone();
            value["action"]["intent"]["workspace"]["path"] = "relative/path".into();
            value
        },
        {
            let mut value = valid.clone();
            value["action"]["intent"]["workspace"]["source_revision"] = "main".into();
            value
        },
        {
            let mut value = valid.clone();
            value["action"]["intent"]["title"] = " ".into();
            value
        },
        {
            let mut value = valid.clone();
            value["expected_revision"] = 1.into();
            value
        },
    ] {
        assert!(parse_command(&serde_json::to_vec(&altered).unwrap()).is_err());
    }
    let duplicate =
        String::from_utf8(bytes)
            .unwrap()
            .replacen("{", "{\"command_id\":\"other\",", 1);
    assert!(matches!(
        parse_command(duplicate.as_bytes()),
        Err(Error::InvalidCommand(_))
    ));
    assert!(matches!(
        parse_command(&vec![b' '; MAX_COMMAND_BYTES + 1]),
        Err(Error::LimitExceeded)
    ));
}

#[test]
fn every_materialized_fact_is_checked_against_the_retained_history() {
    let dir = private_dir();
    {
        let mut store = Store::open(dir.path()).unwrap();
        store.apply(&submit("create", "task-one")).unwrap();
        store.apply(&cancel("cancel", "task-one", 1)).unwrap();
    }
    let path = dir.path().join(TASK_DIR).join("task-one.json");
    let original = std::fs::read(&path).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&original).unwrap();
    let mutate = |change: &dyn Fn(&mut serde_json::Value)| {
        let mut value = document.clone();
        change(&mut value);
        value
    };
    let mutations: Vec<serde_json::Value> = vec![
        mutate(&|value| value["schema"] = "future".into()),
        mutate(&|value| value["commands"][1]["receipt"]["sequence"] = 1.into()),
        mutate(&|value| value["task"]["intent"]["prompt"] = "Changed.".into()),
        mutate(&|value| value["task"]["status"] = "queued".into()),
        mutate(&|value| value["task"]["execution"] = "completed".into()),
        mutate(&|value| value["task"]["task_id"] = "other".into()),
        mutate(&|value| {
            value["commands"][0]["receipt"]["request_digest"] = "sha256:forged".into();
        }),
        mutate(&|value| value["commands"][0]["receipt"]["schema"] = "future".into()),
        mutate(&|value| value["commands"][0]["receipt"]["task_id"] = "other".into()),
        mutate(&|value| value["commands"][1] = value["commands"][0].clone()),
        mutate(&|value| value["commands"][0]["request"] = "{}".into()),
        mutate(&|value| value["unknown"] = true.into()),
    ];
    for mutation in mutations {
        let bytes = serde_json::to_vec(&mutation).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let store = Store::open(dir.path()).unwrap();
        assert!(store.show("task-one").is_err(), "{mutation}");
        assert!(store.list().is_err());
        assert!(
            Store::open(dir.path())
                .unwrap()
                .apply(&cancel("again", "task-one", 2))
                .is_err()
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            bytes,
            "corruption must not be rewritten"
        );
    }
    std::fs::write(&path, b"{\"schema\":").unwrap();
    assert!(matches!(
        Store::open(dir.path()).unwrap().show("task-one"),
        Err(Error::Corrupt(_))
    ));
    std::fs::write(&path, original).unwrap();
    assert_eq!(Store::open(dir.path()).unwrap().list().unwrap().len(), 1);
}

#[test]
fn missing_document_is_never_recreated_in_an_initialized_store() {
    let dir = private_dir();
    {
        let mut store = Store::open(dir.path()).unwrap();
        store.apply(&submit("create", "task-one")).unwrap();
    }
    std::fs::remove_file(dir.path().join(STORE_FILE)).unwrap();
    let reopened = Store::open(dir.path());
    assert!(
        matches!(reopened, Err(Error::Corrupt(_))),
        "{:?}",
        reopened.err()
    );
    assert!(!dir.path().join(STORE_FILE).exists());
}

#[test]
fn failed_writes_never_acknowledge_and_reopen_resolves_retry_identity() {
    for point in [Fault::BeforeRename, Fault::AfterRename] {
        let dir = private_dir();
        let bytes = submit("create", "task-one");
        {
            let mut store = Store::open(dir.path()).unwrap();
            store.fault.set(Some(point));
            assert!(matches!(store.apply(&bytes), Err(Error::Io(_))));
            assert!(matches!(store.list(), Err(Error::ReopenRequired)));
            assert!(matches!(store.apply(&bytes), Err(Error::ReopenRequired)));
        }
        let mut store = Store::open(dir.path()).unwrap();
        let expected_before_retry = usize::from(point == Fault::AfterRename);
        assert_eq!(store.list().unwrap().len(), expected_before_retry);
        let receipt = store.apply(&bytes).unwrap();
        assert_eq!(receipt.sequence, 1);
        assert_eq!(task_file(dir.path(), "task-one").commands.len(), 1);
        assert_eq!(store.list().unwrap().len(), 1);
    }
}

#[test]
fn stale_pending_bytes_are_not_replayed_or_allowed_to_accumulate() {
    let dir = private_dir();
    drop(Store::open(dir.path()).unwrap());
    {
        let mut pending = private_open(&dir.path().join(PENDING_FILE), true, true).unwrap();
        pending.write_all(b"unfinished replacement").unwrap();
    }
    let store = Store::open(dir.path()).unwrap();
    assert!(store.list().unwrap().is_empty());
    assert!(!dir.path().join(PENDING_FILE).exists());
}

#[cfg(unix)]
#[test]
fn private_files_symlinks_and_hard_links_are_checked() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let parent = private_dir();
    let store_path = parent.path().join("store");
    drop(Store::open(&store_path).unwrap());
    assert_eq!(
        std::fs::metadata(&store_path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for name in [STORE_FILE, LOCK_FILE, IDENTITY_FILE] {
        assert_eq!(
            std::fs::metadata(store_path.join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let alias = parent.path().join("alias");
    symlink(&store_path, &alias).unwrap();
    assert!(matches!(Store::open(&alias), Err(Error::UnsafePath)));
    for name in [STORE_FILE, LOCK_FILE, PENDING_FILE] {
        let isolated = private_dir();
        drop(Store::open(isolated.path()).unwrap());
        let path = isolated.path().join(name);
        if path.exists() {
            std::fs::remove_file(&path).unwrap();
        }
        symlink(store_path.join(STORE_FILE), &path).unwrap();
        assert!(Store::open(isolated.path()).is_err());
    }
    let copy = parent.path().join("hardlink");
    std::fs::hard_link(store_path.join(STORE_FILE), &copy).unwrap();
    assert!(matches!(Store::open(&store_path), Err(Error::UnsafePath)));
}

#[test]
fn a_writer_of_one_task_never_blocks_readers_or_other_tasks() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    store.apply(&submit("create", "task-one")).unwrap();
    // Another process is writing task-one.
    let held = store.lock_task("task-one").unwrap();
    let started = Instant::now();
    let mut other = Store::open_waiting(dir.path(), Duration::from_millis(50)).unwrap();
    assert_eq!(other.show("task-one").unwrap().revision, 1);
    other.apply(&submit("create-two", "task-two")).unwrap();
    assert_eq!(other.list().unwrap().len(), 2);
    assert!(started.elapsed() < Duration::from_secs(1));
    // Only a write to the same task waits, and it refuses past its wait.
    assert!(matches!(
        other.apply(&cancel("cancel", "task-one", 1)),
        Err(Error::Busy)
    ));
    drop(held);
    assert_eq!(
        other
            .apply(&cancel("cancel", "task-one", 1))
            .unwrap()
            .status,
        Status::Cancelled
    );
}

#[test]
fn replaced_lock_invalidates_a_live_store() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    std::fs::remove_file(dir.path().join(LOCK_FILE)).unwrap();
    drop(private_open(&dir.path().join(LOCK_FILE), true, true).unwrap());
    assert!(matches!(
        store.apply(&submit("create", "task-one")),
        Err(Error::UnsafePath)
    ));
    assert!(matches!(store.list(), Err(Error::UnsafePath)));
}

#[cfg(unix)]
#[test]
fn opening_an_existing_public_directory_does_not_change_its_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = private_dir();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(Store::open(dir.path()), Err(Error::UnsafePath)));
    assert_eq!(
        std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert!(!dir.path().join(LOCK_FILE).exists());
}

#[test]
fn retained_capacity_refuses_new_work_but_preserves_exact_retries() {
    let dir = private_dir();
    let mut commands = Vec::new();
    for index in 0..MAX_TASKS {
        let task_id = format!("task-{index}");
        commands.push(submit(&format!("create-{index}"), &task_id));
        commands.push(cancel(&format!("cancel-{index}"), &task_id, 1));
    }
    let document = legacy_document(&commands);
    write_legacy(dir.path(), &document);
    let mut store = Store::open(dir.path()).unwrap();
    let retry = submit("create-0", "task-0");
    let original = document.commands[0].receipt.clone();
    assert_eq!(store.apply(&retry).unwrap(), original);
    assert!(matches!(
        store.apply(&submit("create-extra", "extra")),
        Err(Error::LimitExceeded)
    ));
    assert!(!dir.path().join(TASK_DIR).join("extra.json").exists());
    drop(store);
    let mut reopened = Store::open(dir.path()).unwrap();
    assert_eq!(reopened.list().unwrap().len(), MAX_TASKS);
    assert_eq!(reopened.apply(&retry).unwrap(), original);
}

#[test]
fn opener_that_beats_the_lock_creator_waits_for_initialization() {
    let dir = private_dir();
    let creator_lock = private_open(&dir.path().join(LOCK_FILE), true, true).unwrap();
    let path = dir.path().to_path_buf();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    let contender = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let result = Store::open(&path).and_then(|store| store.list());
        result_tx.send(result).unwrap();
    });
    started_rx.recv().unwrap();
    assert!(result_rx.recv_timeout(Duration::from_millis(100)).is_err());
    creator_lock.lock().unwrap();
    initialize(dir.path()).unwrap();
    drop(creator_lock);
    assert!(
        result_rx
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap()
            .is_empty()
    );
    contender.join().unwrap();
}

#[cfg(unix)]
#[test]
fn new_directory_components_are_private_and_reopenable() {
    use std::os::unix::fs::PermissionsExt;
    let dir = private_dir();
    let path = dir.path().join("first/second/tasks");
    drop(Store::open(&path).unwrap());
    for part in ["first", "first/second", "first/second/tasks"] {
        assert_eq!(
            std::fs::metadata(dir.path().join(part))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    assert!(Store::open(&path).unwrap().list().unwrap().is_empty());
}

#[test]
fn missing_stable_lock_refuses_repeatedly_without_recreating_it() {
    let dir = private_dir();
    drop(Store::open(dir.path()).unwrap());
    std::fs::remove_file(dir.path().join(LOCK_FILE)).unwrap();
    for _ in 0..2 {
        assert!(matches!(Store::open(dir.path()), Err(Error::Corrupt(_))));
        assert!(!dir.path().join(LOCK_FILE).exists());
    }
}

#[test]
fn visible_rename_requires_a_successful_barrier_before_retry_acknowledgement() {
    let dir = private_dir();
    let bytes = submit("create", "task-one");
    {
        let mut store = Store::open(dir.path()).unwrap();
        store.fault.set(Some(Fault::AfterRename));
        assert!(matches!(store.apply(&bytes), Err(Error::Io(_))));
    }
    // The renamed state is readable, but a barrier failure must still prevent
    // acknowledging the original command on retry.
    assert_eq!(task_file(dir.path(), "task-one").commands.len(), 1);
    BARRIER_SYNC_FAIL.with(|fault| fault.set(true));
    assert!(matches!(
        Store::open(dir.path()).unwrap().apply(&bytes),
        Err(Error::Io(_))
    ));
    let mut store = Store::open(dir.path()).unwrap();
    assert_eq!(store.apply(&bytes).unwrap().sequence, 1);
    assert_eq!(task_file(dir.path(), "task-one").commands.len(), 1);
}

fn continue_with(command_id: &str, task_id: &str, revision: u64, prompt: &str) -> Vec<u8> {
    serde_json::to_vec(&Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: command_id.into(),
        task_id: task_id.into(),
        expected_revision: Some(revision),
        action: Action::Continue {
            prompt: prompt.into(),
        },
    })
    .unwrap()
}

fn correct_with(command_id: &str, task_id: &str, revision: u64, prompt: &str) -> Vec<u8> {
    serde_json::to_vec(&Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: command_id.into(),
        task_id: task_id.into(),
        expected_revision: Some(revision),
        action: Action::Correct {
            prompt: prompt.into(),
            reason: "Steered".into(),
        },
    })
    .unwrap()
}

#[test]
fn only_an_ended_turn_continues_and_the_newest_instruction_applies() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    store.apply(&submit("submit", "task")).unwrap();
    // A queued turn is not ended.
    assert!(matches!(
        store.apply(&continue_with("early", "task", 1, "More")),
        Err(Error::InvalidTransition)
    ));
    store.apply(&cancel("cancel", "task", 1)).unwrap();
    let receipt = store
        .apply(&continue_with(
            "follow-up",
            "task",
            2,
            "Try again with logging.",
        ))
        .unwrap();
    assert_eq!(
        (receipt.revision, receipt.status, receipt.execution),
        (3, Status::Queued, Execution::NotStarted)
    );
    let task = store.show("task").unwrap();
    assert_eq!(task.effective_prompt(), "Try again with logging.");
    assert_eq!(task.turn_started(), 3);
    // A turn cancelled before it ran leaves no earlier run.
    assert!(task.earlier.is_empty());
    assert_eq!(task.cancellation_reason, None);
    // A correction after the follow-up replaces it before the turn runs;
    // the ledger holds it until a run reads it.
    store
        .apply(&correct_with("correct", "task", 3, "Only the parser."))
        .unwrap();
    let task = store.show("task").unwrap();
    assert_eq!(task.effective_prompt(), "Only the parser.");
    assert_eq!(task.unconsumed_steers().len(), 1);
    // A stale follow-up is refused, and the journal replays after reopening.
    assert!(matches!(
        store.apply(&continue_with("stale", "task", 3, "More")),
        Err(Error::RevisionMismatch)
    ));
    drop(store);
    let reopened = Store::open(dir.path()).unwrap();
    assert_eq!(reopened.show("task").unwrap(), task);
}

#[test]
fn follow_ups_are_bounded_per_task() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    store.apply(&submit("submit", "task")).unwrap();
    store.apply(&cancel("cancel-0", "task", 1)).unwrap();
    let mut revision = 2;
    // Turns cancelled before running stay at turn one, so a store bounds
    // runs by MAX_TURNS and commands by MAX_COMMANDS.
    for index in 0..8 {
        let receipt = store
            .apply(&continue_with(
                &format!("again-{index}"),
                "task",
                revision,
                "Again",
            ))
            .unwrap();
        let cancelled = store
            .apply(&cancel(
                &format!("cancel-{}", index + 1),
                "task",
                receipt.revision,
            ))
            .unwrap();
        revision = cancelled.revision;
    }
    assert_eq!(store.show("task").unwrap().follow_ups.len(), 8);
    assert!(matches!(
        store.apply(&continue_with("empty", "task", revision, "  ")),
        Err(Error::InvalidCommand(_))
    ));
}

/// A shell command's script is `-c SCRIPT` on Unix. On Windows it travels
/// in the environment, never on a command line that Cygwin and the C
/// runtime would split differently, and the command line holds only the
/// fixed runner, which has no backslash.
#[test]
fn a_shell_script_never_rides_a_windows_command_line() {
    let script = "printf '%s' \"a\\\"b\\\\c\"\necho done";
    let (arguments, variables) = owner::shell_arguments(script).unwrap();
    if cfg!(windows) {
        assert_eq!(arguments, ["-c", owner::SCRIPT_RUNNER]);
        assert_eq!(variables, [(owner::SCRIPT_VARIABLE, script.to_owned())]);
        assert!(!owner::SCRIPT_RUNNER.contains('\\'));
        let long = "x".repeat(owner::WINDOWS_SCRIPT_MAX + 1);
        assert!(owner::shell_arguments(&long).is_err());
    } else {
        assert_eq!(arguments, ["-c", script]);
        assert!(variables.is_empty());
    }
    assert!(owner::SCRIPT_RUNNER.contains(owner::SCRIPT_VARIABLE));
}

/// One process of [`processes_writing_different_tasks_never_wait_on_each_other`].
#[test]
#[ignore = "run by its parent test in a separate process"]
fn concurrent_writer_child() {
    let Ok(job) = std::env::var("CODER_TASK_STORE_CHILD") else {
        return;
    };
    let mut parts = job.split('|');
    let dir = PathBuf::from(parts.next().unwrap());
    let index: usize = parts.next().unwrap().parse().unwrap();
    let rounds: u64 = parts.next().unwrap().parse().unwrap();
    let task = format!("process-{index}");
    let mut slowest = Duration::ZERO;
    let mut timed = |operation: &mut dyn FnMut() -> Receipt| {
        let started = Instant::now();
        let receipt = operation();
        slowest = slowest.max(started.elapsed());
        receipt
    };
    timed(&mut || {
        Store::open(&dir)
            .unwrap()
            .apply(&submit(&format!("create-{index}"), &task))
            .unwrap()
    });
    for round in 1..=rounds {
        let bytes = correct_with(&format!("steer-{index}-{round}"), &task, round, "Again.");
        let receipt = timed(&mut || Store::open(&dir).unwrap().apply(&bytes).unwrap());
        assert_eq!(receipt.revision, round + 1);
    }
    println!("slowest-micros {}", slowest.as_micros());
}

#[test]
fn processes_writing_different_tasks_never_wait_on_each_other() {
    const PROCESSES: usize = 8;
    const ROUNDS: u64 = 10;
    let dir = private_dir();
    drop(Store::open(dir.path()).unwrap());
    let children: Vec<_> = (0..PROCESSES)
        .map(|index| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "task::tests::concurrent_writer_child",
                    "--exact",
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(
                    "CODER_TASK_STORE_CHILD",
                    format!("{}|{index}|{ROUNDS}", dir.path().display()),
                )
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let mut slowest = 0u128;
    for child in children {
        let output = child.wait_with_output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let micros: u128 = stdout
            .lines()
            .find_map(|line| line.split("slowest-micros ").nth(1))
            .unwrap_or_else(|| panic!("the child reports its slowest operation: {stdout}"))
            .trim()
            .parse()
            .unwrap();
        slowest = slowest.max(micros);
    }
    println!("slowest open+apply across {PROCESSES} processes: {slowest} us");
    // No process waited out another's write: each operation is one small
    // file replacement and one identity append.
    assert!(slowest < 1_000_000, "{slowest} us");
    let tasks = Store::open(dir.path()).unwrap().list().unwrap();
    assert_eq!(tasks.len(), PROCESSES);
    assert!(tasks.iter().all(|task| task.revision == ROUNDS + 1));
    let identities = read_identities(dir.path()).unwrap().0;
    assert_eq!(identities.len(), PROCESSES * (ROUNDS as usize + 1));
}

#[test]
fn concurrent_writers_of_one_task_are_held_to_its_revision() {
    let dir = private_dir();
    Store::open(dir.path())
        .unwrap()
        .apply(&submit("create", "task-one"))
        .unwrap();
    let path = dir.path().to_path_buf();
    let writers: Vec<_> = (0..8)
        .map(|index| {
            let path = path.clone();
            std::thread::spawn(move || {
                Store::open(&path).unwrap().apply(&correct_with(
                    &format!("steer-{index}"),
                    "task-one",
                    1,
                    "Mine.",
                ))
            })
        })
        .collect();
    let results: Vec<_> = writers
        .into_iter()
        .map(|writer| writer.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .all(|error| matches!(error, Error::RevisionMismatch))
    );
    let task = Store::open(dir.path()).unwrap().show("task-one").unwrap();
    assert_eq!((task.revision, task.corrections.len()), (2, 1));
    // A refused command reserved nothing.
    assert_eq!(read_identities(dir.path()).unwrap().0.len(), 2);
}

#[test]
fn a_legacy_document_migrates_once_and_keeps_every_task_and_identity() {
    let dir = private_dir();
    let commands = vec![
        submit("create-a", "task-a"),
        submit("create-b", "task-b"),
        cancel("cancel-a", "task-a", 1),
        continue_with("again-a", "task-a", 2, "Once more."),
        correct_with("steer-b", "task-b", 1, "Only the parser."),
        submit("create-c", "task-c"),
    ];
    let document = legacy_document(&commands);
    let original = write_legacy(dir.path(), &document);
    {
        let mut pending = private_open(&dir.path().join(PENDING_FILE), true, true).unwrap();
        pending.write_all(b"unfinished replacement").unwrap();
    }
    let mut store = Store::open(dir.path()).unwrap();
    // The document is kept, byte for byte, beside the migrated store.
    assert!(!dir.path().join(LEGACY_STORE_FILE).exists());
    assert!(!dir.path().join(PENDING_FILE).exists());
    assert_eq!(
        std::fs::read(dir.path().join(LEGACY_BACKUP_FILE)).unwrap(),
        original
    );
    assert_eq!(
        store.list().unwrap(),
        document.tasks.values().cloned().collect::<Vec<_>>()
    );
    // Every legacy identity still names its bytes and original receipt.
    for (bytes, accepted) in commands.iter().zip(&document.commands) {
        assert_eq!(store.apply(bytes).unwrap(), accepted.receipt);
    }
    assert!(matches!(
        store.apply(&submit("create-a", "task-d")),
        Err(Error::Conflict)
    ));
    // The next command continues the task's own sequence.
    let receipt = store.apply(&cancel("cancel-c", "task-c", 1)).unwrap();
    assert_eq!(receipt.sequence, 7);
    drop(store);
    // A second open migrates nothing again.
    let store = Store::open(dir.path()).unwrap();
    assert_eq!(store.list().unwrap().len(), 3);
    assert!(!dir.path().join(LEGACY_STORE_FILE).exists());
    assert_eq!(
        read_identities(dir.path()).unwrap().0.len(),
        commands.len() + 1
    );
}

#[test]
fn a_migration_a_crash_interrupted_runs_again_from_the_untouched_document() {
    let dir = private_dir();
    let commands = vec![submit("create-a", "task-a"), submit("create-b", "task-b")];
    let document = legacy_document(&commands);
    let original = write_legacy(dir.path(), &document);
    // A crash left a partial task directory and identity log, no marker.
    make_private_directory(&dir.path().join(TASK_DIR)).unwrap();
    {
        let path = dir.path().join(TASK_DIR).join("task-a.json");
        let mut file = private_open(&path, true, true).unwrap();
        file.write_all(b"{\"schema\":").unwrap();
        let path = dir.path().join(TASK_DIR).join(".task-b.json.tmp");
        let mut file = private_open(&path, true, true).unwrap();
        file.write_all(b"partial").unwrap();
        let path = dir.path().join(IDENTITY_FILE);
        let mut file = private_open(&path, true, true).unwrap();
        file.write_all(b"{\"command_id\":\"create-a\"").unwrap();
    }
    let store = Store::open(dir.path()).unwrap();
    assert_eq!(
        store.list().unwrap(),
        document.tasks.values().cloned().collect::<Vec<_>>()
    );
    assert_eq!(
        std::fs::read(dir.path().join(LEGACY_BACKUP_FILE)).unwrap(),
        original
    );
    assert_eq!(read_identities(dir.path()).unwrap().0.len(), 2);
    // A legacy document left beside a finished migration is moved aside,
    // never over the first backup, and never read again.
    std::fs::write(dir.path().join(LEGACY_STORE_FILE), b"{}").unwrap();
    drop(Store::open(dir.path()).unwrap());
    assert!(!dir.path().join(LEGACY_STORE_FILE).exists());
    assert_eq!(
        std::fs::read(dir.path().join(LEGACY_BACKUP_FILE)).unwrap(),
        original
    );
    assert_eq!(
        std::fs::read(dir.path().join("tasks.v2.1.json")).unwrap(),
        b"{}"
    );
}

#[test]
fn a_corrupt_legacy_document_is_never_migrated_or_replaced() {
    let dir = private_dir();
    let mut document = legacy_document(&[submit("create", "task-one")]);
    document.tasks.get_mut("task-one").unwrap().revision = 9;
    let original = write_legacy(dir.path(), &document);
    assert!(matches!(Store::open(dir.path()), Err(Error::Corrupt(_))));
    assert_eq!(
        std::fs::read(dir.path().join(LEGACY_STORE_FILE)).unwrap(),
        original
    );
    assert!(!dir.path().join(STORE_FILE).exists());
}

#[test]
fn leftovers_of_a_crashed_write_are_ignored_and_cleared() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    store.apply(&submit("create", "task-one")).unwrap();
    for name in [".task-one.json.tmp", ".stray.json.tmp"] {
        let path = dir.path().join(TASK_DIR).join(name);
        let mut file = private_open(&path, true, true).unwrap();
        file.write_all(b"{\"schema\":\"partial").unwrap();
    }
    // An identity append a crash cut short.
    {
        let path = dir.path().join(IDENTITY_FILE);
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"{\"command_id\":\"lost\",\"task").unwrap();
    }
    let mut store = Store::open(dir.path()).unwrap();
    assert_eq!(store.list().unwrap().len(), 1);
    assert_eq!(store.show("task-one").unwrap().revision, 1);
    store.apply(&cancel("cancel", "task-one", 1)).unwrap();
    assert!(
        !dir.path()
            .join(TASK_DIR)
            .join(".task-one.json.tmp")
            .exists()
    );
    let identities = read_identities(dir.path()).unwrap().0;
    assert_eq!(
        identities
            .iter()
            .map(|item| item.command_id.as_str())
            .collect::<Vec<_>>(),
        ["create", "cancel"]
    );
    assert!(
        std::fs::read(dir.path().join(IDENTITY_FILE))
            .unwrap()
            .ends_with(b"\n")
    );
    // A task identity that is not a plain identifier is never a path.
    assert!(matches!(store.show("../tasks"), Err(Error::NotFound)));
}

/// Only a store being initialized or migrated holds its stable lock past
/// an open; a follower reads that as nothing new yet, up to its limit.
#[test]
fn a_follower_reads_a_store_held_for_migration_as_nothing_new_yet() {
    let dir = private_dir();
    write_legacy(
        dir.path(),
        &legacy_document(&[submit("create", "task-one")]),
    );
    let lock = private_open(&dir.path().join(LOCK_FILE), false, true).unwrap();
    lock.lock().unwrap();
    let mut reading = Reading::within(Duration::from_millis(20), Duration::from_millis(150));
    let started = Instant::now();
    let refused = loop {
        match reading.show(dir.path(), "task-one") {
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Ok(Some(_)) => panic!("a held store was read"),
            Err(error) => break error,
        }
    };
    assert!(matches!(refused, Error::Busy));
    assert!(started.elapsed() >= Duration::from_millis(150));
    drop(lock);
    let task = reading.show(dir.path(), "task-one").unwrap().unwrap();
    assert_eq!(task.revision, 1);
}

/// #10231's measurement: open the store and create one task beside 1,000
/// existing ones (about 4 KB each), five times. The 1,000 come from a
/// legacy document, so the first open also times the migration.
#[test]
#[ignore = "a measurement, run with --ignored --nocapture"]
fn measure_create_beside_a_thousand_tasks() {
    let dir = private_dir();
    let long = "Inspect the failing test and propose a fix. ".repeat(90);
    let make = |command_id: &str, task_id: &str| {
        let mut value: serde_json::Value =
            serde_json::from_slice(&submit(command_id, task_id)).unwrap();
        value["action"]["intent"]["prompt"] = long.clone().into();
        serde_json::to_vec(&value).unwrap()
    };
    let mut commands = Vec::new();
    for index in 0..1000 {
        let task_id = format!("task-{index}");
        commands.push(make(&format!("create-{index}"), &task_id));
        commands.push(cancel(&format!("cancel-{index}"), &task_id, 1));
    }
    let size = write_legacy(dir.path(), &legacy_document(&commands)).len();
    let started = Instant::now();
    drop(Store::open(dir.path()).unwrap());
    println!("measure: migrated {size} bytes in {:?}", started.elapsed());
    for index in 0..5 {
        let started = Instant::now();
        let mut store = Store::open(dir.path()).unwrap();
        store
            .apply(&make(&format!("new-{index}"), &format!("fresh-{index}")))
            .unwrap();
        drop(store);
        println!("measure: open+create {:?}", started.elapsed());
    }
    let started = Instant::now();
    let tasks = Store::open(dir.path()).unwrap().list().unwrap();
    println!(
        "measure: list {} tasks {:?}",
        tasks.len(),
        started.elapsed()
    );
}
