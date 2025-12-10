//! Pytest Formatter Module
//!
//! Converts GeneratedTest objects to executable pytest format.
//! Detects task types from test data and generates appropriate assertions.
//!
//! This module is shared between the testgen CLI and hillclimber.

use regex::Regex;
use crate::{GeneratedTest, TestCategory};

// ============================================================================
// GUARDRAIL: NO TASK-SPECIFIC HARDCODING
//
// This file must NEVER contain:
// - Task IDs (e.g., "regex-log", "path-tracing")
// - Task-specific patterns (e.g., IPv4 format, date format)
// - Task-specific hints (e.g., "use lookahead for IPv4")
// - Task-specific file paths (e.g., "/app/regex.txt")
//
// All knowledge must come from the GeneratedTest objects.
// Task type detection uses keywords and patterns FROM the test data.
// ============================================================================

// ============================================================================
// Task Type Detection
// ============================================================================

/// Signals detected from analyzing test data to determine task type.
#[derive(Debug, Default)]
pub struct TaskSignals {
    /// Whether the task involves regex pattern matching
    pub has_regex_pattern: bool,
    /// Detected output file location (e.g., "/app/regex.txt")
    pub output_file_location: Option<String>,
    /// How the task should be validated
    pub validation_method: ValidationType,
    /// Whether inputs contain multiline data
    pub has_multiline_input: bool,
}

/// Validation method for test assertions.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum ValidationType {
    /// Use re.findall() for regex pattern matching
    RegexMatch,
    /// Use Path.exists() for file existence checks
    FileExists,
    /// Use subprocess/command execution
    #[default]
    CommandOutput,
}

/// Detect task type from test data and task description without hardcoding task IDs.
///
/// Analyzes all tests and the task description to find signals that indicate
/// what kind of task this is, then determines the appropriate validation method.
///
/// The `task_description` parameter is optional but highly recommended - it allows
/// extraction of output file paths that may not appear in the generated tests.
/// This is LEGITIMATE because the path comes from the task itself, not TB2 knowledge.
pub fn detect_task_type(tests: &[GeneratedTest], task_description: Option<&str>) -> TaskSignals {
    let mut signals = TaskSignals::default();

    // Regex for extracting file paths like "/app/regex.txt", "/output/file.py"
    let path_regex = Regex::new(r"/[a-zA-Z0-9/_.-]+\.(txt|py|json|log|sh)").unwrap();

    // FIRST: Try to extract file path from task description (most reliable source)
    // This is legitimate because the path comes from the task itself, not hardcoded.
    if let Some(desc) = task_description {
        // Look for "Save ... in /path" patterns
        let save_regex =
            Regex::new(r"(?i)(?:save|write|output|store)\s+(?:your\s+)?(?:\w+\s+)*(?:in|to|at)\s+(/[/\w.-]+)")
                .unwrap();

        if let Some(caps) = save_regex.captures(desc) {
            if let Some(m) = caps.get(1) {
                signals.output_file_location = Some(m.as_str().to_string());
            }
        }

        // Fall back to general path extraction from description
        if signals.output_file_location.is_none() {
            if let Some(m) = path_regex.find(desc) {
                signals.output_file_location = Some(m.as_str().to_string());
            }
        }

        // Check for regex keywords in description
        let desc_lower = desc.to_lowercase();
        if desc_lower.contains("regex")
            || desc_lower.contains("re.findall")
            || desc_lower.contains("pattern")
        {
            signals.has_regex_pattern = true;
        }
    }

    for test in tests {
        // Combine input and reasoning for keyword scanning
        let text = format!("{} {}", test.input, test.reasoning).to_lowercase();

        // Detect regex-related keywords
        if text.contains("regex")
            || text.contains("pattern")
            || text.contains("re.findall")
            || text.contains("match")
            || text.contains("multiline")
        {
            signals.has_regex_pattern = true;
        }

        // Extract file paths from test input or reasoning (fallback if not in description)
        if signals.output_file_location.is_none() {
            if let Some(m) = path_regex.find(&test.input) {
                signals.output_file_location = Some(m.as_str().to_string());
            } else if let Some(m) = path_regex.find(&test.reasoning) {
                signals.output_file_location = Some(m.as_str().to_string());
            }
        }

        // Check for multiline data
        if test.input.contains('\n') || test.input.contains("\\n") {
            signals.has_multiline_input = true;
        }
    }

    // Determine validation method based on detected signals
    signals.validation_method = if signals.has_regex_pattern {
        ValidationType::RegexMatch
    } else if signals.output_file_location.is_some() {
        ValidationType::FileExists
    } else {
        ValidationType::CommandOutput
    };

    signals
}

/// Extract file path from text using regex.
pub fn extract_path_from_text(text: &str) -> Option<String> {
    let path_regex = Regex::new(r"/[a-zA-Z0-9/_.-]+\.(txt|py|json|log|sh)").unwrap();
    path_regex.find(text).map(|m| m.as_str().to_string())
}

/// Convert Docker paths to relative paths for local execution.
///
/// Terminal-Bench tasks use `/app/` as the container working directory.
/// When running locally, we need to convert these to relative paths
/// since pytest runs from the workspace directory.
///
/// Examples:
/// - `/app/regex.txt` -> `regex.txt`
/// - `/app/subdir/file.py` -> `subdir/file.py`
/// - `/other/path.txt` -> `/other/path.txt` (unchanged, absolute path outside /app/)
pub fn docker_path_to_relative(path: &str) -> String {
    if path.starts_with("/app/") {
        // Strip /app/ prefix - pytest runs from workspace which maps to /app/
        path[5..].to_string()
    } else {
        // Keep other paths as-is
        path.to_string()
    }
}

/// Parse expected output string into Python list format.
///
/// Handles various formats:
/// - "['2023-02-28']" -> ["2023-02-28"]
/// - "2023-02-28" -> ["2023-02-28"]
/// - "\"2023-02-28\"" -> ["2023-02-28"]
/// - "null" or empty -> []
pub fn parse_expected_output(raw: &str) -> String {
    let trimmed = raw.trim();

    // Handle null/empty cases
    if trimmed.is_empty() || trimmed == "null" || trimmed == "None" {
        return "[]".to_string();
    }

    // Already a list format - convert single quotes to double quotes
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return trimmed.replace('\'', "\"");
    }

    // Strip outer quotes if present (e.g., "\"2023-02-28\"" -> "2023-02-28")
    let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    // Single value - wrap in list
    format!("[\"{}\"]", escape_string(unquoted))
}

// ============================================================================
// String Utilities
// ============================================================================

/// Escape string for Python string literal.
pub fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Escape string for Python docstring.
pub fn escape_docstring(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace("\"\"\"", "'''")
        .replace('\n', " ")
}

/// Sanitize test ID to valid Python function name.
pub fn sanitize_function_name(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

/// Get display name for a test category.
pub fn category_display_name(category: &TestCategory) -> &'static str {
    match category {
        TestCategory::AntiCheat => "Anti-Cheat",
        TestCategory::Existence => "Existence",
        TestCategory::Correctness => "Correctness",
        TestCategory::Boundary => "Boundary",
        TestCategory::Integration => "Integration",
        TestCategory::Format => "Format",
        TestCategory::HappyPath => "Happy Path",
        TestCategory::EdgeCase => "Edge Case",
        TestCategory::InvalidInput => "Invalid Input",
    }
}

/// Extract file path from a test input string (simple extraction).
pub fn extract_path(input: &str) -> String {
    if let Some(start) = input.find('/') {
        let rest = &input[start..];
        let end = rest
            .find(|c: char| c.is_whitespace())
            .unwrap_or(rest.len());
        rest[..end].to_string()
    } else {
        input.to_string()
    }
}

// ============================================================================
// Main Formatter
// ============================================================================

/// Convert generated tests to pytest file format.
///
/// # Arguments
///
/// * `tests` - Vector of generated tests from testgen
/// * `task_id` - Task identifier for the header comment
/// * `task_description` - Optional task description to extract output file paths from
///
/// # Returns
///
/// A string containing valid Python pytest code with REAL assertions
pub fn format_as_pytest(
    tests: &[GeneratedTest],
    task_id: &str,
    task_description: Option<&str>,
) -> String {
    let mut output = String::new();

    // Detect task type from test data AND task description
    // The task_description is used to extract output file paths (e.g., "/app/regex.txt")
    // This is LEGITIMATE because the path comes from the task itself, not TB2 knowledge
    let signals = detect_task_type(tests, task_description);

    // Header
    output.push_str("# Generated tests for ");
    output.push_str(task_id);
    output.push_str("\n# Auto-generated by TestGen\n\n");

    // Imports - conditional based on detected task type
    output.push_str("import pytest\n");
    if signals.validation_method == ValidationType::RegexMatch {
        output.push_str("import re\n");
    }
    output.push_str("import os\n");
    if signals.validation_method == ValidationType::CommandOutput {
        output.push_str("import subprocess\n");
    }
    output.push_str("from pathlib import Path\n\n");

    // Helper functions - only include what's needed
    if signals.validation_method == ValidationType::CommandOutput {
        output.push_str(r#"def run_command(cmd):
    """Run a shell command and return output."""
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    return result.stdout.strip()

"#);
    }

    output.push_str(r#"def read_file(path):
    """Read file contents, return None if not exists."""
    try:
        return Path(path).read_text().strip()
    except FileNotFoundError:
        return None

"#);

    // Group tests by category
    let categories = [
        TestCategory::AntiCheat,
        TestCategory::Existence,
        TestCategory::Correctness,
        TestCategory::Boundary,
        TestCategory::Integration,
        TestCategory::Format,
        TestCategory::HappyPath,
        TestCategory::EdgeCase,
        TestCategory::InvalidInput,
    ];

    for category in categories {
        let category_tests: Vec<_> = tests.iter().filter(|t| t.category == category).collect();

        if category_tests.is_empty() {
            continue;
        }

        // Category header
        output.push_str(&format!(
            "\n# ============================================================================\n"
        ));
        output.push_str(&format!("# {} Tests\n", category_display_name(&category)));
        output.push_str(
            "# ============================================================================\n\n",
        );

        for test in category_tests {
            // Generate test function
            let func_name = sanitize_function_name(&test.id);
            let docstring = escape_docstring(&test.reasoning);

            output.push_str(&format!("def test_{}():\n", func_name));
            output.push_str(&format!("    \"\"\"{}\"\"\"\n", docstring));

            // Generate test body based on category, content, AND detected signals
            let body = generate_test_body(test, &category, &signals);
            for line in body.lines() {
                if line.is_empty() {
                    output.push_str("\n");
                } else {
                    output.push_str(&format!("    {}\n", line));
                }
            }
            output.push('\n');
        }
    }

    output
}

// ============================================================================
// Test Body Generators
// ============================================================================

/// Generate the test function body based on test content and detected signals.
fn generate_test_body(test: &GeneratedTest, category: &TestCategory, signals: &TaskSignals) -> String {
    match (category, &signals.validation_method) {
        // Existence tests always check file existence
        (TestCategory::Existence, _) => generate_existence_body(test, signals),

        // Correctness tests with regex validation
        (TestCategory::Correctness, ValidationType::RegexMatch) => {
            generate_correctness_regex(test, signals)
        }

        // Correctness tests with command output
        (TestCategory::Correctness, ValidationType::CommandOutput) => {
            generate_correctness_command(test)
        }

        // Boundary tests use same logic as correctness (based on validation type)
        (TestCategory::Boundary, ValidationType::RegexMatch) => {
            generate_correctness_regex(test, signals)
        }

        (TestCategory::Boundary, ValidationType::CommandOutput) => {
            generate_correctness_command(test)
        }

        // Integration tests with regex
        (TestCategory::Integration, ValidationType::RegexMatch) => {
            generate_integration_regex(test, signals)
        }

        // Integration tests with command
        (TestCategory::Integration, ValidationType::CommandOutput) => {
            generate_integration_command(test)
        }

        // Anti-cheat tests are conceptual
        (TestCategory::AntiCheat, _) => generate_anti_cheat_body(test),

        // Format tests use correctness pattern
        (TestCategory::Format, ValidationType::RegexMatch) => {
            generate_correctness_regex(test, signals)
        }
        (TestCategory::Format, _) => generate_correctness_command(test),

        // Happy path uses correctness pattern
        (TestCategory::HappyPath, ValidationType::RegexMatch) => {
            generate_correctness_regex(test, signals)
        }
        (TestCategory::HappyPath, _) => generate_correctness_command(test),

        // Edge cases use boundary pattern
        (TestCategory::EdgeCase, ValidationType::RegexMatch) => {
            generate_correctness_regex(test, signals)
        }
        (TestCategory::EdgeCase, _) => generate_correctness_command(test),

        // Invalid input tests
        (TestCategory::InvalidInput, ValidationType::RegexMatch) => {
            generate_correctness_regex(test, signals)
        }
        (TestCategory::InvalidInput, _) => generate_correctness_command(test),

        // Fallback for file existence
        (_, ValidationType::FileExists) => generate_existence_body(test, signals),
    }
}

/// Generate body for existence tests.
fn generate_existence_body(test: &GeneratedTest, signals: &TaskSignals) -> String {
    // Use detected file path from test data, or extract from input
    // NOTE: Do NOT hardcode paths like /app/regex.txt - that's TB2-specific cheating
    // Path must come from the test data itself
    let path = if let Some(ref p) = signals.output_file_location {
        p.clone()
    } else if let Some(p) = extract_path_from_text(&test.input) {
        p
    } else if let Some(p) = extract_path_from_text(&test.reasoning) {
        p
    } else {
        extract_path(&test.input)
    };

    // Convert Docker paths to relative for local execution
    let relative_path = docker_path_to_relative(&path);

    if !path.is_empty() && path.contains('/') {
        format!(
            r#"path = Path("{}")
assert path.exists(), f"Expected {{path}} to exist"
assert path.stat().st_size > 0, f"Expected {{path}} to be non-empty""#,
            escape_string(&relative_path)
        )
    } else {
        format!(
            r#"# Input: {}
# This test verifies existence of expected outputs
# Unable to determine specific file path from test data
pass  # TODO: Implement specific check"#,
            escape_string(&test.input)
        )
    }
}

/// Generate body for anti-cheat tests.
fn generate_anti_cheat_body(test: &GeneratedTest) -> String {
    format!(
        r#"# Anti-cheat check: {}
# Verify no prohibited tools/patterns were used
pass  # Anti-cheat validation happens at runtime"#,
        escape_string(&test.input)
    )
}

/// Generate body for correctness tests with regex validation.
///
/// This generates REAL assertions that will fail if the regex is wrong.
fn generate_correctness_regex(test: &GeneratedTest, signals: &TaskSignals) -> String {
    // Get file path from signals - must come from test data, NOT hardcoded
    // NOTE: Do NOT hardcode paths like /app/regex.txt - that's TB2-specific cheating
    // Convert Docker paths to relative for local execution
    let file_path = match &signals.output_file_location {
        Some(p) => docker_path_to_relative(p),
        None => {
            // No path found in test data - generate a skip
            return format!(
                r#"# Test: {}
# Unable to determine regex file path from test data
pytest.skip("Missing regex file path in test data")"#,
                escape_string(&test.reasoning)
            );
        }
    };

    if let Some(expected) = &test.expected_output {
        let expected_parsed = parse_expected_output(expected);

        format!(
            r#"pattern = Path("{}").read_text().strip()
test_input = "{}"
matches = re.findall(pattern, test_input, re.MULTILINE)
expected = {}
assert matches == expected, f"Expected {{expected}}, got {{matches}}""#,
            escape_string(&file_path),
            escape_string(&test.input),
            expected_parsed
        )
    } else {
        // No expected output - verify pattern compiles and runs without error
        format!(
            r#"pattern = Path("{}").read_text().strip()
try:
    re.compile(pattern)
    test_input = "{}"
    matches = re.findall(pattern, test_input, re.MULTILINE)
    # Pattern compiled and executed successfully
except re.error as e:
    pytest.fail(f"Invalid regex pattern: {{e}}")"#,
            escape_string(&file_path),
            escape_string(&test.input)
        )
    }
}

/// Generate body for correctness tests with command output validation.
fn generate_correctness_command(test: &GeneratedTest) -> String {
    if let Some(exp) = &test.expected_output {
        format!(
            r#"# Test: {}
result = run_command("{}")
expected = "{}"
assert result == expected, f"Got {{result}}, expected {{expected}}""#,
            escape_string(&test.reasoning),
            escape_string(&test.input),
            escape_string(exp)
        )
    } else {
        format!(
            r#"# Test: {}
# Running command/check without specific expected output
try:
    result = run_command("{}")
    # Test passed if no exception
except Exception as e:
    pytest.fail(f"Command failed: {{e}}")"#,
            escape_string(&test.reasoning),
            escape_string(&test.input)
        )
    }
}

/// Generate body for integration tests with regex validation.
fn generate_integration_regex(test: &GeneratedTest, signals: &TaskSignals) -> String {
    // Get file path from signals - must come from test data, NOT hardcoded
    // NOTE: Do NOT hardcode paths like /app/regex.txt - that's TB2-specific cheating
    // Convert Docker paths to relative for local execution
    let file_path = match &signals.output_file_location {
        Some(p) => docker_path_to_relative(p),
        None => {
            return format!(
                r#"# Integration test: {}
# Unable to determine regex file path from test data
pytest.skip("Missing regex file path in test data")"#,
                escape_string(&test.reasoning)
            );
        }
    };

    // Format input - handle multiline content
    let input_formatted = if test.input.contains('\n') {
        format!("\"\"\"{}\"\"\"", test.input.trim())
    } else {
        format!("\"{}\"", escape_string(&test.input))
    };

    if let Some(expected) = &test.expected_output {
        let expected_parsed = parse_expected_output(expected);

        format!(
            r#"pattern = Path("{}").read_text().strip()
log_content = {}.strip()
matches = re.findall(pattern, log_content, re.MULTILINE)
expected = {}
assert matches == expected, f"Expected {{expected}}, got {{matches}}""#,
            escape_string(&file_path),
            input_formatted,
            expected_parsed
        )
    } else {
        format!(
            r#"pattern = Path("{}").read_text().strip()
try:
    re.compile(pattern)
    log_content = {}.strip()
    matches = re.findall(pattern, log_content, re.MULTILINE)
    # Integration test executed successfully
except re.error as e:
    pytest.fail(f"Invalid regex pattern: {{e}}")"#,
            escape_string(&file_path),
            input_formatted
        )
    }
}

/// Generate body for integration tests with command output validation.
fn generate_integration_command(test: &GeneratedTest) -> String {
    if let Some(expected) = &test.expected_output {
        format!(
            r#"# Integration test: {}
result = run_command("{}")
expected = "{}"
assert result == expected, f"Integration test failed: got {{result}}, expected {{expected}}""#,
            escape_string(&test.reasoning),
            escape_string(&test.input),
            escape_string(expected)
        )
    } else {
        format!(
            r#"# Integration test: {}
# Full workflow validation
try:
    result = run_command("{}")
    # Integration test executed successfully
except Exception as e:
    pytest.fail(f"Integration test failed: {{e}}")"#,
            escape_string(&test.reasoning),
            escape_string(&test.input)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test(id: &str, input: &str, expected: Option<&str>, reasoning: &str, category: TestCategory) -> GeneratedTest {
        GeneratedTest {
            id: id.to_string(),
            input: input.to_string(),
            expected_output: expected.map(|s| s.to_string()),
            reasoning: reasoning.to_string(),
            category,
            confidence: 0.9,
        }
    }

    #[test]
    fn test_detect_regex_task() {
        let tests = vec![make_test(
            "test1",
            "192.168.1.1 2023-01-15",
            Some("['2023-01-15']"),
            "Match date with regex pattern",
            TestCategory::Correctness,
        )];

        let signals = detect_task_type(&tests, None);
        assert!(signals.has_regex_pattern);
        assert_eq!(signals.validation_method, ValidationType::RegexMatch);
    }

    #[test]
    fn test_parse_expected_output_list() {
        assert_eq!(parse_expected_output("['2023-01-15']"), "[\"2023-01-15\"]");
    }

    #[test]
    fn test_parse_expected_output_null() {
        assert_eq!(parse_expected_output("null"), "[]");
    }

    #[test]
    fn test_parse_expected_output_quoted_single() {
        // FM sometimes outputs quoted strings like "\"2023-02-28\""
        assert_eq!(parse_expected_output("\"2023-02-28\""), "[\"2023-02-28\"]");
    }

    #[test]
    fn test_parse_expected_output_single_quoted() {
        // Also handle single quotes
        assert_eq!(parse_expected_output("'2023-02-28'"), "[\"2023-02-28\"]");
    }

    #[test]
    fn test_parse_expected_output_plain_value() {
        // Plain value without quotes
        assert_eq!(parse_expected_output("2023-02-28"), "[\"2023-02-28\"]");
    }

    #[test]
    fn test_format_as_pytest_regex() {
        // Test data must include file path in reasoning for detection
        let tests = vec![make_test(
            "correct_1",
            "192.168.1.1 2023-01-15",
            Some("['2023-01-15']"),
            "Match date with regex pattern from /output/pattern.txt",
            TestCategory::Correctness,
        )];
        // Pass a task description mentioning the regex file path
        let task_desc = "Write a regex and save it in /output/pattern.txt";
        let output = format_as_pytest(&tests, "regex-test", Some(task_desc));

        // Should include re import
        assert!(output.contains("import re"));
        // Should generate real assertions (path detected from task description)
        assert!(output.contains("re.findall(pattern, test_input"), "Output was: {}", output);
        assert!(output.contains("assert matches == expected"));
        // Should NOT contain pass stubs for tests with expected output
        assert!(!output.contains("pass  # TODO"));
    }

    #[test]
    fn test_detect_task_type_with_description() {
        let tests = vec![make_test(
            "test1",
            "192.168.1.1 2023-01-15",
            Some("['2023-01-15']"),
            "Tests with dates",  // No file path in reasoning
            TestCategory::Correctness,
        )];

        // Without task description - no path found
        let signals = detect_task_type(&tests, None);
        assert!(signals.output_file_location.is_none());

        // With task description - path extracted
        let signals = detect_task_type(&tests, Some("Save your regex in /app/regex.txt"));
        assert_eq!(signals.output_file_location, Some("/app/regex.txt".to_string()));
        assert!(signals.has_regex_pattern);
    }
}
