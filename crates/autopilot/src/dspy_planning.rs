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
//! use autopilot::dspy_planning::{PlanningPipeline, PlanningInput};
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

use dsrs::{example, LM, Predict, Predictor, Signature};
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

    /// Determine if task is complex enough for deep planning.
    fn is_complex_task(input: &PlanningInput) -> bool {
        // Heuristics for complexity
        let issue_len = input.issue_description.len();
        let file_count = input.relevant_files.lines().count();

        // Complex if: long description, many files, or explicit keywords
        issue_len > 500
            || file_count > 10
            || input
                .issue_description
                .to_lowercase()
                .contains("refactor")
            || input
                .issue_description
                .to_lowercase()
                .contains("architecture")
            || input
                .issue_description
                .to_lowercase()
                .contains("redesign")
    }

    /// Run the planning pipeline.
    pub async fn plan(&self, input: &PlanningInput) -> anyhow::Result<PlanningResult> {
        let use_deep_planning = Self::is_complex_task(input);

        let prediction = if use_deep_planning {
            self.run_deep_planning(input).await?
        } else {
            self.run_basic_planning(input).await?
        };

        // Parse results
        let files_to_modify = Self::parse_json_array(&Self::get_string(&prediction, "files_to_modify"));
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
    async fn run_basic_planning(&self, input: &PlanningInput) -> anyhow::Result<dsrs::Prediction> {
        let planner = Predict::new(PlanningSignature::new());

        let example = example! {
            "repository_summary": "input" => input.repository_summary.clone(),
            "issue_description": "input" => input.issue_description.clone(),
            "relevant_files": "input" => input.relevant_files.clone(),
        };

        let prediction = if let Some(lm) = &self.lm {
            planner.forward_with_config(example, lm.clone()).await?
        } else {
            planner.forward(example).await?
        };

        Ok(prediction)
    }

    /// Run deep planning with chain-of-thought for complex tasks.
    async fn run_deep_planning(&self, input: &PlanningInput) -> anyhow::Result<dsrs::Prediction> {
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
            deep_planner.forward_with_config(example, lm.clone()).await?
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
        assert!(!PlanningPipeline::is_complex_task(&simple));

        let complex = PlanningInput {
            repository_summary: "App".to_string(),
            issue_description: "Refactor the entire authentication system".to_string(),
            relevant_files: "src/auth.rs".to_string(),
            code_patterns: None,
        };
        assert!(PlanningPipeline::is_complex_task(&complex));
    }
}
