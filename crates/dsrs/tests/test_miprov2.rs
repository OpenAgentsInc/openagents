use dsrs::{Example, LmUsage, MIPROv2, Prediction, PromptCandidate, PromptingTips, Trace};
use rstest::*;

#[rstest]
fn test_trace_formatting() {
    let inputs = Example::new(
        [("question".to_string(), "What is 2+2?".into())].into(),
        vec!["question".to_string()],
        vec![],
    );

    let outputs = Prediction::new(
        [("answer".to_string(), "4".into())].into(),
        Default::default(),
    );

    let trace = Trace::new(inputs, outputs, Some(1.0));
    let formatted = trace.format_for_prompt();

    assert!(formatted.contains("question"));
    assert!(formatted.contains("What is 2+2?"));
    assert!(formatted.contains("answer"));
    assert!(formatted.contains("4"));
    assert!(formatted.contains("Score: 1.000"));
}

#[rstest]
fn test_trace_formatting_without_score() {
    let inputs = Example::new(
        [("input".to_string(), "test".into())].into(),
        vec!["input".to_string()],
        vec![],
    );

    let outputs = Prediction::new(
        [("output".to_string(), "result".into())].into(),
        LmUsage::default(),
    );

    let trace = Trace::new(inputs, outputs, None);
    let formatted = trace.format_for_prompt();

    assert!(formatted.contains("Input:"));
    assert!(formatted.contains("Output:"));
    assert!(!formatted.contains("Score:"));
}

#[rstest]
fn test_trace_with_multiple_fields() {
    let inputs = Example::new(
        [
            ("field1".to_string(), "value1".into()),
            ("field2".to_string(), "value2".into()),
            ("field3".to_string(), "value3".into()),
        ]
        .into(),
        vec![
            "field1".to_string(),
            "field2".to_string(),
            "field3".to_string(),
        ],
        vec![],
    );

    let outputs = Prediction::new(
        [
            ("out1".to_string(), "res1".into()),
            ("out2".to_string(), "res2".into()),
        ]
        .into(),
        LmUsage::default(),
    );

    let trace = Trace::new(inputs, outputs, Some(0.75));
    let formatted = trace.format_for_prompt();

    assert!(formatted.contains("field1"));
    assert!(formatted.contains("field2"));
    assert!(formatted.contains("field3"));
    assert!(formatted.contains("out1"));
    assert!(formatted.contains("out2"));
    assert!(formatted.contains("Score: 0.750"));
}

#[rstest]
fn test_prompting_tips_default() {
    let tips = PromptingTips::default_tips();

    assert!(!tips.tips.is_empty());
    assert!(tips.tips.len() >= 15, "Should have at least 15 tips");

    // Verify some expected tips are present
    let tips_text = tips.tips.join(" ");
    assert!(tips_text.contains("clear"));
    assert!(tips_text.contains("chain-of-thought") || tips_text.contains("reasoning"));
}

#[rstest]
fn test_prompting_tips_formatting() {
    let tips = PromptingTips::default_tips();
    let formatted = tips.format_for_prompt();

    assert!(!formatted.is_empty());
    assert!(formatted.contains("1."));
    assert!(formatted.contains("\n"));

    // Check that all tips are numbered
    for i in 1..=tips.tips.len() {
        assert!(formatted.contains(&format!("{}.", i)));
    }
}

#[rstest]
fn test_prompting_tips_custom() {
    let tips = PromptingTips {
        tips: vec![
            "Tip one".to_string(),
            "Tip two".to_string(),
            "Tip three".to_string(),
        ],
    };

    let formatted = tips.format_for_prompt();
    assert!(formatted.contains("1. Tip one"));
    assert!(formatted.contains("2. Tip two"));
    assert!(formatted.contains("3. Tip three"));
}

// ========================================================================
// PromptCandidate Tests
// ========================================================================

#[rstest]
fn test_prompt_candidate_creation() {
    let instruction = "Test instruction".to_string();
    let demos = vec![Example::default()];

    let candidate = PromptCandidate::new(instruction.clone(), demos.clone());

    assert_eq!(candidate.instruction, instruction);
    assert_eq!(candidate.demos.len(), 1);
    assert_eq!(candidate.score, 0.0);
}

#[rstest]
fn test_prompt_candidate_with_score() {
    let candidate = PromptCandidate::new("test".to_string(), vec![]).with_score(0.85);

    assert_eq!(candidate.score, 0.85);
    assert_eq!(candidate.instruction, "test");
}

#[rstest]
fn test_prompt_candidate_score_update() {
    let candidate = PromptCandidate::new("test".to_string(), vec![]);
    assert_eq!(candidate.score, 0.0);

    let updated = candidate.with_score(0.95);
    assert_eq!(updated.score, 0.95);
}

// ========================================================================
// MIPROv2 Configuration Tests
// ========================================================================

#[rstest]
fn test_miprov2_default_configuration() {
    let optimizer = MIPROv2::builder().build();

    assert_eq!(optimizer.num_candidates, 10);
    assert_eq!(optimizer.max_bootstrapped_demos, 3);
    assert_eq!(optimizer.max_labeled_demos, 3);
    assert_eq!(optimizer.num_trials, 20);
    assert_eq!(optimizer.minibatch_size, 25);
    assert_eq!(optimizer.temperature, 1.0);
    assert!(optimizer.track_stats);
    assert!(optimizer.prompt_model.is_none());
}

#[rstest]
fn test_miprov2_custom_configuration() {
    let optimizer = MIPROv2::builder()
        .num_candidates(5)
        .max_bootstrapped_demos(2)
        .max_labeled_demos(4)
        .num_trials(10)
        .minibatch_size(15)
        .temperature(0.7)
        .track_stats(false)
        .build();

    assert_eq!(optimizer.num_candidates, 5);
    assert_eq!(optimizer.max_bootstrapped_demos, 2);
    assert_eq!(optimizer.max_labeled_demos, 4);
    assert_eq!(optimizer.num_trials, 10);
    assert_eq!(optimizer.minibatch_size, 15);
    assert_eq!(optimizer.temperature, 0.7);
    assert!(!optimizer.track_stats);
}

#[rstest]
fn test_miprov2_minimal_configuration() {
    let optimizer = MIPROv2::builder()
        .num_candidates(1)
        .minibatch_size(1)
        .build();

    assert_eq!(optimizer.num_candidates, 1);
    assert_eq!(optimizer.minibatch_size, 1);
}

// ========================================================================
// Trace Selection Tests
// ========================================================================

#[rstest]
fn test_select_best_traces_basic() {
    let optimizer = MIPROv2::builder().build();

    let traces = vec![
        Trace::new(Example::default(), Prediction::default(), Some(0.5)),
        Trace::new(Example::default(), Prediction::default(), Some(0.9)),
        Trace::new(Example::default(), Prediction::default(), Some(0.3)),
        Trace::new(Example::default(), Prediction::default(), Some(0.7)),
    ];

    let best = optimizer.select_best_traces(&traces, 2);
    assert_eq!(best.len(), 2);
    assert_eq!(best[0].score, Some(0.9));
    assert_eq!(best[1].score, Some(0.7));
}

#[rstest]
fn test_select_best_traces_more_than_available() {
    let optimizer = MIPROv2::builder().build();

    let traces = vec![
        Trace::new(Example::default(), Prediction::default(), Some(0.8)),
        Trace::new(Example::default(), Prediction::default(), Some(0.6)),
    ];

    let best = optimizer.select_best_traces(&traces, 5);
    assert_eq!(best.len(), 2, "Should return only available traces");
}

#[rstest]
fn test_select_best_traces_with_none_scores() {
    let optimizer = MIPROv2::builder().build();

    let traces = vec![
        Trace::new(Example::default(), Prediction::default(), Some(0.5)),
        Trace::new(Example::default(), Prediction::default(), None),
        Trace::new(Example::default(), Prediction::default(), Some(0.9)),
        Trace::new(Example::default(), Prediction::default(), None),
    ];

    let best = optimizer.select_best_traces(&traces, 3);
    assert_eq!(best.len(), 2, "Should only select traces with scores");
    assert!(best.iter().all(|t| t.score.is_some()));
}

#[rstest]
fn test_select_best_traces_all_none_scores() {
    let optimizer = MIPROv2::builder().build();

    let traces = vec![
        Trace::new(Example::default(), Prediction::default(), None),
        Trace::new(Example::default(), Prediction::default(), None),
    ];

    let best = optimizer.select_best_traces(&traces, 2);
    assert_eq!(best.len(), 0, "Should return empty if no scores");
}

#[rstest]
fn test_select_best_traces_equal_scores() {
    let optimizer = MIPROv2::builder().build();

    let traces = vec![
        Trace::new(Example::default(), Prediction::default(), Some(0.5)),
        Trace::new(Example::default(), Prediction::default(), Some(0.5)),
        Trace::new(Example::default(), Prediction::default(), Some(0.5)),
    ];

    let best = optimizer.select_best_traces(&traces, 2);
    assert_eq!(best.len(), 2);
    assert_eq!(best[0].score, Some(0.5));
    assert_eq!(best[1].score, Some(0.5));
}

#[rstest]
fn test_select_best_traces_zero_selection() {
    let optimizer = MIPROv2::builder().build();

    let traces = vec![Trace::new(
        Example::default(),
        Prediction::default(),
        Some(0.8),
    )];

    let best = optimizer.select_best_traces(&traces, 0);
    assert_eq!(best.len(), 0);
}

#[rstest]
fn test_select_best_traces_single_trace() {
    let optimizer = MIPROv2::builder().build();

    let traces = vec![Trace::new(
        Example::default(),
        Prediction::default(),
        Some(0.75),
    )];

    let best = optimizer.select_best_traces(&traces, 1);
    assert_eq!(best.len(), 1);
    assert_eq!(best[0].score, Some(0.75));
}

#[rstest]
fn test_select_best_traces_descending_order() {
    let optimizer = MIPROv2::builder().build();

    let traces = vec![
        Trace::new(Example::default(), Prediction::default(), Some(0.1)),
        Trace::new(Example::default(), Prediction::default(), Some(0.2)),
        Trace::new(Example::default(), Prediction::default(), Some(0.3)),
        Trace::new(Example::default(), Prediction::default(), Some(0.4)),
        Trace::new(Example::default(), Prediction::default(), Some(0.5)),
    ];

    let best = optimizer.select_best_traces(&traces, 3);
    assert_eq!(best.len(), 3);
    assert_eq!(best[0].score, Some(0.5));
    assert_eq!(best[1].score, Some(0.4));
    assert_eq!(best[2].score, Some(0.3));
}

// ========================================================================
// Prompt Candidate Creation Tests
// ========================================================================

#[rstest]
fn test_create_prompt_candidates_basic() {
    let optimizer = MIPROv2::builder().max_labeled_demos(2).build();

    let traces = vec![
        Trace::new(
            Example::new(
                [("q".to_string(), "Q1".into())].into(),
                vec!["q".to_string()],
                vec![],
            ),
            Prediction::default(),
            Some(0.8),
        ),
        Trace::new(
            Example::new(
                [("q".to_string(), "Q2".into())].into(),
                vec!["q".to_string()],
                vec![],
            ),
            Prediction::default(),
            Some(0.9),
        ),
    ];

    let instructions = vec!["Instruction 1".to_string(), "Instruction 2".to_string()];

    let candidates = optimizer.create_prompt_candidates(instructions, &traces);

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].instruction, "Instruction 1");
    assert_eq!(candidates[1].instruction, "Instruction 2");
    // Both should have the same demos (best from traces)
    assert_eq!(candidates[0].demos.len(), 2);
    assert_eq!(candidates[1].demos.len(), 2);
}

#[rstest]
fn test_create_prompt_candidates_more_traces_than_max() {
    let optimizer = MIPROv2::builder().max_labeled_demos(2).build();

    let traces = vec![
        Trace::new(Example::default(), Prediction::default(), Some(0.5)),
        Trace::new(Example::default(), Prediction::default(), Some(0.9)),
        Trace::new(Example::default(), Prediction::default(), Some(0.3)),
        Trace::new(Example::default(), Prediction::default(), Some(0.7)),
    ];

    let instructions = vec!["Test".to_string()];
    let candidates = optimizer.create_prompt_candidates(instructions, &traces);

    assert_eq!(candidates.len(), 1);
    // Should only use max_labeled_demos (2) best traces
    assert_eq!(candidates[0].demos.len(), 2);
}

#[rstest]
fn test_create_prompt_candidates_empty_instructions() {
    let optimizer = MIPROv2::builder().build();
    let traces = vec![Trace::new(
        Example::default(),
        Prediction::default(),
        Some(0.8),
    )];

    let candidates = optimizer.create_prompt_candidates(vec![], &traces);
    assert_eq!(candidates.len(), 0);
}

#[rstest]
fn test_create_prompt_candidates_no_scored_traces() {
    let optimizer = MIPROv2::builder().build();
    let traces = vec![
        Trace::new(Example::default(), Prediction::default(), None),
        Trace::new(Example::default(), Prediction::default(), None),
    ];

    let instructions = vec!["Test".to_string()];
    let candidates = optimizer.create_prompt_candidates(instructions, &traces);

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].demos.len(), 0);
}

// ========================================================================
// Edge Case Tests
// ========================================================================

#[rstest]
fn test_trace_clone() {
    let trace = Trace::new(Example::default(), Prediction::default(), Some(0.85));

    let cloned = trace.clone();
    assert_eq!(cloned.score, Some(0.85));
}

#[rstest]
fn test_prompt_candidate_clone() {
    let candidate = PromptCandidate::new("test instruction".to_string(), vec![Example::default()]);

    let cloned = candidate.clone();
    assert_eq!(cloned.instruction, "test instruction");
    assert_eq!(cloned.demos.len(), 1);
}

#[rstest]
fn test_format_signature_fields_with_descriptions() {
    let optimizer = MIPROv2::builder().build();

    // This is a basic structural test - in real usage, this would be tested
    // with actual signature implementations
    // Here we're just verifying the method exists and returns a string
    use dsrs::core::MetaSignature;
    use serde_json::Value;

    struct TestSignature;
    impl MetaSignature for TestSignature {
        fn input_fields(&self) -> Value {
            serde_json::json!({
                "question": {
                    "type": "String",
                    "desc": "The question to answer"
                }
            })
        }

        fn output_fields(&self) -> Value {
            serde_json::json!({
                "answer": {
                    "type": "String",
                    "desc": "The answer to the question"
                }
            })
        }

        fn instruction(&self) -> String {
            "Test instruction".to_string()
        }

        fn update_instruction(&mut self, _instruction: String) -> anyhow::Result<()> {
            Ok(())
        }

        fn set_demos(&mut self, _demos: Vec<Example>) -> anyhow::Result<()> {
            Ok(())
        }

        fn demos(&self) -> Vec<Example> {
            vec![]
        }

        fn append(&mut self, _name: &str, _value: Value) -> anyhow::Result<()> {
            Ok(())
        }
    }

    let sig = TestSignature;
    let formatted = optimizer.format_signature_fields(&sig);

    assert!(formatted.contains("Input Fields:"));
    assert!(formatted.contains("Output Fields:"));
    assert!(formatted.contains("question"));
    assert!(formatted.contains("answer"));
}

// ========================================================================
// Property-based Tests
// ========================================================================

#[rstest]
fn test_select_best_traces_always_returns_requested_or_less() {
    let optimizer = MIPROv2::builder().build();

    for num_traces in 1..=10 {
        for num_select in 0..=15 {
            let traces: Vec<Trace> = (0..num_traces)
                .map(|i| {
                    Trace::new(
                        Example::default(),
                        Prediction::default(),
                        Some(i as f32 / 10.0),
                    )
                })
                .collect();

            let selected = optimizer.select_best_traces(&traces, num_select);
            assert!(selected.len() <= num_select);
            assert!(selected.len() <= num_traces);
        }
    }
}

#[rstest]
fn test_prompt_candidates_count_matches_instructions() {
    let optimizer = MIPROv2::builder().build();
    let traces = vec![Trace::new(
        Example::default(),
        Prediction::default(),
        Some(0.8),
    )];

    for num_instructions in 0..=10 {
        let instructions: Vec<String> = (0..num_instructions)
            .map(|i| format!("Instruction {}", i))
            .collect();

        let candidates = optimizer.create_prompt_candidates(instructions, &traces);
        assert_eq!(candidates.len(), num_instructions);
    }
}
