use dsrs::optimizer::gepa::GEPACandidate;
use dsrs::optimizer::pareto::ParetoFrontier;

fn make_test_candidate(instruction: &str) -> GEPACandidate {
    GEPACandidate {
        id: 0,
        instruction: instruction.to_string(),
        module_name: "test_module".to_string(),
        example_scores: Vec::new(),
        parent_id: None,
        generation: 0,
    }
}

#[test]
fn test_frontier_empty() {
    let frontier = ParetoFrontier::new();
    assert!(frontier.is_empty());
    assert_eq!(frontier.len(), 0);
    assert!(frontier.sample_proportional_to_coverage().is_none());
}

#[test]
fn test_add_first_candidate() {
    let mut frontier = ParetoFrontier::new();
    let candidate = make_test_candidate("instruction 1");
    let scores = vec![0.8, 0.7, 0.9];

    let added = frontier.add_candidate(candidate, &scores);
    assert!(added);
    assert_eq!(frontier.len(), 1);
}

#[test]
fn test_pareto_dominance() {
    let mut frontier = ParetoFrontier::new();

    // Add first candidate - wins on example 0
    let candidate1 = make_test_candidate("instruction 1");
    frontier.add_candidate(candidate1, &[0.9, 0.5, 0.5]);

    // Add second candidate - wins on examples 1 and 2
    let candidate2 = make_test_candidate("instruction 2");
    frontier.add_candidate(candidate2, &[0.5, 0.9, 0.9]);

    // Both should be on frontier (complementary strengths)
    assert_eq!(frontier.len(), 2);

    // Add dominated candidate - loses on all examples
    let candidate3 = make_test_candidate("instruction 3");
    let added = frontier.add_candidate(candidate3, &[0.3, 0.3, 0.3]);

    // Should not be added
    assert!(!added);
    assert_eq!(frontier.len(), 2);
}

#[test]
fn test_coverage_weighted_sampling() {
    let mut frontier = ParetoFrontier::new();

    // Add candidates with different coverage
    frontier.add_candidate(make_test_candidate("wins on 1"), &[0.9, 0.3, 0.3, 0.3]);
    frontier.add_candidate(make_test_candidate("wins on 3"), &[0.3, 0.9, 0.9, 0.9]);

    assert_eq!(frontier.len(), 2);

    // Sample should return one of the candidates
    let sampled = frontier.sample_proportional_to_coverage();
    assert!(sampled.is_some());
}

#[test]
fn test_statistics() {
    let mut frontier = ParetoFrontier::new();

    frontier.add_candidate(make_test_candidate("c1"), &[0.9, 0.5, 0.5]);
    frontier.add_candidate(make_test_candidate("c2"), &[0.5, 0.9, 0.9]);

    let stats = frontier.statistics();
    assert_eq!(stats.num_candidates, 2);
    assert_eq!(stats.num_examples_covered, 3);
}
