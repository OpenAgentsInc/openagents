use crate::{Example, Prediction};
use serde::{Deserialize, Serialize};
/// Feedback-based evaluation for GEPA optimizer
///
/// This module provides structures and traits for rich, textual feedback
/// that guides the GEPA optimization process.
use std::collections::HashMap;

/// Rich evaluation metric with both score and textual feedback
///
/// GEPA uses this to understand *why* a score was assigned, enabling
/// more targeted prompt improvements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackMetric {
    /// Numerical score (typically 0.0 to 1.0, but can be any range)
    pub score: f32,

    /// Rich textual feedback explaining the score
    ///
    /// Examples:
    /// - "✓ Retrieved 3/3 correct documents"
    /// - "✗ Code failed to compile: missing semicolon on line 5"
    /// - "Partially correct: got answer '42' but expected '42.0'"
    pub feedback: String,

    /// Optional structured metadata for additional context
    ///
    /// Can include:
    /// - Intermediate outputs from pipeline stages
    /// - Error messages and stack traces
    /// - Performance metrics (latency, tokens, cost)
    /// - Domain-specific diagnostics
    pub metadata: HashMap<String, serde_json::Value>,
}

impl FeedbackMetric {
    /// Create a new feedback metric
    pub fn new(score: f32, feedback: impl Into<String>) -> Self {
        Self {
            score,
            feedback: feedback.into(),
            metadata: HashMap::new(),
        }
    }

    /// Create a feedback metric with metadata
    pub fn with_metadata(
        score: f32,
        feedback: impl Into<String>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            score,
            feedback: feedback.into(),
            metadata,
        }
    }

    /// Add metadata to an existing feedback metric
    pub fn add_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

impl Default for FeedbackMetric {
    fn default() -> Self {
        Self {
            score: 0.0,
            feedback: String::new(),
            metadata: HashMap::new(),
        }
    }
}

/// Trait for evaluators that provide rich feedback
///
/// This extends the basic Evaluator trait to return feedback alongside scores.
#[allow(async_fn_in_trait)]
pub trait FeedbackEvaluator {
    /// Evaluate an example and return both score and feedback
    async fn feedback_metric(&self, example: &Example, prediction: &Prediction) -> FeedbackMetric;

    /// Evaluate with multiple objectives (for multi-objective optimization)
    async fn multi_objective_metric(
        &self,
        example: &Example,
        prediction: &Prediction,
    ) -> Vec<FeedbackMetric> {
        // Default: single objective
        vec![self.feedback_metric(example, prediction).await]
    }
}

/// Execution trace capturing program behavior
///
/// Captures the full execution path of a module, including intermediate
/// steps, errors, and environmental feedback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Input example
    pub inputs: Example,

    /// Final prediction (if successful)
    pub outputs: Option<Prediction>,

    /// Evaluation feedback
    pub feedback: Option<FeedbackMetric>,

    /// Intermediate steps in the execution
    ///
    /// Each entry is (step_name, step_output)
    pub intermediate_steps: Vec<(String, serde_json::Value)>,

    /// Errors encountered during execution
    pub errors: Vec<String>,

    /// Execution metadata (timing, cost, etc.)
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ExecutionTrace {
    /// Create a simple trace with just inputs and outputs
    pub fn simple(inputs: Example, outputs: Prediction) -> Self {
        Self {
            inputs,
            outputs: Some(outputs),
            feedback: None,
            intermediate_steps: Vec::new(),
            errors: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new trace builder
    pub fn builder(inputs: Example) -> ExecutionTraceBuilder {
        ExecutionTraceBuilder::new(inputs)
    }

    /// Add feedback to the trace
    pub fn with_feedback(mut self, feedback: FeedbackMetric) -> Self {
        self.feedback = Some(feedback);
        self
    }

    /// Check if execution was successful
    pub fn is_successful(&self) -> bool {
        self.outputs.is_some() && self.errors.is_empty()
    }

    /// Get score if available
    pub fn score(&self) -> Option<f32> {
        self.feedback.as_ref().map(|f| f.score)
    }

    /// Format trace for LLM reflection
    pub fn format_for_reflection(&self) -> String {
        let mut result = String::new();

        // Input
        result.push_str("Input:\n");
        result.push_str(&format!("{:?}\n\n", self.inputs));

        // Intermediate steps
        if !self.intermediate_steps.is_empty() {
            result.push_str("Execution Steps:\n");
            for (i, (step_name, output)) in self.intermediate_steps.iter().enumerate() {
                result.push_str(&format!("{}. {}: {:?}\n", i + 1, step_name, output));
            }
            result.push('\n');
        }

        // Output
        if let Some(ref outputs) = self.outputs {
            result.push_str("Output:\n");
            result.push_str(&format!("{:?}\n\n", outputs));
        }

        // Errors
        if !self.errors.is_empty() {
            result.push_str("Errors:\n");
            for error in &self.errors {
                result.push_str(&format!("- {}\n", error));
            }
            result.push('\n');
        }

        // Feedback
        if let Some(ref feedback) = self.feedback {
            result.push_str("Evaluation:\n");
            result.push_str(&format!("Score: {:.3}\n", feedback.score));
            result.push_str(&format!("Feedback: {}\n", feedback.feedback));
        }

        result
    }
}

/// Builder for ExecutionTrace
pub struct ExecutionTraceBuilder {
    trace: ExecutionTrace,
}

impl ExecutionTraceBuilder {
    pub fn new(inputs: Example) -> Self {
        Self {
            trace: ExecutionTrace {
                inputs,
                outputs: None,
                feedback: None,
                intermediate_steps: Vec::new(),
                errors: Vec::new(),
                metadata: HashMap::new(),
            },
        }
    }

    pub fn outputs(mut self, outputs: Prediction) -> Self {
        self.trace.outputs = Some(outputs);
        self
    }

    pub fn feedback(mut self, feedback: FeedbackMetric) -> Self {
        self.trace.feedback = Some(feedback);
        self
    }

    pub fn add_step(mut self, name: impl Into<String>, output: serde_json::Value) -> Self {
        self.trace.intermediate_steps.push((name.into(), output));
        self
    }

    pub fn add_error(mut self, error: impl Into<String>) -> Self {
        self.trace.errors.push(error.into());
        self
    }

    pub fn add_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.trace.metadata.insert(key.into(), value);
        self
    }

    pub fn build(self) -> ExecutionTrace {
        self.trace
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_feedback_metric_creation() {
        let feedback = FeedbackMetric::new(0.8, "Good result");
        assert_eq!(feedback.score, 0.8);
        assert_eq!(feedback.feedback, "Good result");
        assert!(feedback.metadata.is_empty());
    }

    #[test]
    fn test_feedback_metric_with_metadata() {
        let mut meta = HashMap::new();
        meta.insert("latency_ms".to_string(), json!(150));

        let feedback = FeedbackMetric::with_metadata(0.9, "Excellent", meta);
        assert_eq!(feedback.score, 0.9);
        assert_eq!(feedback.metadata.get("latency_ms").unwrap(), &json!(150));
    }

    #[test]
    fn test_execution_trace_builder() {
        use std::collections::HashMap;
        let mut input_data = HashMap::new();
        input_data.insert("question".to_string(), json!("What is 2+2?"));
        let inputs = crate::Example::new(input_data, vec!["question".to_string()], vec![]);

        let mut pred_data = HashMap::new();
        pred_data.insert("answer".to_string(), json!("4"));
        let prediction = crate::Prediction::new(pred_data, crate::LmUsage::default());

        let trace = ExecutionTrace::builder(inputs)
            .add_step("parse", json!("2+2"))
            .add_step("compute", json!(4))
            .outputs(prediction)
            .feedback(FeedbackMetric::new(1.0, "Correct"))
            .build();

        assert!(trace.is_successful());
        assert_eq!(trace.score(), Some(1.0));
        assert_eq!(trace.intermediate_steps.len(), 2);
    }

    #[test]
    fn test_trace_with_errors() {
        use std::collections::HashMap;
        let mut input_data = HashMap::new();
        input_data.insert("question".to_string(), json!("Invalid"));
        let inputs = crate::Example::new(input_data, vec!["question".to_string()], vec![]);

        let trace = ExecutionTrace::builder(inputs)
            .add_error("Parse failed")
            .build();

        assert!(!trace.is_successful());
        assert_eq!(trace.errors.len(), 1);
    }
}
