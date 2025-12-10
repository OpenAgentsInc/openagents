//! TestGen Meta-Reasoner
//!
//! Config evolution with guardrails. Proposes incremental config changes
//! based on run results to improve test generation quality over time.

use crate::error::{Result, TestGenError};
use crate::types::{
    ConfigChangeType, ModelType, TestGenAnalysis, TestGenConfig,
    TestGenConfigChange, TestGenConfigInput, TestGenRun,
};
use fm_bridge::{CompletionOptions, FMClient};

// ============================================================================
// Guardrails
// ============================================================================

/// Guardrail constraints for config changes
#[derive(Debug, Clone)]
pub struct Guardrails {
    /// Maximum temperature change per iteration
    pub max_temperature_delta: f64,
    /// Maximum tests per category change per iteration
    pub max_tests_delta: u32,
    /// Maximum rounds per category change per iteration
    pub max_rounds_delta: u32,
    /// Maximum weight change per iteration
    pub max_weight_delta: f64,
    /// Minimum tests per category (hard floor)
    pub min_tests_per_category: u32,
}

impl Default for Guardrails {
    fn default() -> Self {
        Self {
            max_temperature_delta: 0.1,
            max_tests_delta: 1,
            max_rounds_delta: 1,
            max_weight_delta: 0.1,
            min_tests_per_category: 2,
        }
    }
}

// ============================================================================
// Validation
// ============================================================================

/// Small epsilon for floating point comparisons
const EPSILON: f64 = 1e-9;

/// Validate config changes against guardrails.
/// Returns Ok(()) if validation passes, or an error if it fails.
pub fn validate_config_change(
    current: &TestGenConfig,
    change: &TestGenConfigChange,
    guardrails: &Guardrails,
) -> Result<()> {
    if matches!(change.change_type, ConfigChangeType::Keep) {
        return Ok(());
    }

    let changes = match &change.changes {
        Some(c) => c,
        None => return Ok(()),
    };

    // Temperature check (with epsilon for floating point precision)
    if let Some(new_temp) = changes.temperature {
        let delta = (new_temp - current.temperature).abs();
        if delta > guardrails.max_temperature_delta + EPSILON {
            return Err(TestGenError::GuardrailViolation(format!(
                "Temperature change too large: {:.3} > {:.1} (capped at ±{:.1})",
                delta, guardrails.max_temperature_delta, guardrails.max_temperature_delta
            )));
        }
    }

    // Min tests per category check
    if let Some(new_min) = changes.min_tests_per_category {
        let delta =
            (new_min as i64 - current.min_tests_per_category as i64).unsigned_abs() as u32;
        if delta > guardrails.max_tests_delta {
            return Err(TestGenError::GuardrailViolation(format!(
                "Min tests change too large: {} > {} (capped at ±{})",
                delta, guardrails.max_tests_delta, guardrails.max_tests_delta
            )));
        }
        if new_min < guardrails.min_tests_per_category {
            return Err(TestGenError::GuardrailViolation(format!(
                "Min tests too low: {} < {} (minimum: {})",
                new_min, guardrails.min_tests_per_category, guardrails.min_tests_per_category
            )));
        }
    }

    // Max tests per category check
    if let Some(new_max) = changes.max_tests_per_category {
        let delta =
            (new_max as i64 - current.max_tests_per_category as i64).unsigned_abs() as u32;
        if delta > guardrails.max_tests_delta {
            return Err(TestGenError::GuardrailViolation(format!(
                "Max tests change too large: {} > {} (capped at ±{})",
                delta, guardrails.max_tests_delta, guardrails.max_tests_delta
            )));
        }

        // Ensure max >= min
        let min_tests = changes
            .min_tests_per_category
            .unwrap_or(current.min_tests_per_category);
        if new_max < min_tests {
            return Err(TestGenError::GuardrailViolation(format!(
                "Max tests per category ({}) < min ({})",
                new_max, min_tests
            )));
        }
    }

    // Max rounds check
    if let Some(new_rounds) = changes.max_rounds_per_category {
        let delta =
            (new_rounds as i64 - current.max_rounds_per_category as i64).unsigned_abs() as u32;
        if delta > guardrails.max_rounds_delta {
            return Err(TestGenError::GuardrailViolation(format!(
                "Max rounds change too large: {} > {} (capped at ±{})",
                delta, guardrails.max_rounds_delta, guardrails.max_rounds_delta
            )));
        }
    }

    // Weight checks (with epsilon for floating point precision)
    if let Some(new_weight) = changes.environment_weight {
        let delta = (new_weight - current.environment_weight).abs();
        if delta > guardrails.max_weight_delta + EPSILON {
            return Err(TestGenError::GuardrailViolation(format!(
                "Environment weight change too large: {:.3} > {:.1}",
                delta, guardrails.max_weight_delta
            )));
        }
    }

    if let Some(new_weight) = changes.anti_cheat_weight {
        let delta = (new_weight - current.anti_cheat_weight).abs();
        if delta > guardrails.max_weight_delta + EPSILON {
            return Err(TestGenError::GuardrailViolation(format!(
                "Anti-cheat weight change too large: {:.3} > {:.1}",
                delta, guardrails.max_weight_delta
            )));
        }
    }

    if let Some(new_weight) = changes.precision_weight {
        let delta = (new_weight - current.precision_weight).abs();
        if delta > guardrails.max_weight_delta + EPSILON {
            return Err(TestGenError::GuardrailViolation(format!(
                "Precision weight change too large: {:.3} > {:.1}",
                delta, guardrails.max_weight_delta
            )));
        }
    }

    Ok(())
}

// ============================================================================
// Config Application
// ============================================================================

/// Apply a config change to create a new config input.
/// If validation fails, returns the current config unchanged.
pub fn apply_config_change(
    current: &TestGenConfig,
    change: &TestGenConfigChange,
) -> TestGenConfigInput {
    // Validate guardrails first
    if let Err(e) = validate_config_change(current, change, &Guardrails::default()) {
        tracing::warn!("Guardrail violation: {}. Keeping current config.", e);
        return config_to_input(current);
    }

    if matches!(change.change_type, ConfigChangeType::Keep) || change.changes.is_none() {
        return config_to_input(current);
    }

    let changes = change.changes.as_ref().unwrap();

    TestGenConfigInput {
        version: Some(increment_version(&current.version)),
        temperature: changes.temperature.or(Some(current.temperature)),
        max_tokens: changes.max_tokens.or(Some(current.max_tokens)),
        min_tests_per_category: changes
            .min_tests_per_category
            .or(Some(current.min_tests_per_category)),
        max_tests_per_category: changes
            .max_tests_per_category
            .or(Some(current.max_tests_per_category)),
        max_rounds_per_category: changes
            .max_rounds_per_category
            .or(Some(current.max_rounds_per_category)),
        environment_weight: changes
            .environment_weight
            .or(Some(current.environment_weight)),
        anti_cheat_weight: changes.anti_cheat_weight.or(Some(current.anti_cheat_weight)),
        precision_weight: changes.precision_weight.or(Some(current.precision_weight)),
        category_order: changes
            .category_order
            .clone()
            .or(Some(current.category_order.clone())),
        category_prompts: changes
            .category_prompts
            .clone()
            .or(current.category_prompts.clone()),
        anti_cheat_prompt: changes
            .anti_cheat_prompt
            .clone()
            .or(current.anti_cheat_prompt.clone()),
        reflection_prompt: changes
            .reflection_prompt
            .clone()
            .or(current.reflection_prompt.clone()),
        primary_model: changes.primary_model.or(Some(current.primary_model)),
        reflection_model: changes.reflection_model.or(Some(current.reflection_model)),
        min_comprehensiveness_score: changes
            .min_comprehensiveness_score
            .or(Some(current.min_comprehensiveness_score)),
        target_comprehensiveness_score: changes
            .target_comprehensiveness_score
            .or(Some(current.target_comprehensiveness_score)),
    }
}

/// Convert a config to input format (for unchanged configs)
fn config_to_input(config: &TestGenConfig) -> TestGenConfigInput {
    TestGenConfigInput {
        version: Some(increment_version(&config.version)),
        temperature: Some(config.temperature),
        max_tokens: Some(config.max_tokens),
        min_tests_per_category: Some(config.min_tests_per_category),
        max_tests_per_category: Some(config.max_tests_per_category),
        max_rounds_per_category: Some(config.max_rounds_per_category),
        environment_weight: Some(config.environment_weight),
        anti_cheat_weight: Some(config.anti_cheat_weight),
        precision_weight: Some(config.precision_weight),
        category_order: Some(config.category_order.clone()),
        category_prompts: config.category_prompts.clone(),
        anti_cheat_prompt: config.anti_cheat_prompt.clone(),
        reflection_prompt: config.reflection_prompt.clone(),
        primary_model: Some(config.primary_model),
        reflection_model: Some(config.reflection_model),
        min_comprehensiveness_score: Some(config.min_comprehensiveness_score),
        target_comprehensiveness_score: Some(config.target_comprehensiveness_score),
    }
}

/// Increment version string (e.g., "1.0.0" -> "1.0.1").
pub fn increment_version(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() == 3 {
        if let Ok(patch) = parts[2].parse::<u32>() {
            return format!("{}.{}.{}", parts[0], parts[1], patch + 1);
        }
    }
    version.to_string()
}

// ============================================================================
// Meta-Reasoning via FM
// ============================================================================

/// Propose a config change using FM inference.
pub async fn propose_config_change(
    client: &FMClient,
    config: &TestGenConfig,
    recent_runs: &[TestGenRun],
    last_analysis: &TestGenAnalysis,
    task_type: &str,
) -> Result<TestGenConfigChange> {
    let prompt = build_meta_prompt(config, recent_runs, last_analysis, task_type);

    let response = client
        .complete(
            prompt,
            Some(CompletionOptions {
                temperature: Some(0.3),
                max_tokens: Some(1000),
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

    parse_config_change(&content)
}

/// Build meta-reasoning prompt for config optimization.
fn build_meta_prompt(
    config: &TestGenConfig,
    recent_runs: &[TestGenRun],
    analysis: &TestGenAnalysis,
    task_type: &str,
) -> String {
    let recent_scores: Vec<i32> = recent_runs.iter().take(5).map(|r| r.score).collect();
    let avg_score = if !recent_scores.is_empty() {
        recent_scores.iter().sum::<i32>() as f64 / recent_scores.len() as f64
    } else {
        0.0
    };

    let mut patterns = Vec::new();
    if analysis.category_balance < 0.6 {
        patterns.push("- Category balance is low (tests are unevenly distributed)");
    }
    if analysis.anti_cheat_coverage < 0.7 {
        patterns.push("- Anti-cheat coverage is low (missing tests for prohibited tools)");
    }
    if analysis.token_efficiency < 0.5 {
        patterns.push("- Token efficiency is low (spending too many tokens for quality)");
    }
    if analysis.reflection_effectiveness < 0.5 {
        patterns.push("- Reflections are not leading to new tests");
    }

    let patterns_text = if patterns.is_empty() {
        "- No major issues detected".to_string()
    } else {
        patterns.join("\n")
    };

    let runs_text = recent_runs
        .iter()
        .take(5)
        .enumerate()
        .map(|(i, r)| {
            format!(
                "- Run {}: Score {}, comprehensiveness={}, balance={:.2}, anti-cheat={:.2}, efficiency={:.2}",
                i + 1,
                r.score,
                r.comprehensiveness_score.map(|s| format!("{:.1}", s)).unwrap_or("N/A".to_string()),
                r.category_balance.unwrap_or(0.0),
                r.anti_cheat_coverage.unwrap_or(0.0),
                r.token_efficiency.unwrap_or(0.0)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are optimizing a test generation system. Your goal is to improve test quality over time.

Current config:
- Temperature: {}
- Min tests per category: {}
- Max tests per category: {}
- Max rounds per category: {}
- Environment weight: {}
- Anti-cheat weight: {}
- Precision weight: {}
- Primary model: {}
- Reflection model: {}

Recent performance (last {} runs):
{}

Average score: {:.0}/1000

Last run analysis:
- Category balance: {:.2} (target: 0.8+)
- Anti-cheat coverage: {:.2} (target: 0.9+)
- Parameter discovery: {:.2} (target: 0.8+)
- Reflection effectiveness: {:.2} (target: 0.7+)
- Token efficiency: {:.2} (target: 0.6+)

Patterns observed:
{}

Task type: {}

**IMPORTANT: Guardrail Constraints**
To ensure stable evolution, changes must be incremental:
- Temperature: Can change by ±0.1 maximum
- Min/Max tests per category: Can change by ±1 maximum
- Max rounds per category: Can change by ±1 maximum
- Weights: Can change by ±0.1 maximum

Return JSON with this exact structure:
{{
  "type": "keep" | "update_params" | "update_prompts" | "update_weights",
  "changes": {{
    "temperature": 0.3,
    "minTestsPerCategory": 2,
    "maxTestsPerCategory": 5,
    "maxRoundsPerCategory": 3,
    "environmentWeight": 0.7,
    "antiCheatWeight": 0.8,
    "precisionWeight": 0.6,
    "primaryModel": "local",
    "reflectionModel": "local"
  }},
  "reasoning": "Explanation of why these changes should improve quality"
}}

Only include fields in "changes" that you want to modify. If no changes are needed, use type "keep" with empty changes."#,
        config.temperature,
        config.min_tests_per_category,
        config.max_tests_per_category,
        config.max_rounds_per_category,
        config.environment_weight,
        config.anti_cheat_weight,
        config.precision_weight,
        config.primary_model.as_str(),
        config.reflection_model.as_str(),
        recent_runs.len(),
        runs_text,
        avg_score,
        analysis.category_balance,
        analysis.anti_cheat_coverage,
        analysis.parameter_discovery,
        analysis.reflection_effectiveness,
        analysis.token_efficiency,
        patterns_text,
        task_type
    )
}

/// Parse LLM response into config change proposal.
fn parse_config_change(response: &str) -> Result<TestGenConfigChange> {
    // Extract JSON from response (handle markdown code blocks)
    let mut json_str = response.trim();

    if json_str.contains("```") {
        // Try to extract JSON from code block
        if let Some(start) = json_str.find("```json") {
            let after_start = &json_str[start + 7..];
            if let Some(end) = after_start.find("```") {
                json_str = &after_start[..end];
            }
        } else if let Some(start) = json_str.find("```") {
            let after_start = &json_str[start + 3..];
            if let Some(end) = after_start.find("```") {
                json_str = &after_start[..end];
            }
        }
    }

    json_str = json_str.trim();

    // Parse JSON
    let parsed: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
        TestGenError::ParseError(format!("Failed to parse config change JSON: {}", e))
    })?;

    // Extract type
    let change_type = match parsed.get("type").and_then(|v| v.as_str()) {
        Some("keep") => ConfigChangeType::Keep,
        Some("update_params") => ConfigChangeType::UpdateParams,
        Some("update_prompts") => ConfigChangeType::UpdatePrompts,
        Some("update_weights") => ConfigChangeType::UpdateWeights,
        _ => ConfigChangeType::Keep,
    };

    // Extract reasoning
    let reasoning = parsed
        .get("reasoning")
        .and_then(|v| v.as_str())
        .unwrap_or("No reasoning provided")
        .to_string();

    // Extract changes
    let changes = if let Some(changes_obj) = parsed.get("changes") {
        let mut input = TestGenConfigInput::default();

        if let Some(v) = changes_obj.get("temperature").and_then(|v| v.as_f64()) {
            input.temperature = Some(v.clamp(0.0, 1.0));
        }
        if let Some(v) = changes_obj
            .get("minTestsPerCategory")
            .and_then(|v| v.as_u64())
        {
            input.min_tests_per_category = Some(v.max(1) as u32);
        }
        if let Some(v) = changes_obj
            .get("maxTestsPerCategory")
            .and_then(|v| v.as_u64())
        {
            input.max_tests_per_category = Some(v.max(1) as u32);
        }
        if let Some(v) = changes_obj
            .get("maxRoundsPerCategory")
            .and_then(|v| v.as_u64())
        {
            input.max_rounds_per_category = Some(v.max(1) as u32);
        }
        if let Some(v) = changes_obj.get("environmentWeight").and_then(|v| v.as_f64()) {
            input.environment_weight = Some(v.clamp(0.0, 1.0));
        }
        if let Some(v) = changes_obj.get("antiCheatWeight").and_then(|v| v.as_f64()) {
            input.anti_cheat_weight = Some(v.clamp(0.0, 1.0));
        }
        if let Some(v) = changes_obj.get("precisionWeight").and_then(|v| v.as_f64()) {
            input.precision_weight = Some(v.clamp(0.0, 1.0));
        }
        if let Some(v) = changes_obj.get("primaryModel").and_then(|v| v.as_str()) {
            input.primary_model = Some(if v == "claude" {
                ModelType::Claude
            } else {
                ModelType::Local
            });
        }
        if let Some(v) = changes_obj.get("reflectionModel").and_then(|v| v.as_str()) {
            input.reflection_model = Some(if v == "claude" {
                ModelType::Claude
            } else {
                ModelType::Local
            });
        }

        Some(input)
    } else {
        None
    };

    Ok(TestGenConfigChange {
        change_type,
        changes,
        reasoning,
        model: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TestCategory;

    fn make_test_config() -> TestGenConfig {
        TestGenConfig {
            id: 1,
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
            config_hash: "test".to_string(),
            is_current: true,
            created_at: "2024-01-01".to_string(),
        }
    }

    #[test]
    fn test_guardrail_valid_change() {
        let config = make_test_config();

        let change = TestGenConfigChange {
            change_type: ConfigChangeType::UpdateParams,
            changes: Some(TestGenConfigInput {
                temperature: Some(0.4), // Delta = 0.1, within limit
                ..Default::default()
            }),
            reasoning: "Test".to_string(),
            model: None,
        };

        let result = validate_config_change(&config, &change, &Guardrails::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_guardrail_temperature_violation() {
        let config = make_test_config();

        let change = TestGenConfigChange {
            change_type: ConfigChangeType::UpdateParams,
            changes: Some(TestGenConfigInput {
                temperature: Some(0.6), // Delta = 0.3, exceeds 0.1 limit
                ..Default::default()
            }),
            reasoning: "Test".to_string(),
            model: None,
        };

        let result = validate_config_change(&config, &change, &Guardrails::default());
        assert!(matches!(result, Err(TestGenError::GuardrailViolation(_))));
    }

    #[test]
    fn test_guardrail_tests_violation() {
        let config = make_test_config();

        let change = TestGenConfigChange {
            change_type: ConfigChangeType::UpdateParams,
            changes: Some(TestGenConfigInput {
                min_tests_per_category: Some(5), // Delta = 3, exceeds 1 limit
                ..Default::default()
            }),
            reasoning: "Test".to_string(),
            model: None,
        };

        let result = validate_config_change(&config, &change, &Guardrails::default());
        assert!(matches!(result, Err(TestGenError::GuardrailViolation(_))));
    }

    #[test]
    fn test_guardrail_min_tests_floor() {
        let config = make_test_config();

        let change = TestGenConfigChange {
            change_type: ConfigChangeType::UpdateParams,
            changes: Some(TestGenConfigInput {
                min_tests_per_category: Some(1), // Below hard floor of 2
                ..Default::default()
            }),
            reasoning: "Test".to_string(),
            model: None,
        };

        let result = validate_config_change(&config, &change, &Guardrails::default());
        assert!(matches!(result, Err(TestGenError::GuardrailViolation(_))));
    }

    #[test]
    fn test_increment_version() {
        assert_eq!(increment_version("1.0.0"), "1.0.1");
        assert_eq!(increment_version("1.2.3"), "1.2.4");
        assert_eq!(increment_version("2.0.99"), "2.0.100");
        assert_eq!(increment_version("invalid"), "invalid");
    }

    #[test]
    fn test_apply_config_change() {
        let config = make_test_config();

        let change = TestGenConfigChange {
            change_type: ConfigChangeType::UpdateParams,
            changes: Some(TestGenConfigInput {
                temperature: Some(0.4),
                ..Default::default()
            }),
            reasoning: "Increase diversity".to_string(),
            model: None,
        };

        let result = apply_config_change(&config, &change);
        assert_eq!(result.temperature, Some(0.4));
        assert_eq!(result.version, Some("1.0.1".to_string()));
    }

    #[test]
    fn test_parse_config_change() {
        let json = r#"{
            "type": "update_params",
            "changes": {
                "temperature": 0.4,
                "minTestsPerCategory": 3
            },
            "reasoning": "Improve quality"
        }"#;

        let result = parse_config_change(json).unwrap();
        assert!(matches!(result.change_type, ConfigChangeType::UpdateParams));
        assert_eq!(result.changes.as_ref().unwrap().temperature, Some(0.4));
        assert_eq!(
            result.changes.as_ref().unwrap().min_tests_per_category,
            Some(3)
        );
    }

    #[test]
    fn test_parse_config_change_with_markdown() {
        let json = r#"
Here's my analysis:

```json
{
    "type": "keep",
    "reasoning": "No changes needed"
}
```

Done!
"#;

        let result = parse_config_change(json).unwrap();
        assert!(matches!(result.change_type, ConfigChangeType::Keep));
    }
}
