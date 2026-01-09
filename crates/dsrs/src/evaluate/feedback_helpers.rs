/// Helper functions for creating rich feedback metrics
///
/// This module provides utilities for common feedback patterns in different domains:
/// - Document retrieval (precision, recall, F1)
/// - Code generation (compilation, execution, testing)
/// - Multi-objective evaluation
/// - Structured error reporting
use super::FeedbackMetric;
use serde_json::json;
use std::collections::{HashMap, HashSet};

// ============================================================================
// Retrieval Feedback Helpers
// ============================================================================

/// Create feedback for document retrieval tasks
///
/// # Arguments
/// * `retrieved` - Documents retrieved by the system
/// * `expected` - Expected/gold documents
/// * `context_docs` - Optional list of all available documents for context
///
/// # Example Feedback
/// ```text
/// Retrieved 3/5 correct documents (Precision: 0.6, Recall: 0.6, F1: 0.6)
/// ✓ Correctly retrieved: doc1, doc2, doc3
/// ✗ Missed: doc4, doc5
/// ✗ Incorrectly retrieved: doc6, doc7
/// ```
pub fn retrieval_feedback(
    retrieved: &[impl AsRef<str>],
    expected: &[impl AsRef<str>],
    context_docs: Option<&[impl AsRef<str>]>,
) -> FeedbackMetric {
    let retrieved_set: HashSet<String> = retrieved.iter().map(|s| s.as_ref().to_string()).collect();

    let expected_set: HashSet<String> = expected.iter().map(|s| s.as_ref().to_string()).collect();

    let correct: Vec<String> = retrieved_set.intersection(&expected_set).cloned().collect();

    let missed: Vec<String> = expected_set.difference(&retrieved_set).cloned().collect();

    let incorrect: Vec<String> = retrieved_set.difference(&expected_set).cloned().collect();

    let precision = if retrieved.is_empty() {
        0.0
    } else {
        correct.len() as f32 / retrieved.len() as f32
    };

    let recall = if expected.is_empty() {
        1.0
    } else {
        correct.len() as f32 / expected.len() as f32
    };

    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    let mut feedback = format!(
        "Retrieved {}/{} correct documents (Precision: {:.3}, Recall: {:.3}, F1: {:.3})\n",
        correct.len(),
        expected.len(),
        precision,
        recall,
        f1
    );

    if !correct.is_empty() {
        feedback.push_str(&format!("Correctly retrieved: {}\n", correct.join(", ")));
    }

    if !missed.is_empty() {
        feedback.push_str(&format!("Missed: {}\n", missed.join(", ")));
    }

    if !incorrect.is_empty() {
        feedback.push_str(&format!(
            "Incorrectly retrieved: {}\n",
            incorrect.join(", ")
        ));
    }

    let mut metadata = HashMap::new();
    metadata.insert("precision".to_string(), json!(precision));
    metadata.insert("recall".to_string(), json!(recall));
    metadata.insert("f1".to_string(), json!(f1));
    metadata.insert("correct_count".to_string(), json!(correct.len()));
    metadata.insert("missed_count".to_string(), json!(missed.len()));
    metadata.insert("incorrect_count".to_string(), json!(incorrect.len()));

    if let Some(docs) = context_docs {
        metadata.insert("total_available".to_string(), json!(docs.len()));
    }

    FeedbackMetric {
        score: f1,
        feedback,
        metadata,
    }
}

// ============================================================================
// Code Generation Feedback Helpers
// ============================================================================

/// Stage in code execution pipeline
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeStage {
    Parse,
    Compile,
    Execute,
    Test,
}

impl std::fmt::Display for CodeStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodeStage::Parse => write!(f, "Parse"),
            CodeStage::Compile => write!(f, "Compile"),
            CodeStage::Execute => write!(f, "Execute"),
            CodeStage::Test => write!(f, "Test"),
        }
    }
}

/// Result of a code stage
#[derive(Debug, Clone)]
pub enum StageResult {
    Success,
    Failure { error: String },
}

/// Create feedback for code generation pipelines
///
/// # Arguments
/// * `stages` - List of (stage, result) tuples showing pipeline progression
/// * `final_score` - Overall score (0.0 to 1.0)
///
/// # Example Feedback
/// ```text
/// ✓ Parse: Success
/// ✓ Compile: Success
/// ✗ Execute: RuntimeError: division by zero on line 10
/// ```
pub fn code_pipeline_feedback(
    stages: &[(CodeStage, StageResult)],
    final_score: f32,
) -> FeedbackMetric {
    let mut feedback = String::new();
    let mut metadata = HashMap::new();

    let mut last_successful_stage = None;
    let mut failure_stage = None;

    for (i, (stage, result)) in stages.iter().enumerate() {
        let stage_name = stage.to_string();
        metadata.insert(format!("stage_{}_name", i), json!(stage_name));

        match result {
            StageResult::Success => {
                feedback.push_str(&format!("{}: Success\n", stage));
                metadata.insert(format!("stage_{}_result", i), json!("success"));
                last_successful_stage = Some(stage);
            }
            StageResult::Failure { error } => {
                feedback.push_str(&format!("{}: {}\n", stage, error));
                metadata.insert(format!("stage_{}_result", i), json!("failure"));
                metadata.insert(format!("stage_{}_error", i), json!(error));
                failure_stage = Some((stage, error));
                break; // Stop at first failure
            }
        }
    }

    if let Some((stage, error)) = failure_stage {
        metadata.insert("failed_at_stage".to_string(), json!(stage.to_string()));
        metadata.insert("failure_error".to_string(), json!(error));
    }

    if let Some(stage) = last_successful_stage {
        metadata.insert(
            "last_successful_stage".to_string(),
            json!(stage.to_string()),
        );
    }

    FeedbackMetric {
        score: final_score,
        feedback,
        metadata,
    }
}

// ============================================================================
// Multi-Objective Feedback Helpers
// ============================================================================

/// Create feedback for multi-objective optimization
///
/// # Arguments
/// * `objectives` - Map of objective name to (score, feedback) pairs
/// * `weights` - Optional weights for aggregating objectives
///
/// # Example Feedback
/// ```text
/// [Correctness] Score: 0.9 - Output matches expected format
/// [Latency] Score: 0.7 - Response took 450ms (target: <300ms)
/// [Privacy] Score: 1.0 - No PII detected in output
/// Overall: 0.87 (weighted average)
/// ```
pub fn multi_objective_feedback(
    objectives: &HashMap<String, (f32, String)>,
    weights: Option<&HashMap<String, f32>>,
) -> FeedbackMetric {
    let mut feedback = String::new();
    let mut metadata = HashMap::new();

    let mut total_score = 0.0;
    let mut total_weight = 0.0;

    let mut objective_names: Vec<_> = objectives.keys().collect();
    objective_names.sort();

    for name in objective_names {
        if let Some((score, obj_feedback)) = objectives.get(name.as_str()) {
            let weight = weights
                .and_then(|w| w.get(name.as_str()))
                .copied()
                .unwrap_or(1.0);

            feedback.push_str(&format!(
                "[{}] Score: {:.3} - {}\n",
                name, score, obj_feedback
            ));

            metadata.insert(format!("objective_{}_score", name), json!(score));
            metadata.insert(format!("objective_{}_weight", name), json!(weight));
            metadata.insert(format!("objective_{}_feedback", name), json!(obj_feedback));

            total_score += score * weight;
            total_weight += weight;
        }
    }

    let aggregate_score = if total_weight > 0.0 {
        total_score / total_weight
    } else {
        0.0
    };

    feedback.push_str(&format!(
        "\nOverall: {:.3} (weighted average)",
        aggregate_score
    ));
    metadata.insert("aggregate_score".to_string(), json!(aggregate_score));
    metadata.insert("num_objectives".to_string(), json!(objectives.len()));

    FeedbackMetric {
        score: aggregate_score,
        feedback,
        metadata,
    }
}

// ============================================================================
// String Similarity Feedback
// ============================================================================

/// Create feedback for string similarity tasks
///
/// Uses simple word-level comparison to provide actionable feedback
pub fn string_similarity_feedback(predicted: &str, expected: &str) -> FeedbackMetric {
    let exact_match = predicted.trim() == expected.trim();

    if exact_match {
        return FeedbackMetric::new(1.0, "Exact match");
    }

    let pred_lower = predicted.to_lowercase();
    let exp_lower = expected.to_lowercase();

    if pred_lower == exp_lower {
        return FeedbackMetric::new(0.95, "Match ignoring case (minor formatting difference)");
    }

    // Word-level comparison
    let pred_words: HashSet<&str> = pred_lower.split_whitespace().collect();
    let exp_words: HashSet<&str> = exp_lower.split_whitespace().collect();

    let common_words: HashSet<_> = pred_words.intersection(&exp_words).collect();
    let missing_words: Vec<_> = exp_words.difference(&pred_words).collect();
    let extra_words: Vec<_> = pred_words.difference(&exp_words).collect();

    let recall = if !exp_words.is_empty() {
        common_words.len() as f32 / exp_words.len() as f32
    } else {
        1.0
    };

    let precision = if !pred_words.is_empty() {
        common_words.len() as f32 / pred_words.len() as f32
    } else {
        0.0
    };

    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    let mut feedback = format!("Partial match (F1: {:.3})\n", f1);
    feedback.push_str(&format!("Expected: \"{}\"\n", expected));
    feedback.push_str(&format!("Predicted: \"{}\"\n", predicted));

    if !missing_words.is_empty() {
        feedback.push_str(&format!(
            "Missing words: {}\n",
            missing_words
                .iter()
                .map(|w| format!("\"{}\"", w))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if !extra_words.is_empty() {
        feedback.push_str(&format!(
            "Extra words: {}\n",
            extra_words
                .iter()
                .map(|w| format!("\"{}\"", w))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    FeedbackMetric::new(f1, feedback)
}

// ============================================================================
// Classification Feedback
// ============================================================================

/// Create feedback for classification tasks
pub fn classification_feedback(
    predicted_class: &str,
    expected_class: &str,
    confidence: Option<f32>,
) -> FeedbackMetric {
    let correct = predicted_class == expected_class;
    let score = if correct { 1.0 } else { 0.0 };

    let mut feedback = if correct {
        format!("Correct classification: \"{}\"", predicted_class)
    } else {
        format!(
            "Incorrect classification\n  Expected: \"{}\"\n  Predicted: \"{}\"",
            expected_class, predicted_class
        )
    };

    if let Some(conf) = confidence {
        feedback.push_str(&format!("\n  Confidence: {:.3}", conf));
    }

    let mut metadata = HashMap::new();
    metadata.insert("predicted_class".to_string(), json!(predicted_class));
    metadata.insert("expected_class".to_string(), json!(expected_class));
    metadata.insert("correct".to_string(), json!(correct));

    if let Some(conf) = confidence {
        metadata.insert("confidence".to_string(), json!(conf));
    }

    FeedbackMetric::with_metadata(score, feedback, metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrieval_feedback_perfect() {
        let retrieved = vec!["doc1", "doc2", "doc3"];
        let expected = vec!["doc1", "doc2", "doc3"];

        let feedback = retrieval_feedback(&retrieved, &expected, None::<&[&str]>);
        assert_eq!(feedback.score, 1.0);
        assert!(feedback.feedback.contains("3/3"));
    }

    #[test]
    fn test_retrieval_feedback_partial() {
        let retrieved = vec!["doc1", "doc2", "doc4"];
        let expected = vec!["doc1", "doc2", "doc3"];

        let feedback = retrieval_feedback(&retrieved, &expected, None::<&[&str]>);
        assert!(feedback.score < 1.0 && feedback.score > 0.0);
        assert!(feedback.feedback.contains("Missed: doc3"));
        assert!(feedback.feedback.contains("Incorrectly retrieved: doc4"));
    }

    #[test]
    fn test_code_pipeline_feedback() {
        let stages = vec![
            (CodeStage::Parse, StageResult::Success),
            (CodeStage::Compile, StageResult::Success),
            (
                CodeStage::Execute,
                StageResult::Failure {
                    error: "Division by zero".to_string(),
                },
            ),
        ];

        let feedback = code_pipeline_feedback(&stages, 0.6);
        assert!(feedback.feedback.contains("Parse"));
        assert!(feedback.feedback.contains("Compile"));
        assert!(feedback.feedback.contains("Execute"));
        assert_eq!(feedback.score, 0.6);
    }

    #[test]
    fn test_multi_objective_feedback() {
        let mut objectives = HashMap::new();
        objectives.insert("accuracy".to_string(), (0.9, "Good accuracy".to_string()));
        objectives.insert("latency".to_string(), (0.7, "Slow response".to_string()));

        let feedback = multi_objective_feedback(&objectives, None);
        assert!(feedback.feedback.contains("[accuracy]"));
        assert!(feedback.feedback.contains("[latency]"));
        assert!((feedback.score - 0.8).abs() < 0.01); // Average of 0.9 and 0.7
    }

    #[test]
    fn test_string_similarity_exact() {
        let feedback = string_similarity_feedback("hello world", "hello world");
        assert_eq!(feedback.score, 1.0);
    }

    #[test]
    fn test_string_similarity_case() {
        let feedback = string_similarity_feedback("Hello World", "hello world");
        assert_eq!(feedback.score, 0.95);
    }

    #[test]
    fn test_classification_feedback() {
        let feedback = classification_feedback("positive", "positive", Some(0.95));
        assert_eq!(feedback.score, 1.0);
        assert!(feedback.feedback.contains("Correct"));

        let feedback = classification_feedback("negative", "positive", Some(0.85));
        assert_eq!(feedback.score, 0.0);
        assert!(feedback.feedback.contains("Incorrect"));
    }
}
