//! Execution graphs (#10703): two parallel nodes and one dependent node
//! run once each; cancellation, unavailable artifacts, failed checks, and
//! unknown dispatch stay attributable per node; rework needs remaining
//! authority or a new offer.

use crate::digest::Digest;
use crate::graph::*;
use crate::lifecycle::CheckLabel;

fn d(text: &str) -> Digest {
    Digest::of_bytes(text.as_bytes())
}

fn node(id: &str, needs: &[&str], binds: &[(&str, &str)]) -> Node {
    Node {
        id: id.into(),
        needs: needs.iter().map(|need| (*need).to_owned()).collect(),
        input: d(id),
        admission: d(&format!("admission-{id}")),
        binds: binds
            .iter()
            .map(|(node, path)| ArtifactBinding {
                node: (*node).into(),
                path: (*path).into(),
            })
            .collect(),
        max_sats: None,
    }
}

/// `a` and `b` in parallel; `merge` needs both and reads `a`'s patch.
fn graph() -> Graph {
    Graph {
        schema: GRAPH_SCHEMA.into(),
        id: "graph-1".into(),
        nodes: vec![
            node("a", &[], &[]),
            node("b", &[], &[]),
            node("merge", &["a", "b"], &[("a", "patch.diff")]),
        ],
        max_parallel: 2,
        max_attempts: 2,
        funding: Funding::Owned,
        publication: false,
    }
}

fn verified(artifacts: &[&str]) -> Report {
    Report::Completed {
        check: CheckLabel::Verified,
        artifacts: artifacts.iter().map(|a| (*a).to_owned()).collect(),
    }
}

#[test]
fn two_parallel_nodes_and_one_dependent_each_run_once() {
    let mut run = GraphRun::new(graph()).unwrap();
    assert_eq!(run.ready(), ["a", "b"]);
    run.dispatched("a", "task-a").unwrap();
    run.dispatched("b", "task-b").unwrap();
    // The dependent waits, and nothing dispatches twice.
    assert!(run.ready().is_empty());
    assert_eq!(
        run.dispatched("a", "task-a2"),
        Err(Refused::AlreadyDispatched)
    );
    assert_eq!(run.dispatched("merge", "task-m"), Err(Refused::NotReady));
    run.report("a", verified(&["patch.diff"]));
    assert!(run.ready().is_empty());
    run.report("b", verified(&[]));
    assert_eq!(run.ready(), ["merge"]);
    run.dispatched("merge", "task-m").unwrap();
    run.report("merge", verified(&[]));
    assert!(run.settled());
    assert_eq!(
        run.attempts.values().copied().collect::<Vec<_>>(),
        [1, 1, 1]
    );
    // Child completion grants no publication the admission lacked.
    assert!(!run.may_publish());
    let mut publishing = graph();
    publishing.publication = true;
    let run = GraphRun::new(publishing).unwrap();
    assert!(!run.may_publish(), "nothing verified yet");
    // The summary is a read.
    let before = run.clone();
    let _ = run.summary();
    assert_eq!(run, before);
}

#[test]
fn the_parallel_bound_holds() {
    let mut wide = graph();
    wide.max_parallel = 1;
    let mut run = GraphRun::new(wide).unwrap();
    assert_eq!(run.ready(), ["a"]);
    run.dispatched("a", "task-a").unwrap();
    assert_eq!(run.dispatched("b", "task-b"), Err(Refused::NotReady));
}

#[test]
fn outcomes_stay_attributable_per_node() {
    // A failed check blocks the dependent with that cause.
    let mut run = GraphRun::new(graph()).unwrap();
    run.dispatched("a", "task-a").unwrap();
    run.dispatched("b", "task-b").unwrap();
    run.report(
        "a",
        Report::Completed {
            check: CheckLabel::CheckFailed,
            artifacts: vec!["patch.diff".into()],
        },
    );
    run.report("b", verified(&[]));
    assert_eq!(
        run.summary(),
        [
            (
                "a".to_owned(),
                NodeState::Failed {
                    task: "task-a".into(),
                    check_failed: true
                }
            ),
            (
                "b".to_owned(),
                NodeState::Completed {
                    task: "task-b".into(),
                    check: CheckLabel::Verified,
                    artifacts: Vec::new()
                }
            ),
            (
                "merge".to_owned(),
                NodeState::Blocked {
                    by: "a".into(),
                    cause: "its check failed".into()
                }
            ),
        ]
    );
    // An unavailable artifact blocks with its path.
    let mut run = GraphRun::new(graph()).unwrap();
    run.dispatched("a", "task-a").unwrap();
    run.dispatched("b", "task-b").unwrap();
    run.report("a", verified(&[]));
    run.report("b", verified(&[]));
    assert_eq!(
        run.states["merge"],
        NodeState::Blocked {
            by: "a".into(),
            cause: "artifact patch.diff is unavailable".into()
        }
    );
    // An unknown dispatch is reconciled, never redispatched, and blocks.
    let mut run = GraphRun::new(graph()).unwrap();
    run.dispatched("a", "task-a").unwrap();
    run.report("a", Report::Unknown);
    assert_eq!(
        run.states["a"],
        NodeState::Unknown {
            task: "task-a".into()
        }
    );
    assert!(!run.ready().contains(&"a".to_owned()));
    assert!(!run.settled());
    assert_eq!(
        run.states["merge"],
        NodeState::Blocked {
            by: "a".into(),
            cause: "its dispatch is unknown".into()
        }
    );
    // An unverified finish does not unlock a dependent.
    let mut run = GraphRun::new(graph()).unwrap();
    run.dispatched("a", "task-a").unwrap();
    run.dispatched("b", "task-b").unwrap();
    run.report(
        "a",
        Report::Completed {
            check: CheckLabel::Unchecked,
            artifacts: vec!["patch.diff".into()],
        },
    );
    run.report("b", verified(&[]));
    assert!(matches!(run.states["merge"], NodeState::Blocked { .. }));
}

#[test]
fn parent_cancellation_stops_waiting_nodes_and_returns_dispatched_tasks() {
    let mut run = GraphRun::new(graph()).unwrap();
    run.dispatched("a", "task-a").unwrap();
    let to_cancel = run.cancel();
    assert_eq!(to_cancel, ["task-a"]);
    assert_eq!(run.states["b"], NodeState::Cancelled { task: None });
    assert_eq!(run.states["merge"], NodeState::Cancelled { task: None });
    // The dispatched task stays in flight until the owner acknowledges.
    assert!(matches!(run.states["a"], NodeState::Dispatched { .. }));
    assert!(run.ready().is_empty());
    run.report("a", Report::Cancelled);
    assert_eq!(
        run.states["a"],
        NodeState::Cancelled {
            task: Some("task-a".into())
        }
    );
    assert!(run.settled());
    assert_eq!(
        run.rework("a", &Authority::Remaining { sats: 0 }),
        Err(Refused::NoAuthority)
    );
}

#[test]
fn rework_needs_remaining_authority_or_a_new_offer() {
    let mut run = GraphRun::new(graph()).unwrap();
    run.dispatched("a", "task-a").unwrap();
    run.dispatched("b", "task-b").unwrap();
    run.report("a", Report::Failed);
    run.report("b", verified(&[]));
    assert!(matches!(run.states["merge"], NodeState::Blocked { .. }));
    assert_eq!(
        run.rework("b", &Authority::Remaining { sats: 0 }),
        Err(Refused::NotFailed)
    );
    // An owned graph never spends.
    assert_eq!(
        run.rework("a", &Authority::Remaining { sats: 5 }),
        Err(Refused::NoAuthority)
    );
    run.rework("a", &Authority::Remaining { sats: 0 }).unwrap();
    assert_eq!(run.states["merge"], NodeState::Waiting);
    run.dispatched("a", "task-a2").unwrap();
    run.report("a", Report::Failed);
    // The attempt bound is spent: only a new offer reworks it.
    assert_eq!(
        run.rework("a", &Authority::Remaining { sats: 0 }),
        Err(Refused::NoAuthority)
    );
    run.rework("a", &Authority::Offer(d("offer-rework-a")))
        .unwrap();
    run.dispatched("a", "task-a3").unwrap();
    run.report("a", verified(&["patch.diff"]));
    assert_eq!(run.ready(), ["merge"]);
    assert_eq!(run.offers["a"], [d("offer-rework-a")]);
}

#[test]
fn funded_graphs_stay_within_their_reservation() {
    let mut paid = graph();
    paid.funding = Funding::Reserved {
        reservation: "res-1".into(),
        max_sats: 1_000,
    };
    for node in &mut paid.nodes {
        node.max_sats = Some(300);
    }
    let mut run = GraphRun::new(paid.clone()).unwrap();
    run.dispatched("a", "task-a").unwrap();
    run.report("a", Report::Failed);
    // 300 spent: 700 remain, so a 400-sat rework fits and an 800 does not.
    assert_eq!(
        run.rework("a", &Authority::Remaining { sats: 800 }),
        Err(Refused::NoAuthority)
    );
    run.rework("a", &Authority::Remaining { sats: 400 })
        .unwrap();
    // A paid node in an owned graph is ineligible, and nodes may not
    // exceed the reservation together.
    let mut owned = paid.clone();
    owned.funding = Funding::Owned;
    assert_eq!(
        GraphRun::new(owned).unwrap_err(),
        Invalid::Ineligible("a".into())
    );
    paid.nodes[0].max_sats = Some(600);
    assert_eq!(
        GraphRun::new(paid).unwrap_err(),
        Invalid::Ineligible("the reservation".into())
    );
}

#[test]
fn malformed_graphs_refuse() {
    let mut cycle = graph();
    cycle.nodes[0].needs = vec!["merge".into()];
    assert_eq!(cycle.validate(), Err(Invalid::Cycle));
    let mut unknown = graph();
    unknown.nodes[2].needs.push("ghost".into());
    assert!(matches!(
        unknown.validate(),
        Err(Invalid::UnknownNeed { .. })
    ));
    let mut unbound = graph();
    unbound.nodes[2].binds.push(ArtifactBinding {
        node: "zzz".into(),
        path: "x".into(),
    });
    assert!(matches!(
        unbound.validate(),
        Err(Invalid::UnboundArtifact { .. })
    ));
    let mut parallel = graph();
    parallel.max_parallel = PARALLEL_MAX + 1;
    assert_eq!(parallel.validate(), Err(Invalid::Parallel));
    let mut duplicate = graph();
    duplicate.nodes[1].id = "a".into();
    assert_eq!(
        duplicate.validate(),
        Err(Invalid::DuplicateNode("a".into()))
    );
}
