//! Guardrails - Safety constraints and resource limits for the learning system
//!
//! Implements boundaries to prevent harmful or wasteful behavior:
//! - Resource limits (tokens, duration)
//! - Safety constraints (blocked files, network access)
//! - Quality thresholds (success rate, consecutive failures)
//! - Behavioral limits (retries, skills, memory)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Rule Types
// ============================================================================

/// Categories of guardrail rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleCategory {
    /// Resource usage limits
    Resource,
    /// Safety constraints
    Safety,
    /// Quality thresholds
    Quality,
    /// Behavioral limits
    Behavior,
    /// Access controls
    Access,
}

/// Severity levels for rule violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warning,
    Error,
    Critical,
}

/// A guardrail rule definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailRule {
    /// Rule ID
    pub id: String,
    /// Rule name
    pub name: String,
    /// Description
    pub description: String,
    /// Rule category
    pub category: RuleCategory,
    /// Severity if violated
    pub severity: Severity,
    /// Whether rule is enabled
    pub enabled: bool,
    /// Check function name (for dispatch)
    pub check_fn: String,
    /// Rule parameters
    pub params: HashMap<String, serde_json::Value>,
}

/// Result of a guardrail check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailResult {
    /// Rule that was checked
    pub rule_id: String,
    /// Whether the check passed
    pub passed: bool,
    /// Severity if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
    /// Message
    pub message: String,
    /// Suggested action
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// Context/details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<HashMap<String, serde_json::Value>>,
    /// Timestamp
    pub timestamp: String,
}

/// Aggregated guardrail status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailStatus {
    /// All checks passed
    pub all_passed: bool,
    /// Number of warnings
    pub warnings: usize,
    /// Number of errors
    pub errors: usize,
    /// Number of critical issues
    pub critical: usize,
    /// Individual results
    pub results: Vec<GuardrailResult>,
    /// Should block operation
    pub should_block: bool,
    /// Timestamp
    pub timestamp: String,
}

// ============================================================================
// Configuration
// ============================================================================

/// Guardrails configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailsConfig {
    /// Maximum tokens per task
    pub max_tokens_per_task: u64,
    /// Maximum duration per task in ms
    pub max_duration_per_task: u64,
    /// Maximum retries per task
    pub max_retries_per_task: u32,
    /// Maximum total tokens per run
    pub max_tokens_per_run: u64,
    /// Maximum duration per run in ms
    pub max_duration_per_run: u64,
    /// Minimum success rate to continue
    pub min_success_rate: f64,
    /// Maximum consecutive failures
    pub max_consecutive_failures: u32,
    /// Blocked file patterns
    pub blocked_patterns: Vec<String>,
    /// Maximum skills to learn per run
    pub max_skills_per_run: u32,
    /// Maximum memory entries
    pub max_memory_entries: u64,
    /// Enable strict mode (block on any violation)
    pub strict_mode: bool,
}

impl Default for GuardrailsConfig {
    fn default() -> Self {
        Self {
            max_tokens_per_task: 50000,
            max_duration_per_task: 300000, // 5 minutes
            max_retries_per_task: 3,
            max_tokens_per_run: 1000000, // 1M tokens
            max_duration_per_run: 3600000, // 1 hour
            min_success_rate: 0.1,
            max_consecutive_failures: 10,
            blocked_patterns: vec![
                "*.env".to_string(),
                "*.pem".to_string(),
                "*.key".to_string(),
                "*credentials*".to_string(),
                "*secrets*".to_string(),
                "*password*".to_string(),
                "~/.ssh/*".to_string(),
                "~/.aws/*".to_string(),
            ],
            max_skills_per_run: 50,
            max_memory_entries: 10000,
            strict_mode: false,
        }
    }
}

// ============================================================================
// Validation Context
// ============================================================================

/// Context for validation checks.
#[derive(Debug, Clone, Default)]
pub struct ValidationContext {
    /// Current task tokens
    pub task_tokens: Option<u64>,
    /// Current task duration
    pub task_duration_ms: Option<u64>,
    /// Current retry count
    pub retry_count: Option<u32>,
    /// Total run tokens
    pub run_tokens: Option<u64>,
    /// Total run duration
    pub run_duration_ms: Option<u64>,
    /// Current success rate
    pub success_rate: Option<f64>,
    /// Consecutive failures
    pub consecutive_failures: Option<u32>,
    /// Files being accessed
    pub file_paths: Option<Vec<String>>,
    /// Skills learned this run
    pub skills_learned: Option<u32>,
    /// Total memory entries
    pub memory_entries: Option<u64>,
    /// Network operations
    pub network_operations: Option<Vec<String>>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a guardrail result.
pub fn create_result(
    rule_id: &str,
    passed: bool,
    message: &str,
    severity: Option<Severity>,
    action: Option<&str>,
    context: Option<HashMap<String, serde_json::Value>>,
) -> GuardrailResult {
    GuardrailResult {
        rule_id: rule_id.to_string(),
        passed,
        message: message.to_string(),
        severity,
        action: action.map(|s| s.to_string()),
        context,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}

/// Aggregate multiple results into a status.
pub fn aggregate_results(results: Vec<GuardrailResult>) -> GuardrailStatus {
    let warnings = results
        .iter()
        .filter(|r| !r.passed && r.severity == Some(Severity::Warning))
        .count();
    let errors = results
        .iter()
        .filter(|r| !r.passed && r.severity == Some(Severity::Error))
        .count();
    let critical = results
        .iter()
        .filter(|r| !r.passed && r.severity == Some(Severity::Critical))
        .count();

    GuardrailStatus {
        all_passed: results.iter().all(|r| r.passed),
        warnings,
        errors,
        critical,
        should_block: errors > 0 || critical > 0,
        results,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}

/// Check if a file path matches blocked patterns.
pub fn matches_blocked_pattern(file_path: &str, patterns: &[String]) -> bool {
    let normalized_path = file_path.to_lowercase();

    for pattern in patterns {
        let normalized_pattern = pattern.to_lowercase();

        // Simple glob matching
        if normalized_pattern.starts_with('*')
            && normalized_pattern.ends_with('*')
            && normalized_pattern.len() > 2
        {
            // Pattern like *credentials*
            let middle = &normalized_pattern[1..normalized_pattern.len() - 1];
            if normalized_path.contains(middle) {
                return true;
            }
        } else if normalized_pattern.starts_with('*') {
            // Pattern like *.env
            let suffix = &normalized_pattern[1..];
            if normalized_path.ends_with(suffix) || normalized_path.contains(suffix) {
                return true;
            }
        } else if normalized_pattern.ends_with('*') {
            // Pattern like ~/.ssh/*
            let prefix = &normalized_pattern[..normalized_pattern.len() - 1];
            if normalized_path.starts_with(prefix) || normalized_path.contains(prefix) {
                return true;
            }
        } else if normalized_pattern.contains('*') {
            // Contains wildcard in middle
            let parts: Vec<&str> = normalized_pattern.split('*').collect();
            if parts.iter().all(|part| normalized_path.contains(part)) {
                return true;
            }
        } else {
            // Exact match
            if normalized_path.contains(&normalized_pattern) {
                return true;
            }
        }
    }

    false
}

/// Get rules by category.
pub fn get_rules_by_category(rules: &[GuardrailRule], category: RuleCategory) -> Vec<&GuardrailRule> {
    rules.iter().filter(|r| r.category == category).collect()
}

/// Get enabled rules.
pub fn get_enabled_rules(rules: &[GuardrailRule]) -> Vec<&GuardrailRule> {
    rules.iter().filter(|r| r.enabled).collect()
}

// ============================================================================
// Built-in Rules
// ============================================================================

/// Get the built-in guardrail rules.
pub fn builtin_rules() -> Vec<GuardrailRule> {
    vec![
        // Resource limits
        GuardrailRule {
            id: "max-tokens-task".to_string(),
            name: "Max Tokens Per Task".to_string(),
            description: "Limit token usage per individual task".to_string(),
            category: RuleCategory::Resource,
            severity: Severity::Warning,
            enabled: true,
            check_fn: "checkTokensPerTask".to_string(),
            params: [("maxTokens".to_string(), serde_json::json!(50000))]
                .into_iter()
                .collect(),
        },
        GuardrailRule {
            id: "max-duration-task".to_string(),
            name: "Max Duration Per Task".to_string(),
            description: "Limit execution time per task".to_string(),
            category: RuleCategory::Resource,
            severity: Severity::Error,
            enabled: true,
            check_fn: "checkDurationPerTask".to_string(),
            params: [("maxDurationMs".to_string(), serde_json::json!(300000))]
                .into_iter()
                .collect(),
        },
        GuardrailRule {
            id: "max-tokens-run".to_string(),
            name: "Max Tokens Per Run".to_string(),
            description: "Limit total token usage per training run".to_string(),
            category: RuleCategory::Resource,
            severity: Severity::Error,
            enabled: true,
            check_fn: "checkTokensPerRun".to_string(),
            params: [("maxTokens".to_string(), serde_json::json!(1000000))]
                .into_iter()
                .collect(),
        },
        GuardrailRule {
            id: "max-duration-run".to_string(),
            name: "Max Duration Per Run".to_string(),
            description: "Limit total execution time per run".to_string(),
            category: RuleCategory::Resource,
            severity: Severity::Error,
            enabled: true,
            check_fn: "checkDurationPerRun".to_string(),
            params: [("maxDurationMs".to_string(), serde_json::json!(3600000))]
                .into_iter()
                .collect(),
        },
        // Safety constraints
        GuardrailRule {
            id: "blocked-files".to_string(),
            name: "Blocked File Patterns".to_string(),
            description: "Prevent access to sensitive files".to_string(),
            category: RuleCategory::Safety,
            severity: Severity::Critical,
            enabled: true,
            check_fn: "checkBlockedFiles".to_string(),
            params: HashMap::new(),
        },
        GuardrailRule {
            id: "no-network-access".to_string(),
            name: "No Network Access".to_string(),
            description: "Prevent unauthorized network operations".to_string(),
            category: RuleCategory::Safety,
            severity: Severity::Critical,
            enabled: false, // Disabled by default
            check_fn: "checkNetworkAccess".to_string(),
            params: HashMap::new(),
        },
        // Quality thresholds
        GuardrailRule {
            id: "min-success-rate".to_string(),
            name: "Minimum Success Rate".to_string(),
            description: "Require minimum success rate to continue".to_string(),
            category: RuleCategory::Quality,
            severity: Severity::Warning,
            enabled: true,
            check_fn: "checkSuccessRate".to_string(),
            params: [("minRate".to_string(), serde_json::json!(0.1))]
                .into_iter()
                .collect(),
        },
        GuardrailRule {
            id: "max-consecutive-failures".to_string(),
            name: "Max Consecutive Failures".to_string(),
            description: "Stop after too many consecutive failures".to_string(),
            category: RuleCategory::Quality,
            severity: Severity::Error,
            enabled: true,
            check_fn: "checkConsecutiveFailures".to_string(),
            params: [("maxFailures".to_string(), serde_json::json!(10))]
                .into_iter()
                .collect(),
        },
        // Behavioral limits
        GuardrailRule {
            id: "max-retries".to_string(),
            name: "Max Retries Per Task".to_string(),
            description: "Limit retry attempts per task".to_string(),
            category: RuleCategory::Behavior,
            severity: Severity::Warning,
            enabled: true,
            check_fn: "checkRetries".to_string(),
            params: [("maxRetries".to_string(), serde_json::json!(3))]
                .into_iter()
                .collect(),
        },
        GuardrailRule {
            id: "max-skills-run".to_string(),
            name: "Max Skills Per Run".to_string(),
            description: "Limit skills learned per run".to_string(),
            category: RuleCategory::Behavior,
            severity: Severity::Warning,
            enabled: true,
            check_fn: "checkSkillsPerRun".to_string(),
            params: [("maxSkills".to_string(), serde_json::json!(50))]
                .into_iter()
                .collect(),
        },
        GuardrailRule {
            id: "max-memory-entries".to_string(),
            name: "Max Memory Entries".to_string(),
            description: "Limit total memory entries".to_string(),
            category: RuleCategory::Behavior,
            severity: Severity::Warning,
            enabled: true,
            check_fn: "checkMemoryEntries".to_string(),
            params: [("maxEntries".to_string(), serde_json::json!(10000))]
                .into_iter()
                .collect(),
        },
    ]
}

// ============================================================================
// Guardrails Service
// ============================================================================

/// The guardrails service for validating operations.
#[derive(Debug)]
pub struct GuardrailsService {
    config: GuardrailsConfig,
    rules: Vec<GuardrailRule>,
}

impl GuardrailsService {
    /// Create a new guardrails service with default config.
    pub fn new() -> Self {
        Self {
            config: GuardrailsConfig::default(),
            rules: builtin_rules(),
        }
    }

    /// Create with custom config.
    pub fn with_config(config: GuardrailsConfig) -> Self {
        Self {
            config,
            rules: builtin_rules(),
        }
    }

    /// Get the current config.
    pub fn config(&self) -> &GuardrailsConfig {
        &self.config
    }

    /// Update the config.
    pub fn update_config(&mut self, config: GuardrailsConfig) {
        self.config = config;
    }

    /// Get all rules.
    pub fn rules(&self) -> &[GuardrailRule] {
        &self.rules
    }

    /// Enable or disable a rule.
    pub fn set_rule_enabled(&mut self, rule_id: &str, enabled: bool) {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == rule_id) {
            rule.enabled = enabled;
        }
    }

    /// Check a single rule against context.
    pub fn check_rule(&self, rule: &GuardrailRule, context: &ValidationContext) -> GuardrailResult {
        if !rule.enabled {
            return create_result(&rule.id, true, "Rule disabled", None, None, None);
        }

        match rule.check_fn.as_str() {
            "checkTokensPerTask" => {
                let limit = rule
                    .params
                    .get("maxTokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(self.config.max_tokens_per_task);
                let current = context.task_tokens.unwrap_or(0);
                if current > limit {
                    create_result(
                        &rule.id,
                        false,
                        &format!("Task tokens ({}) exceed limit ({})", current, limit),
                        Some(rule.severity),
                        Some("Reduce prompt size or split task"),
                        Some(
                            [
                                ("current".to_string(), serde_json::json!(current)),
                                ("limit".to_string(), serde_json::json!(limit)),
                            ]
                            .into_iter()
                            .collect(),
                        ),
                    )
                } else {
                    create_result(&rule.id, true, "Task tokens within limit", None, None, None)
                }
            }
            "checkDurationPerTask" => {
                let limit = rule
                    .params
                    .get("maxDurationMs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(self.config.max_duration_per_task);
                let current = context.task_duration_ms.unwrap_or(0);
                if current > limit {
                    create_result(
                        &rule.id,
                        false,
                        &format!("Task duration ({}ms) exceeds limit ({}ms)", current, limit),
                        Some(rule.severity),
                        Some("Increase timeout or simplify task"),
                        None,
                    )
                } else {
                    create_result(&rule.id, true, "Task duration within limit", None, None, None)
                }
            }
            "checkTokensPerRun" => {
                let limit = rule
                    .params
                    .get("maxTokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(self.config.max_tokens_per_run);
                let current = context.run_tokens.unwrap_or(0);
                if current > limit {
                    create_result(
                        &rule.id,
                        false,
                        &format!("Run tokens ({}) exceed limit ({})", current, limit),
                        Some(rule.severity),
                        Some("End run to avoid excessive token usage"),
                        None,
                    )
                } else {
                    create_result(&rule.id, true, "Run tokens within limit", None, None, None)
                }
            }
            "checkDurationPerRun" => {
                let limit = rule
                    .params
                    .get("maxDurationMs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(self.config.max_duration_per_run);
                let current = context.run_duration_ms.unwrap_or(0);
                if current > limit {
                    create_result(
                        &rule.id,
                        false,
                        &format!("Run duration ({}ms) exceeds limit ({}ms)", current, limit),
                        Some(rule.severity),
                        Some("End run to avoid timeout"),
                        None,
                    )
                } else {
                    create_result(&rule.id, true, "Run duration within limit", None, None, None)
                }
            }
            "checkBlockedFiles" => {
                let files = context.file_paths.as_ref();
                if let Some(files) = files {
                    for file in files {
                        if matches_blocked_pattern(file, &self.config.blocked_patterns) {
                            return create_result(
                                &rule.id,
                                false,
                                &format!("Access to blocked file: {}", file),
                                Some(rule.severity),
                                Some("Remove file from operation"),
                                None,
                            );
                        }
                    }
                }
                create_result(&rule.id, true, "No blocked files accessed", None, None, None)
            }
            "checkNetworkAccess" => {
                let operations = context.network_operations.as_ref();
                if let Some(ops) = operations {
                    if !ops.is_empty() {
                        return create_result(
                            &rule.id,
                            false,
                            &format!("Network access not allowed: {}", ops.join(", ")),
                            Some(rule.severity),
                            Some("Disable network operations"),
                            None,
                        );
                    }
                }
                create_result(&rule.id, true, "No network access", None, None, None)
            }
            "checkSuccessRate" => {
                let min_rate = rule
                    .params
                    .get("minRate")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(self.config.min_success_rate);
                let current = context.success_rate.unwrap_or(1.0);
                if current < min_rate {
                    create_result(
                        &rule.id,
                        false,
                        &format!(
                            "Success rate ({:.1}%) below minimum ({:.1}%)",
                            current * 100.0,
                            min_rate * 100.0
                        ),
                        Some(rule.severity),
                        Some("Review failing tasks and adjust approach"),
                        None,
                    )
                } else {
                    create_result(&rule.id, true, "Success rate acceptable", None, None, None)
                }
            }
            "checkConsecutiveFailures" => {
                let limit = rule
                    .params
                    .get("maxFailures")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(self.config.max_consecutive_failures as u64) as u32;
                let current = context.consecutive_failures.unwrap_or(0);
                if current >= limit {
                    create_result(
                        &rule.id,
                        false,
                        &format!("Too many consecutive failures ({})", current),
                        Some(rule.severity),
                        Some("Pause and review approach"),
                        None,
                    )
                } else {
                    create_result(
                        &rule.id,
                        true,
                        "Consecutive failures within limit",
                        None,
                        None,
                        None,
                    )
                }
            }
            "checkRetries" => {
                let limit = rule
                    .params
                    .get("maxRetries")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(self.config.max_retries_per_task as u64) as u32;
                let current = context.retry_count.unwrap_or(0);
                if current > limit {
                    create_result(
                        &rule.id,
                        false,
                        &format!("Retries ({}) exceed limit ({})", current, limit),
                        Some(rule.severity),
                        Some("Skip task and move to next"),
                        None,
                    )
                } else {
                    create_result(&rule.id, true, "Retries within limit", None, None, None)
                }
            }
            "checkSkillsPerRun" => {
                let limit = rule
                    .params
                    .get("maxSkills")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(self.config.max_skills_per_run as u64) as u32;
                let current = context.skills_learned.unwrap_or(0);
                if current > limit {
                    create_result(
                        &rule.id,
                        false,
                        &format!("Skills learned ({}) exceed limit ({})", current, limit),
                        Some(rule.severity),
                        Some("Review skill quality before adding more"),
                        None,
                    )
                } else {
                    create_result(&rule.id, true, "Skills within limit", None, None, None)
                }
            }
            "checkMemoryEntries" => {
                let limit = rule
                    .params
                    .get("maxEntries")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(self.config.max_memory_entries);
                let current = context.memory_entries.unwrap_or(0);
                if current > limit {
                    create_result(
                        &rule.id,
                        false,
                        &format!("Memory entries ({}) exceed limit ({})", current, limit),
                        Some(rule.severity),
                        Some("Prune old or low-value memories"),
                        None,
                    )
                } else {
                    create_result(&rule.id, true, "Memory entries within limit", None, None, None)
                }
            }
            _ => create_result(
                &rule.id,
                true,
                &format!("Unknown check function: {}", rule.check_fn),
                None,
                None,
                None,
            ),
        }
    }

    /// Validate a context against all enabled rules.
    pub fn validate(&self, context: &ValidationContext) -> GuardrailStatus {
        let enabled_rules = get_enabled_rules(&self.rules);
        let results: Vec<GuardrailResult> = enabled_rules
            .iter()
            .map(|rule| self.check_rule(rule, context))
            .collect();
        aggregate_results(results)
    }

    /// Validate and return error if blocked.
    pub fn validate_or_fail(&self, context: &ValidationContext) -> Result<GuardrailStatus, GuardrailsError> {
        let status = self.validate(context);

        if self.config.strict_mode && !status.all_passed {
            let violations: Vec<_> = status.results.iter().filter(|r| !r.passed).cloned().collect();
            return Err(GuardrailsError {
                reason: "validation_failed".to_string(),
                message: format!(
                    "Guardrail violations: {}",
                    violations
                        .iter()
                        .map(|v| v.message.clone())
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
                violations,
            });
        }

        if status.should_block {
            let violations: Vec<_> = status
                .results
                .iter()
                .filter(|r| !r.passed && matches!(r.severity, Some(Severity::Error | Severity::Critical)))
                .cloned()
                .collect();
            return Err(GuardrailsError {
                reason: "blocked".to_string(),
                message: format!(
                    "Critical guardrail violations: {}",
                    violations
                        .iter()
                        .map(|v| v.message.clone())
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
                violations,
            });
        }

        Ok(status)
    }

    /// Check if file access is allowed.
    pub fn check_file_access(&self, file_path: &str) -> GuardrailResult {
        if matches_blocked_pattern(file_path, &self.config.blocked_patterns) {
            create_result(
                "blocked-files",
                false,
                &format!("Access to blocked file: {}", file_path),
                Some(Severity::Critical),
                Some("Remove file from operation"),
                None,
            )
        } else {
            create_result("blocked-files", true, "File access allowed", None, None, None)
        }
    }

    /// Check if tokens are within limit.
    pub fn check_tokens(&self, current: u64, limit: u64, scope: &str) -> GuardrailResult {
        let rule_id = if scope == "task" {
            "max-tokens-task"
        } else {
            "max-tokens-run"
        };
        if current > limit {
            create_result(
                rule_id,
                false,
                &format!("{} tokens ({}) exceed limit ({})", scope, current, limit),
                Some(if scope == "task" {
                    Severity::Warning
                } else {
                    Severity::Error
                }),
                None,
                None,
            )
        } else {
            create_result(
                rule_id,
                true,
                &format!("{} tokens within limit", scope),
                None,
                None,
                None,
            )
        }
    }

    /// Check if duration is within limit.
    pub fn check_duration(&self, current: u64, limit: u64, scope: &str) -> GuardrailResult {
        let rule_id = if scope == "task" {
            "max-duration-task"
        } else {
            "max-duration-run"
        };
        if current > limit {
            create_result(
                rule_id,
                false,
                &format!("{} duration ({}ms) exceeds limit ({}ms)", scope, current, limit),
                Some(Severity::Error),
                None,
                None,
            )
        } else {
            create_result(
                rule_id,
                true,
                &format!("{} duration within limit", scope),
                None,
                None,
                None,
            )
        }
    }
}

impl Default for GuardrailsService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Error returned when guardrails block an operation.
#[derive(Debug, Clone)]
pub struct GuardrailsError {
    pub reason: String,
    pub message: String,
    pub violations: Vec<GuardrailResult>,
}

impl std::fmt::Display for GuardrailsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for GuardrailsError {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GuardrailsConfig::default();
        assert_eq!(config.max_tokens_per_task, 50000);
        assert_eq!(config.max_duration_per_task, 300000);
        assert!(!config.strict_mode);
    }

    #[test]
    fn test_matches_blocked_pattern_env() {
        let patterns = vec!["*.env".to_string()];
        assert!(matches_blocked_pattern(".env", &patterns));
        assert!(matches_blocked_pattern("config/.env", &patterns));
        assert!(!matches_blocked_pattern("env.ts", &patterns));
    }

    #[test]
    fn test_matches_blocked_pattern_credentials() {
        let patterns = vec!["*credentials*".to_string()];
        assert!(matches_blocked_pattern("credentials.json", &patterns));
        assert!(matches_blocked_pattern("aws_credentials", &patterns));
        assert!(matches_blocked_pattern("/path/to/credentials/file", &patterns));
        assert!(!matches_blocked_pattern("regular_file.txt", &patterns));
    }

    #[test]
    fn test_matches_blocked_pattern_ssh() {
        let patterns = vec!["~/.ssh/*".to_string()];
        assert!(matches_blocked_pattern("~/.ssh/id_rsa", &patterns));
        assert!(matches_blocked_pattern("~/.ssh/known_hosts", &patterns));
    }

    #[test]
    fn test_create_result() {
        let result = create_result("test-rule", true, "All good", None, None, None);
        assert_eq!(result.rule_id, "test-rule");
        assert!(result.passed);
        assert_eq!(result.message, "All good");
    }

    #[test]
    fn test_aggregate_results() {
        let results = vec![
            create_result("rule1", true, "OK", None, None, None),
            create_result("rule2", false, "Warning", Some(Severity::Warning), None, None),
            create_result("rule3", false, "Error", Some(Severity::Error), None, None),
        ];
        let status = aggregate_results(results);
        assert!(!status.all_passed);
        assert_eq!(status.warnings, 1);
        assert_eq!(status.errors, 1);
        assert!(status.should_block);
    }

    #[test]
    fn test_builtin_rules_count() {
        let rules = builtin_rules();
        assert_eq!(rules.len(), 11);
    }

    #[test]
    fn test_guardrails_service_validate() {
        let service = GuardrailsService::new();
        let context = ValidationContext::default();
        let status = service.validate(&context);
        assert!(status.all_passed);
    }

    #[test]
    fn test_guardrails_service_check_tokens_exceeded() {
        let service = GuardrailsService::new();
        let context = ValidationContext {
            task_tokens: Some(100000), // Exceeds 50000 limit
            ..Default::default()
        };
        let status = service.validate(&context);
        assert!(!status.all_passed);
        assert_eq!(status.warnings, 1);
    }

    #[test]
    fn test_guardrails_service_check_blocked_file() {
        let service = GuardrailsService::new();
        let result = service.check_file_access(".env");
        assert!(!result.passed);
        assert_eq!(result.severity, Some(Severity::Critical));
    }

    #[test]
    fn test_guardrails_service_check_allowed_file() {
        let service = GuardrailsService::new();
        let result = service.check_file_access("src/main.rs");
        assert!(result.passed);
    }

    #[test]
    fn test_guardrails_service_set_rule_enabled() {
        let mut service = GuardrailsService::new();
        service.set_rule_enabled("blocked-files", false);
        let rule = service.rules().iter().find(|r| r.id == "blocked-files").unwrap();
        assert!(!rule.enabled);
    }

    #[test]
    fn test_validate_or_fail_blocked() {
        let service = GuardrailsService::new();
        let context = ValidationContext {
            run_tokens: Some(2000000), // Exceeds 1M limit
            ..Default::default()
        };
        let result = service.validate_or_fail(&context);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.reason, "blocked");
    }

    #[test]
    fn test_get_rules_by_category() {
        let rules = builtin_rules();
        let resource_rules = get_rules_by_category(&rules, RuleCategory::Resource);
        assert_eq!(resource_rules.len(), 4);
    }

    #[test]
    fn test_get_enabled_rules() {
        let rules = builtin_rules();
        let enabled = get_enabled_rules(&rules);
        // All but "no-network-access" are enabled by default
        assert_eq!(enabled.len(), 10);
    }
}
