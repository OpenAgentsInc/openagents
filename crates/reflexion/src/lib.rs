//! Reflexion - Self-critique and reflection system for MechaCoder
//!
//! Based on Reflexion research showing +11% improvement from verbal reinforcement.
//!
//! Key insight: After failures, generate verbal self-critique that gets injected
//! into subsequent attempts. This provides "verbal reinforcement" without
//! parameter updates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Error Types
// ============================================================================

/// Classification of error types for targeted reflection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    /// TypeScript type mismatch
    TypeError,
    /// Missing or wrong import
    ImportError,
    /// Syntax issues
    SyntaxError,
    /// Runtime exceptions
    RuntimeError,
    /// Test assertion failed
    TestFailure,
    /// Build/compilation error
    BuildError,
    /// Task timed out
    Timeout,
    /// Tool execution failed
    ToolError,
    /// Wrong behavior/output
    LogicError,
    /// Unclassified
    Unknown,
}

impl Default for ErrorType {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Classify an error message into an error type.
pub fn classify_error(error_message: &str) -> ErrorType {
    let lower = error_message.to_lowercase();

    if lower.contains("type") && (lower.contains("not assignable") || lower.contains("ts2")) {
        return ErrorType::TypeError;
    }
    if lower.contains("cannot find module")
        || lower.contains("import")
        || lower.contains("ts2307")
    {
        return ErrorType::ImportError;
    }
    if lower.contains("syntax") || lower.contains("unexpected token") || lower.contains("parsing")
    {
        return ErrorType::SyntaxError;
    }
    if lower.contains("timeout") || lower.contains("timed out") {
        return ErrorType::Timeout;
    }
    if (lower.contains("test") && lower.contains("fail"))
        || (lower.contains("expect")
            && (lower.contains("fail") || lower.contains("tobe") || lower.contains("toequal")))
    {
        return ErrorType::TestFailure;
    }
    if lower.contains("build") || lower.contains("compile") || lower.contains("bundle") {
        return ErrorType::BuildError;
    }
    if lower.contains("tool") || lower.contains("command failed") {
        return ErrorType::ToolError;
    }
    if lower.contains("runtime") || lower.contains("exception") || lower.contains("error:") {
        return ErrorType::RuntimeError;
    }

    ErrorType::Unknown
}

// ============================================================================
// Failure Context
// ============================================================================

/// Context about a failure for reflection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureContext {
    /// Unique ID for this failure
    pub id: String,
    /// Task that was being attempted
    pub task_description: String,
    /// What was tried
    pub attempt_description: String,
    /// The error or failure that occurred
    pub error_message: String,
    /// Error type classification
    pub error_type: ErrorType,
    /// Files that were involved
    pub files_involved: Vec<String>,
    /// Code that was written (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_written: Option<String>,
    /// Skills that were used (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills_used: Option<Vec<String>>,
    /// Attempt number (1-indexed)
    pub attempt_number: u32,
    /// Duration of the failed attempt in ms
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Timestamp
    pub timestamp: String,
    /// Project context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

impl Default for FailureContext {
    fn default() -> Self {
        Self {
            id: generate_failure_id(),
            task_description: String::new(),
            attempt_description: String::new(),
            error_message: String::new(),
            error_type: ErrorType::Unknown,
            files_involved: Vec::new(),
            code_written: None,
            skills_used: None,
            attempt_number: 1,
            duration_ms: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            project_id: None,
        }
    }
}

/// Options for creating a failure context
#[derive(Debug, Clone, Default)]
pub struct FailureContextOptions {
    pub attempt_description: Option<String>,
    pub files_involved: Option<Vec<String>>,
    pub code_written: Option<String>,
    pub skills_used: Option<Vec<String>>,
    pub attempt_number: Option<u32>,
    pub duration_ms: Option<u64>,
    pub project_id: Option<String>,
}

/// Create a failure context from task execution result.
pub fn create_failure_context(
    task_description: &str,
    error_message: &str,
    options: Option<FailureContextOptions>,
) -> FailureContext {
    let opts = options.unwrap_or_default();
    FailureContext {
        id: generate_failure_id(),
        task_description: task_description.to_string(),
        attempt_description: opts
            .attempt_description
            .unwrap_or_else(|| task_description.to_string()),
        error_message: error_message.to_string(),
        error_type: classify_error(error_message),
        files_involved: opts.files_involved.unwrap_or_default(),
        code_written: opts.code_written,
        skills_used: opts.skills_used,
        attempt_number: opts.attempt_number.unwrap_or(1),
        duration_ms: opts.duration_ms,
        timestamp: chrono::Utc::now().to_rfc3339(),
        project_id: opts.project_id,
    }
}

// ============================================================================
// Reflection
// ============================================================================

/// A reflection generated after a failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reflection {
    /// Unique ID for this reflection
    pub id: String,
    /// Reference to the failure this reflects on
    pub failure_id: String,
    /// What went wrong (diagnosis)
    pub what_went_wrong: String,
    /// Why it went wrong (root cause)
    pub why_it_went_wrong: String,
    /// What to try differently (action plan)
    pub what_to_try_next: String,
    /// Specific fix suggestion (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
    /// Lessons learned (for memory)
    pub lessons_learned: Vec<String>,
    /// Confidence in this reflection (0-1)
    pub confidence: f64,
    /// Whether this reflection led to success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub led_to_success: Option<bool>,
    /// Timestamp
    pub timestamp: String,
}

impl Default for Reflection {
    fn default() -> Self {
        Self {
            id: generate_reflection_id(),
            failure_id: String::new(),
            what_went_wrong: String::new(),
            why_it_went_wrong: String::new(),
            what_to_try_next: String::new(),
            suggested_fix: None,
            lessons_learned: Vec::new(),
            confidence: 0.7,
            led_to_success: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Options for creating a reflection
#[derive(Debug, Clone)]
pub struct ReflectionData {
    pub what_went_wrong: String,
    pub why_it_went_wrong: String,
    pub what_to_try_next: String,
    pub suggested_fix: Option<String>,
    pub lessons_learned: Option<Vec<String>>,
    pub confidence: Option<f64>,
}

/// Create a reflection from structured data.
pub fn create_reflection(failure_id: &str, data: ReflectionData) -> Reflection {
    Reflection {
        id: generate_reflection_id(),
        failure_id: failure_id.to_string(),
        what_went_wrong: data.what_went_wrong,
        why_it_went_wrong: data.why_it_went_wrong,
        what_to_try_next: data.what_to_try_next,
        suggested_fix: data.suggested_fix,
        lessons_learned: data.lessons_learned.unwrap_or_default(),
        confidence: data.confidence.unwrap_or(0.7),
        led_to_success: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}

// ============================================================================
// Reflection History
// ============================================================================

/// History of reflections for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflectionHistory {
    /// Task being worked on
    pub task_description: String,
    /// All failures encountered
    pub failures: Vec<FailureContext>,
    /// All reflections generated
    pub reflections: Vec<Reflection>,
    /// Whether the task was eventually successful
    pub succeeded: bool,
    /// Total attempts
    pub total_attempts: u32,
    /// Successful reflection (if task succeeded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub successful_reflection_id: Option<String>,
}

// ============================================================================
// Skill Extraction
// ============================================================================

/// Extract a skill pattern from a successful reflection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedSkillPattern {
    /// Suggested skill name
    pub name: String,
    /// Description of what the skill does
    pub description: String,
    /// Error patterns this skill addresses
    pub error_patterns: Vec<String>,
    /// The fix/solution
    pub solution: String,
    /// Category for the skill
    pub category: String,
    /// Source reflection ID
    pub source_reflection_id: String,
}

// ============================================================================
// Prompt Building
// ============================================================================

/// Build a prompt for generating a reflection from a failure.
pub fn build_reflection_prompt(context: &FailureContext) -> String {
    let mut parts = vec![
        "You are a coding assistant reflecting on a failed attempt. Analyze the failure and provide actionable insights.".to_string(),
        String::new(),
        "## Failed Task".to_string(),
        context.task_description.clone(),
        String::new(),
        "## What Was Tried".to_string(),
        context.attempt_description.clone(),
        String::new(),
        "## Error".to_string(),
        format!("Type: {:?}", context.error_type),
        context.error_message.clone(),
        String::new(),
        "## Files Involved".to_string(),
        if context.files_involved.is_empty() {
            "None specified".to_string()
        } else {
            context.files_involved.join(", ")
        },
    ];

    if let Some(ref code) = context.code_written {
        parts.push(String::new());
        parts.push("## Code Written".to_string());
        parts.push("```".to_string());
        parts.push(code.clone());
        parts.push("```".to_string());
    }

    if let Some(ref skills) = context.skills_used {
        if !skills.is_empty() {
            parts.push(String::new());
            parts.push("## Skills Used".to_string());
            parts.push(skills.join(", "));
        }
    }

    parts.push(String::new());
    parts.push("## Your Reflection".to_string());
    parts.push("Provide a structured reflection with:".to_string());
    parts.push("1. **What went wrong**: Diagnose the specific issue".to_string());
    parts.push("2. **Why it went wrong**: Identify the root cause".to_string());
    parts.push("3. **What to try next**: Concrete action plan".to_string());
    parts.push("4. **Suggested fix**: If applicable, provide the exact fix".to_string());
    parts.push("5. **Lessons learned**: What to remember for future tasks".to_string());
    parts.push(String::new());
    parts.push("Be specific and actionable. Focus on what can be done differently.".to_string());

    parts.join("\n")
}

/// Format reflections for injection into the system prompt.
pub fn format_reflections_for_prompt(reflections: &[Reflection]) -> String {
    if reflections.is_empty() {
        return String::new();
    }

    let formatted: Vec<String> = reflections
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut lines = vec![
                format!("### Reflection {}", i + 1),
                format!("**What went wrong**: {}", r.what_went_wrong),
                format!("**Why**: {}", r.why_it_went_wrong),
                format!("**What to try**: {}", r.what_to_try_next),
            ];

            if let Some(ref fix) = r.suggested_fix {
                lines.push(format!("**Suggested fix**: {}", fix));
            }

            if !r.lessons_learned.is_empty() {
                lines.push(format!("**Lessons**: {}", r.lessons_learned.join("; ")));
            }

            lines.join("\n")
        })
        .collect();

    [
        "## Previous Attempt Reflections",
        "",
        "You previously attempted this task and encountered issues. Here are reflections to guide your next attempt:",
        "",
        &formatted.join("\n\n"),
        "",
        "Use these insights to avoid repeating the same mistakes.",
    ]
    .join("\n")
}

/// Prompt for extracting a skill from a successful reflection.
pub fn build_skill_extraction_prompt(reflection: &Reflection, failure: &FailureContext) -> String {
    let mut lines = vec![
        "You are extracting a reusable skill from a successful debugging reflection.".to_string(),
        String::new(),
        "## Original Error".to_string(),
        format!("Type: {:?}", failure.error_type),
        failure.error_message.clone(),
        String::new(),
        "## Successful Reflection".to_string(),
        format!("What went wrong: {}", reflection.what_went_wrong),
        format!("Why: {}", reflection.why_it_went_wrong),
        format!("Solution: {}", reflection.what_to_try_next),
    ];

    if let Some(ref fix) = reflection.suggested_fix {
        lines.push(format!("Fix: {}", fix));
    }

    lines.push(String::new());
    lines.push("## Extract Skill".to_string());
    lines.push("Create a reusable skill pattern with:".to_string());
    lines.push("1. **name**: Short descriptive name (e.g., 'Fix Missing Import')".to_string());
    lines.push("2. **description**: What the skill does".to_string());
    lines
        .push("3. **errorPatterns**: List of error message patterns that trigger this skill".to_string());
    lines.push("4. **solution**: The fix procedure as code or steps".to_string());
    lines.push("5. **category**: One of: debugging, testing, refactoring, git, build".to_string());
    lines.push(String::new());
    lines.push("Output as JSON.".to_string());

    lines.join("\n")
}

// ============================================================================
// Heuristic Reflection Generator
// ============================================================================

/// Generate heuristic-based reflection for common error types.
/// Useful when FM is unavailable or for simple errors.
pub fn generate_heuristic_reflection(failure: &FailureContext) -> Reflection {
    let (what_went_wrong, why_it_went_wrong, what_to_try_next, suggested_fix, lessons) =
        match failure.error_type {
            ErrorType::ImportError => (
                "Failed to import a required module or symbol.".to_string(),
                "The import path may be wrong, or the symbol isn't exported.".to_string(),
                "Search for the correct export location and fix the import.".to_string(),
                Some("Use grep to find where the symbol is exported, then update the import.".to_string()),
                vec!["Always verify import paths exist before using them.".to_string()],
            ),
            ErrorType::TypeError => (
                "TypeScript type mismatch between expected and actual types.".to_string(),
                "The types don't align - could be missing properties, wrong types, or inference issues.".to_string(),
                "Check the type definition and ensure the value matches exactly.".to_string(),
                Some("Add explicit type annotations or use type assertions where appropriate.".to_string()),
                vec!["Check type definitions before using values.".to_string()],
            ),
            ErrorType::SyntaxError => (
                "Code has a syntax error that prevents parsing.".to_string(),
                "Missing or extra brackets, quotes, or invalid syntax.".to_string(),
                "Review the code around the error location for syntax issues.".to_string(),
                None,
                vec!["Use an editor with syntax highlighting to catch errors early.".to_string()],
            ),
            ErrorType::TestFailure => (
                "A test assertion failed - expected value doesn't match actual.".to_string(),
                "Either the test expectation is wrong or the implementation is incorrect.".to_string(),
                "Check both the test and the implementation to find the mismatch.".to_string(),
                None,
                vec!["Review test expectations carefully before implementing.".to_string()],
            ),
            ErrorType::Timeout => (
                "The operation took too long and timed out.".to_string(),
                "Could be an infinite loop, waiting for unavailable resource, or genuinely slow operation.".to_string(),
                "Add logging to identify where time is spent, or increase timeout if legitimate.".to_string(),
                None,
                vec!["Add timeout handling and progress logging for long operations.".to_string()],
            ),
            ErrorType::RuntimeError => (
                "A runtime error occurred during execution.".to_string(),
                "Null/undefined access, invalid operation, or unhandled edge case.".to_string(),
                "Add null checks and defensive coding around the error location.".to_string(),
                None,
                vec!["Handle edge cases and add null checks proactively.".to_string()],
            ),
            ErrorType::BuildError => (
                "Build/compilation failed.".to_string(),
                "Configuration issue, missing dependency, or incompatible code.".to_string(),
                "Check build logs for specific errors and fix them one by one.".to_string(),
                None,
                vec!["Run builds frequently to catch errors early.".to_string()],
            ),
            ErrorType::ToolError => (
                "A tool or command failed to execute.".to_string(),
                "Tool may not be available, or arguments may be incorrect.".to_string(),
                "Verify the tool is installed and arguments are correct.".to_string(),
                None,
                vec!["Test tool commands in isolation before using in automation.".to_string()],
            ),
            ErrorType::LogicError => (
                "The code produces wrong output or behavior.".to_string(),
                "Logic bug in the implementation.".to_string(),
                "Add debugging output to trace the execution and find where it diverges.".to_string(),
                None,
                vec!["Write tests for expected behavior before implementing.".to_string()],
            ),
            ErrorType::Unknown => {
                let truncated = if failure.error_message.len() > 100 {
                    format!("{}...", &failure.error_message[..100])
                } else {
                    failure.error_message.clone()
                };
                (
                    format!("The task failed with: {}", truncated),
                    "The exact cause is unclear from the error message.".to_string(),
                    "Read the error message carefully and search for similar issues.".to_string(),
                    None,
                    vec!["When unclear, search for the error message online.".to_string()],
                )
            }
        };

    create_reflection(
        &failure.id,
        ReflectionData {
            what_went_wrong,
            why_it_went_wrong,
            what_to_try_next,
            suggested_fix,
            lessons_learned: Some(lessons),
            confidence: Some(0.6), // Lower confidence for heuristic
        },
    )
}

// ============================================================================
// ID Generation
// ============================================================================

/// Generate a unique failure ID.
pub fn generate_failure_id() -> String {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let random: u32 = rand_simple();
    format!("fail-{:x}-{:06x}", timestamp, random & 0xFFFFFF)
}

/// Generate a unique reflection ID.
pub fn generate_reflection_id() -> String {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let random: u32 = rand_simple();
    format!("refl-{:x}-{:06x}", timestamp, random & 0xFFFFFF)
}

/// Simple random number generator (no external dependency)
fn rand_simple() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let nanos = duration.subsec_nanos();
    let secs = duration.as_secs() as u32;
    nanos.wrapping_mul(1103515245).wrapping_add(secs)
}

// ============================================================================
// In-Memory Store
// ============================================================================

/// Simple in-memory store for reflections
#[derive(Debug, Default)]
pub struct ReflexionStore {
    /// Failures keyed by task description hash
    failures: HashMap<String, Vec<FailureContext>>,
    /// Reflections keyed by task description hash
    reflections: HashMap<String, Vec<Reflection>>,
    /// Reflection by ID for quick lookup
    reflection_by_id: HashMap<String, Reflection>,
    /// Failure by ID for quick lookup
    failure_by_id: HashMap<String, FailureContext>,
    /// Number of skills learned
    pub skills_learned: u32,
}

impl ReflexionStore {
    /// Create a new empty store
    pub fn new() -> Self {
        Self::default()
    }

    /// Hash a task description for grouping
    fn hash_task(task_description: &str) -> String {
        let mut hash: i32 = 0;
        for c in task_description.chars() {
            hash = ((hash << 5).wrapping_sub(hash)).wrapping_add(c as i32);
        }
        format!("task-{:x}", hash.unsigned_abs())
    }

    /// Record a failure
    pub fn record_failure(&mut self, failure: FailureContext) {
        let task_key = Self::hash_task(&failure.task_description);
        self.failure_by_id
            .insert(failure.id.clone(), failure.clone());
        self.failures
            .entry(task_key)
            .or_default()
            .push(failure);
    }

    /// Get failures for a task
    pub fn get_failures(&self, task_description: &str) -> Vec<FailureContext> {
        let task_key = Self::hash_task(task_description);
        self.failures.get(&task_key).cloned().unwrap_or_default()
    }

    /// Record a reflection
    pub fn record_reflection(&mut self, reflection: Reflection) {
        // Get the failure to find task description
        if let Some(failure) = self.failure_by_id.get(&reflection.failure_id) {
            let task_key = Self::hash_task(&failure.task_description);
            self.reflection_by_id
                .insert(reflection.id.clone(), reflection.clone());
            self.reflections
                .entry(task_key)
                .or_default()
                .push(reflection);
        }
    }

    /// Get reflections for a task
    pub fn get_reflections(&self, task_description: &str) -> Vec<Reflection> {
        let task_key = Self::hash_task(task_description);
        self.reflections.get(&task_key).cloned().unwrap_or_default()
    }

    /// Get a reflection by ID
    pub fn get_reflection(&self, reflection_id: &str) -> Option<&Reflection> {
        self.reflection_by_id.get(reflection_id)
    }

    /// Get a failure by ID
    pub fn get_failure(&self, failure_id: &str) -> Option<&FailureContext> {
        self.failure_by_id.get(failure_id)
    }

    /// Mark a reflection as successful
    pub fn mark_success(&mut self, reflection_id: &str) {
        if let Some(reflection) = self.reflection_by_id.get_mut(reflection_id) {
            reflection.led_to_success = Some(true);
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> ReflexionStats {
        let total_failures: usize = self.failures.values().map(|f| f.len()).sum();
        let total_reflections: usize = self.reflections.values().map(|r| r.len()).sum();
        let successful_reflections = self
            .reflection_by_id
            .values()
            .filter(|r| r.led_to_success == Some(true))
            .count();

        ReflexionStats {
            total_failures,
            total_reflections,
            successful_reflections,
            skills_learned: self.skills_learned as usize,
        }
    }

    /// Get reflection history for a task
    pub fn get_history(&self, task_description: &str) -> ReflectionHistory {
        let failures = self.get_failures(task_description);
        let reflections = self.get_reflections(task_description);
        let successful = reflections.iter().find(|r| r.led_to_success == Some(true));

        ReflectionHistory {
            task_description: task_description.to_string(),
            failures: failures.clone(),
            reflections: reflections.clone(),
            succeeded: successful.is_some(),
            total_attempts: failures.len() as u32,
            successful_reflection_id: successful.map(|r| r.id.clone()),
        }
    }
}

/// Statistics about the reflexion system
#[derive(Debug, Clone, Default)]
pub struct ReflexionStats {
    pub total_failures: usize,
    pub total_reflections: usize,
    pub successful_reflections: usize,
    pub skills_learned: usize,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_error_type_error() {
        assert_eq!(
            classify_error("Type 'string' is not assignable to type 'number'"),
            ErrorType::TypeError
        );
        assert_eq!(classify_error("error TS2322: type mismatch"), ErrorType::TypeError);
    }

    #[test]
    fn test_classify_error_import_error() {
        assert_eq!(
            classify_error("Cannot find module './foo'"),
            ErrorType::ImportError
        );
        assert_eq!(classify_error("error TS2307: Cannot find module"), ErrorType::ImportError);
    }

    #[test]
    fn test_classify_error_syntax_error() {
        assert_eq!(
            classify_error("Unexpected token '}'"),
            ErrorType::SyntaxError
        );
        assert_eq!(classify_error("Syntax error: parsing failed"), ErrorType::SyntaxError);
    }

    #[test]
    fn test_classify_error_test_failure() {
        assert_eq!(classify_error("Test failed: expected 1"), ErrorType::TestFailure);
        assert_eq!(
            classify_error("expect(foo).toBe(bar) failed"),
            ErrorType::TestFailure
        );
    }

    #[test]
    fn test_classify_error_timeout() {
        assert_eq!(classify_error("Operation timed out"), ErrorType::Timeout);
        assert_eq!(classify_error("Timeout exceeded"), ErrorType::Timeout);
    }

    #[test]
    fn test_classify_error_unknown() {
        assert_eq!(classify_error("Something weird happened"), ErrorType::Unknown);
    }

    #[test]
    fn test_create_failure_context() {
        let ctx = create_failure_context("Fix the bug", "Type 'string' is not assignable to type 'number'", None);
        assert!(!ctx.id.is_empty());
        assert_eq!(ctx.task_description, "Fix the bug");
        assert_eq!(ctx.error_message, "Type 'string' is not assignable to type 'number'");
        assert_eq!(ctx.error_type, ErrorType::TypeError);
    }

    #[test]
    fn test_create_failure_context_with_options() {
        let opts = FailureContextOptions {
            attempt_description: Some("Tried adding a type annotation".to_string()),
            files_involved: Some(vec!["src/foo.ts".to_string()]),
            attempt_number: Some(3),
            ..Default::default()
        };
        let ctx = create_failure_context("Fix the bug", "Type error", Some(opts));
        assert_eq!(ctx.attempt_description, "Tried adding a type annotation");
        assert_eq!(ctx.files_involved, vec!["src/foo.ts"]);
        assert_eq!(ctx.attempt_number, 3);
    }

    #[test]
    fn test_create_reflection() {
        let refl = create_reflection(
            "fail-123",
            ReflectionData {
                what_went_wrong: "Import was wrong".to_string(),
                why_it_went_wrong: "Path was incorrect".to_string(),
                what_to_try_next: "Use the correct path".to_string(),
                suggested_fix: Some("Change './foo' to '../foo'".to_string()),
                lessons_learned: Some(vec!["Check paths carefully".to_string()]),
                confidence: Some(0.9),
            },
        );
        assert!(!refl.id.is_empty());
        assert_eq!(refl.failure_id, "fail-123");
        assert_eq!(refl.what_went_wrong, "Import was wrong");
        assert_eq!(refl.confidence, 0.9);
    }

    #[test]
    fn test_generate_heuristic_reflection() {
        let ctx = create_failure_context("Fix imports", "Cannot find module './bar'", None);
        let refl = generate_heuristic_reflection(&ctx);
        assert!(refl.what_went_wrong.contains("import"));
        assert!(refl.confidence < 0.7); // Heuristic has lower confidence
    }

    #[test]
    fn test_build_reflection_prompt() {
        let ctx = create_failure_context("Fix the bug", "Type error", None);
        let prompt = build_reflection_prompt(&ctx);
        assert!(prompt.contains("Failed Task"));
        assert!(prompt.contains("Fix the bug"));
        assert!(prompt.contains("Type error"));
    }

    #[test]
    fn test_format_reflections_for_prompt_empty() {
        assert_eq!(format_reflections_for_prompt(&[]), "");
    }

    #[test]
    fn test_format_reflections_for_prompt() {
        let refl = create_reflection(
            "fail-123",
            ReflectionData {
                what_went_wrong: "Import was wrong".to_string(),
                why_it_went_wrong: "Path was incorrect".to_string(),
                what_to_try_next: "Use the correct path".to_string(),
                suggested_fix: None,
                lessons_learned: Some(vec!["Check paths".to_string()]),
                confidence: None,
            },
        );
        let prompt = format_reflections_for_prompt(&[refl]);
        assert!(prompt.contains("Previous Attempt Reflections"));
        assert!(prompt.contains("Import was wrong"));
    }

    #[test]
    fn test_reflexion_store() {
        let mut store = ReflexionStore::new();

        // Record failure
        let failure = create_failure_context("Fix bug", "Type error", None);
        let failure_id = failure.id.clone();
        store.record_failure(failure);

        // Check failure was stored
        let failures = store.get_failures("Fix bug");
        assert_eq!(failures.len(), 1);

        // Record reflection
        let refl = create_reflection(
            &failure_id,
            ReflectionData {
                what_went_wrong: "Wrong".to_string(),
                why_it_went_wrong: "Because".to_string(),
                what_to_try_next: "Try this".to_string(),
                suggested_fix: None,
                lessons_learned: None,
                confidence: None,
            },
        );
        let refl_id = refl.id.clone();
        store.record_reflection(refl);

        // Check reflection was stored
        let reflections = store.get_reflections("Fix bug");
        assert_eq!(reflections.len(), 1);

        // Mark success
        store.mark_success(&refl_id);
        let updated = store.get_reflection(&refl_id).unwrap();
        assert_eq!(updated.led_to_success, Some(true));

        // Check stats
        let stats = store.get_stats();
        assert_eq!(stats.total_failures, 1);
        assert_eq!(stats.total_reflections, 1);
        assert_eq!(stats.successful_reflections, 1);
    }

    #[test]
    fn test_get_history() {
        let mut store = ReflexionStore::new();

        let failure = create_failure_context("Fix bug", "Error", None);
        store.record_failure(failure.clone());

        let refl = create_reflection(
            &failure.id,
            ReflectionData {
                what_went_wrong: "Wrong".to_string(),
                why_it_went_wrong: "Because".to_string(),
                what_to_try_next: "Try".to_string(),
                suggested_fix: None,
                lessons_learned: None,
                confidence: None,
            },
        );
        store.record_reflection(refl);

        let history = store.get_history("Fix bug");
        assert_eq!(history.task_description, "Fix bug");
        assert_eq!(history.failures.len(), 1);
        assert_eq!(history.reflections.len(), 1);
        assert!(!history.succeeded);
        assert_eq!(history.total_attempts, 1);
    }
}
