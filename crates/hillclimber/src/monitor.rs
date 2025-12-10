//! HillClimber Monitor Module
//!
//! Validates actions BEFORE execution to catch obvious mistakes.
//! Rule-based, no LLM needed for fast validation.
//!
//! Part of the MAP (Modular Agentic Planner) architecture.

use crate::types::{ActionContext, MonitorDecision};
use regex::Regex;

// ============================================================================
// GUARDRAIL: NO TASK-SPECIFIC HARDCODING
//
// This file must NEVER contain:
// - Task IDs (e.g., "regex-log", "path-tracing")
// - Task-specific patterns (e.g., IPv4 format, date format)
// - Task-specific hints (e.g., "use lookahead for IPv4")
// - Task-specific file paths (e.g., "/app/regex.txt")
//
// All knowledge must come from:
// 1. The task description (passed as parameter)
// 2. General process knowledge (TDD, iteration)
// ============================================================================

// ============================================================================
// Validation Rules
// ============================================================================

/// Rule: Prevent writing outside workspace.
fn check_workspace_bounds(ctx: &ActionContext) -> Option<MonitorDecision> {
    if ctx.tool_name == "write_file" || ctx.tool_name == "edit_file" {
        let path = ctx
            .args
            .get("path")
            .or_else(|| ctx.args.get("file_path"))
            .and_then(|v| v.as_str());

        if let Some(path) = path {
            // Absolute path outside workspace
            if path.starts_with('/') && !path.starts_with(&ctx.workspace.to_string_lossy().as_ref()) && !path.starts_with("/app") {
                return Some(MonitorDecision::deny_with_suggestion(
                    format!("Cannot write outside workspace: {}", path),
                    "Use relative path or /app/ prefix",
                ));
            }
            // Parent directory traversal
            if path.contains("..") {
                return Some(MonitorDecision::deny_with_suggestion(
                    format!("Path traversal not allowed: {}", path),
                    "Use direct path without ..",
                ));
            }
        }
    }
    None
}

/// Rule: Prevent dangerous shell commands.
fn check_dangerous_commands(ctx: &ActionContext) -> Option<MonitorDecision> {
    if ctx.tool_name == "run_command" {
        let cmd = ctx
            .args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Note: Rust regex doesn't support lookahead (?!), so we use explicit checks

        // Check for rm -rf on dangerous system paths (not /app)
        let rm_rf_re = Regex::new(r"rm\s+-rf?\s+(/\w+)").ok();
        if let Some(re) = &rm_rf_re {
            if let Some(caps) = re.captures(cmd) {
                let path = &caps[1];
                // Allow /app, reject other system paths
                if path != "/app" && !path.starts_with("/app/") {
                    return Some(MonitorDecision::deny_with_suggestion(
                        "Cannot delete system directories".to_string(),
                        "Use a safer alternative",
                    ));
                }
            }
        }

        // Simple patterns without lookahead
        let dangerous_patterns: Vec<(&str, &str)> = vec![
            (r"rm\s+-rf?\s+\*", "Cannot delete all files"),
            (r"chmod\s+777", "Overly permissive chmod"),
            (r"curl.*\|\s*(ba)?sh", "Cannot pipe curl to shell"),
            (r"wget.*\|\s*(ba)?sh", "Cannot pipe wget to shell"),
            (r"\bsudo\b", "Cannot use sudo"),
            (r"\bdd\s+.*of=/dev", "Cannot write to block devices"),
        ];

        for (pattern, reason) in dangerous_patterns {
            if let Ok(re) = Regex::new(pattern) {
                if re.is_match(cmd) {
                    return Some(MonitorDecision::deny_with_suggestion(
                        reason.to_string(),
                        "Use a safer alternative",
                    ));
                }
            }
        }
    }
    None
}

/// Rule: Warn about overwriting files without testing first.
fn check_test_before_submit(ctx: &ActionContext) -> Option<MonitorDecision> {
    if ctx.tool_name == "write_file" {
        let path = ctx
            .args
            .get("path")
            .or_else(|| ctx.args.get("file_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Generic check: warn if overwriting a previously modified file without testing
        if ctx.turn_number > 1 && ctx.modified_files.contains(&path.to_string()) {
            // Check if they've tested before rewriting
            let has_tested_recently = ctx.previous_actions.iter().any(|a| {
                a.contains("verify_progress") || a.contains("test") || a.contains("pytest")
            });

            if !has_tested_recently {
                return Some(MonitorDecision::allow_with_warning(format!(
                    "Overwriting {} without testing. Consider using verify_progress first.",
                    path
                )));
            }
        }
    }
    None
}

/// Rule: Prevent infinite loops (same action repeated).
fn check_repetition(ctx: &ActionContext) -> Option<MonitorDecision> {
    if ctx.previous_actions.len() >= 3 {
        let last_3: Vec<&String> = ctx.previous_actions.iter().rev().take(3).collect();
        let action_signature = create_action_signature_internal(&ctx.tool_name, &ctx.args);

        // Check if this exact action was done recently
        let repeats = last_3.iter().filter(|a| a.as_str() == action_signature).count();
        if repeats >= 2 {
            return Some(MonitorDecision::deny_with_suggestion(
                format!("Action repeated {} times: {}", repeats + 1, ctx.tool_name),
                "Try a different approach or use verify_progress to see current state",
            ));
        }
    }
    None
}

// ============================================================================
// Main Monitor
// ============================================================================

/// All validation rules in priority order.
type ValidationRule = fn(&ActionContext) -> Option<MonitorDecision>;

const VALIDATION_RULES: &[ValidationRule] = &[
    check_workspace_bounds,
    check_dangerous_commands,
    check_repetition,
    check_test_before_submit,
];

/// Monitor an action before execution.
///
/// # Arguments
///
/// * `ctx` - Action context with tool, args, and execution state
///
/// # Returns
///
/// Decision on whether to allow the action
pub fn monitor_action(ctx: &ActionContext) -> MonitorDecision {
    let mut warnings: Vec<String> = Vec::new();

    for rule in VALIDATION_RULES {
        if let Some(decision) = rule(ctx) {
            if !decision.allowed {
                // Immediate rejection
                return decision;
            }

            if let Some(warning) = decision.warning {
                warnings.push(warning);
            }
        }
    }

    // All rules passed
    if warnings.is_empty() {
        MonitorDecision::allow()
    } else {
        MonitorDecision::allow_with_warning(warnings.join("; "))
    }
}

/// Create an action signature for tracking.
pub fn create_action_signature(tool_name: &str, args: &serde_json::Value) -> String {
    create_action_signature_internal(tool_name, args)
}

fn create_action_signature_internal(tool_name: &str, args: &serde_json::Value) -> String {
    // Create a normalized signature for comparison
    let mut normalized_args = serde_json::Map::new();

    if let Some(obj) = args.as_object() {
        for (key, value) in obj {
            if key == "content" || key == "new_string" {
                // For content, just use length to detect same writes
                let len = value.as_str().map(|s| s.len()).unwrap_or(0);
                normalized_args.insert(key.clone(), serde_json::json!(format!("content:{}", len)));
            } else {
                normalized_args.insert(key.clone(), value.clone());
            }
        }
    }

    format!(
        "{}:{}",
        tool_name,
        serde_json::to_string(&normalized_args).unwrap_or_default()
    )
}

/// Check if two actions are the same.
pub fn is_same_action(
    action1_tool: &str,
    action1_args: &serde_json::Value,
    action2_tool: &str,
    action2_args: &serde_json::Value,
) -> bool {
    create_action_signature(action1_tool, action1_args)
        == create_action_signature(action2_tool, action2_args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_context(
        tool_name: &str,
        args: serde_json::Value,
    ) -> ActionContext {
        ActionContext {
            tool_name: tool_name.to_string(),
            args,
            workspace: PathBuf::from("/app/workspace"),
            task_id: "test-task".to_string(),
            modified_files: vec![],
            turn_number: 1,
            previous_actions: vec![],
        }
    }

    #[test]
    fn test_workspace_bounds() {
        // Writing to /app/ should be allowed
        let ctx = create_test_context(
            "write_file",
            serde_json::json!({"path": "/app/solution.txt", "content": "test"}),
        );
        let decision = monitor_action(&ctx);
        assert!(decision.allowed);

        // Writing outside workspace should be rejected
        let ctx = create_test_context(
            "write_file",
            serde_json::json!({"path": "/etc/passwd", "content": "test"}),
        );
        let decision = monitor_action(&ctx);
        assert!(!decision.allowed);

        // Path traversal should be rejected
        let ctx = create_test_context(
            "write_file",
            serde_json::json!({"path": "/app/../etc/passwd", "content": "test"}),
        );
        let decision = monitor_action(&ctx);
        assert!(!decision.allowed);
    }

    #[test]
    fn test_dangerous_commands() {
        // Safe command should be allowed
        let ctx = create_test_context(
            "run_command",
            serde_json::json!({"command": "ls -la"}),
        );
        let decision = monitor_action(&ctx);
        assert!(decision.allowed);

        // rm -rf /etc should be rejected
        let ctx = create_test_context(
            "run_command",
            serde_json::json!({"command": "rm -rf /etc"}),
        );
        let decision = monitor_action(&ctx);
        assert!(!decision.allowed);

        // rm -rf /app should be allowed (workspace)
        let ctx = create_test_context(
            "run_command",
            serde_json::json!({"command": "rm -rf /app/temp"}),
        );
        let decision = monitor_action(&ctx);
        assert!(decision.allowed);

        // sudo should be rejected
        let ctx = create_test_context(
            "run_command",
            serde_json::json!({"command": "sudo cat /etc/shadow"}),
        );
        let decision = monitor_action(&ctx);
        assert!(!decision.allowed);
    }

    #[test]
    fn test_repetition_detection() {
        // Create signature with same args as the action (including content)
        let signature = create_action_signature("write_file", &serde_json::json!({"path": "/app/test.txt", "content": "test"}));

        let mut ctx = create_test_context(
            "write_file",
            serde_json::json!({"path": "/app/test.txt", "content": "test"}),
        );
        ctx.previous_actions = vec![
            "other_action".to_string(),
            signature.clone(),
            signature.clone(),
        ];

        let decision = monitor_action(&ctx);
        assert!(!decision.allowed);
        assert!(decision.reason.unwrap().contains("repeated"));
    }

    #[test]
    fn test_action_signature() {
        let sig1 = create_action_signature(
            "write_file",
            &serde_json::json!({"path": "/app/test.txt", "content": "hello"}),
        );
        let sig2 = create_action_signature(
            "write_file",
            &serde_json::json!({"path": "/app/test.txt", "content": "world"}),
        );
        let sig3 = create_action_signature(
            "write_file",
            &serde_json::json!({"path": "/app/other.txt", "content": "hello"}),
        );

        // Same path, same content length -> same signature
        assert_eq!(sig1, sig2);
        // Different path -> different signature
        assert_ne!(sig1, sig3);
    }
}
