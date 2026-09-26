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
                    path: "/example/workspace".into(),
                    source_revision: None,
                },
                configuration: RequestedConfiguration {
                    adapter: "microluna".into(),
                    model: Some("example/model".into()),
                },
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
    assert_eq!(store.document.sequence, 1);
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
    assert_eq!(store.document.sequence, 2);
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
    let original = std::fs::read(dir.path().join(STORE_FILE)).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&original).unwrap();
    let mutations: Vec<serde_json::Value> = vec![
        {
            let mut value = document.clone();
            value["schema"] = "future".into();
            value
        },
        {
            let mut value = document.clone();
            value["sequence"] = 0.into();
            value
        },
        {
            let mut value = document.clone();
            value["tasks"]["task-one"]["intent"]["prompt"] = "Changed.".into();
            value
        },
        {
            let mut value = document.clone();
            value["tasks"]["task-one"]["status"] = "queued".into();
            value
        },
        {
            let mut value = document.clone();
            value["tasks"]["task-one"]["execution"] = "completed".into();
            value
        },
        {
            let mut value = document.clone();
            value["commands"][0]["receipt"]["request_digest"] = "sha256:forged".into();
            value
        },
        {
            let mut value = document.clone();
            value["commands"][0]["receipt"]["schema"] = "future".into();
            value
        },
        {
            let mut value = document.clone();
            value["commands"][0]["receipt"]["task_id"] = "other".into();
            value
        },
        {
            let mut value = document.clone();
            value["commands"][1] = value["commands"][0].clone();
            value
        },
        {
            let mut value = document.clone();
            value["commands"][0]["request"] = "{}".into();
            value
        },
        {
            let mut value = document.clone();
            value["unknown"] = true.into();
            value
        },
    ];
    for mutation in mutations {
        let bytes = serde_json::to_vec(&mutation).unwrap();
        std::fs::write(dir.path().join(STORE_FILE), &bytes).unwrap();
        assert!(Store::open(dir.path()).is_err());
        assert_eq!(
            std::fs::read(dir.path().join(STORE_FILE)).unwrap(),
            bytes,
            "corruption must not be rewritten as an empty inbox"
        );
    }
    std::fs::write(dir.path().join(STORE_FILE), b"{\"schema\":").unwrap();
    assert!(matches!(Store::open(dir.path()), Err(Error::Corrupt(_))));
    std::fs::write(dir.path().join(STORE_FILE), original).unwrap();
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
    assert!(matches!(Store::open(dir.path()), Err(Error::Corrupt(_))));
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
        assert_eq!(store.document.commands.len(), 1);
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
    for name in [STORE_FILE, LOCK_FILE] {
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
fn second_opener_waits_for_the_stable_lock_then_reads_committed_state() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    let path = dir.path().to_path_buf();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let store = Store::open(&path).unwrap();
        result_tx.send(store.list().unwrap()).unwrap();
    });
    started_rx.recv().unwrap();
    assert!(result_rx.recv_timeout(Duration::from_millis(100)).is_err());
    store.apply(&submit("create", "task-one")).unwrap();
    drop(store);
    assert_eq!(
        result_rx
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .len(),
        1
    );
    handle.join().unwrap();
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
    let mut store = Store::open(dir.path()).unwrap();
    let mut full = store.document.clone();
    for index in 0..MAX_TASKS {
        let task_id = format!("task-{index}");
        for bytes in [
            submit(&format!("create-{index}"), &task_id),
            cancel(&format!("cancel-{index}"), &task_id, 1),
        ] {
            let command = parse_command(&bytes).unwrap();
            full.sequence += 1;
            let receipt = transition(
                &command,
                &digest_bytes(&bytes),
                full.sequence,
                &mut full.tasks,
            )
            .unwrap();
            full.commands.push(Accepted {
                request: String::from_utf8(bytes).unwrap(),
                receipt,
            });
        }
    }
    store.commit(&full).unwrap();
    store.document = full;
    let retry = submit("create-0", "task-0");
    let original = store.document.commands[0].receipt.clone();
    assert_eq!(store.apply(&retry).unwrap(), original);
    assert!(matches!(
        store.apply(&submit("create-extra", "extra")),
        Err(Error::LimitExceeded)
    ));
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
    let creator = Store {
        dir: dir.path().to_path_buf(),
        document: Document {
            schema: STORE_SCHEMA.into(),
            sequence: 0,
            tasks: BTreeMap::new(),
            commands: Vec::new(),
        },
        lock: creator_lock,
        healthy: true,
        fault: std::cell::Cell::new(None),
    };
    creator.commit(&creator.document).unwrap();
    drop(creator);
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
fn visible_rename_requires_a_successful_reopen_barrier_before_retry_acknowledgement() {
    let dir = private_dir();
    let bytes = submit("create", "task-one");
    {
        let mut store = Store::open(dir.path()).unwrap();
        store.fault.set(Some(Fault::AfterRename));
        assert!(matches!(store.apply(&bytes), Err(Error::Io(_))));
    }
    // The renamed state is readable, but a barrier failure must still prevent
    // opening a handle that could acknowledge the original command on retry.
    assert_eq!(
        read_document(&dir.path().join(STORE_FILE))
            .unwrap()
            .sequence,
        1
    );
    OPEN_SYNC_FAIL.with(|fault| fault.set(true));
    assert!(matches!(Store::open(dir.path()), Err(Error::Io(_))));
    let mut store = Store::open(dir.path()).unwrap();
    assert_eq!(store.apply(&bytes).unwrap().sequence, 1);
    assert_eq!(store.document.commands.len(), 1);
}
