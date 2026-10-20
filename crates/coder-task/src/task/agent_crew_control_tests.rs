//! Crew revocation uses native member state and the external checkpoint seam.
use super::*;
use crate::task::agent::{self, Store};
use coder_host::access::crew::JobRole;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

fn member(dir: &tempfile::TempDir) -> Store {
    let root = dir.path().join("host");
    let workspace = dir.path().join("work");
    std::fs::create_dir_all(workspace.join(".git")).unwrap();
    let store = Store::new(&root, "paul").unwrap();
    let record = store
        .open_as(&workspace, 100, agent::preset(JobRole::SalesLead.preset()))
        .unwrap();
    store.ensure_key(record, 100).unwrap();
    store
}
fn control(action: ControlAction, expected: Option<String>) -> Control {
    Control {
        cohort: "floor".into(),
        selection: Selection::AllSales,
        action,
        expected,
        reason: "Synthetic owner revocation.".into(),
    }
}
#[test]
fn revoked_pending_subjects_never_resume_after_reload_or_explicit_resume() {
    let dir = tempfile::tempdir().unwrap();
    let store = member(&dir);
    let root = dir.path().join("host");
    let mut guard = Guard::open(&root).unwrap();
    let original = guard.pending_stamp("paul").unwrap();
    guard.checkpoint_pending(&original, || Ok(())).unwrap();
    guard
        .book
        .begin(
            "stop",
            "owner",
            control(ControlAction::Stop, None),
            &["paul".into()],
            101,
        )
        .unwrap();
    guard.save().unwrap();
    assert!(
        guard
            .checkpoint_pending::<()>(&original, || panic!("revoked callback"))
            .is_err()
    );
    drop(guard);
    let mut guard = Guard::open(&root).unwrap();
    assert!(guard.book.blocked(&store.load().unwrap().unwrap()));
    assert!(
        guard
            .book
            .begin(
                "another",
                "owner",
                control(ControlAction::Pause, None),
                &["paul".into()],
                102
            )
            .is_err()
    );
    let receipt = guard
        .book
        .begin(
            "stop",
            "owner",
            control(ControlAction::Stop, None),
            &["paul".into()],
            103,
        )
        .unwrap();
    assert_eq!(receipt.at, 101);
    guard
        .book
        .finish(
            "stop",
            [(
                "paul".into(),
                json!({"state":"partial","external_effect":"unknown"}),
            )]
            .into(),
        )
        .unwrap();
    guard.save().unwrap();
    let expected = guard.book.digest.clone();
    guard
        .book
        .begin(
            "resume",
            "owner",
            control(ControlAction::Resume, Some(expected)),
            &["paul".into()],
            104,
        )
        .unwrap();
    guard
        .book
        .finish(
            "resume",
            [("paul".into(), json!({"state":"complete"}))].into(),
        )
        .unwrap();
    guard.save().unwrap();
    let current = guard.pending_stamp("paul").unwrap();
    assert_ne!(current, original);
    assert!(
        guard
            .checkpoint_pending::<()>(&original, || panic!("old approval reused"))
            .is_err()
    );
    guard.checkpoint_pending(&current, || Ok(())).unwrap();
    assert_eq!(
        guard.book.history[&nostr::contracts::digest_bytes(b"stop")].members["paul"]["external_effect"],
        "unknown"
    );
}
#[test]
fn external_delivery_racing_stop_retains_original_unknown_without_new_dispatch() {
    let dir = tempfile::tempdir().unwrap();
    let _store = member(&dir);
    let root = dir.path().join("host");
    let guard = Guard::open(&root).unwrap();
    let stamp = guard.pending_stamp("paul").unwrap();
    let checkpointed = Arc::new(AtomicUsize::new(0));
    let admitted = dir.path().join("external-admitted.json");
    let exact = json!({"item":"outbox:fixture-1","owner_subject_sha256":"a".repeat(64),"crew":stamp,"state":"admitted"});
    let bytes = serde_json::to_vec(&exact).unwrap();
    guard
        .checkpoint_pending(&stamp, || {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&admitted)
                .map_err(|e| e.to_string())?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|e| e.to_string())?;
            File::open(dir.path()).unwrap().sync_all().unwrap();
            checkpointed.fetch_add(1, Ordering::SeqCst);
            Ok(nostr::contracts::digest_bytes(&bytes))
        })
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(&admitted).unwrap()).unwrap(),
        exact
    );
    drop(guard);
    let (entered, ready) = std::sync::mpsc::channel();
    let (release, blocked) = std::sync::mpsc::channel();
    // Delivery belongs to an isolated external adapter after its durable
    // checkpoint; Coder never delivers and cannot invent its outcome.
    let delivery = std::thread::spawn(move || {
        entered.send(()).unwrap();
        blocked.recv().unwrap();
        "unknown"
    });
    ready
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    let mut guard = Guard::open(&root).unwrap();
    guard
        .book
        .begin(
            "stop",
            "owner",
            control(ControlAction::Stop, None),
            &["paul".into()],
            101,
        )
        .unwrap();
    guard.save().unwrap();
    assert!(
        guard
            .checkpoint_pending(&stamp, || {
                checkpointed.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .is_err()
    );
    release.send(()).unwrap();
    assert_eq!(delivery.join().unwrap(), "unknown");
    assert_eq!(checkpointed.load(Ordering::SeqCst), 1);
    assert_eq!(std::fs::read(&admitted).unwrap(), bytes);
}
#[test]
fn native_state_and_key_are_required_at_the_actual_outbox_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let store = member(&dir);
    let guard = Guard::open(&dir.path().join("host")).unwrap();
    let original = guard.pending_stamp("paul").unwrap();
    let mut record = store.load().unwrap().unwrap();
    record.state = State::Stopped;
    store.save(&record).unwrap();
    assert!(
        guard
            .checkpoint_pending::<()>(&original, || panic!("stopped native member"))
            .is_err()
    );
    record.state = State::Active;
    record.pubkey = Some("b".repeat(64));
    store.save(&record).unwrap();
    assert!(
        guard
            .checkpoint_pending::<()>(&original, || panic!("different key"))
            .is_err()
    );
}
#[test]
fn replaced_lock_state_root_and_shared_files_refuse_the_original_writer() {
    use std::os::unix::fs::PermissionsExt;
    for target in ["writer.lock", "state.json", "root"] {
        let dir = tempfile::tempdir().unwrap();
        let _store = member(&dir);
        let root = dir.path().join("host");
        let mut guard = Guard::open(&root).unwrap();
        guard.save().unwrap();
        if target == "root" {
            std::fs::rename(&root, dir.path().join("old-host")).unwrap();
            std::fs::create_dir(&root).unwrap();
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        } else {
            let path = root.join("crew-control").join(target);
            std::fs::rename(&path, path.with_extension("old")).unwrap();
            std::fs::write(&path, b"replacement").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert!(guard.check().is_err(), "{target}");
        assert!(guard.save().is_err(), "{target}");
    }
    let dir = tempfile::tempdir().unwrap();
    let _store = member(&dir);
    let root = dir.path().join("host");
    {
        let mut guard = Guard::open(&root).unwrap();
        guard.save().unwrap();
    }
    let path = root.join("crew-control/state.json");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(Guard::open(&root).is_err());
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o644
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::hard_link(&path, root.join("duplicate.json")).unwrap();
    assert!(Guard::open(&root).is_err());
}
