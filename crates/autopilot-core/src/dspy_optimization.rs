//! DSPy optimization infrastructure for autopilot.
//!
//! Provides metrics, training data collection, and optimization runners
//! for the planning, execution, and verification signatures.
//!
//! # Optimization Strategy
//!
//! 1. Collect training data from successful autopilot sessions
//! 2. Start with MIPROv2 for instruction optimization
//! 3. Graduate to GEPA for complex signatures
//! 4. Store optimized modules in ~/.openagents/dspy/optimized/

use dsrs::{Example, GLOBAL_SETTINGS, Predict, Prediction, Predictor, Signature, example};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};

// ============================================================================
// Signature Definitions
// ============================================================================

/// Validate file paths for correctness.
#[Signature]
struct PathValidationSignature {
    /// Validate a file path and confirm it looks correct for the codebase.

    /// Path to validate
    #[input]
    path: String,

    /// Repository root or context
    #[input]
    codebase_root: String,

    /// Whether the path looks valid
    #[output]
    valid: bool,

    /// Reason for the decision
    #[output]
    reason: String,
}

/// Detect if a step description is actionable.
#[Signature]
struct ActionableStepSignature {
    /// Determine whether a step is concrete and actionable.

    /// Step description
    #[input]
    step: String,

    /// Whether the step is actionable
    #[output]
    actionable: bool,

    /// Suggested improvement if not actionable
    #[output]
    suggested_improvement: String,
}

// ============================================================================
// Planning Metrics
// ============================================================================

/// Evaluate planning quality based on structural validity and actionability.
///
/// Scoring breakdown:
/// - 25%: files_to_modify are valid paths
/// - 25%: implementation_steps are actionable
/// - 25%: test_strategy is concrete
/// - 25%: confidence matches complexity
pub fn planning_metric(_example: &Example, prediction: &Prediction) -> f32 {
    let mut score = 0.0;

    // 1. Check if files_to_modify are valid paths (25%)
    let files = prediction.get("files_to_modify", None);
    if valid_json_array(&files) && paths_look_valid(&files) {
        score += 0.25;
    }

    // 2. Check if steps are actionable (25%)
    let steps = prediction.get("implementation_steps", None);
    if valid_json_array(&steps) && steps_are_actionable(&steps) {
        score += 0.25;
    }

    // 3. Check test strategy is concrete (25%)
    let tests = prediction.get("test_strategy", None);
    if test_strategy_is_concrete(&tests) {
        score += 0.25;
    }

    // 4. Confidence matches complexity (25%)
    let complexity = prediction.get("estimated_complexity", None);
    let confidence = prediction.get("confidence", None);
    if confidence_matches_complexity(&complexity, &confidence) {
        score += 0.25;
    }

    score
}

/// Evaluate execution decision quality.
///
/// Scoring breakdown:
/// - 33%: action is valid
/// - 33%: params are well-formed JSON
/// - 34%: reasoning is substantive
pub fn execution_metric(_example: &Example, prediction: &Prediction) -> f32 {
    let mut score = 0.0;

    // Action is valid
    let action = prediction.get("next_action", None);
    if valid_action(&action) {
        score += 0.33;
    }

    // Params are well-formed JSON
    let params = prediction.get("action_params", None);
    if valid_json(&params) {
        score += 0.33;
    }

    // Reasoning explains the choice
    let reasoning = prediction.get("reasoning", None);
    if reasoning_is_substantive(&reasoning) {
        score += 0.34;
    }

    score
}

/// Evaluate verification quality.
///
/// Scoring breakdown:
/// - 25%: verdict is valid
/// - 25%: explanation is substantive
/// - 25%: confidence is calibrated
/// - 25%: next_action is provided when verdict is RETRY
pub fn verification_metric(_example: &Example, prediction: &Prediction) -> f32 {
    let mut score = 0.0;

    // Verdict is valid
    let verdict = prediction.get("verdict", None);
    if valid_verdict(&verdict) {
        score += 0.25;
    }

    // Explanation is substantive
    let explanation = prediction.get("explanation", None);
    if is_substantive_text(&explanation, 20) {
        score += 0.25;
    }

    // Confidence is calibrated (between 0.0 and 1.0)
    let confidence = prediction.get("confidence", None);
    if confidence_is_calibrated(&confidence) {
        score += 0.25;
    }

    // Next action provided for RETRY
    let verdict_str = verdict.as_str().unwrap_or("");
    let next_action = prediction.get("next_action", None);
    if verdict_str.to_uppercase() == "RETRY" {
        if is_substantive_text(&next_action, 10) {
            score += 0.25;
        }
    } else {
        // If not RETRY, get full points
        score += 0.25;
    }

    score
}

// ============================================================================
// Metric Helper Functions
// ============================================================================

/// Check if a value is a valid JSON array.
fn valid_json_array(val: &serde_json::Value) -> bool {
    if let Some(s) = val.as_str() {
        serde_json::from_str::<Vec<serde_json::Value>>(s).is_ok()
    } else {
        val.is_array()
    }
}

/// Check if DSPy is configured.
fn dspy_ready() -> bool {
    GLOBAL_SETTINGS
        .read()
        .map(|guard| guard.is_some())
        .unwrap_or_else(|poisoned| poisoned.into_inner().is_some())
}

/// Run an async prediction from a sync context.
fn run_prediction<F>(future: F) -> Option<Prediction>
where
    F: Future<Output = anyhow::Result<Prediction>>,
{
    if !dspy_ready() {
        return None;
    }

    let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        catch_unwind(AssertUnwindSafe(|| {
            tokio::task::block_in_place(|| handle.block_on(future))
        }))
    } else if let Ok(runtime) = tokio::runtime::Runtime::new() {
        catch_unwind(AssertUnwindSafe(|| runtime.block_on(future)))
    } else {
        return None;
    };

    match result {
        Ok(Ok(prediction)) => Some(prediction),
        _ => None,
    }
}

/// Extract a boolean from prediction output.
fn prediction_bool(prediction: &Prediction, key: &str) -> Option<bool> {
    let val = prediction.get(key, None);
    if let Some(b) = val.as_bool() {
        Some(b)
    } else if let Some(s) = val.as_str() {
        match s.to_lowercase().as_str() {
            "true" | "yes" | "1" => Some(true),
            "false" | "no" | "0" => Some(false),
            _ => None,
        }
    } else {
        None
    }
}

/// Check if paths look like valid file paths.
fn paths_look_valid(val: &serde_json::Value) -> bool {
    let paths: Vec<String> = if let Some(s) = val.as_str() {
        serde_json::from_str(s).unwrap_or_default()
    } else if let Some(arr) = val.as_array() {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    } else {
        return false;
    };

    if paths.is_empty() {
        return false;
    }

    if let Some(valid) = dspy_paths_valid(&paths) {
        return valid;
    }

    paths_look_valid_heuristic(&paths)
}

fn dspy_paths_valid(paths: &[String]) -> Option<bool> {
    let validator = Predict::new(PathValidationSignature::new());

    for path in paths {
        let example = example! {
            "path": "input" => path.clone(),
            "codebase_root": "input" => ".".to_string(),
        };

        let prediction = run_prediction(validator.forward(example))?;
        let valid = prediction_bool(&prediction, "valid")?;
        if !valid {
            return Some(false);
        }
    }

    Some(true)
}

fn paths_look_valid_heuristic(paths: &[String]) -> bool {
    paths
        .iter()
        .all(|p| (p.contains('/') || p.contains('.')) && !p.contains(' ') && !p.starts_with("http"))
}

/// Check if implementation steps are actionable.
fn steps_are_actionable(val: &serde_json::Value) -> bool {
    let steps: Vec<String> = if let Some(s) = val.as_str() {
        serde_json::from_str(s).unwrap_or_default()
    } else if let Some(arr) = val.as_array() {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    } else {
        return false;
    };

    if steps.is_empty() {
        return false;
    }

    if let Some(actionable) = dspy_steps_actionable(&steps) {
        return actionable;
    }

    steps_are_actionable_heuristic(&steps)
}

fn dspy_steps_actionable(steps: &[String]) -> Option<bool> {
    let classifier = Predict::new(ActionableStepSignature::new());

    for step in steps {
        let example = example! {
            "step": "input" => step.clone(),
        };

        let prediction = run_prediction(classifier.forward(example))?;
        let actionable = prediction_bool(&prediction, "actionable")?;
        if !actionable {
            return Some(false);
        }
    }

    Some(true)
}

fn steps_are_actionable_heuristic(steps: &[String]) -> bool {
    let action_verbs = [
        "add",
        "create",
        "update",
        "modify",
        "remove",
        "delete",
        "implement",
        "write",
        "read",
        "run",
        "test",
        "fix",
        "change",
        "refactor",
        "move",
        "rename",
        "install",
        "configure",
        "set",
        "define",
        "import",
        "export",
        "build",
        "deploy",
        "check",
        "verify",
        "ensure",
    ];

    steps.iter().all(|step| {
        let lower = step.to_lowercase();
        action_verbs.iter().any(|verb| lower.starts_with(verb))
    })
}

/// Check if test strategy is concrete.
fn test_strategy_is_concrete(val: &serde_json::Value) -> bool {
    if let Some(s) = val.as_str() {
        // Must be at least 20 chars and mention testing keywords
        let lower = s.to_lowercase();
        s.len() >= 20
            && (lower.contains("test")
                || lower.contains("verify")
                || lower.contains("check")
                || lower.contains("assert")
                || lower.contains("expect"))
    } else {
        false
    }
}

/// Check if confidence matches complexity.
fn confidence_matches_complexity(
    complexity: &serde_json::Value,
    confidence: &serde_json::Value,
) -> bool {
    let complexity_str = complexity.as_str().unwrap_or("MEDIUM").to_uppercase();
    let confidence_val = confidence.as_f64().unwrap_or(0.5);

    match complexity_str.as_str() {
        "LOW" | "SIMPLE" | "EASY" => confidence_val >= 0.7,
        "HIGH" | "COMPLEX" | "HARD" => confidence_val <= 0.8,
        _ => confidence_val >= 0.3 && confidence_val <= 0.9, // MEDIUM
    }
}

/// Check if action is a valid execution action.
fn valid_action(val: &serde_json::Value) -> bool {
    if let Some(s) = val.as_str() {
        let upper = s.to_uppercase();
        matches!(
            upper.as_str(),
            "EDIT_FILE"
                | "EDIT"
                | "WRITE"
                | "RUN_COMMAND"
                | "RUN"
                | "COMMAND"
                | "BASH"
                | "READ_FILE"
                | "READ"
                | "COMPLETE"
                | "DONE"
                | "FINISHED"
        )
    } else {
        false
    }
}

/// Check if value is valid JSON.
fn valid_json(val: &serde_json::Value) -> bool {
    if let Some(s) = val.as_str() {
        serde_json::from_str::<serde_json::Value>(s).is_ok()
    } else {
        !val.is_null()
    }
}

/// Check if reasoning is substantive.
fn reasoning_is_substantive(val: &serde_json::Value) -> bool {
    is_substantive_text(val, 20)
}

/// Check if text is substantive (has minimum length).
fn is_substantive_text(val: &serde_json::Value, min_len: usize) -> bool {
    if let Some(s) = val.as_str() {
        s.len() >= min_len
    } else {
        false
    }
}

/// Check if verdict is valid.
fn valid_verdict(val: &serde_json::Value) -> bool {
    if let Some(s) = val.as_str() {
        let upper = s.to_uppercase();
        matches!(
            upper.as_str(),
            "PASS"
                | "PASSED"
                | "SUCCESS"
                | "FAIL"
                | "FAILED"
                | "RETRY"
                | "ITERATE"
                | "CONTINUE"
                | "APPROVE"
                | "APPROVED"
                | "REVISE"
                | "REJECT"
                | "REJECTED"
        )
    } else {
        false
    }
}

/// Check if confidence is calibrated (between 0 and 1).
fn confidence_is_calibrated(val: &serde_json::Value) -> bool {
    if let Some(n) = val.as_f64() {
        (0.0..=1.0).contains(&n)
    } else if let Some(s) = val.as_str() {
        if let Ok(n) = s.parse::<f64>() {
            (0.0..=1.0).contains(&n)
        } else {
            false
        }
    } else {
        false
    }
}

// ============================================================================
// Training Data Structures
// ============================================================================

/// Example for planning optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningExample {
    // Inputs
    pub repository_summary: String,
    pub issue_description: String,
    pub relevant_files: String,
    // Expected outputs (from successful sessions)
    pub expected_analysis: String,
    pub expected_files: Vec<String>,
    pub expected_steps: Vec<String>,
    pub expected_test_strategy: String,
}

impl PlanningExample {
    /// Convert to DSPy Example.
    pub fn to_example(&self) -> Example {
        let mut data = std::collections::HashMap::new();
        data.insert(
            "repository_summary".to_string(),
            serde_json::Value::String(self.repository_summary.clone()),
        );
        data.insert(
            "issue_description".to_string(),
            serde_json::Value::String(self.issue_description.clone()),
        );
        data.insert(
            "relevant_files".to_string(),
            serde_json::Value::String(self.relevant_files.clone()),
        );
        data.insert(
            "analysis".to_string(),
            serde_json::Value::String(self.expected_analysis.clone()),
        );
        data.insert(
            "files_to_modify".to_string(),
            serde_json::Value::String(
                serde_json::to_string(&self.expected_files).unwrap_or_default(),
            ),
        );
        data.insert(
            "implementation_steps".to_string(),
            serde_json::Value::String(
                serde_json::to_string(&self.expected_steps).unwrap_or_default(),
            ),
        );
        data.insert(
            "test_strategy".to_string(),
            serde_json::Value::String(self.expected_test_strategy.clone()),
        );

        let input_keys = vec![
            "repository_summary".to_string(),
            "issue_description".to_string(),
            "relevant_files".to_string(),
        ];
        let output_keys = vec![
            "analysis".to_string(),
            "files_to_modify".to_string(),
            "implementation_steps".to_string(),
            "test_strategy".to_string(),
        ];

        Example::new(data, input_keys, output_keys)
    }
}

/// Example for execution optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionExample {
    // Inputs
    pub plan_step: String,
    pub current_file_state: String,
    pub execution_history: String,
    // Expected outputs
    pub expected_action: String,
    pub expected_params: serde_json::Value,
    pub expected_reasoning: String,
}

impl ExecutionExample {
    /// Convert to DSPy Example.
    pub fn to_example(&self) -> Example {
        let mut data = std::collections::HashMap::new();
        data.insert(
            "plan_step".to_string(),
            serde_json::Value::String(self.plan_step.clone()),
        );
        data.insert(
            "current_file_state".to_string(),
            serde_json::Value::String(self.current_file_state.clone()),
        );
        data.insert(
            "execution_history".to_string(),
            serde_json::Value::String(self.execution_history.clone()),
        );
        data.insert(
            "next_action".to_string(),
            serde_json::Value::String(self.expected_action.clone()),
        );
        data.insert(
            "action_params".to_string(),
            serde_json::Value::String(
                serde_json::to_string(&self.expected_params).unwrap_or_default(),
            ),
        );
        data.insert(
            "reasoning".to_string(),
            serde_json::Value::String(self.expected_reasoning.clone()),
        );

        let input_keys = vec![
            "plan_step".to_string(),
            "current_file_state".to_string(),
            "execution_history".to_string(),
        ];
        let output_keys = vec![
            "next_action".to_string(),
            "action_params".to_string(),
            "reasoning".to_string(),
        ];

        Example::new(data, input_keys, output_keys)
    }
}

/// Example for verification optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationExample {
    // Inputs
    pub requirements: Vec<String>,
    pub solution_summary: String,
    pub code_changes: String,
    pub build_output: String,
    pub test_output: String,
    // Expected outputs
    pub expected_verdict: String,
    pub expected_explanation: String,
}

impl VerificationExample {
    /// Convert to DSPy Example.
    pub fn to_example(&self) -> Example {
        let mut data = std::collections::HashMap::new();
        data.insert(
            "requirements".to_string(),
            serde_json::Value::String(self.requirements.join("; ")),
        );
        data.insert(
            "solution_summary".to_string(),
            serde_json::Value::String(self.solution_summary.clone()),
        );
        data.insert(
            "code_changes".to_string(),
            serde_json::Value::String(self.code_changes.clone()),
        );
        data.insert(
            "build_output".to_string(),
            serde_json::Value::String(self.build_output.clone()),
        );
        data.insert(
            "test_output".to_string(),
            serde_json::Value::String(self.test_output.clone()),
        );
        data.insert(
            "verdict".to_string(),
            serde_json::Value::String(self.expected_verdict.clone()),
        );
        data.insert(
            "explanation".to_string(),
            serde_json::Value::String(self.expected_explanation.clone()),
        );

        let input_keys = vec![
            "requirements".to_string(),
            "solution_summary".to_string(),
            "code_changes".to_string(),
            "build_output".to_string(),
            "test_output".to_string(),
        ];
        let output_keys = vec!["verdict".to_string(), "explanation".to_string()];

        Example::new(data, input_keys, output_keys)
    }
}

/// Dataset for optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationDataset {
    pub planning_examples: Vec<PlanningExample>,
    pub execution_examples: Vec<ExecutionExample>,
    pub verification_examples: Vec<VerificationExample>,
}

impl OptimizationDataset {
    /// Create an empty dataset.
    pub fn new() -> Self {
        Self {
            planning_examples: Vec::new(),
            execution_examples: Vec::new(),
            verification_examples: Vec::new(),
        }
    }

    /// Load dataset from JSON file.
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let dataset: Self = serde_json::from_str(&contents)?;
        Ok(dataset)
    }

    /// Save dataset to JSON file.
    pub fn to_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    /// Get planning examples as DSPy Examples.
    pub fn planning_as_examples(&self) -> Vec<Example> {
        self.planning_examples
            .iter()
            .map(|e| e.to_example())
            .collect()
    }

    /// Get execution examples as DSPy Examples.
    pub fn execution_as_examples(&self) -> Vec<Example> {
        self.execution_examples
            .iter()
            .map(|e| e.to_example())
            .collect()
    }

    /// Get verification examples as DSPy Examples.
    pub fn verification_as_examples(&self) -> Vec<Example> {
        self.verification_examples
            .iter()
            .map(|e| e.to_example())
            .collect()
    }
}

impl Default for OptimizationDataset {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Optimizer Types
// ============================================================================

/// Type of optimizer to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizerType {
    /// MIPROv2 - instruction optimization
    MIPRO,
    /// COPRO - cooperative optimization
    COPRO,
    /// Bootstrap Few-Shot
    BootstrapFewShot,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_json_array() {
        let arr = serde_json::json!(["a", "b"]);
        assert!(valid_json_array(&arr));

        let str_arr = serde_json::json!(r#"["a", "b"]"#);
        assert!(valid_json_array(&str_arr));

        let not_arr = serde_json::json!("not an array");
        assert!(!valid_json_array(&not_arr));
    }

    #[test]
    fn test_paths_look_valid() {
        let valid = serde_json::json!(["src/main.rs", "Cargo.toml"]);
        assert!(paths_look_valid(&valid));

        let invalid = serde_json::json!(["http://example.com", "file with space.txt"]);
        assert!(!paths_look_valid(&invalid));
    }

    #[test]
    fn test_steps_are_actionable() {
        let good = serde_json::json!(["Add new function", "Update tests", "Run build"]);
        assert!(steps_are_actionable(&good));

        let bad = serde_json::json!(["The function", "Some tests"]);
        assert!(!steps_are_actionable(&bad));
    }

    #[test]
    fn test_confidence_matches_complexity() {
        let low = serde_json::json!("LOW");
        let high_conf = serde_json::json!(0.9);
        assert!(confidence_matches_complexity(&low, &high_conf));

        let high = serde_json::json!("HIGH");
        let low_conf = serde_json::json!(0.5);
        assert!(confidence_matches_complexity(&high, &low_conf));

        // High complexity should not have very high confidence
        let too_high = serde_json::json!(0.95);
        assert!(!confidence_matches_complexity(&high, &too_high));
    }

    #[test]
    fn test_valid_action() {
        assert!(valid_action(&serde_json::json!("EDIT_FILE")));
        assert!(valid_action(&serde_json::json!("run_command")));
        assert!(valid_action(&serde_json::json!("READ")));
        assert!(!valid_action(&serde_json::json!("INVALID")));
    }

    #[test]
    fn test_dataset_serialization() {
        let dataset = OptimizationDataset::new();
        let json = serde_json::to_string(&dataset).unwrap();
        let parsed: OptimizationDataset = serde_json::from_str(&json).unwrap();
        assert!(parsed.planning_examples.is_empty());
    }

    #[test]
    fn test_planning_example_to_dspy() {
        let example = PlanningExample {
            repository_summary: "Rust CLI".to_string(),
            issue_description: "Add feature".to_string(),
            relevant_files: "src/main.rs".to_string(),
            expected_analysis: "Need to add...".to_string(),
            expected_files: vec!["src/main.rs".to_string()],
            expected_steps: vec!["Add function".to_string()],
            expected_test_strategy: "Run cargo test".to_string(),
        };

        let dspy_example = example.to_example();
        // Verify it has the expected input keys
        assert_eq!(dspy_example.input_keys.len(), 3);
        assert!(
            dspy_example
                .input_keys
                .contains(&"repository_summary".to_string())
        );
    }
}
