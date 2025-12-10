//! TestGen Analyzer
//!
//! Programmatic analysis of test generation runs to extract quality metrics.
//! Used by the evolution system to evaluate config performance.

use crate::environment::EnvironmentInfo;
use crate::types::{GeneratedTest, ReflectionEntry, ReflectionAction, TestCategory, TestGenAnalysis};
use regex::Regex;
use std::collections::{HashMap, HashSet};

/// Trajectory data for analysis
#[derive(Debug, Clone)]
pub struct TestGenTrajectory {
    pub session_id: String,
    pub task_id: String,
    pub task_description: String,
    pub total_tests: u32,
    pub total_rounds: u32,
    pub category_rounds: HashMap<String, u32>,
    pub comprehensiveness_score: Option<f64>,
    pub total_tokens_used: u32,
    pub duration_ms: u64,
    pub tests: Vec<GeneratedTest>,
    pub reflections: Vec<ReflectionEntry>,
    pub environment: EnvironmentInfo,
    pub uncertainties: Vec<String>,
}

/// Analyze a complete test generation trajectory.
pub fn analyze_testgen_run(trajectory: &TestGenTrajectory) -> TestGenAnalysis {
    let category_dist = analyze_category_distribution(&trajectory.tests);
    let balance = calculate_category_balance(&category_dist);
    let anti_cheat = analyze_anti_cheat_coverage(
        &trajectory.tests,
        &trajectory.environment,
        &trajectory.task_description,
    );
    let param_discovery = analyze_parameter_discovery(&trajectory.tests, &trajectory.environment);
    let reflection_eff = analyze_reflection_effectiveness(
        &trajectory.reflections,
        &trajectory.tests,
        &trajectory.category_rounds,
    );
    let token_eff = analyze_token_efficiency(
        trajectory.total_tokens_used,
        trajectory.comprehensiveness_score.unwrap_or(5.0),
    );

    TestGenAnalysis {
        category_distribution: category_dist,
        category_balance: balance,
        anti_cheat_coverage: anti_cheat,
        parameter_discovery: param_discovery,
        reflection_effectiveness: reflection_eff,
        token_efficiency: token_eff,
        overall_score: 0, // Will be computed by scoring module
    }
}

// ============================================================================
// Category Distribution Analysis
// ============================================================================

/// Count tests by category.
pub fn analyze_category_distribution(tests: &[GeneratedTest]) -> HashMap<TestCategory, u32> {
    let mut distribution = HashMap::new();

    // Initialize all primary categories
    for category in TestCategory::primary_categories() {
        distribution.insert(*category, 0);
    }

    // Count tests
    for test in tests {
        *distribution.entry(test.category).or_insert(0) += 1;
    }

    distribution
}

/// Calculate how balanced the category distribution is.
/// Returns 0-1, where 1 = perfectly balanced (equal counts per category).
pub fn calculate_category_balance(distribution: &HashMap<TestCategory, u32>) -> f64 {
    if distribution.is_empty() {
        return 0.0;
    }

    let counts: Vec<f64> = distribution.values().map(|&c| c as f64).collect();
    let total: f64 = counts.iter().sum();

    if total == 0.0 {
        return 0.0;
    }

    let num_categories = counts.len() as f64;
    let ideal = total / num_categories;

    // Calculate variance
    let variance: f64 =
        counts.iter().map(|&c| (c - ideal).powi(2)).sum::<f64>() / num_categories;

    // Normalize: 0 variance = 1.0, max variance = 0.0
    let max_variance = ideal.powi(2) * (num_categories - 1.0);

    if max_variance > 0.0 {
        (1.0 - variance / max_variance).max(0.0)
    } else {
        1.0
    }
}

// ============================================================================
// Anti-Cheat Coverage Analysis
// ============================================================================

/// Analyze how well anti-cheat tests cover prohibited tools.
/// Returns 0-1, where 1 = all prohibited tools have anti-cheat tests.
pub fn analyze_anti_cheat_coverage(
    tests: &[GeneratedTest],
    environment: &EnvironmentInfo,
    _task_description: &str,
) -> f64 {
    let prohibited_tools = &environment.tools.prohibited;

    if prohibited_tools.is_empty() {
        // No prohibited tools = no anti-cheat needed = perfect coverage
        return 1.0;
    }

    // Find anti-cheat tests (check reasoning/input for anti-cheat keywords)
    let anti_cheat_tests: Vec<_> = tests
        .iter()
        .filter(|t| {
            let reasoning_lower = t.reasoning.to_lowercase();
            let input_lower = t.input.to_lowercase();
            reasoning_lower.contains("anti-cheat")
                || reasoning_lower.contains("prohibited")
                || input_lower.contains("prohibited")
        })
        .collect();

    if anti_cheat_tests.is_empty() {
        // Prohibited tools exist but no anti-cheat tests = 0 coverage
        return 0.0;
    }

    // Check if anti-cheat tests mention prohibited tools
    let tool_names: Vec<String> = prohibited_tools.iter().map(|t| t.name.to_lowercase()).collect();

    let mut covered_tools = HashSet::new();

    for test in &anti_cheat_tests {
        let test_text = format!(
            "{} {} {}",
            test.input,
            test.expected_output.as_deref().unwrap_or(""),
            test.reasoning
        )
        .to_lowercase();

        for tool_name in &tool_names {
            if test_text.contains(tool_name) {
                covered_tools.insert(tool_name.clone());
            }
        }
    }

    if tool_names.is_empty() {
        1.0
    } else {
        covered_tools.len() as f64 / tool_names.len() as f64
    }
}

// ============================================================================
// Parameter Discovery Analysis
// ============================================================================

/// Analyze how well tests discovered parameters from environment files.
/// Returns 0-1, where 1 = all parameters from file previews are used in tests.
pub fn analyze_parameter_discovery(
    tests: &[GeneratedTest],
    environment: &EnvironmentInfo,
) -> f64 {
    let mut discovered_params = HashSet::new();

    // Simple heuristic: look for function parameters, variable names in previews
    let param_re = Regex::new(r"(?:def|function|let|const|var)\s+(\w+)|\(([^)]+)\)").ok();

    for file in &environment.files.task_files {
        if let Some(ref re) = param_re {
            for cap in re.captures_iter(&file.preview) {
                if let Some(m) = cap.get(1).or_else(|| cap.get(2)) {
                    let param = m.as_str().split(',').next().unwrap_or("").trim();
                    if !param.is_empty() && param.len() > 1 {
                        discovered_params.insert(param.to_lowercase());
                    }
                }
            }
        }
    }

    if discovered_params.is_empty() {
        // No parameters to discover = perfect coverage
        return 1.0;
    }

    // Check if tests use these parameters
    let test_text: String = tests
        .iter()
        .map(|t| {
            format!(
                "{} {} {}",
                t.input,
                t.expected_output.as_deref().unwrap_or(""),
                t.reasoning
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    let used_params = discovered_params
        .iter()
        .filter(|p| test_text.contains(p.as_str()))
        .count();

    used_params as f64 / discovered_params.len() as f64
}

// ============================================================================
// Reflection Effectiveness Analysis
// ============================================================================

/// Analyze how effective reflections were at improving test quality.
/// Returns 0-1, where 1 = reflections led to many new tests.
pub fn analyze_reflection_effectiveness(
    reflections: &[ReflectionEntry],
    _tests: &[GeneratedTest],
    category_rounds: &HashMap<String, u32>,
) -> f64 {
    if reflections.is_empty() {
        return 0.0;
    }

    // Count refining reflections (these should lead to new tests)
    let refining_reflections = reflections
        .iter()
        .filter(|r| matches!(r.action, ReflectionAction::Refining))
        .count();

    // Estimate tests added per reflection
    let total_rounds: u32 = category_rounds.values().sum();
    let num_categories = category_rounds.len().max(1) as f64;
    let avg_rounds = total_rounds as f64 / num_categories;

    // More rounds = more reflections = more tests added
    // Normalize: 1 round = 0.0, 3+ rounds = 1.0
    let round_score = ((avg_rounds - 1.0) / 2.0).clamp(0.0, 1.0);

    // Combine: reflection count + round score
    let reflection_score = (refining_reflections as f64 / 5.0).min(1.0); // 5+ reflections = 1.0

    (round_score + reflection_score) / 2.0
}

// ============================================================================
// Token Efficiency Analysis
// ============================================================================

/// Analyze token efficiency: quality per token spent.
/// Returns comprehensiveness score per 1k tokens (normalized to 0-1).
pub fn analyze_token_efficiency(total_tokens: u32, comprehensiveness_score: f64) -> f64 {
    if total_tokens == 0 || comprehensiveness_score == 0.0 {
        return 0.0;
    }

    // Normalize: comprehensiveness per 1k tokens
    // Higher is better, but cap at 1.0 (10 comprehensiveness / 10k tokens = 1.0)
    // Formula: (comprehensiveness / tokens) * 1000 / 10
    let efficiency = (comprehensiveness_score / total_tokens as f64) * 1000.0;
    let normalized = efficiency / 10.0; // Scale to 0-1 range
    normalized.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_distribution() {
        let tests = vec![
            GeneratedTest {
                id: "1".to_string(),
                input: "test".to_string(),
                expected_output: None,
                reasoning: "test".to_string(),
                category: TestCategory::Correctness,
                confidence: 0.9,
            },
            GeneratedTest {
                id: "2".to_string(),
                input: "test".to_string(),
                expected_output: None,
                reasoning: "test".to_string(),
                category: TestCategory::Correctness,
                confidence: 0.9,
            },
            GeneratedTest {
                id: "3".to_string(),
                input: "test".to_string(),
                expected_output: None,
                reasoning: "test".to_string(),
                category: TestCategory::Boundary,
                confidence: 0.9,
            },
        ];

        let dist = analyze_category_distribution(&tests);
        assert_eq!(dist.get(&TestCategory::Correctness), Some(&2));
        assert_eq!(dist.get(&TestCategory::Boundary), Some(&1));
    }

    #[test]
    fn test_category_balance_perfect() {
        let mut dist = HashMap::new();
        dist.insert(TestCategory::Existence, 5);
        dist.insert(TestCategory::Correctness, 5);
        dist.insert(TestCategory::Boundary, 5);

        let balance = calculate_category_balance(&dist);
        assert!((balance - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_category_balance_imbalanced() {
        let mut dist = HashMap::new();
        dist.insert(TestCategory::Existence, 10);
        dist.insert(TestCategory::Correctness, 1);

        let balance = calculate_category_balance(&dist);
        assert!(balance < 0.5);
    }

    #[test]
    fn test_token_efficiency() {
        // 8 comprehensiveness with 10k tokens
        let eff = analyze_token_efficiency(10000, 8.0);
        assert!((eff - 0.08).abs() < 0.01);

        // 10 comprehensiveness with 10k tokens = 1.0 (max)
        let eff = analyze_token_efficiency(10000, 10.0);
        assert!((eff - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_anti_cheat_no_prohibited() {
        let tests = vec![];
        let env = EnvironmentInfo::minimal();
        let coverage = analyze_anti_cheat_coverage(&tests, &env, "test task");
        assert!((coverage - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_anti_cheat_with_coverage() {
        let tests = vec![GeneratedTest {
            id: "1".to_string(),
            input: "test python usage".to_string(),
            expected_output: None,
            reasoning: "anti-cheat: verify python is not used".to_string(),
            category: TestCategory::AntiCheat,
            confidence: 0.9,
        }];

        let env = EnvironmentInfo::minimal().with_prohibited_tool("python", None);

        let coverage = analyze_anti_cheat_coverage(&tests, &env, "test task");
        assert!((coverage - 1.0).abs() < 0.001);
    }
}
