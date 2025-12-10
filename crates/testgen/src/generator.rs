//! TestGen Generator
//!
//! Iterative test generation engine using FM inference.
//! Generates tests category-by-category with reflection rounds.

use crate::environment::EnvironmentInfo;
use crate::error::{Result, TestGenError};
use crate::types::{
    GeneratedTest, ReflectionAction, ReflectionEntry, TestCategory, TestGenContext,
};
use fm_bridge::{CompletionOptions, FMClient};
use std::collections::HashMap;

// ============================================================================
// Configuration
// ============================================================================

/// Iteration configuration settings
#[derive(Debug, Clone)]
pub struct IterationConfig {
    pub min_tests_per_category: u32,
    pub target_tests_per_category: u32,
    pub max_rounds_per_category: u32,
    pub enable_global_refinement: bool,
    pub min_comprehensiveness_score: f64,
    pub max_global_refinement_rounds: u32,
    pub min_total_tests: u32,
    pub target_total_tests: u32,
    pub max_total_rounds: u32,
    pub max_total_tokens: u32,
    pub max_total_time_ms: u64,
    pub temperature: f64,
}

impl Default for IterationConfig {
    fn default() -> Self {
        Self {
            min_tests_per_category: 2,
            target_tests_per_category: 5,
            max_rounds_per_category: 3,
            enable_global_refinement: true,
            min_comprehensiveness_score: 8.0,
            max_global_refinement_rounds: 2,
            min_total_tests: 15,
            target_total_tests: 30,
            max_total_rounds: 12,
            max_total_tokens: 100000,
            max_total_time_ms: 180000,
            temperature: 0.3,
        }
    }
}

// ============================================================================
// Emitter Interface
// ============================================================================

/// Callback interface for generation progress
pub trait TestGenEmitter: Send + Sync {
    fn on_progress(&self, phase: &str, category: Option<TestCategory>, round: u32, status: &str);
    fn on_test(&self, test: &GeneratedTest);
    fn on_reflection(&self, entry: &ReflectionEntry);
    fn on_complete(&self, total_tests: u32, total_rounds: u32, duration_ms: u64);
    fn on_error(&self, error: &str);
}

/// No-op emitter for when callbacks aren't needed
pub struct NoopEmitter;

impl TestGenEmitter for NoopEmitter {
    fn on_progress(&self, _: &str, _: Option<TestCategory>, _: u32, _: &str) {}
    fn on_test(&self, _: &GeneratedTest) {}
    fn on_reflection(&self, _: &ReflectionEntry) {}
    fn on_complete(&self, _: u32, _: u32, _: u64) {}
    fn on_error(&self, _: &str) {}
}

// ============================================================================
// Generator State
// ============================================================================

#[derive(Debug, Default)]
struct GeneratorState {
    tests: HashMap<TestCategory, Vec<GeneratedTest>>,
    reflections: Vec<ReflectionEntry>,
    category_rounds: HashMap<String, u32>,
    total_rounds: u32,
    total_tokens_used: u32,
    comprehensiveness_score: Option<f64>,
}

impl GeneratorState {
    fn new() -> Self {
        Self::default()
    }

    fn get_tests(&self, category: TestCategory) -> &[GeneratedTest] {
        self.tests.get(&category).map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn add_test(&mut self, category: TestCategory, test: GeneratedTest) {
        self.tests.entry(category).or_default().push(test);
    }

    fn count_tests(&self, category: TestCategory) -> u32 {
        self.tests.get(&category).map(|v| v.len() as u32).unwrap_or(0)
    }

    fn total_tests(&self) -> u32 {
        self.tests.values().map(|v| v.len() as u32).sum()
    }

    fn is_category_complete(&self, category: TestCategory, config: &IterationConfig) -> bool {
        let count = self.count_tests(category);
        let rounds = self.category_rounds.get(category.as_str()).copied().unwrap_or(0);
        count >= config.min_tests_per_category || rounds >= config.max_rounds_per_category
    }

    fn should_reflect(&self, category: TestCategory, config: &IterationConfig) -> bool {
        let count = self.count_tests(category);
        let rounds = self.category_rounds.get(category.as_str()).copied().unwrap_or(0);
        rounds < config.max_rounds_per_category && count < config.target_tests_per_category
    }

    fn into_result(self) -> GenerationResult {
        let all_tests: Vec<GeneratedTest> = self.tests.into_values().flatten().collect();
        GenerationResult {
            tests: all_tests,
            reflections: self.reflections,
            category_rounds: self.category_rounds,
            total_tokens_used: self.total_tokens_used,
            comprehensiveness_score: self.comprehensiveness_score,
        }
    }
}

/// Result of test generation
#[derive(Debug)]
pub struct GenerationResult {
    pub tests: Vec<GeneratedTest>,
    pub reflections: Vec<ReflectionEntry>,
    pub category_rounds: HashMap<String, u32>,
    pub total_tokens_used: u32,
    pub comprehensiveness_score: Option<f64>,
}

// ============================================================================
// Test Generator
// ============================================================================

/// Test generator using FM inference
pub struct TestGenerator {
    client: FMClient,
    config: IterationConfig,
}

impl TestGenerator {
    /// Create a new generator with default config
    pub fn new(client: FMClient) -> Self {
        Self {
            client,
            config: IterationConfig::default(),
        }
    }

    /// Create a generator with custom config
    pub fn with_config(client: FMClient, config: IterationConfig) -> Self {
        Self { client, config }
    }

    /// Get categories for a given context
    pub fn get_categories_for_context(
        context: TestGenContext,
        custom_categories: Option<&[TestCategory]>,
    ) -> Vec<TestCategory> {
        match context {
            TestGenContext::Benchmark => vec![
                TestCategory::AntiCheat,
                TestCategory::Existence,
                TestCategory::Correctness,
                TestCategory::Boundary,
                TestCategory::Integration,
            ],
            TestGenContext::Commander => vec![
                TestCategory::Existence,
                TestCategory::Correctness,
                TestCategory::Boundary,
            ],
            TestGenContext::MechaCoder => {
                vec![TestCategory::Correctness, TestCategory::Boundary]
            }
            TestGenContext::Custom => custom_categories
                .map(|c| c.to_vec())
                .unwrap_or_else(|| vec![TestCategory::Correctness]),
        }
    }

    /// Generate tests for a single category round
    pub async fn generate_for_category(
        &self,
        task_description: &str,
        task_id: &str,
        environment: &EnvironmentInfo,
        category: TestCategory,
        existing_tests: &[GeneratedTest],
        round: u32,
    ) -> Result<(Vec<GeneratedTest>, u32)> {
        let prompt = build_category_prompt(
            task_description,
            task_id,
            environment,
            category,
            existing_tests,
            round,
        );

        let response = self
            .client
            .complete(
                prompt,
                Some(CompletionOptions {
                    temperature: Some(self.config.temperature as f32),
                    max_tokens: Some(2048),
                    ..Default::default()
                }),
            )
            .await
            .map_err(TestGenError::FmBridge)?;

        let content = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let tokens = response
            .usage
            .as_ref()
            .and_then(|u| u.total_tokens)
            .unwrap_or(0);

        let tests = parse_tests_response(&content, category);

        Ok((tests, tokens))
    }

    /// Reflect on a category to identify gaps
    pub async fn reflect_on_category(
        &self,
        category: TestCategory,
        existing_tests: &[GeneratedTest],
    ) -> Result<(String, u32)> {
        let prompt = format!(
            r#"Review the {} existing tests for category {}:

{}

What edge cases or scenarios are missing? Provide a brief reflection (1-2 sentences)."#,
            existing_tests.len(),
            category,
            existing_tests
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{}. {}: {}", i + 1, t.id, t.reasoning))
                .collect::<Vec<_>>()
                .join("\n")
        );

        let response = self
            .client
            .complete(
                prompt,
                Some(CompletionOptions {
                    temperature: Some(0.3),
                    max_tokens: Some(512),
                    ..Default::default()
                }),
            )
            .await
            .map_err(TestGenError::FmBridge)?;

        let content = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let tokens = response
            .usage
            .as_ref()
            .and_then(|u| u.total_tokens)
            .unwrap_or(0);

        Ok((content, tokens))
    }

    /// Run full iterative generation
    pub async fn generate_iteratively(
        &self,
        task_description: &str,
        task_id: &str,
        environment: &EnvironmentInfo,
        context: TestGenContext,
        emitter: &dyn TestGenEmitter,
    ) -> Result<GenerationResult> {
        let mut state = GeneratorState::new();
        let start_time = std::time::Instant::now();

        let categories = Self::get_categories_for_context(context, None);

        emitter.on_progress(
            "category_generation",
            None,
            0,
            &format!("Analyzing task: \"{}\"", truncate(task_description, 60)),
        );

        // Phase 1: Category-based iteration
        for category in &categories {
            let mut round = 1;

            while !state.is_category_complete(*category, &self.config)
                && round <= self.config.max_rounds_per_category
                && state.total_rounds < self.config.max_total_rounds
                && state.total_tokens_used < self.config.max_total_tokens
            {
                emitter.on_progress(
                    "category_generation",
                    Some(*category),
                    round,
                    &format!("Generating {} tests", category),
                );

                // Generate tests for this category/round
                let (tests, tokens) = self
                    .generate_for_category(
                        task_description,
                        task_id,
                        environment,
                        *category,
                        state.get_tests(*category),
                        round,
                    )
                    .await?;

                state.total_tokens_used += tokens;

                // Check token limit
                if state.total_tokens_used >= self.config.max_total_tokens {
                    tracing::warn!(
                        "Token limit reached: {} >= {}",
                        state.total_tokens_used,
                        self.config.max_total_tokens
                    );
                    break;
                }

                // Emit and store tests
                for test in tests {
                    emitter.on_test(&test);
                    state.add_test(*category, test);
                }

                // Reflect if needed
                if state.should_reflect(*category, &self.config) {
                    let (reflection_text, reflection_tokens) = self
                        .reflect_on_category(*category, state.get_tests(*category))
                        .await?;

                    state.total_tokens_used += reflection_tokens;

                    let entry = ReflectionEntry {
                        category: Some(*category),
                        reflection_text,
                        action: ReflectionAction::Refining,
                    };
                    emitter.on_reflection(&entry);
                    state.reflections.push(entry);
                }

                state
                    .category_rounds
                    .insert(category.as_str().to_string(), round);
                round += 1;
                state.total_rounds += 1;

                // Check time limit
                if start_time.elapsed().as_millis() as u64 > self.config.max_total_time_ms {
                    tracing::warn!("Time limit reached");
                    break;
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let total_tests = state.total_tests();
        let total_rounds = state.total_rounds;

        emitter.on_complete(total_tests, total_rounds, duration_ms);

        Ok(state.into_result())
    }
}

// ============================================================================
// Prompt Building
// ============================================================================

fn build_category_prompt(
    task_description: &str,
    _task_id: &str,
    environment: &EnvironmentInfo,
    category: TestCategory,
    existing_tests: &[GeneratedTest],
    round: u32,
) -> String {
    // Extract output file path from task description
    // Look for patterns like "Save ... in /path", "Write to /path", file paths in code examples
    let output_file_context = extract_output_file_context(task_description);

    let existing_tests_text = if !existing_tests.is_empty() {
        format!(
            "\n## Existing Tests for {}\n{}",
            category,
            existing_tests
                .iter()
                .enumerate()
                .map(|(i, t)| format!(
                    "{}. {}: {} -> {}",
                    i + 1,
                    t.id,
                    t.input,
                    t.expected_output.as_deref().unwrap_or("null")
                ))
                .collect::<Vec<_>>()
                .join("\n")
        )
    } else {
        String::new()
    };

    let reflection_prompt = if round > 1 {
        format!(
            "\n## Reflection\nYou've already generated {} tests for {}. What edge cases or scenarios are still missing? Generate 1-3 additional tests that fill gaps.",
            existing_tests.len(),
            category
        )
    } else {
        String::new()
    };

    let prohibited_tools = if !environment.tools.prohibited.is_empty() {
        environment
            .tools
            .prohibited
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        "None".to_string()
    };

    // Detect if this is a regex task to add format-specific guidance
    let task_lower = task_description.to_lowercase();
    let format_requirements = if task_lower.contains("regex")
        || task_lower.contains("re.findall")
        || task_lower.contains("pattern")
    {
        r#"

## CRITICAL: Output Format Requirements
- expectedOutput MUST be a JSON array of strings
- Example: ["2023-01-15"] or ["2023-02-28"]
- Python's re.findall() returns a list of strings
- Use [] (empty array) for non-matching cases, NOT null
- Input MUST be a concrete log line, not "test input with..." descriptions
- Date format is YYYY-MM-DD (e.g., "2023-01-15"), NOT timestamps with time"#
    } else {
        ""
    };

    // Build output file context section if we found paths
    let output_file_section = if !output_file_context.is_empty() {
        format!(
            r#"

## CRITICAL: Solution Output Location
The task specifies the solution should be saved to: {}

YOU MUST include this file path in your test reasoning field!
Example reasoning: "Tests regex from {} with single date and IPv4"

Tests need to know where to read the solution from. Without the path in reasoning, tests cannot execute."#,
            output_file_context, output_file_context
        )
    } else {
        String::new()
    };

    format!(
        r#"You are generating test cases to verify a solution works correctly.

## What We're Testing
Task: {}

## Test Category: {}
{}

## Context
- Platform: {}
- Prohibited Tools: {}
- Files: {} files, {} previews
{}{}{}{}

## Your Task
Generate 2-5 test cases for the {} category. Each test should:
1. Have a CONCRETE input (actual test data, not descriptions)
2. Have a CONCRETE expected output (never null for correctness tests)
3. Include reasoning (why this test matters)

## Output Format
Generate tests as JSON array:
[
  {{
    "id": "{}_1",
    "input": "CONCRETE test data here (e.g., '192.168.1.1 2023-01-15')",
    "expectedOutput": "['2023-01-15']",
    "reasoning": "why this test is important",
    "confidence": 0.9
  }}
]

IMPORTANT:
- Input must be ACTUAL data, not descriptions like "test input with dates"
- expectedOutput must be CONCRETE, not null (use [] for no-match cases)
- For regex tasks, expectedOutput should be a JSON array of strings

Respond with valid JSON array only. No markdown, no explanation."#,
        task_description,
        category,
        get_category_description(category),
        environment.platform.platform_type,
        prohibited_tools,
        environment.files.listing.len(),
        environment.files.task_files.len(),
        existing_tests_text,
        reflection_prompt,
        format_requirements,
        output_file_section,
        category,
        category.as_str()
    )
}

/// Extract output file path from task description
/// Looks for patterns like "Save ... in /path", "Write to /path", etc.
fn extract_output_file_context(task_description: &str) -> String {
    // Use regex to find file paths in the task description
    // Pattern: looks for /app/... or /path/... style paths
    let path_regex = regex::Regex::new(r"(/(?:app|tmp|home|var|usr|opt)[/\w.-]+\.\w+)")
        .expect("Invalid regex");

    // Also look for "Save ... in /path" or "Write to /path" patterns
    let save_regex = regex::Regex::new(r"(?i)(?:save|write|output|store)\s+(?:your\s+)?(?:\w+\s+)*(?:in|to|at)\s+(/[/\w.-]+)")
        .expect("Invalid regex");

    // Try save/write pattern first (more specific)
    if let Some(caps) = save_regex.captures(task_description) {
        if let Some(m) = caps.get(1) {
            return m.as_str().to_string();
        }
    }

    // Fall back to general path detection
    if let Some(m) = path_regex.find(task_description) {
        return m.as_str().to_string();
    }

    String::new()
}

fn get_category_description(category: TestCategory) -> &'static str {
    match category {
        TestCategory::AntiCheat => {
            r#"Verify that the solution follows the rules and doesn't use forbidden tools or shortcuts.

What we're checking:
- If certain tools are prohibited (e.g., "don't use Python"), verify they're not used
- If the task says "implement from scratch", verify no pre-built solutions are used
- Catch attempts to game the system or take shortcuts"#
        }
        TestCategory::Existence => {
            r#"Test that the solution produces the required outputs.

What we're checking:
- Does the output file exist where it should?
- Is the file non-empty (not just created but actually has content)?
- Are files created in the correct location?"#
        }
        TestCategory::Correctness => {
            r#"Test that the solution works correctly for normal inputs.

What we're checking:
- Does it produce the right output for typical inputs?
- Does the output format match what's expected?
- Does it handle the main use case correctly?"#
        }
        TestCategory::Boundary => {
            r#"Test edge cases and limits mentioned in the task.

What we're checking:
- Minimum and maximum values (if ranges are mentioned)
- Values just outside valid ranges (should fail gracefully)
- Edge cases like empty input, single item, maximum size"#
        }
        TestCategory::Integration => {
            r#"Test how the solution works with the rest of the system.

What we're checking:
- Does it work correctly with existing files?
- Does it handle multi-step processes correctly?
- Does it integrate properly with other components?"#
        }
        _ => "Test for this category.",
    }
}

// ============================================================================
// Response Parsing
// ============================================================================

fn parse_tests_response(content: &str, category: TestCategory) -> Vec<GeneratedTest> {
    let mut json_content = content.trim();

    // Strip markdown code blocks
    if json_content.starts_with("```") {
        if let Some(start) = json_content.find('[') {
            json_content = &json_content[start..];
        }
        if let Some(end) = json_content.rfind(']') {
            json_content = &json_content[..=end];
        }
    }

    let parsed: std::result::Result<Vec<serde_json::Value>, _> = serde_json::from_str(json_content);

    match parsed {
        Ok(tests) => tests
            .into_iter()
            .filter_map(|t| {
                let id = t.get("id")?.as_str()?.to_string();
                let input = t.get("input")?.as_str()?.to_string();
                let expected_output = t
                    .get("expectedOutput")
                    .and_then(|v| {
                        if v.is_null() {
                            None
                        } else {
                            v.as_str().map(String::from)
                        }
                    })
                    .filter(|s| s != "null" && s != "None" && !s.is_empty());
                let reasoning = t
                    .get("reasoning")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let confidence = t.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.5);

                Some(GeneratedTest {
                    id,
                    input,
                    expected_output,
                    reasoning,
                    category,
                    confidence,
                })
            })
            .collect(),
        Err(e) => {
            tracing::warn!("Failed to parse tests response: {}", e);
            Vec::new()
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tests_response() {
        let json = r#"[
            {
                "id": "correctness_1",
                "input": "test input",
                "expectedOutput": "expected",
                "reasoning": "test reasoning",
                "confidence": 0.9
            }
        ]"#;

        let tests = parse_tests_response(json, TestCategory::Correctness);
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].id, "correctness_1");
        assert_eq!(tests[0].input, "test input");
        assert_eq!(tests[0].expected_output, Some("expected".to_string()));
    }

    #[test]
    fn test_parse_tests_response_with_null() {
        let json = r#"[
            {
                "id": "test_1",
                "input": "test",
                "expectedOutput": null,
                "reasoning": "test",
                "confidence": 0.8
            }
        ]"#;

        let tests = parse_tests_response(json, TestCategory::Correctness);
        assert_eq!(tests.len(), 1);
        assert!(tests[0].expected_output.is_none());
    }

    #[test]
    fn test_get_categories_for_context() {
        let benchmark_cats = TestGenerator::get_categories_for_context(TestGenContext::Benchmark, None);
        assert_eq!(benchmark_cats.len(), 5);
        assert!(benchmark_cats.contains(&TestCategory::AntiCheat));

        let commander_cats = TestGenerator::get_categories_for_context(TestGenContext::Commander, None);
        assert_eq!(commander_cats.len(), 3);
        assert!(!commander_cats.contains(&TestCategory::AntiCheat));

        let mecha_cats = TestGenerator::get_categories_for_context(TestGenContext::MechaCoder, None);
        assert_eq!(mecha_cats.len(), 2);
    }

    #[test]
    fn test_generator_state() {
        let mut state = GeneratorState::new();
        let config = IterationConfig::default();

        assert!(!state.is_category_complete(TestCategory::Correctness, &config));

        // Add 2 tests (min required)
        state.add_test(
            TestCategory::Correctness,
            GeneratedTest {
                id: "1".to_string(),
                input: "test".to_string(),
                expected_output: None,
                reasoning: "test".to_string(),
                category: TestCategory::Correctness,
                confidence: 0.9,
            },
        );
        state.add_test(
            TestCategory::Correctness,
            GeneratedTest {
                id: "2".to_string(),
                input: "test".to_string(),
                expected_output: None,
                reasoning: "test".to_string(),
                category: TestCategory::Correctness,
                confidence: 0.9,
            },
        );

        assert!(state.is_category_complete(TestCategory::Correctness, &config));
        assert_eq!(state.total_tests(), 2);
    }
}
