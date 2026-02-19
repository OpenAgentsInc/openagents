//! DSPy-powered planning pipeline for autopilot.
//!
//! Uses typed signatures to create structured implementation plans:
//!
//! 1. **PlanningSignature** - Analyze repository and issue to create plan
//! 2. **DeepPlanningSignature** - Chain-of-thought for complex tasks
//!
//! # Example
//!
//! ```rust,ignore
//! use autopilot_core::dspy_planning::{PlanningPipeline, PlanningInput};
//!
//! let pipeline = PlanningPipeline::new();
//! let input = PlanningInput {
//!     repository_summary: "Rust CLI application".to_string(),
//!     issue_description: "Add logout button".to_string(),
//!     relevant_files: "src/ui.rs\nsrc/auth.rs".to_string(),
//! };
//!
//! let result = pipeline.plan(&input).await?;
//! println!("Files to modify: {:?}", result.files_to_modify);
//! ```

use dsrs::callbacks::DspyCallback;
use dsrs::{LM, Predict, Predictor, Signature, example};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Signature Definitions
// ============================================================================

/// Planning signature - creates implementation plan from repository and issue.
#[Signature]
struct PlanningSignature {
    /// Software Planning: Analyze repository structure and issue requirements
    /// to create a detailed implementation plan.

    /// High-level overview of the codebase structure and purpose
    #[input]
    repository_summary: String,

    /// Description of what needs to be implemented or fixed
    #[input]
    issue_description: String,

    /// Files discovered during preflight that may be relevant
    #[input]
    relevant_files: String,

    /// Deep understanding of the problem and approach
    #[output]
    analysis: String,

    /// JSON array of file paths that need modification
    #[output]
    files_to_modify: String,

    /// JSON array of implementation steps in order
    #[output]
    implementation_steps: String,

    /// Strategy for verifying the solution works
    #[output]
    test_strategy: String,

    /// Potential issues or challenges to watch for
    #[output]
    risk_factors: String,

    /// Overall complexity assessment: LOW, MEDIUM, or HIGH
    #[output]
    estimated_complexity: String,

    /// Confidence in the plan (0.0-1.0)
    #[output]
    confidence: f32,
}

/// Deep planning signature with chain-of-thought for complex tasks.
#[Signature(cot)]
struct DeepPlanningSignature {
    /// Deep Planning with Chain-of-Thought: For complex multi-file changes,
    /// architectural decisions, or tasks requiring careful reasoning.
    /// Think step-by-step about the problem before proposing a solution.

    /// High-level overview of the codebase structure and purpose
    #[input]
    repository_summary: String,

    /// Description of what needs to be implemented or fixed
    #[input]
    issue_description: String,

    /// Files discovered during preflight that may be relevant
    #[input]
    relevant_files: String,

    /// Existing code patterns and conventions to follow
    #[input]
    code_patterns: String,

    /// Deep understanding of the problem and approach
    #[output]
    analysis: String,

    /// JSON array of file paths that need modification
    #[output]
    files_to_modify: String,

    /// JSON array of implementation steps in order
    #[output]
    implementation_steps: String,

    /// Strategy for verifying the solution works
    #[output]
    test_strategy: String,

    /// Potential issues or challenges to watch for
    #[output]
    risk_factors: String,

    /// Overall complexity assessment: LOW, MEDIUM, or HIGH
    #[output]
    estimated_complexity: String,

    /// Confidence in the plan (0.0-1.0)
    #[output]
    confidence: f32,
}

/// Task complexity classifier - decides planning depth.
#[Signature]
struct TaskComplexityClassifier {
    /// Classify task complexity for planning depth.
    /// Replaces keyword-based heuristics with learned classification.

    /// Description of the task to classify
    #[input]
    task_description: String,

    /// Count of relevant files identified
    #[input]
    file_count: u32,

    /// Repository context and patterns
    #[input]
    codebase_context: String,

    /// Complexity label: Simple/Moderate/Complex/VeryComplex
    #[output]
    complexity: String,

    /// Rationale for the classification
    #[output]
    reasoning: String,

    /// Confidence in the classification (0.0-1.0)
    #[output]
    confidence: f32,
}

// ============================================================================
// Pipeline Types
// ============================================================================

/// Complexity level for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Complexity {
    Low,
    Medium,
    High,
}

impl From<&str> for Complexity {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "LOW" | "SIMPLE" | "EASY" => Self::Low,
            "HIGH" | "COMPLEX" | "HARD" => Self::High,
            _ => Self::Medium,
        }
    }
}

/// Input to the planning pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningInput {
    /// High-level overview of the codebase
    pub repository_summary: String,
    /// Description of what needs to be done
    pub issue_description: String,
    /// Files discovered during preflight
    pub relevant_files: String,
    /// Optional: existing code patterns to follow
    pub code_patterns: Option<String>,
}

/// Result from the planning pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningResult {
    /// Deep understanding of the problem
    pub analysis: String,
    /// Files that need to be modified
    pub files_to_modify: Vec<String>,
    /// Ordered implementation steps
    pub implementation_steps: Vec<String>,
    /// How to verify the solution
    pub test_strategy: String,
    /// Potential issues to watch for
    pub risk_factors: Vec<String>,
    /// Overall complexity
    pub complexity: Complexity,
    /// Confidence in the plan (0.0-1.0)
    pub confidence: f32,
}

// ============================================================================
// Planning Pipeline
// ============================================================================

/// DSPy-powered planning pipeline.
#[derive(Clone)]
pub struct PlanningPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for PlanningPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanningPipeline {
    /// Create a new planning pipeline using the global LM.
    pub fn new() -> Self {
        Self { lm: None }
    }

    /// Create a pipeline with a specific LM.
    pub fn with_lm(lm: Arc<LM>) -> Self {
        Self { lm: Some(lm) }
    }

    /// Helper to get string from prediction value.
    fn get_string(prediction: &dsrs::Prediction, key: &str) -> String {
        let val = prediction.get(key, None);
        if let Some(s) = val.as_str() {
            s.to_string()
        } else {
            val.to_string().trim_matches('"').to_string()
        }
    }

    /// Helper to get f32 from prediction value.
    fn get_f32(prediction: &dsrs::Prediction, key: &str) -> f32 {
        let val = prediction.get(key, None);
        if let Some(n) = val.as_f64() {
            n as f32
        } else if let Some(s) = val.as_str() {
            s.parse().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Parse JSON array string into Vec<String>.
    fn parse_json_array(s: &str) -> Vec<String> {
        serde_json::from_str::<Vec<String>>(s).unwrap_or_else(|_| {
            // Fallback: split by newlines if not valid JSON
            s.lines()
                .map(|l| l.trim().trim_start_matches('-').trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
    }

    /// Build codebase context for complexity classification.
    fn build_codebase_context(input: &PlanningInput) -> String {
        let mut context = String::new();
        context.push_str("Repository summary:\n");
        context.push_str(&input.repository_summary);
        context.push_str("\nRelevant files:\n");
        context.push_str(&input.relevant_files);
        context.push_str("\nCode patterns:\n");
        if let Some(patterns) = &input.code_patterns {
            if patterns.trim().is_empty() {
                context.push_str("None");
            } else {
                context.push_str(patterns);
            }
        } else {
            context.push_str("None");
        }
        context
    }

    /// Determine if deep planning should be used based on classifier output.
    fn should_use_deep_planning(complexity: &str, confidence: f32) -> Option<bool> {
        if complexity.trim().is_empty() {
            return None;
        }

        if confidence < 0.4 {
            return None;
        }

        let normalized = complexity
            .to_lowercase()
            .replace([' ', '-', '_'], "")
            .trim()
            .to_string();

        match normalized.as_str() {
            "complex" | "verycomplex" | "veryhigh" | "high" => Some(true),
            "simple" | "moderate" | "low" | "medium" => Some(false),
            _ => None,
        }
    }

    /// Heuristic fallback for complexity when classifier is unavailable.
    fn heuristic_complexity(input: &PlanningInput) -> bool {
        let issue_len = input.issue_description.len();
        let file_count = input.relevant_files.lines().count();

        issue_len > 500
            || file_count > 10
            || input.issue_description.to_lowercase().contains("refactor")
            || input
                .issue_description
                .to_lowercase()
                .contains("architecture")
            || input.issue_description.to_lowercase().contains("redesign")
    }

    /// Classify task complexity using DSPy.
    #[expect(dead_code)]
    async fn classify_complexity(&self, input: &PlanningInput) -> anyhow::Result<(String, f32)> {
        self.classify_complexity_with_callback(input, None).await
    }

    /// Classify task complexity using DSPy with streaming callback.
    async fn classify_complexity_with_callback(
        &self,
        input: &PlanningInput,
        callback: Option<&dyn DspyCallback>,
    ) -> anyhow::Result<(String, f32)> {
        let classifier = Predict::new(TaskComplexityClassifier::new());
        let file_count = input
            .relevant_files
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count() as u32;

        let example = example! {
            "task_description": "input" => input.issue_description.clone(),
            "file_count": "input" => file_count,
            "codebase_context": "input" => Self::build_codebase_context(input),
        };

        let prediction = if let Some(lm) = &self.lm {
            classifier
                .forward_with_streaming(example, lm.clone(), callback)
                .await?
        } else {
            classifier.forward(example).await?
        };

        Ok((
            Self::get_string(&prediction, "complexity"),
            Self::get_f32(&prediction, "confidence"),
        ))
    }

    /// Run the planning pipeline.
    pub async fn plan(&self, input: &PlanningInput) -> anyhow::Result<PlanningResult> {
        self.plan_with_callback(input, None).await
    }

    /// Run the planning pipeline with streaming callback.
    pub async fn plan_with_callback(
        &self,
        input: &PlanningInput,
        callback: Option<&dyn DspyCallback>,
    ) -> anyhow::Result<PlanningResult> {
        let (complexity_label, confidence) = self
            .classify_complexity_with_callback(input, callback)
            .await
            .unwrap_or_else(|_| ("".to_string(), 0.0));

        let use_deep_planning = Self::should_use_deep_planning(&complexity_label, confidence)
            .unwrap_or_else(|| Self::heuristic_complexity(input));

        let prediction = if use_deep_planning {
            self.run_deep_planning_with_callback(input, callback)
                .await?
        } else {
            self.run_basic_planning_with_callback(input, callback)
                .await?
        };

        // Parse results
        let files_to_modify =
            Self::parse_json_array(&Self::get_string(&prediction, "files_to_modify"));
        let implementation_steps =
            Self::parse_json_array(&Self::get_string(&prediction, "implementation_steps"));
        let risk_factors = Self::parse_json_array(&Self::get_string(&prediction, "risk_factors"));

        let complexity_str = Self::get_string(&prediction, "estimated_complexity");
        let complexity = Complexity::from(complexity_str.as_str());

        Ok(PlanningResult {
            analysis: Self::get_string(&prediction, "analysis"),
            files_to_modify,
            implementation_steps,
            test_strategy: Self::get_string(&prediction, "test_strategy"),
            risk_factors,
            complexity,
            confidence: Self::get_f32(&prediction, "confidence"),
        })
    }

    /// Run basic planning for simpler tasks.
    #[expect(dead_code)]
    async fn run_basic_planning(&self, input: &PlanningInput) -> anyhow::Result<dsrs::Prediction> {
        self.run_basic_planning_with_callback(input, None).await
    }

    /// Run basic planning with streaming callback.
    async fn run_basic_planning_with_callback(
        &self,
        input: &PlanningInput,
        callback: Option<&dyn DspyCallback>,
    ) -> anyhow::Result<dsrs::Prediction> {
        let planner = Predict::new(PlanningSignature::new());

        let example = example! {
            "repository_summary": "input" => input.repository_summary.clone(),
            "issue_description": "input" => input.issue_description.clone(),
            "relevant_files": "input" => input.relevant_files.clone(),
        };

        let prediction = if let Some(lm) = &self.lm {
            planner
                .forward_with_streaming(example, lm.clone(), callback)
                .await?
        } else {
            planner.forward(example).await?
        };

        Ok(prediction)
    }

    /// Run deep planning with chain-of-thought for complex tasks.
    #[expect(dead_code)]
    async fn run_deep_planning(&self, input: &PlanningInput) -> anyhow::Result<dsrs::Prediction> {
        self.run_deep_planning_with_callback(input, None).await
    }

    /// Run deep planning with streaming callback.
    async fn run_deep_planning_with_callback(
        &self,
        input: &PlanningInput,
        callback: Option<&dyn DspyCallback>,
    ) -> anyhow::Result<dsrs::Prediction> {
        let deep_planner = Predict::new(DeepPlanningSignature::new());

        let code_patterns = input
            .code_patterns
            .clone()
            .unwrap_or_else(|| "Follow existing patterns in the codebase".to_string());

        let example = example! {
            "repository_summary": "input" => input.repository_summary.clone(),
            "issue_description": "input" => input.issue_description.clone(),
            "relevant_files": "input" => input.relevant_files.clone(),
            "code_patterns": "input" => code_patterns,
        };

        let prediction = if let Some(lm) = &self.lm {
            deep_planner
                .forward_with_streaming(example, lm.clone(), callback)
                .await?
        } else {
            deep_planner.forward(example).await?
        };

        Ok(prediction)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_parsing() {
        assert_eq!(Complexity::from("LOW"), Complexity::Low);
        assert_eq!(Complexity::from("low"), Complexity::Low);
        assert_eq!(Complexity::from("SIMPLE"), Complexity::Low);
        assert_eq!(Complexity::from("MEDIUM"), Complexity::Medium);
        assert_eq!(Complexity::from("medium"), Complexity::Medium);
        assert_eq!(Complexity::from("HIGH"), Complexity::High);
        assert_eq!(Complexity::from("COMPLEX"), Complexity::High);
        assert_eq!(Complexity::from("unknown"), Complexity::Medium);
    }

    #[test]
    fn test_planning_input_serialization() {
        let input = PlanningInput {
            repository_summary: "Rust CLI app".to_string(),
            issue_description: "Add feature".to_string(),
            relevant_files: "src/main.rs\nsrc/lib.rs".to_string(),
            code_patterns: None,
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: PlanningInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.issue_description, input.issue_description);
    }

    #[test]
    fn test_json_array_parsing() {
        // Valid JSON
        let valid = r#"["file1.rs", "file2.rs"]"#;
        let parsed = PlanningPipeline::parse_json_array(valid);
        assert_eq!(parsed, vec!["file1.rs", "file2.rs"]);

        // Fallback for non-JSON
        let lines = "- file1.rs\n- file2.rs";
        let parsed = PlanningPipeline::parse_json_array(lines);
        assert_eq!(parsed, vec!["file1.rs", "file2.rs"]);
    }

    #[test]
    fn test_is_complex_task() {
        let simple = PlanningInput {
            repository_summary: "App".to_string(),
            issue_description: "Fix typo".to_string(),
            relevant_files: "README.md".to_string(),
            code_patterns: None,
        };
        assert!(!PlanningPipeline::heuristic_complexity(&simple));

        let complex = PlanningInput {
            repository_summary: "App".to_string(),
            issue_description: "Refactor the entire authentication system".to_string(),
            relevant_files: "src/auth.rs".to_string(),
            code_patterns: None,
        };
        assert!(PlanningPipeline::heuristic_complexity(&complex));
    }

    #[test]
    fn test_should_use_deep_planning() {
        assert_eq!(
            PlanningPipeline::should_use_deep_planning("Complex", 0.8),
            Some(true)
        );
        assert_eq!(
            PlanningPipeline::should_use_deep_planning("Very Complex", 0.7),
            Some(true)
        );
        assert_eq!(
            PlanningPipeline::should_use_deep_planning("Moderate", 0.9),
            Some(false)
        );
        assert_eq!(
            PlanningPipeline::should_use_deep_planning("Simple", 0.9),
            Some(false)
        );
        assert_eq!(
            PlanningPipeline::should_use_deep_planning("Complex", 0.2),
            None
        );
    }
}
