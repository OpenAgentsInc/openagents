//! What a pair of snapshots establishes: writes seen, and partial
//! observations that read as unverifiable rather than clean.
//!
//! The observation is descriptor-relative and never follows a link,
//! which exists on Unix only — so every behavioral test here is a Unix
//! test, and a platform without it gets a refusal instead.

use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;

#[cfg(unix)]
use coder_boundary::snapshot::{Change, Fault, Limits, Verdict};
use coder_boundary::snapshot::{Snapshot, compare};
use tempfile::TempDir;

fn write(dir: &Path, rel: &str, text: &str) {
    std::fs::write(dir.join(rel), text).unwrap();
}

#[cfg(unix)]
fn changes(verdict: &Verdict) -> &[Change] {
    match verdict {
        Verdict::Changed(changes) => changes,
        verdict => panic!("expected Changed, got {verdict:?}"),
    }
}

#[cfg(unix)]
#[test]
fn an_unchanged_tree_observes_clean() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.rs", "one");
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let before = Snapshot::observe(dir.path());
    let after = Snapshot::observe(dir.path());
    assert!(before.is_complete(), "{:?}", before.faults());
    assert_eq!(before.digest(), after.digest());
    assert_eq!(compare(&before, &after), Verdict::Clean);
}

#[cfg(unix)]
#[test]
fn creation_removal_and_modification_are_seen() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "kept.rs", "same");
    write(dir.path(), "gone.rs", "bye");
    write(dir.path(), "edited.rs", "before");
    let before = Snapshot::observe(dir.path());

    write(dir.path(), "new.rs", "hi");
    std::fs::remove_file(dir.path().join("gone.rs")).unwrap();
    write(dir.path(), "edited.rs", "after");
    let after = Snapshot::observe(dir.path());

    assert_eq!(
        changes(&compare(&before, &after)),
        &[
            Change::Modified {
                path: Path::new("edited.rs").to_path_buf()
            },
            Change::Removed {
                path: Path::new("gone.rs").to_path_buf()
            },
            Change::Created {
                path: Path::new("new.rs").to_path_buf()
            },
        ]
    );
}

/// A file already dirty before the first observation is still compared
/// by content — the case a `git status` string cannot see.
#[cfg(unix)]
#[test]
fn a_content_change_to_an_already_dirty_file_is_seen() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "dirty.rs", "not what HEAD holds");
    let before = Snapshot::observe(dir.path());
    write(dir.path(), "dirty.rs", "changed again");
    let after = Snapshot::observe(dir.path());
    assert_eq!(
        changes(&compare(&before, &after)),
        &[Change::Modified {
            path: Path::new("dirty.rs").to_path_buf()
        }]
    );
}

/// A rewrite that changes only the modification time is still a write.
#[cfg(unix)]
#[test]
fn a_rewrite_with_identical_content_is_seen() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("touched.rs");
    write(dir.path(), "touched.rs", "same bytes");
    let before = Snapshot::observe(dir.path());
    // The kernel's file-time clock is coarser than the wall clock on some
    // platforms (Linux ticks at a few milliseconds), so wait until the
    // rewrite lands on a later timestamp rather than for a fixed interval.
    let stamped = std::fs::metadata(&file).unwrap().modified().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        write(dir.path(), "touched.rs", "same bytes");
        if std::fs::metadata(&file).unwrap().modified().unwrap() != stamped {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "file time never advanced"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let after = Snapshot::observe(dir.path());
    assert_eq!(
        changes(&compare(&before, &after)),
        &[Change::Modified {
            path: Path::new("touched.rs").to_path_buf()
        }]
    );
}

/// The root's own metadata is an entry too, at the empty path.
#[cfg(unix)]
#[test]
fn a_change_to_the_roots_own_metadata_is_seen() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = TempDir::new().unwrap();
    let before = Snapshot::observe(dir.path());
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o750)).unwrap();
    let after = Snapshot::observe(dir.path());
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        changes(&compare(&before, &after)),
        &[Change::Modified {
            path: PathBuf::new()
        }]
    );
}

#[cfg(unix)]
#[test]
fn a_rename_is_a_rename_not_a_delete_and_create() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "old.rs", "moved");
    let before = Snapshot::observe(dir.path());
    std::fs::rename(dir.path().join("old.rs"), dir.path().join("new.rs")).unwrap();
    let after = Snapshot::observe(dir.path());
    assert_eq!(
        changes(&compare(&before, &after)),
        &[Change::Renamed {
            from: Path::new("old.rs").to_path_buf(),
            to: Path::new("new.rs").to_path_buf(),
            altered: false
        }]
    );
}

#[cfg(unix)]
#[test]
fn a_renamed_and_altered_file_reports_both() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "old.rs", "moved");
    let before = Snapshot::observe(dir.path());
    let moved = dir.path().join("new.rs");
    std::fs::rename(dir.path().join("old.rs"), &moved).unwrap();
    std::fs::write(&moved, "moved and changed").unwrap();
    let after = Snapshot::observe(dir.path());
    assert_eq!(
        changes(&compare(&before, &after)),
        &[Change::Renamed {
            from: Path::new("old.rs").to_path_buf(),
            to: Path::new("new.rs").to_path_buf(),
            altered: true
        }]
    );
}

/// A directory rename pairs on inode, so the whole moved subtree reads
/// as renames rather than as a delete-and-create of everything in it.
#[cfg(unix)]
#[test]
fn a_renamed_directory_carries_its_entries_with_it() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("old")).unwrap();
    write(dir.path(), "old/inner.rs", "inside");
    let before = Snapshot::observe(dir.path());
    std::fs::rename(dir.path().join("old"), dir.path().join("new")).unwrap();
    let after = Snapshot::observe(dir.path());
    let verdict = compare(&before, &after);
    let changes = changes(&verdict);
    assert!(
        changes
            .iter()
            .all(|change| matches!(change, Change::Renamed { .. })),
        "a rename read as something else: {changes:?}"
    );
    assert!(
        changes.iter().any(|change| matches!(
            change,
            Change::Renamed { from, to, .. }
            if from == Path::new("old") && to == Path::new("new")
        )),
        "the directory's own rename is missing: {changes:?}"
    );
    assert!(
        changes.iter().any(|change| matches!(
            change,
            Change::Renamed { from, to, .. }
            if from == Path::new("old/inner.rs") && to == Path::new("new/inner.rs")
        )),
        "the child's rename is missing: {changes:?}"
    );
}

/// A symlink is recorded as a symlink. It is never opened, let alone
/// followed, so a link that names something outside the root does not
/// pull the outside into the observation.
#[cfg(unix)]
#[test]
fn a_symlink_is_observed_not_followed() {
    let outside = TempDir::new().unwrap();
    write(outside.path(), "secret.txt", "not the walk's business");
    let dir = TempDir::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();

    let before = Snapshot::observe(dir.path());
    assert!(before.is_complete(), "{:?}", before.faults());
    // The root's own entry and the link — and nothing under the link.
    assert_eq!(before.len(), 2, "the walk descended into the link");

    // Writing the outside through the link's target changes nothing the
    // observation owns.
    write(outside.path(), "secret.txt", "rewritten");
    write(outside.path(), "new.txt", "also outside");
    let after = Snapshot::observe(dir.path());
    assert_eq!(compare(&before, &after), Verdict::Clean);

    // Retargeting the link is a change to the link.
    std::fs::remove_file(dir.path().join("link")).unwrap();
    std::os::unix::fs::symlink("/nonexistent", dir.path().join("link")).unwrap();
    let retargeted = Snapshot::observe(dir.path());
    assert_eq!(
        changes(&compare(&after, &retargeted)),
        &[Change::Modified {
            path: Path::new("link").to_path_buf()
        }]
    );
}

#[cfg(unix)]
#[test]
fn a_path_that_changes_kind_is_retyped() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "thing", "was a file");
    let before = Snapshot::observe(dir.path());
    std::fs::remove_file(dir.path().join("thing")).unwrap();
    std::os::unix::fs::symlink("/tmp", dir.path().join("thing")).unwrap();
    let after = Snapshot::observe(dir.path());
    assert_eq!(
        changes(&compare(&before, &after)),
        &[Change::Retyped {
            path: Path::new("thing").to_path_buf()
        }]
    );
}

/// An unreadable entry makes the observation partial, and a partial
/// observation cannot establish that nothing was written — even when two
/// of them look identical.
#[cfg(unix)]
#[test]
fn an_unreadable_tree_is_unverifiable_not_clean() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "locked.rs", "unreadable");
    std::fs::set_permissions(
        dir.path().join("locked.rs"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();

    let before = Snapshot::observe(dir.path());
    let after = Snapshot::observe(dir.path());
    let verdict = compare(&before, &after);

    // Clean up before asserting, so a failure does not strand the file.
    std::fs::set_permissions(
        dir.path().join("locked.rs"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    // A privileged reader is not locked out, and there is nothing left
    // for this case to check.
    if before.is_complete() {
        return;
    }
    assert!(
        matches!(before.faults()[0], Fault::Read { .. }),
        "{:?}",
        before.faults()
    );
    assert!(
        matches!(verdict, Verdict::Unverifiable(_)),
        "two equally partial observations read as clean"
    );
}

/// Every listed entry counts against the entries bound — a failed one
/// included — so a tree cannot slip past the bound on faults.
#[cfg(unix)]
#[test]
fn failed_entries_count_against_the_bound_too() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = TempDir::new().unwrap();
    for n in 0..4 {
        let name = format!("f{n}");
        write(dir.path(), &name, "x");
        std::fs::set_permissions(
            dir.path().join(&name),
            std::fs::Permissions::from_mode(0o000),
        )
        .unwrap();
    }
    let snapshot = Snapshot::observe_within(dir.path(), Limits::bounded(3, u64::MAX));
    for n in 0..4 {
        std::fs::set_permissions(
            dir.path().join(format!("f{n}")),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    assert!(!snapshot.is_complete());
    assert!(
        snapshot
            .faults()
            .iter()
            .any(|fault| matches!(fault, Fault::Entries { limit: 3 })),
        "{:?}",
        snapshot.faults()
    );
}

#[cfg(unix)]
#[test]
fn an_entry_bound_is_unverifiable_not_truncated_clean() {
    let dir = TempDir::new().unwrap();
    for n in 0..4 {
        write(dir.path(), &format!("f{n}"), "x");
    }
    let snapshot = Snapshot::observe_within(dir.path(), Limits::bounded(2, u64::MAX));
    assert!(!snapshot.is_complete());
    assert!(
        matches!(snapshot.faults()[0], Fault::Entries { limit: 2 }),
        "{:?}",
        snapshot.faults()
    );
    // Identical bounds, identical tree: still no ground for clean.
    let again = Snapshot::observe_within(dir.path(), Limits::bounded(2, u64::MAX));
    assert!(compare(&snapshot, &again).is_unverifiable());
}

#[cfg(unix)]
#[test]
fn a_byte_bound_is_unverifiable() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "big", &"x".repeat(64));
    let snapshot = Snapshot::observe_within(dir.path(), Limits::bounded(1000, 8));
    assert!(!snapshot.is_complete());
    assert!(
        matches!(snapshot.faults()[0], Fault::Bytes { limit: 8 }),
        "{:?}",
        snapshot.faults()
    );
}

#[cfg(unix)]
#[test]
fn a_missing_root_is_unverifiable() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("absent");
    let snapshot = Snapshot::observe(&missing);
    assert!(matches!(snapshot.faults()[0], Fault::Root { .. }));
    let verdict = compare(&snapshot, &snapshot);
    assert!(matches!(verdict, Verdict::Unverifiable(_)));
}

/// The observation is deterministic: the same tree, walked again, names
/// itself the same way and compares clean.
#[cfg(unix)]
#[test]
fn observation_is_deterministic() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("a")).unwrap();
    write(dir.path(), "a/1.rs", "one");
    write(dir.path(), "a/2.rs", "two");
    write(dir.path(), "b.rs", "three");
    let first = Snapshot::observe(dir.path());
    let second = Snapshot::observe(dir.path());
    assert_eq!(first.digest(), second.digest());
    assert!(compare(&first, &second).is_clean());
}

/// Where no-follow cannot be promised there is no observation at all —
/// a refusal, not a quiet partial walk.
#[cfg(not(unix))]
#[test]
fn observation_is_refused_where_no_follow_cannot_be_promised() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.rs", "one");
    let snapshot = Snapshot::observe(dir.path());
    assert!(!snapshot.is_complete());
    assert!(compare(&snapshot, &snapshot).is_unverifiable());
}

#[cfg(unix)]
#[test]
fn replacing_a_file_with_identical_content_and_mtime_is_seen() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let path = dir.path().join("same.rs");
    std::fs::write(&path, "same").unwrap();
    let original = std::fs::File::open(&path).unwrap();
    let metadata = original.metadata().unwrap();
    let before = Snapshot::observe(dir.path());
    let replacement = outside.path().join("replacement");
    std::fs::write(&replacement, "same").unwrap();
    let file = std::fs::File::options()
        .write(true)
        .open(&replacement)
        .unwrap();
    file.set_modified(metadata.modified().unwrap()).unwrap();
    std::fs::set_permissions(&replacement, metadata.permissions()).unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    let after = Snapshot::observe(dir.path());
    assert_eq!(
        changes(&compare(&before, &after)),
        &[Change::Modified {
            path: Path::new("same.rs").to_path_buf()
        }]
    );
}
