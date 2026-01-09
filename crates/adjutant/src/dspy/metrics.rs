//! Evaluation metrics for Adjutant's DSPy signatures.
//!
//! These metrics are used by MIPROv2 and other optimizers to evaluate
//! the quality of signature predictions. Each metric returns a score
//! from 0.0 to 1.0.

use dsrs::{Example, Prediction};

// ============================================================================
// Subtask Planning Metric
// ============================================================================

/// Evaluate planning quality for subtask generation.
///
/// Scoring breakdown:
/// - 25%: subtasks is valid JSON array with required fields
/// - 25%: each subtask has valid action type (read, edit, bash)
/// - 25%: target paths look valid for file operations
/// - 25%: instructions are actionable (start with verb)
pub fn subtask_planning_metric(_example: &Example, prediction: &Prediction) -> f32 {
    let mut score = 0.0;

    let subtasks = prediction.get("subtasks", None);

    // 1. Valid JSON array with required fields (25%)
    if valid_subtasks_json(&subtasks) {
        score += 0.25;
    }

    // 2. Valid action types (25%)
    if has_valid_action_types(&subtasks) {
        score += 0.25;
    }

    // 3. Valid target paths (25%)
    if has_valid_targets(&subtasks) {
        score += 0.25;
    }

    // 4. Actionable instructions (25%)
    if has_actionable_instructions(&subtasks) {
        score += 0.25;
    }

    score
}

// ============================================================================
// Subtask Execution Metric
// ============================================================================

/// Evaluate execution quality for a single subtask.
///
/// Scoring breakdown:
/// - 33%: result is valid JSON with expected fields
/// - 33%: old_string/new_string are well-formed (for edits)
/// - 34%: reasoning is substantive
pub fn subtask_execution_metric(_example: &Example, prediction: &Prediction) -> f32 {
    let mut score = 0.0;

    // 1. Valid result JSON (33%)
    let result = prediction.get("result", None);
    if valid_result_json(&result) {
        score += 0.33;
    }

    // 2. Well-formed edit strings or command (33%)
    if has_valid_edit_or_command(&result) {
        score += 0.33;
    }

    // 3. Substantive reasoning (34%)
    let reasoning = prediction.get("reasoning", None);
    if is_substantive_text(&reasoning, 20) {
        score += 0.34;
    }

    score
}

// ============================================================================
// Synthesis Metric
// ============================================================================

/// Evaluate synthesis quality.
///
/// Scoring breakdown:
/// - 25%: success is a valid boolean
/// - 25%: summary is substantive
/// - 25%: modified_files is valid JSON array
/// - 25%: confidence is calibrated (0.0-1.0)
pub fn synthesis_metric(_example: &Example, prediction: &Prediction) -> f32 {
    let mut score = 0.0;

    // 1. Valid success boolean (25%)
    let success = prediction.get("success", None);
    if success.is_boolean() || is_bool_string(&success) {
        score += 0.25;
    }

    // 2. Substantive summary (25%)
    let summary = prediction.get("summary", None);
    if is_substantive_text(&summary, 15) {
        score += 0.25;
    }

    // 3. Valid modified_files JSON array (25%)
    let files = prediction.get("modified_files", None);
    if valid_json_array(&files) {
        score += 0.25;
    }

    // 4. Calibrated confidence (25%)
    let confidence = prediction.get("confidence", None);
    if confidence_is_calibrated(&confidence) {
        score += 0.25;
    }

    score
}

// ============================================================================
// Combined Metric
// ============================================================================

/// Combined metric across all phases for AdjutantModule evaluation.
pub fn combined_metric(example: &Example, prediction: &Prediction) -> f32 {
    // Weight planning higher as it's the foundation
    let planning_score = subtask_planning_metric(example, prediction);
    let synthesis_score = synthesis_metric(example, prediction);

    // 60% planning, 40% synthesis (execution is per-subtask)
    planning_score * 0.6 + synthesis_score * 0.4
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Check if subtasks JSON is valid with required fields.
fn valid_subtasks_json(val: &serde_json::Value) -> bool {
    let arr = if let Some(s) = val.as_str() {
        match serde_json::from_str::<Vec<serde_json::Value>>(s) {
            Ok(a) => a,
            Err(_) => return false,
        }
    } else if let Some(arr) = val.as_array() {
        arr.clone()
    } else {
        return false;
    };

    // Check each item has required fields
    arr.iter().all(|item| {
        item.get("id").is_some()
            && item.get("action").is_some()
            && item.get("target").is_some()
            && item.get("instruction").is_some()
    })
}

/// Check if all subtasks have valid action types.
fn has_valid_action_types(val: &serde_json::Value) -> bool {
    let arr = parse_subtasks_array(val);
    if arr.is_empty() {
        return false;
    }

    arr.iter().all(|item| {
        if let Some(action) = item.get("action").and_then(|a| a.as_str()) {
            matches!(action, "read" | "edit" | "bash")
        } else {
            false
        }
    })
}

/// Check if target paths look valid.
fn has_valid_targets(val: &serde_json::Value) -> bool {
    let arr = parse_subtasks_array(val);
    if arr.is_empty() {
        return false;
    }

    arr.iter().all(|item| {
        let action = item.get("action").and_then(|a| a.as_str()).unwrap_or("");
        let target = item.get("target").and_then(|t| t.as_str()).unwrap_or("");

        match action {
            // read/edit need file paths
            "read" | "edit" => {
                (target.contains('/') || target.contains('.'))
                    && !target.contains(' ')
                    && !target.starts_with("http")
            }
            // bash doesn't require target
            "bash" => true,
            _ => false,
        }
    })
}

/// Check if instructions are actionable.
fn has_actionable_instructions(val: &serde_json::Value) -> bool {
    let action_verbs = [
        "add", "create", "update", "modify", "remove", "delete", "implement", "write", "read",
        "run", "test", "fix", "change", "refactor", "move", "rename", "install", "configure",
        "set", "define", "import", "export", "build", "deploy", "check", "verify", "ensure",
        "understand", "analyze", "find", "search", "execute",
    ];

    let arr = parse_subtasks_array(val);
    if arr.is_empty() {
        return false;
    }

    arr.iter().all(|item| {
        if let Some(inst) = item.get("instruction").and_then(|i| i.as_str()) {
            let lower = inst.to_lowercase();
            action_verbs.iter().any(|verb| lower.starts_with(verb))
        } else {
            false
        }
    })
}

/// Parse subtasks from Value (string JSON or array).
fn parse_subtasks_array(val: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(s) = val.as_str() {
        serde_json::from_str(s).unwrap_or_default()
    } else if let Some(arr) = val.as_array() {
        arr.clone()
    } else {
        Vec::new()
    }
}

/// Check if result JSON is valid.
fn valid_result_json(val: &serde_json::Value) -> bool {
    if let Some(s) = val.as_str() {
        serde_json::from_str::<serde_json::Value>(s).is_ok()
    } else {
        !val.is_null()
    }
}

/// Check if result has valid edit strings or command.
fn has_valid_edit_or_command(val: &serde_json::Value) -> bool {
    let obj = if let Some(s) = val.as_str() {
        match serde_json::from_str::<serde_json::Value>(s) {
            Ok(o) => o,
            Err(_) => return false,
        }
    } else {
        val.clone()
    };

    // Either has old_string/new_string (edit) or command (bash)
    (obj.get("old_string").is_some() && obj.get("new_string").is_some())
        || obj.get("command").is_some()
        // Or it's a read result (any non-empty content)
        || (obj.is_string() && !obj.as_str().unwrap_or("").is_empty())
}

/// Check if text is substantive (has minimum length).
fn is_substantive_text(val: &serde_json::Value, min_len: usize) -> bool {
    if let Some(s) = val.as_str() {
        s.len() >= min_len
    } else {
        false
    }
}

/// Check if value is a valid JSON array.
fn valid_json_array(val: &serde_json::Value) -> bool {
    if let Some(s) = val.as_str() {
        serde_json::from_str::<Vec<serde_json::Value>>(s).is_ok()
    } else {
        val.is_array()
    }
}

/// Check if confidence is calibrated (0.0 to 1.0).
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

/// Check if value is a boolean string.
fn is_bool_string(val: &serde_json::Value) -> bool {
    if let Some(s) = val.as_str() {
        matches!(s.to_lowercase().as_str(), "true" | "false")
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsrs::prediction;

    #[test]
    fn test_planning_metric_valid() {
        let prediction = prediction! {
            "subtasks" => r#"[{"id":"1","action":"edit","target":"lib.rs","instruction":"Add function"}]"#,
            "reasoning" => "Need to add the function to the main lib file",
            "confidence" => 0.85,
        };

        let score = subtask_planning_metric(&Example::default(), &prediction);
        assert!(score > 0.5, "Score should be > 0.5, got {}", score);
    }

    #[test]
    fn test_planning_metric_invalid_action() {
        let prediction = prediction! {
            "subtasks" => r#"[{"id":"1","action":"invalid","target":"lib.rs","instruction":"Do something"}]"#,
        };

        let score = subtask_planning_metric(&Example::default(), &prediction);
        assert!(score < 0.75, "Score should be < 0.75 for invalid action");
    }

    #[test]
    fn test_execution_metric_valid_edit() {
        let prediction = prediction! {
            "result" => r#"{"old_string": "fn old()", "new_string": "fn new()"}"#,
            "reasoning" => "Renamed the function for clarity",
            "success" => true,
        };

        let score = subtask_execution_metric(&Example::default(), &prediction);
        assert!(score > 0.9, "Score should be > 0.9, got {}", score);
    }

    #[test]
    fn test_synthesis_metric_valid() {
        let prediction = prediction! {
            "success" => true,
            "summary" => "Successfully added the hello world function",
            "modified_files" => r#"["src/lib.rs"]"#,
            "confidence" => 0.95,
        };

        let score = synthesis_metric(&Example::default(), &prediction);
        assert!(score > 0.9, "Score should be > 0.9, got {}", score);
    }
}
