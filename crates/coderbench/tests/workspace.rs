//! The bridge from independent filesystem evidence to a benchmark grade.
use coder_boundary::snapshot::{Limits, Snapshot};
use coderbench::Workspace;

#[test]
fn a_partial_snapshot_cannot_supply_a_clean_workspace() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), "contents").unwrap();
    let partial = Snapshot::observe_within(root.path(), Limits::bounded(10, 1));
    let complete = Snapshot::observe(root.path());
    assert!(!partial.is_complete());
    assert!(Workspace::between(&partial, &complete).is_err());
    assert!(Workspace::between(&complete, &partial).is_err());
    assert!(Workspace::between(&partial, &partial).is_err());
}

#[test]
fn renames_and_deletions_keep_every_affected_path() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("old"), "rename").unwrap();
    std::fs::write(root.path().join("deleted"), "remove").unwrap();
    let before = Snapshot::observe(root.path());
    std::fs::rename(root.path().join("old"), root.path().join("new")).unwrap();
    std::fs::remove_file(root.path().join("deleted")).unwrap();
    let after = Snapshot::observe(root.path());
    let workspace = Workspace::between(&before, &after).unwrap();
    assert_eq!(workspace.changed, ["deleted", "new", "old"]);
}
