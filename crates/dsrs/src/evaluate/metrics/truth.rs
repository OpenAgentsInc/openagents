//! Truth metrics - expensive, accurate, run for final decisions.
//!
//! These metrics are more costly but provide ground truth evaluation.
//! They should only be run after proxy metrics pass.

use super::{Metric, MetricScore, MetricTier};
use crate::data::example::Example;
use anyhow::Result;
use async_trait::async_trait;

/// Metric that uses an LLM as a judge to evaluate quality.
#[derive(Debug, Clone)]
pub struct LlmJudgeMetric {
    /// Name of the metric.
    name: String,
    /// System prompt for the judge.
    system_prompt: String,
    /// Evaluation criteria.
    criteria: Vec<String>,
    /// Estimated cost per evaluation in msats.
    cost_msats: u64,
}

impl LlmJudgeMetric {
    /// Create a new LLM judge metric.
    pub fn new() -> Self {
        Self {
            name: "llm_judge".into(),
            system_prompt: DEFAULT_JUDGE_PROMPT.into(),
            criteria: vec![
                "Correctness".into(),
                "Completeness".into(),
                "Code quality".into(),
            ],
            cost_msats: 100, // ~100 msats per evaluation
        }
    }

    /// Set custom system prompt.
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Set evaluation criteria.
    pub fn with_criteria(mut self, criteria: Vec<impl Into<String>>) -> Self {
        self.criteria = criteria.into_iter().map(|c| c.into()).collect();
        self
    }

    /// Set cost estimate.
    pub fn with_cost(mut self, msats: u64) -> Self {
        self.cost_msats = msats;
        self
    }

    /// Set custom name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl Default for LlmJudgeMetric {
    fn default() -> Self {
        Self::new()
    }
}

const DEFAULT_JUDGE_PROMPT: &str = r#"You are an expert code reviewer evaluating AI-generated solutions.

Score the solution on a scale of 0.0 to 1.0 based on:
1. Correctness - Does it solve the problem correctly?
2. Completeness - Does it handle edge cases?
3. Code quality - Is it well-structured and maintainable?

Respond with only a JSON object: {"score": 0.0-1.0, "reason": "brief explanation"}
"#;

#[async_trait]
impl Metric for LlmJudgeMetric {
    fn name(&self) -> &str {
        &self.name
    }

    fn tier(&self) -> MetricTier {
        MetricTier::Truth
    }

    fn cost_estimate(&self) -> u64 {
        self.cost_msats
    }

    async fn evaluate(&self, input: &Example, output: &Example) -> Result<MetricScore> {
        // In a real implementation, this would call an LLM to evaluate
        // For now, we return a placeholder that indicates this needs LM integration

        // Format input/output for the judge
        let _input_str = format!("{:?}", input);
        let _output_str = format!("{:?}", output);

        // TODO: Integrate with dsrs LM infrastructure
        // let lm = dsrs::settings::get_default_lm()?;
        // let response = lm.chat(vec![
        //     Message::system(&self.system_prompt),
        //     Message::user(&format!("Input: {}\n\nOutput: {}", input_str, output_str)),
        // ]).await?;

        // For now, return a mock score
        // Real implementation would parse JSON response from LLM
        Ok(MetricScore::new(0.5)
            .with_confidence(0.5)
            .with_details("LLM judge not yet integrated - returning placeholder")
            .with_cost(self.cost_msats))
    }
}

/// Metric that runs code in a sandbox and checks execution success.
#[derive(Debug, Clone)]
pub struct SandboxMetric {
    /// Name of the metric.
    name: String,
    /// Commands to run for verification.
    commands: Vec<String>,
    /// Timeout in seconds.
    timeout_secs: u64,
    /// Estimated cost per evaluation in msats.
    cost_msats: u64,
}

impl SandboxMetric {
    /// Create a new sandbox metric.
    pub fn new() -> Self {
        Self {
            name: "sandbox".into(),
            commands: vec!["cargo test".into()],
            timeout_secs: 120,
            cost_msats: 500, // ~500 msats for sandbox execution
        }
    }

    /// Set verification commands.
    pub fn with_commands(mut self, commands: Vec<impl Into<String>>) -> Self {
        self.commands = commands.into_iter().map(|c| c.into()).collect();
        self
    }

    /// Set timeout.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set cost estimate.
    pub fn with_cost(mut self, msats: u64) -> Self {
        self.cost_msats = msats;
        self
    }

    /// Set custom name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl Default for SandboxMetric {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Metric for SandboxMetric {
    fn name(&self) -> &str {
        &self.name
    }

    fn tier(&self) -> MetricTier {
        MetricTier::Truth
    }

    fn cost_estimate(&self) -> u64 {
        self.cost_msats
    }

    async fn evaluate(&self, _input: &Example, _output: &Example) -> Result<MetricScore> {
        // In a real implementation, this would:
        // 1. Create a sandbox with the output code
        // 2. Run the verification commands
        // 3. Check exit codes

        // TODO: Integrate with PylonSandboxProvider
        // let sandbox = PylonSandboxProvider::generate()
        //     .with_timeout(Duration::from_secs(self.timeout_secs));
        // let result = sandbox.run_commands(self.commands.clone()).await?;

        // For now, return a mock score
        Ok(MetricScore::new(0.5)
            .with_confidence(0.5)
            .with_details("Sandbox metric not yet integrated - returning placeholder")
            .with_cost(self.cost_msats))
    }
}

/// Metric that computes semantic diff against gold standard.
#[derive(Debug, Clone)]
pub struct DiffMetric {
    /// Name of the metric.
    name: String,
    /// How strict the comparison should be.
    strictness: DiffStrictness,
    /// Estimated cost per evaluation in msats.
    cost_msats: u64,
}

/// Strictness level for diff comparison.
#[derive(Debug, Clone, Copy, Default)]
pub enum DiffStrictness {
    /// Exact character match required.
    Exact,
    /// Whitespace-normalized comparison.
    #[default]
    NormalizedWhitespace,
    /// AST-equivalent comparison (for code).
    AstEquivalent,
    /// Semantic equivalence (LLM-judged).
    Semantic,
}

impl DiffMetric {
    /// Create a new diff metric.
    pub fn new() -> Self {
        Self {
            name: "diff".into(),
            strictness: DiffStrictness::default(),
            cost_msats: 0, // Free for exact/normalized, costs for semantic
        }
    }

    /// Set strictness level.
    pub fn with_strictness(mut self, strictness: DiffStrictness) -> Self {
        self.strictness = strictness;
        // Semantic comparison requires LLM
        if matches!(strictness, DiffStrictness::Semantic) {
            self.cost_msats = 50;
        }
        self
    }

    /// Set custom name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Normalize whitespace in a string.
    fn normalize_whitespace(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Compare two strings with the configured strictness.
    fn compare(&self, actual: &str, expected: &str) -> MetricScore {
        match self.strictness {
            DiffStrictness::Exact => {
                if actual == expected {
                    MetricScore::perfect()
                } else {
                    MetricScore::fail("Exact match failed")
                }
            }
            DiffStrictness::NormalizedWhitespace => {
                let norm_actual = Self::normalize_whitespace(actual);
                let norm_expected = Self::normalize_whitespace(expected);
                if norm_actual == norm_expected {
                    MetricScore::perfect()
                } else {
                    // Calculate similarity score
                    let similarity = strsim_like_ratio(&norm_actual, &norm_expected);
                    MetricScore::new(similarity).with_details(format!(
                        "Normalized comparison: {:.1}% similar",
                        similarity * 100.0
                    ))
                }
            }
            DiffStrictness::AstEquivalent => {
                // TODO: Implement AST comparison
                MetricScore::new(0.5).with_details("AST comparison not yet implemented")
            }
            DiffStrictness::Semantic => {
                // TODO: Use LLM for semantic comparison
                MetricScore::new(0.5)
                    .with_details("Semantic comparison not yet implemented")
                    .with_cost(self.cost_msats)
            }
        }
    }
}

/// Simple string similarity ratio (Levenshtein-based approximation).
fn strsim_like_ratio(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    // Simple character overlap ratio
    let a_chars: std::collections::HashSet<char> = a.chars().collect();
    let b_chars: std::collections::HashSet<char> = b.chars().collect();
    let intersection = a_chars.intersection(&b_chars).count();
    let union = a_chars.union(&b_chars).count();

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

impl Default for DiffMetric {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Metric for DiffMetric {
    fn name(&self) -> &str {
        &self.name
    }

    fn tier(&self) -> MetricTier {
        MetricTier::Truth
    }

    fn cost_estimate(&self) -> u64 {
        self.cost_msats
    }

    async fn evaluate(&self, input: &Example, output: &Example) -> Result<MetricScore> {
        // Get expected output from input (usually in "expected" or "gold" field)
        let expected = input
            .data
            .get("expected")
            .or_else(|| input.data.get("gold"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Get actual output (usually in "output" or "result" field)
        let actual = output
            .data
            .get("output")
            .or_else(|| output.data.get("result"))
            .or_else(|| output.data.get("answer"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if expected.is_empty() {
            return Ok(MetricScore::new(0.5).with_details("No expected output to compare against"));
        }

        Ok(self.compare(actual, expected))
    }
}

/// Metric that checks if unit tests pass.
#[derive(Debug, Clone)]
pub struct TestPassMetric {
    /// Name of the metric.
    name: String,
    /// Test command to run.
    test_command: String,
    /// Timeout in seconds.
    timeout_secs: u64,
    /// Estimated cost per evaluation in msats.
    cost_msats: u64,
}

impl TestPassMetric {
    /// Create a new test pass metric.
    pub fn new() -> Self {
        Self {
            name: "tests_pass".into(),
            test_command: "cargo test".into(),
            timeout_secs: 300,
            cost_msats: 500,
        }
    }

    /// Set test command.
    pub fn with_command(mut self, cmd: impl Into<String>) -> Self {
        self.test_command = cmd.into();
        self
    }

    /// Set timeout.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set cost estimate.
    pub fn with_cost(mut self, msats: u64) -> Self {
        self.cost_msats = msats;
        self
    }

    /// Set custom name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl Default for TestPassMetric {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Metric for TestPassMetric {
    fn name(&self) -> &str {
        &self.name
    }

    fn tier(&self) -> MetricTier {
        MetricTier::Truth
    }

    fn cost_estimate(&self) -> u64 {
        self.cost_msats
    }

    async fn evaluate(&self, _input: &Example, _output: &Example) -> Result<MetricScore> {
        // In a real implementation, this would:
        // 1. Apply the output code changes
        // 2. Run the test command in a sandbox
        // 3. Parse test results

        // TODO: Integrate with sandbox execution
        Ok(MetricScore::new(0.5)
            .with_confidence(0.5)
            .with_details("Test pass metric not yet integrated - returning placeholder")
            .with_cost(self.cost_msats))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_example(fields: &[(&str, &str)]) -> Example {
        let mut ex = Example::default();
        for (k, v) in fields {
            ex.data.insert(k.to_string(), serde_json::Value::String(v.to_string()));
        }
        ex
    }

    #[tokio::test]
    async fn test_diff_metric_exact() {
        let metric = DiffMetric::new().with_strictness(DiffStrictness::Exact);
        let input = make_example(&[("expected", "hello world")]);
        let output = make_example(&[("output", "hello world")]);

        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 1.0);

        let output = make_example(&[("output", "hello  world")]);
        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 0.0);
    }

    #[tokio::test]
    async fn test_diff_metric_normalized() {
        let metric = DiffMetric::new().with_strictness(DiffStrictness::NormalizedWhitespace);
        let input = make_example(&[("expected", "hello world")]);
        let output = make_example(&[("output", "hello   world")]);

        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 1.0);
    }

    #[test]
    fn test_metric_tiers() {
        assert_eq!(LlmJudgeMetric::new().tier(), MetricTier::Truth);
        assert_eq!(SandboxMetric::new().tier(), MetricTier::Truth);
        assert_eq!(DiffMetric::new().tier(), MetricTier::Truth);
        assert_eq!(TestPassMetric::new().tier(), MetricTier::Truth);
    }

    #[test]
    fn test_cost_estimates() {
        assert!(LlmJudgeMetric::new().cost_estimate() > 0);
        assert!(SandboxMetric::new().cost_estimate() > 0);
        assert_eq!(DiffMetric::new().cost_estimate(), 0); // Normalized is free
        assert!(TestPassMetric::new().cost_estimate() > 0);
    }
}
