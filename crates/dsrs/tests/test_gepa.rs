use dsrs::optimizer::gepa::GEPACandidate;

#[test]
fn test_candidate_creation() {
    let candidate = GEPACandidate {
        id: 1,
        instruction: "Test instruction".to_string(),
        module_name: "test".to_string(),
        example_scores: vec![0.8, 0.9, 0.7],
        parent_id: None,
        generation: 0,
    };

    assert_eq!(candidate.average_score(), 0.8);
}

#[test]
fn test_candidate_mutation() {
    let parent = GEPACandidate {
        id: 1,
        instruction: "Original".to_string(),
        module_name: "test".to_string(),
        example_scores: vec![0.8],
        parent_id: None,
        generation: 0,
    };

    let child = parent.mutate("Improved".to_string(), 1);
    assert_eq!(child.instruction, "Improved");
    assert_eq!(child.parent_id, Some(1));
    assert_eq!(child.generation, 1);
}
