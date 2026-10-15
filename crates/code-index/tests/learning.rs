mod common;
use code_index::{learning::*, recipes::Context, store};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn feedback_is_deduplicated_scoped_and_contains_no_question_text_or_labels() {
    let dir = tempfile::tempdir().unwrap();
    let partitions = Partitions {
        schema: "openagents.task-partitions.v1".into(),
        repository: "repo".into(),
        workspace: "private".into(),
        training_cutoff_ms: 10,
        groups: BTreeMap::new(),
    };
    let observation = serde_json::json!({"schema":"openagents.lookup-observation.v1","repository":"repo","workspace":"private","question_digest":code_index::digest(b"private question"),"question":"private question","status":"no_match","required_roles_complete":false,"elapsed_ms":17});
    for name in ["lookup-1.json", "lookup-2.json"] {
        std::fs::write(
            dir.path().join(name),
            serde_json::to_vec(&observation).unwrap(),
        )
        .unwrap();
    }
    let mut other = observation.clone();
    other["workspace"] = "other".into();
    std::fs::write(
        dir.path().join("lookup-3.json"),
        serde_json::to_vec(&other).unwrap(),
    )
    .unwrap();
    let feedback = collect(dir.path(), &partitions, None).unwrap();
    assert_eq!(feedback.observations.len(), 1);
    assert_eq!(feedback.observations[0].elapsed_ms, 17);
    assert!(feedback.observations[0].group.is_none());
    let saved = serde_json::to_string(&feedback).unwrap();
    assert!(!saved.contains("private question"));
    assert!(!saved.contains("label"));
    assert_eq!(feedback.notices.len(), 1);
}

#[test]
fn saved_selector_rollback_checks_the_active_baseline_and_installed_artifact() {
    let repo = common::fixture();
    let cache = tempfile::tempdir().unwrap();
    let context = Context::open(repo.path(), "HEAD", "test").unwrap();
    let artifact = context.verify(&common::artifact(&context)).unwrap();
    let index = store::Store::new(cache.path(), &artifact.snapshot);
    let bundle = store::bind(&context, &artifact, Default::default()).unwrap();
    index.install(&bundle, None).unwrap();
    let mut selector = Selector {
        schema: "openagents.task-selector.v1".into(),
        repository: artifact.snapshot.repository.clone(),
        workspace: "test".into(),
        artifact_digest: atif::digest(&serde_json::to_value(&artifact).unwrap()),
        partition_digest: "partitions".into(),
        training_digest: "training-one".into(),
        training_groups: BTreeSet::from(["training".into()]),
        weights: BTreeMap::new(),
        fixture_only: false,
    };
    let mut ids = Vec::new();
    // Synthetic admitted records test persistence only. They are never exported
    // or treated as a real model-quality experiment.
    for training in ["training-one", "training-two"] {
        selector.training_digest = training.into();
        let id = atif::digest(&serde_json::to_value(&selector).unwrap());
        let receipt = SelectorReceipt {
            schema: "openagents.task-selector-comparison.v1".into(),
            candidate_digest: id.clone(),
            baseline_digest: ids.last().cloned(),
            artifact_digest: selector.artifact_digest.clone(),
            partition_digest: selector.partition_digest.clone(),
            dataset_digest: "synthetic".into(),
            evaluation_digest: "synthetic".into(),
            minimum_gain: 0.01,
            candidate_recall: 1.0,
            baseline_recall: 0.0,
            passed: true,
        };
        for (name, value) in [
            (
                format!("selector-{id}.json"),
                serde_json::to_vec(&selector).unwrap(),
            ),
            (
                format!("selector-{id}-receipt.json"),
                serde_json::to_vec(&receipt).unwrap(),
            ),
        ] {
            code_index::private::write(&index.directory, &name, &value).unwrap();
        }
        ids.push(id);
    }
    code_index::private::write(
        &index.directory,
        "selector-active.json",
        &serde_json::to_vec(&ids[1]).unwrap(),
    )
    .unwrap();
    assert!(active(&index.directory, &artifact).unwrap().is_some());
    assert!(rollback(&index.directory, &ids[0], &ids[0]).is_err());
    rollback(&index.directory, &ids[0], &ids[1]).unwrap();
    assert_eq!(
        active(&index.directory, &artifact)
            .unwrap()
            .unwrap()
            .training_digest,
        "training-one"
    );
    let mut changed = artifact.clone();
    changed.recipes[0]
        .description
        .push_str(" Changed artifact.");
    index
        .install(
            &store::bind(&context, &changed, Default::default()).unwrap(),
            index.active().unwrap().as_deref(),
        )
        .unwrap();
    assert!(rollback(&index.directory, &ids[1], &ids[0]).is_err());
    assert!(active(&index.directory, &changed).is_err());
}
