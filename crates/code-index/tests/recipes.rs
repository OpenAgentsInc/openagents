mod common;
use code_index::recipes::*;

#[test]
fn references_bind_to_objects_not_dirty_worktree_and_never_claim_replay() {
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let mut artifact = common::artifact(&context);
    artifact.recipes[0].evidence = EvidenceClass::ReplayVerified;
    std::fs::write(dir.path().join("src/lib.rs"), "pub enum Changed {}\n").unwrap();
    let checked = context.verify(&artifact).unwrap();
    assert_eq!(checked.recipes[0].evidence, EvidenceClass::SourceVerified);
    assert_eq!(
        checked.recipes[0].roles[0].references[0].excerpt,
        "pub enum Kind { A }"
    );
}

#[test]
fn invented_or_escaping_references_and_wrong_workspaces_fail() {
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let artifact = common::artifact(&context);
    for change in 0..5 {
        let mut bad = artifact.clone();
        match change {
            0 => bad.recipes[0].roles[0].references[0].path = "../outside.rs".into(),
            1 => bad.recipes[0].roles[0].references[0].symbol = Some("demo::Invented".into()),
            2 => bad.recipes[0].roles[0].references[0].excerpt = "invented".into(),
            3 => bad.snapshot.workspace = "other".into(),
            _ => bad.recipes[0].roles[0].when = Some("undeclared".into()),
        }
        assert!(context.verify(&bad).is_err());
    }
}

#[test]
fn check_packages_and_new_file_integration_need_real_source() {
    let dir = common::fixture();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let mut artifact = common::artifact(&context);
    artifact.recipes[0].checks[0].package = "other".into();
    assert!(context.verify(&artifact).is_err());
    artifact.recipes[0].checks[0].package = "demo".into();
    artifact.recipes[0].new_files[0].directory = "absent".into();
    assert!(context.verify(&artifact).is_err());
}
