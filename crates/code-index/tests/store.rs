mod common;
use code_index::{recipes::Context, store::*};
use std::collections::BTreeSet;

#[test]
fn discovery_scopes_catch_new_siblings_and_stale_never_clears_itself() {
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let bundle = bind(&context, &common::artifact(&context), BTreeSet::new()).unwrap();
    std::fs::write(dir.path().join("src/new.rs"), "pub fn sibling() {}\n").unwrap();
    assert!(
        dirty(&bundle, dir.path(), &context.graph)
            .unwrap()
            .contains("add-kind")
    );
    common::git(dir.path(), &["add", "."]);
    common::git(dir.path(), &["commit", "-qm", "sibling"]);
    let next = Context::open(dir.path(), "HEAD", "test").unwrap();
    let refreshed = refresh(&bundle, &next).unwrap();
    assert!(refreshed.stale.contains_key("add-kind"));
    assert!(
        refresh(&refreshed, &next)
            .unwrap()
            .stale
            .contains_key("add-kind")
    );
}

#[test]
fn unrelated_source_can_carry_forward_but_manifest_changes_invalidate() {
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let bundle = bind(&context, &common::artifact(&context), BTreeSet::new()).unwrap();
    std::fs::write(dir.path().join("notes.md"), "A separate note.\n").unwrap();
    common::git(dir.path(), &["add", "."]);
    common::git(dir.path(), &["commit", "-qm", "note"]);
    let next = Context::open(dir.path(), "HEAD", "test").unwrap();
    assert!(refresh(&bundle, &next).unwrap().stale.is_empty());
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"renamed\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    common::git(dir.path(), &["add", "."]);
    common::git(dir.path(), &["commit", "-qm", "rename"]);
    let next = Context::open(dir.path(), "HEAD", "test").unwrap();
    assert!(
        refresh(&bundle, &next)
            .unwrap()
            .stale
            .contains_key("add-kind")
    );
}

#[test]
fn object_store_isolates_workspaces_and_guards_activation_and_rollback() {
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let bundle = bind(&context, &common::artifact(&context), BTreeSet::new()).unwrap();
    let cache = tempfile::tempdir().unwrap();
    let store = Store::new(cache.path(), &context.snapshot);
    let a = store.install(&bundle, None).unwrap();
    assert_eq!(store.active().unwrap(), Some(a.clone()));
    let mut private = bundle.clone();
    private.private_sources.insert("local-source".into());
    let b = store.install(&private, Some(&a)).unwrap();
    assert!(store.install(&bundle, Some(&a)).is_err());
    store.rollback(&a, Some(&b)).unwrap();
    assert_eq!(store.active().unwrap(), Some(a.clone()));
    let mut other = context.snapshot.clone();
    other.workspace = "other".into();
    assert!(Store::new(cache.path(), &other).load(&a).is_err());
    std::fs::write(store.directory.join(format!("{a}.json")), "{}").unwrap();
    assert!(store.load(&a).is_err());
}

#[test]
fn publication_has_no_parent_and_private_withdrawal_blocks_export() {
    let dir = common::fixture();
    common::git(dir.path(), &["config", "user.name", "Test"]);
    common::git(dir.path(), &["config", "user.email", "test@example.com"]);
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let bundle = bind(&context, &common::artifact(&context), BTreeSet::new()).unwrap();
    let commit = publish(dir.path(), &bundle, None).unwrap();
    assert_eq!(
        common::git(dir.path(), &["rev-list", "--count", &commit]),
        "1"
    );
    assert_eq!(
        common::git(dir.path(), &["ls-tree", "--name-only", &commit]),
        "COMMIT\ntask-index.json"
    );
    let mut private = bundle;
    private.private_sources.insert("source".into());
    assert!(publish(dir.path(), &private, None).is_err());
    let withdrawn = withdraw(&private, "source");
    assert!(withdrawn.artifact.recipes.is_empty());
    assert!(publish(dir.path(), &withdrawn, None).is_err());
}

#[test]
fn withdrawing_a_private_source_removes_derived_objects_and_rollback_targets() {
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let bundle = bind(
        &context,
        &common::artifact(&context),
        ["private-source".into()].into(),
    )
    .unwrap();
    let cache = tempfile::tempdir().unwrap();
    let store = Store::new(cache.path(), &context.snapshot);
    let id = store.install(&bundle, None).unwrap();
    for name in [
        "selector-active.json",
        "selector-old.json",
        "selector-old-receipt.json",
    ] {
        std::fs::write(store.directory.join(name), b"private derivative").unwrap();
    }
    assert_eq!(
        store.withdraw_source("private-source", Some(&id)).unwrap(),
        1
    );
    assert!(store.load(&id).is_err());
    assert!(!store.directory.join("selector-active.json").exists());
    assert!(!store.directory.join("selector-old.json").exists());
    assert!(!store.directory.join("selector-old-receipt.json").exists());
    let active = store.active().unwrap().unwrap();
    assert!(store.load(&active).unwrap().artifact.recipes.is_empty());
}

#[test]
fn published_bundle_reaches_a_clone_without_touching_its_worktree() {
    let dir = common::fixture();
    let space = tempfile::tempdir().unwrap();
    let remote = space.path().join("remote.git");
    common::git(
        space.path(),
        &["init", "-q", "--bare", remote.to_str().unwrap()],
    );
    common::git(
        dir.path(),
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    common::git(dir.path(), &["config", "user.name", "Test"]);
    common::git(dir.path(), &["config", "user.email", "test@example.com"]);
    common::git(
        dir.path(),
        &["push", "-q", "origin", "HEAD:refs/heads/main"],
    );
    let context = Context::open(dir.path(), "HEAD", "public").unwrap();
    let bundle = bind(&context, &common::artifact(&context), BTreeSet::new()).unwrap();
    publish(dir.path(), &bundle, Some("origin")).unwrap();
    let clone = space.path().join("clone");
    common::git(
        space.path(),
        &[
            "clone",
            "-q",
            "-b",
            "main",
            remote.to_str().unwrap(),
            clone.to_str().unwrap(),
        ],
    );
    let id = fetch(&clone, "origin", "public", &space.path().join("cache")).unwrap();
    let snapshot = code_index::recipes::snapshot(&clone, "HEAD", "public").unwrap();
    let stored = Store::new(&space.path().join("cache"), &snapshot)
        .load(&id)
        .unwrap();
    assert!(stored.stale.is_empty());
    assert_eq!(common::git(&clone, &["status", "--short"]), "");
}

#[test]
fn github_transport_spellings_share_a_repository_identity() {
    assert_eq!(
        code_index::recipes::normalize_remote("git@github.com:OpenAgentsInc/openagents.git"),
        code_index::recipes::normalize_remote("https://github.com/OpenAgentsInc/openagents.git")
    );
}
