use code_index::{demand::*, episodes::*};
use serde_json::json;

fn corpus() -> Corpus {
    let mut corpus = Corpus::default();
    for (id, family, question) in [
        ("a", "task-1", "Add a model"),
        ("b", "task-1", "Add a model"),
        ("c", "task-2", "Add a model"),
        ("d", "task-3", "Place a model"),
    ] {
        corpus.episodes.push(Episode {
            id: id.into(),
            family: family.into(),
            source_id: id.into(),
            turn: 1,
            question: question.into(),
            repository: None,
            revision: None,
            subagent: id == "b",
            parent: None,
            operations: vec![Operation {
                call_id: "x".into(),
                name: "Read".into(),
                arguments: json!({}),
                activity: Activity::Read,
                phase: Phase::Discovery,
                at_ms: None,
                duration_ms: None,
                failed: None,
                output_bytes: None,
            }],
            usage: Usage::default(),
            claimed_outcome: None,
        });
    }
    corpus
}

#[test]
fn delegates_do_not_inflate_support_or_claim_unknown_costs() {
    let d = rank(&corpus(), None, Assumptions::default()).unwrap();
    let add = d
        .candidates
        .iter()
        .find(|c| c.question == "Add a model")
        .unwrap();
    assert_eq!(add.episodes.len(), 3);
    assert_eq!(add.families.len(), 2);
    assert_eq!(add.navigation_by_family.values().sum::<usize>(), 2);
    assert_eq!(add.expected_net_saving, None);
}

#[test]
fn semantic_grouping_keeps_different_operations_and_partitions_separate() {
    let c = corpus();
    let mut grouping = Grouping {
        schema: "openagents.task-grouping.v1".into(),
        corpus_digest: corpus_digest(&c).unwrap(),
        teacher: "frozen".into(),
        assignments: c
            .episodes
            .iter()
            .map(|e| Assignment {
                episode: e.id.clone(),
                cluster: Some("models".into()),
                operation: if e.id == "d" { "place" } else { "add" }.into(),
                aliases: vec![],
                partition: Partition::Training,
            })
            .collect(),
    };
    assert_eq!(
        rank(&c, Some(&grouping), Assumptions::default())
            .unwrap()
            .candidates
            .len(),
        2
    );
    grouping.assignments[1].partition = Partition::Locked;
    assert!(rank(&c, Some(&grouping), Assumptions::default()).is_err());
    grouping.corpus_digest = "wrong".into();
    assert!(rank(&c, Some(&grouping), Assumptions::default()).is_err());
}

#[test]
fn cost_assumptions_are_explicit_and_validated() {
    let d = rank(
        &corpus(),
        None,
        Assumptions {
            future_uses: Some(10.0),
            avoidable_fraction: Some(0.5),
            cost_per_navigation: Some(2.0),
            build_cost: Some(3.0),
            refresh_cost: Some(1.0),
        },
    )
    .unwrap();
    assert_eq!(d.candidates[0].expected_net_saving, Some(6.0));
    assert!(
        rank(
            &corpus(),
            None,
            Assumptions {
                avoidable_fraction: Some(1.1),
                ..Default::default()
            }
        )
        .is_err()
    );
}
