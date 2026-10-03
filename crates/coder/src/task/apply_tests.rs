//! `chat apply` carries a local task's change into its checkout (#10343).

use std::collections::BTreeMap;

use super::*;
use crate::task::local::{RECORD_SCHEMA, Record};

fn git(dir: &Path, args: &[&str]) -> String {
    let output = local::git().arg("-C").arg(dir).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A checkout with one commit, a task worktree of it, and the task's
/// record in `store`.
fn scratch() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let top = root.path().canonicalize().unwrap();
    let checkout = top.join("checkout");
    std::fs::create_dir_all(&checkout).unwrap();
    git(&checkout, &["init", "-q", "-b", "main"]);
    for (key, value) in [
        ("user.email", "t@t"),
        ("user.name", "t"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", "/dev/null"),
    ] {
        git(&checkout, &["config", key, value]);
    }
    std::fs::write(checkout.join("a.txt"), "one\n").unwrap();
    git(&checkout, &["add", "-A"]);
    git(&checkout, &["commit", "-q", "-m", "start"]);
    let base = git(&checkout, &["rev-parse", "HEAD"]);
    let worktree = top.join("worktrees").join("checkout-task");
    git(
        &checkout,
        &[
            "worktree",
            "add",
            "--detach",
            "-q",
            worktree.to_str().unwrap(),
        ],
    );
    let store = top.join("store");
    std::fs::create_dir_all(&store).unwrap();
    local::save(
        &store,
        &Record {
            schema: RECORD_SCHEMA.into(),
            task: "task".into(),
            thread: None,
            project: "checkout".into(),
            checkout: checkout.display().to_string(),
            worktree: worktree.display().to_string(),
            base,
            turns: Vec::new(),
            ends: BTreeMap::new(),
            requested: None,
            shape: local::Shape::default(),
            hooks: None,
            archived: None,
        },
    )
    .unwrap();
    (root, checkout, worktree, store)
}

#[test]
fn a_tasks_change_and_new_files_land_uncommitted_in_the_checkout() {
    let (_root, checkout, worktree, store) = scratch();
    std::fs::write(worktree.join("a.txt"), "two\n").unwrap();
    std::fs::write(worktree.join("README.md"), "# Readme\n").unwrap();
    let applied = apply(&store, "task").unwrap();
    assert_eq!(applied.checkout, checkout);
    assert_eq!(applied.files, vec!["README.md", "a.txt"]);
    assert_eq!(
        std::fs::read_to_string(checkout.join("a.txt")).unwrap(),
        "two\n"
    );
    assert_eq!(
        std::fs::read_to_string(checkout.join("README.md")).unwrap(),
        "# Readme\n"
    );
    // Nothing was committed, and the worktree's own index is untouched.
    assert_eq!(git(&checkout, &["log", "--format=%s"]), "start");
    assert_eq!(git(&worktree, &["diff", "--cached", "--name-only"]), "");
}

#[test]
fn a_committed_change_in_the_worktree_applies_too() {
    let (_root, checkout, worktree, store) = scratch();
    std::fs::write(worktree.join("a.txt"), "three\n").unwrap();
    git(&worktree, &["commit", "-q", "-am", "three"]);
    apply(&store, "task").unwrap();
    assert_eq!(
        std::fs::read_to_string(checkout.join("a.txt")).unwrap(),
        "three\n"
    );
}

#[test]
fn a_checkout_with_changes_of_its_own_or_no_change_refuses() {
    let (_root, checkout, worktree, store) = scratch();
    assert_eq!(
        apply(&store, "task").unwrap_err(),
        "This task changed nothing to apply."
    );
    std::fs::write(worktree.join("a.txt"), "two\n").unwrap();
    std::fs::write(checkout.join("a.txt"), "mine\n").unwrap();
    let refused = apply(&store, "task").unwrap_err();
    assert!(refused.contains("has changes of its own"), "{refused}");
    assert_eq!(
        std::fs::read_to_string(checkout.join("a.txt")).unwrap(),
        "mine\n"
    );
    assert!(apply(&store, "missing").is_err());
}
