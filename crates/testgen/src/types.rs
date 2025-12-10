//! Core domain types for TestGen
//!
//! These types map to the SQLite schema in the testgen_* tables.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Enums
// ============================================================================

/// Test category - matches TB2 categories plus anti_cheat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestCategory {
    AntiCheat,
    Existence,
    Correctness,
    Boundary,
    Integration,
    // Legacy categories for backward compatibility
    Format,
    HappyPath,
    EdgeCase,
    InvalidInput,
}

impl TestCategory {
    /// Get all primary categories used in iterative generation
    pub fn primary_categories() -> &'static [TestCategory] {
        &[
            TestCategory::AntiCheat,
            TestCategory::Existence,
            TestCategory::Correctness,
            TestCategory::Boundary,
            TestCategory::Integration,
        ]
    }

    /// Convert to string for display
    pub fn as_str(&self) -> &'static str {
        match self {
            TestCategory::AntiCheat => "anti_cheat",
            TestCategory::Existence => "existence",
            TestCategory::Correctness => "correctness",
            TestCategory::Boundary => "boundary",
            TestCategory::Integration => "integration",
            TestCategory::Format => "format",
            TestCategory::HappyPath => "happy_path",
            TestCategory::EdgeCase => "edge_case",
            TestCategory::InvalidInput => "invalid_input",
        }
    }
}

impl std::fmt::Display for TestCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Model selection for inference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    #[default]
    Local,
    Claude,
}

impl ModelType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelType::Local => "local",
            ModelType::Claude => "claude",
        }
    }
}

/// Test generation context - determines which categories are relevant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TestGenContext {
    /// TB2 - all 5 categories including anti_cheat
    #[default]
    Benchmark,
    /// User prompts - correctness, boundary, existence
    Commander,
    /// Autonomous coding - correctness, boundary
    MechaCoder,
    /// Caller-specified categories
    Custom,
}

/// Config change type from meta-reasoner
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConfigChangeType {
    #[default]
    Keep,
    UpdateParams,
    UpdatePrompts,
    UpdateWeights,
}

/// Reflection action during generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReflectionAction {
    Refining,
    Assessing,
    Complete,
}

// ============================================================================
// Core Structs
// ============================================================================

/// A single generated test case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTest {
    pub id: String,
    pub input: String,
    #[serde(default)]
    pub expected_output: Option<String>,
    pub reasoning: String,
    pub category: TestCategory,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

fn default_confidence() -> f64 {
    0.5
}

/// Test generation configuration - the "knobs" being tuned
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestGenConfig {
    pub id: i64,
    pub version: String,
    pub temperature: f64,
    pub max_tokens: u32,
    pub min_tests_per_category: u32,
    pub max_tests_per_category: u32,
    pub max_rounds_per_category: u32,
    pub environment_weight: f64,
    pub anti_cheat_weight: f64,
    pub precision_weight: f64,
    pub category_order: Vec<TestCategory>,
    #[serde(default)]
    pub category_prompts: Option<HashMap<TestCategory, String>>,
    #[serde(default)]
    pub anti_cheat_prompt: Option<String>,
    #[serde(default)]
    pub reflection_prompt: Option<String>,
    pub primary_model: ModelType,
    pub reflection_model: ModelType,
    pub min_comprehensiveness_score: f64,
    pub target_comprehensiveness_score: f64,
    pub config_hash: String,
    pub is_current: bool,
    pub created_at: String,
}

impl Default for TestGenConfig {
    fn default() -> Self {
        Self {
            id: 0,
            version: "1.0.0".to_string(),
            temperature: 0.3,
            max_tokens: 2048,
            min_tests_per_category: 2,
            max_tests_per_category: 5,
            max_rounds_per_category: 3,
            environment_weight: 0.7,
            anti_cheat_weight: 0.8,
            precision_weight: 0.6,
            category_order: TestCategory::primary_categories().to_vec(),
            category_prompts: None,
            anti_cheat_prompt: None,
            reflection_prompt: None,
            primary_model: ModelType::Local,
            reflection_model: ModelType::Local,
            min_comprehensiveness_score: 7.0,
            target_comprehensiveness_score: 8.5,
            config_hash: String::new(),
            is_current: false,
            created_at: String::new(),
        }
    }
}

/// Input for creating a new config (without auto-generated fields)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestGenConfigInput {
    pub version: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub min_tests_per_category: Option<u32>,
    pub max_tests_per_category: Option<u32>,
    pub max_rounds_per_category: Option<u32>,
    pub environment_weight: Option<f64>,
    pub anti_cheat_weight: Option<f64>,
    pub precision_weight: Option<f64>,
    pub category_order: Option<Vec<TestCategory>>,
    pub category_prompts: Option<HashMap<TestCategory, String>>,
    pub anti_cheat_prompt: Option<String>,
    pub reflection_prompt: Option<String>,
    pub primary_model: Option<ModelType>,
    pub reflection_model: Option<ModelType>,
    pub min_comprehensiveness_score: Option<f64>,
    pub target_comprehensiveness_score: Option<f64>,
}

/// Run record - every test generation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestGenRun {
    pub id: i64,
    pub run_id: String,
    pub session_id: String,
    pub config_id: i64,
    pub task_id: String,
    pub total_tests: u32,
    pub comprehensiveness_score: Option<f64>,
    pub duration_ms: u64,
    pub total_tokens: u32,
    pub category_balance: Option<f64>,
    pub anti_cheat_coverage: Option<f64>,
    pub parameter_discovery: Option<f64>,
    pub reflection_effectiveness: Option<f64>,
    pub token_efficiency: Option<f64>,
    pub meta_model: Option<String>,
    pub proposed_change: Option<String>,
    pub change_accepted: bool,
    pub score: i32,
    pub is_best: bool,
    pub created_at: String,
}

/// Input for creating a new run (without auto-generated fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestGenRunInput {
    pub run_id: String,
    pub session_id: String,
    pub config_id: i64,
    pub task_id: String,
    pub total_tests: u32,
    pub comprehensiveness_score: Option<f64>,
    pub duration_ms: u64,
    pub total_tokens: u32,
    pub category_balance: Option<f64>,
    pub anti_cheat_coverage: Option<f64>,
    pub parameter_discovery: Option<f64>,
    pub reflection_effectiveness: Option<f64>,
    pub token_efficiency: Option<f64>,
    pub meta_model: Option<String>,
    pub proposed_change: Option<String>,
    pub change_accepted: bool,
    pub score: i32,
}

/// Complete analysis of a test generation run
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestGenAnalysis {
    /// Distribution of tests across categories
    pub category_distribution: HashMap<TestCategory, u32>,
    /// How balanced the distribution is (0-1, 1 = perfectly balanced)
    pub category_balance: f64,
    /// Coverage of anti-cheat tests (0-1, 1 = all prohibited tools covered)
    pub anti_cheat_coverage: f64,
    /// Coverage of parameter discovery (0-1, 1 = all parameters discovered)
    pub parameter_discovery: f64,
    /// How effective reflections were (0-1, 1 = reflections added many new tests)
    pub reflection_effectiveness: f64,
    /// Token efficiency: comprehensiveness per 1k tokens
    pub token_efficiency: f64,
    /// Overall composite score (0-1000) - computed by scoring module
    pub overall_score: i32,
}

/// Config change proposal from meta-reasoner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestGenConfigChange {
    #[serde(rename = "type")]
    pub change_type: ConfigChangeType,
    #[serde(default)]
    pub changes: Option<TestGenConfigInput>,
    pub reasoning: String,
    #[serde(default)]
    pub model: Option<String>,
}

impl Default for TestGenConfigChange {
    fn default() -> Self {
        Self {
            change_type: ConfigChangeType::Keep,
            changes: None,
            reasoning: "No changes proposed".to_string(),
            model: None,
        }
    }
}

/// Best config per task type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestGenBestConfig {
    pub task_type: String, // "_global_" | "conversion" | "implementation" | etc.
    pub config_id: i64,
    pub run_id: i64,
    pub score: i32,
    pub pass_count: u32,
    pub total_runs: u32,
    pub is_override: bool,
    pub updated_at: String,
}

/// Evolution history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestGenEvolution {
    pub id: i64,
    pub from_config_id: Option<i64>,
    pub to_config_id: Option<i64>,
    pub changes: serde_json::Value,
    pub reasoning: String,
    pub expected_improvement: Option<String>,
    pub actual_improvement: Option<f64>,
    pub quality_delta: Option<f64>,
    pub created_at: String,
}

/// Aggregate statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestGenStats {
    pub total_runs: u64,
    pub total_configs: u64,
    pub average_score: f64,
    pub best_score: i32,
    pub average_comprehensiveness: f64,
    pub average_token_efficiency: f64,
    pub config_evolution_count: u64,
}

/// Task-specific statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestGenTaskStats {
    pub task_id: String,
    pub total_runs: u64,
    pub average_score: f64,
    pub best_score: i32,
    pub best_config_id: Option<i64>,
    pub average_comprehensiveness: f64,
    pub average_token_efficiency: f64,
}

/// Reflection entry during generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionEntry {
    pub category: Option<TestCategory>,
    pub reflection_text: String,
    pub action: ReflectionAction,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a unique run ID
pub fn generate_run_id() -> String {
    let now = chrono::Utc::now();
    let date_str = now.format("%Y%m%d").to_string();
    let time_str = now.format("%H%M%S").to_string();
    let random: String = uuid::Uuid::new_v4().to_string()[..6].to_string();
    format!("tg-{}-{}-{}", date_str, time_str, random)
}

/// Generate a unique session ID
pub fn generate_session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_as_str() {
        assert_eq!(TestCategory::AntiCheat.as_str(), "anti_cheat");
        assert_eq!(TestCategory::Correctness.as_str(), "correctness");
    }

    #[test]
    fn test_generate_run_id() {
        let id = generate_run_id();
        assert!(id.starts_with("tg-"));
        assert!(id.len() > 20);
    }

    #[test]
    fn test_default_config() {
        let config = TestGenConfig::default();
        assert_eq!(config.temperature, 0.3);
        assert_eq!(config.min_tests_per_category, 2);
        assert_eq!(config.max_tests_per_category, 5);
    }
}
