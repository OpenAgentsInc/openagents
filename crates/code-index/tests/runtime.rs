mod common;
use code_index::{recipes::Context, runtime::*, store::*};
use serde_json::json;

#[test]
fn runtime_offers_only_installed_indexes_and_records_no_question_text() {
    let dir = common::fixture();
    let cache = tempfile::tempdir().unwrap();
    let context = Context::open(dir.path(), "HEAD", "test").unwrap();
    let config = Configuration {
        store: Some(cache.path().into()),
        workspace: "test".into(),
    };
    assert!(!available(dir.path(), &config));
    let bundle = bind(&context, &common::artifact(&context), Default::default()).unwrap();
    let store = Store::new(cache.path(), &context.snapshot);
    store.install(&bundle, None).unwrap();
    assert!(available(dir.path(), &config));
    let answer = execute(
        dir.path(),
        &json!({"question":"register a kind UNIQUE_PRIVATE_PHRASE"}),
        &config,
    )
    .unwrap();
    assert_eq!(answer["status"], "matched");
    let observations = std::fs::read_dir(store.directory.join("observations"))
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(observations.len(), 1);
    let text =
        std::fs::read_to_string(observations.into_iter().next().unwrap().unwrap().path()).unwrap();
    assert!(!text.contains("UNIQUE_PRIVATE_PHRASE"));
    std::fs::write(dir.path().join("src/lib.rs"), "pub enum Kind { A, B }\n").unwrap();
    let answer = execute(dir.path(), &json!({"question":"register a kind"}), &config).unwrap();
    assert_eq!(answer["status"], "stale");
    assert_eq!(answer["required_roles_complete"], false);
}
