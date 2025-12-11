//! Healer - Self-healing Subagent
//!
//! A self-healing subagent that wakes up automatically when agent trajectories
//! go off the rails, diagnoses what went wrong, and tries to repair or safely
//! contain the damage.
//!
//! The Healer works by:
//! 1. Detecting failure scenarios from orchestrator events
//! 2. Building context from git status, progress files, and trajectories
//! 3. Planning and executing "spells" (recovery operations)
//! 4. Reporting outcomes and updating counters for rate limiting

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

// ============================================================================
// Healer Scenarios
// ============================================================================

/// Scenarios that trigger Healer invocation.
/// Each scenario maps to a specific failure pattern in the orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HealerScenario {
    /// Init script failed due to TypeScript errors
    InitScriptTypecheckFailure,
    /// Init script failed due to test failures
    InitScriptTestFailure,
    /// Init script failed due to env issues (network, disk, etc.)
    InitScriptEnvironmentFailure,
    /// Post-work verification (tests/typecheck) failed
    VerificationFailed,
    /// A subtask failed during execution
    SubtaskFailed,
    /// A subtask is stuck (no progress for N minutes)
    SubtaskStuck,
    /// Unexpected runtime error in orchestrator
    RuntimeError,
}

impl std::fmt::Display for HealerScenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealerScenario::InitScriptTypecheckFailure => write!(f, "InitScriptTypecheckFailure"),
            HealerScenario::InitScriptTestFailure => write!(f, "InitScriptTestFailure"),
            HealerScenario::InitScriptEnvironmentFailure => write!(f, "InitScriptEnvironmentFailure"),
            HealerScenario::VerificationFailed => write!(f, "VerificationFailed"),
            HealerScenario::SubtaskFailed => write!(f, "SubtaskFailed"),
            HealerScenario::SubtaskStuck => write!(f, "SubtaskStuck"),
            HealerScenario::RuntimeError => write!(f, "RuntimeError"),
        }
    }
}

// ============================================================================
// Healer Spells
// ============================================================================

/// Spell identifiers - each spell is a controlled recovery operation.
/// Spells are designed to never make things worse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealerSpellId {
    /// git restore + git clean -fd
    RewindUncommittedChanges,
    /// git reset to last known-good commit
    RewindToLastGreenCommit,
    /// Mark task blocked, create follow-up
    MarkTaskBlockedWithFollowup,
    /// Fall back to minimal subagent
    RetryWithMinimalSubagent,
    /// Resume Claude Code session for recovery
    RetryWithClaudeCodeResume,
    /// Emergency typecheck fix (from safe-mode)
    FixTypecheckErrors,
    /// Emergency test fix
    FixTestErrors,
    /// Update progress.md with failure details
    UpdateProgressWithGuidance,
    /// Validate tasks.jsonl structure
    RunTasksDoctorLikeChecks,
}

impl HealerSpellId {
    /// Check if this spell requires LLM invocation.
    pub fn requires_llm(&self) -> bool {
        matches!(
            self,
            HealerSpellId::FixTypecheckErrors
                | HealerSpellId::FixTestErrors
                | HealerSpellId::RetryWithClaudeCodeResume
                | HealerSpellId::RetryWithMinimalSubagent
        )
    }
}

impl std::fmt::Display for HealerSpellId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealerSpellId::RewindUncommittedChanges => write!(f, "rewind_uncommitted_changes"),
            HealerSpellId::RewindToLastGreenCommit => write!(f, "rewind_to_last_green_commit"),
            HealerSpellId::MarkTaskBlockedWithFollowup => write!(f, "mark_task_blocked_with_followup"),
            HealerSpellId::RetryWithMinimalSubagent => write!(f, "retry_with_minimal_subagent"),
            HealerSpellId::RetryWithClaudeCodeResume => write!(f, "retry_with_claude_code_resume"),
            HealerSpellId::FixTypecheckErrors => write!(f, "fix_typecheck_errors"),
            HealerSpellId::FixTestErrors => write!(f, "fix_test_errors"),
            HealerSpellId::UpdateProgressWithGuidance => write!(f, "update_progress_with_guidance"),
            HealerSpellId::RunTasksDoctorLikeChecks => write!(f, "run_tasks_doctor_like_checks"),
        }
    }
}

/// Result from executing a spell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerSpellResult {
    /// Whether the spell executed successfully
    pub success: bool,
    /// Whether the spell made changes to the repo/tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changes_applied: Option<bool>,
    /// Human-readable summary of what happened
    pub summary: String,
    /// Optional error message if spell failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Files modified by this spell
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files_modified: Option<Vec<String>>,
    /// Tasks created or modified by this spell
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks_affected: Option<Vec<String>>,
}

impl HealerSpellResult {
    /// Create a successful spell result.
    pub fn success(summary: impl Into<String>) -> Self {
        Self {
            success: true,
            changes_applied: None,
            summary: summary.into(),
            error: None,
            files_modified: None,
            tasks_affected: None,
        }
    }

    /// Create a failed spell result.
    pub fn failure(summary: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            success: false,
            changes_applied: None,
            summary: summary.into(),
            error: Some(error.into()),
            files_modified: None,
            tasks_affected: None,
        }
    }
}

// ============================================================================
// Healer Outcome
// ============================================================================

/// Outcome status after Healer runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealerOutcomeStatus {
    /// Problem fixed, orchestrator can continue
    Resolved,
    /// Problem contained (task blocked, progress updated), stop gracefully
    Contained,
    /// Could not fix or contain, orchestrator should abort
    Unresolved,
    /// Healer chose not to run (policy/limits)
    Skipped,
}

/// Overall result from a Healer invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerOutcome {
    /// The scenario that triggered Healer
    pub scenario: HealerScenario,
    /// Final status
    pub status: HealerOutcomeStatus,
    /// Spells that were attempted
    pub spells_tried: Vec<HealerSpellId>,
    /// Spells that succeeded
    pub spells_succeeded: Vec<HealerSpellId>,
    /// Human-readable summary
    pub summary: String,
    /// Whether verification passed after healing (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_passed: Option<bool>,
    /// ATIF trajectory session ID for this Healer invocation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trajectory_session_id: Option<String>,
}

impl HealerOutcome {
    /// Check if the problem was resolved.
    pub fn is_resolved(&self) -> bool {
        self.status == HealerOutcomeStatus::Resolved
    }

    /// Check if the problem was contained.
    pub fn is_contained(&self) -> bool {
        self.status == HealerOutcomeStatus::Contained
    }
}

/// A persisted record of a healing attempt for deduplication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingAttempt {
    /// Unique deduplication key for this failure
    pub key: String,
    /// Scenario that triggered the attempt
    pub scenario: HealerScenario,
    /// Task identifier the attempt belonged to
    pub task_id: String,
    /// Subtask identifier if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtask_id: Option<String>,
    /// Hash of the error output used in the dedup key
    pub error_hash: String,
    /// When the attempt finished
    pub timestamp: String,
    /// Outcome status recorded for this attempt
    pub outcome: HealerOutcomeStatus,
    /// Spells tried for this attempt
    pub spells_tried: Vec<HealerSpellId>,
    /// Spells that succeeded during this attempt
    pub spells_succeeded: Vec<HealerSpellId>,
    /// Summary of the attempt result
    pub summary: String,
}

// ============================================================================
// Git Status
// ============================================================================

/// Git repository status for Healer context.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitStatus {
    /// Whether the repo has uncommitted changes
    pub is_dirty: bool,
    /// List of modified files
    pub modified_files: Vec<String>,
    /// List of untracked files
    pub untracked_files: Vec<String>,
    /// Current branch name
    pub current_branch: String,
    /// Last commit SHA
    pub last_commit_sha: String,
    /// Last commit message
    pub last_commit_message: String,
}

// ============================================================================
// Healer Heuristics
// ============================================================================

/// Heuristics computed from available data to inform spell selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerHeuristics {
    /// The detected scenario
    pub scenario: HealerScenario,
    /// Number of times this subtask has failed
    pub failure_count: u32,
    /// Whether this appears to be a flaky failure (different errors each time)
    pub is_flaky: bool,
    /// Whether the failure is likely due to missing imports
    pub has_missing_imports: bool,
    /// Whether the failure is likely due to type errors
    pub has_type_errors: bool,
    /// Whether the failure is likely due to test assertions
    pub has_test_assertions: bool,
    /// Detected error patterns from output
    pub error_patterns: Vec<String>,
    /// Previous attempts made (if any)
    pub previous_attempts: u32,
}

impl Default for HealerHeuristics {
    fn default() -> Self {
        Self {
            scenario: HealerScenario::RuntimeError,
            failure_count: 0,
            is_flaky: false,
            has_missing_imports: false,
            has_type_errors: false,
            has_test_assertions: false,
            error_patterns: Vec::new(),
            previous_attempts: 0,
        }
    }
}

// ============================================================================
// Healer Counters
// ============================================================================

/// Counters to track Healer invocations for rate limiting.
#[derive(Debug, Clone, Default)]
pub struct HealerCounters {
    /// Total Healer invocations this session
    pub session_invocations: u32,
    /// Healer invocations per subtask (keyed by subtask ID)
    pub subtask_invocations: HashMap<String, u32>,
    /// Spells attempted this session (keyed by spell ID)
    pub spells_attempted: HashMap<HealerSpellId, u32>,
    /// Follow-up containment tasks created (keyed by taskId:scenario)
    pub followup_keys: HashSet<String>,
    /// Healing attempts keyed by deduplication key
    pub healing_attempts: HashMap<String, HealingAttempt>,
}

impl HealerCounters {
    /// Create fresh counters for a new session.
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment session invocation count.
    pub fn increment_session(&mut self) {
        self.session_invocations += 1;
    }

    /// Increment subtask invocation count.
    pub fn increment_subtask(&mut self, subtask_id: &str) {
        *self.subtask_invocations.entry(subtask_id.to_string()).or_insert(0) += 1;
    }

    /// Increment spell attempt count.
    pub fn increment_spell(&mut self, spell_id: HealerSpellId) {
        *self.spells_attempted.entry(spell_id).or_insert(0) += 1;
    }

    /// Get subtask invocation count.
    pub fn get_subtask_count(&self, subtask_id: &str) -> u32 {
        *self.subtask_invocations.get(subtask_id).unwrap_or(&0)
    }

    /// Record a healing attempt.
    pub fn record_attempt(&mut self, attempt: HealingAttempt) {
        self.healing_attempts.insert(attempt.key.clone(), attempt);
    }

    /// Check if an attempt already exists.
    pub fn has_attempt(&self, key: &str) -> bool {
        self.healing_attempts.contains_key(key)
    }
}

// ============================================================================
// Healer Config
// ============================================================================

/// Configuration for the Healer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerConfig {
    /// Whether Healer is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum invocations per session
    #[serde(default = "default_max_session")]
    pub max_invocations_per_session: u32,
    /// Maximum invocations per subtask
    #[serde(default = "default_max_subtask")]
    pub max_invocations_per_subtask: u32,
    /// Scenario toggles
    #[serde(default)]
    pub scenarios: HealerScenarioConfig,
    /// Allowed spells (if empty, all are allowed)
    #[serde(default)]
    pub allowed_spells: Vec<HealerSpellId>,
    /// Forbidden spells
    #[serde(default)]
    pub forbidden_spells: Vec<HealerSpellId>,
}

fn default_true() -> bool { true }
fn default_max_session() -> u32 { 2 }
fn default_max_subtask() -> u32 { 1 }

impl Default for HealerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_invocations_per_session: 2,
            max_invocations_per_subtask: 1,
            scenarios: HealerScenarioConfig::default(),
            allowed_spells: Vec::new(),
            forbidden_spells: Vec::new(),
        }
    }
}

/// Scenario-specific toggles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerScenarioConfig {
    #[serde(default = "default_true")]
    pub on_init_failure: bool,
    #[serde(default = "default_true")]
    pub on_verification_failure: bool,
    #[serde(default = "default_true")]
    pub on_subtask_failure: bool,
    #[serde(default)]
    pub on_stuck_subtask: bool,
    #[serde(default = "default_true")]
    pub on_runtime_error: bool,
}

impl Default for HealerScenarioConfig {
    fn default() -> Self {
        Self {
            on_init_failure: true,
            on_verification_failure: true,
            on_subtask_failure: true,
            on_stuck_subtask: false,
            on_runtime_error: true,
        }
    }
}

// ============================================================================
// Policy Decision
// ============================================================================

/// Result of checking whether Healer should run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerPolicyDecision {
    /// Whether Healer should run
    pub run: bool,
    /// The detected scenario (if run=true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario: Option<HealerScenario>,
    /// Reason for the decision (especially if run=false)
    pub reason: String,
}

impl HealerPolicyDecision {
    /// Create a decision to run Healer.
    pub fn should_run(scenario: HealerScenario, reason: impl Into<String>) -> Self {
        Self {
            run: true,
            scenario: Some(scenario),
            reason: reason.into(),
        }
    }

    /// Create a decision to skip Healer.
    pub fn skip(reason: impl Into<String>) -> Self {
        Self {
            run: false,
            scenario: None,
            reason: reason.into(),
        }
    }

    /// Create a decision to skip with known scenario.
    pub fn skip_scenario(scenario: HealerScenario, reason: impl Into<String>) -> Self {
        Self {
            run: false,
            scenario: Some(scenario),
            reason: reason.into(),
        }
    }
}

// ============================================================================
// Policy Functions
// ============================================================================

/// Check if a scenario is enabled in the config.
pub fn is_scenario_enabled(scenario: HealerScenario, config: &HealerConfig) -> bool {
    match scenario {
        HealerScenario::InitScriptTypecheckFailure
        | HealerScenario::InitScriptTestFailure
        | HealerScenario::InitScriptEnvironmentFailure => config.scenarios.on_init_failure,
        HealerScenario::VerificationFailed => config.scenarios.on_verification_failure,
        HealerScenario::SubtaskFailed => config.scenarios.on_subtask_failure,
        HealerScenario::SubtaskStuck => config.scenarios.on_stuck_subtask,
        HealerScenario::RuntimeError => config.scenarios.on_runtime_error,
    }
}

/// Check if Healer has exceeded session invocation limit.
pub fn has_exceeded_session_limit(counters: &HealerCounters, config: &HealerConfig) -> bool {
    counters.session_invocations >= config.max_invocations_per_session
}

/// Check if Healer has exceeded per-subtask invocation limit.
pub fn has_exceeded_subtask_limit(
    subtask_id: Option<&str>,
    counters: &HealerCounters,
    config: &HealerConfig,
) -> bool {
    match subtask_id {
        Some(id) => counters.get_subtask_count(id) >= config.max_invocations_per_subtask,
        None => false,
    }
}

/// Determine if Healer should run for a given scenario.
pub fn should_run_healer(
    scenario: HealerScenario,
    config: &HealerConfig,
    counters: &HealerCounters,
    subtask_id: Option<&str>,
) -> HealerPolicyDecision {
    // 1. Check if Healer is enabled
    if !config.enabled {
        return HealerPolicyDecision::skip("Healer is disabled in config");
    }

    // 2. Check if scenario is enabled
    if !is_scenario_enabled(scenario, config) {
        return HealerPolicyDecision::skip_scenario(
            scenario,
            format!("Scenario '{}' is disabled in config", scenario),
        );
    }

    // 3. Check session limit
    if has_exceeded_session_limit(counters, config) {
        return HealerPolicyDecision::skip_scenario(
            scenario,
            format!(
                "Session limit reached ({}/{})",
                counters.session_invocations, config.max_invocations_per_session
            ),
        );
    }

    // 4. Check subtask limit
    if scenario == HealerScenario::SubtaskFailed {
        if has_exceeded_subtask_limit(subtask_id, counters, config) {
            let count = subtask_id.map(|id| counters.get_subtask_count(id)).unwrap_or(0);
            return HealerPolicyDecision::skip_scenario(
                scenario,
                format!(
                    "Subtask limit reached for '{}' ({}/{})",
                    subtask_id.unwrap_or("unknown"),
                    count,
                    config.max_invocations_per_subtask
                ),
            );
        }
    }

    // 5. All checks passed
    HealerPolicyDecision::should_run(scenario, format!("Triggering Healer for scenario '{}'", scenario))
}

// ============================================================================
// Spell Planning
// ============================================================================

/// Get the default spell sequence for a scenario.
pub fn get_scenario_spells(scenario: HealerScenario) -> Vec<HealerSpellId> {
    match scenario {
        HealerScenario::InitScriptTypecheckFailure => vec![
            HealerSpellId::FixTypecheckErrors,
            HealerSpellId::UpdateProgressWithGuidance,
            HealerSpellId::MarkTaskBlockedWithFollowup,
        ],
        HealerScenario::InitScriptTestFailure => vec![
            HealerSpellId::FixTestErrors,
            HealerSpellId::UpdateProgressWithGuidance,
            HealerSpellId::MarkTaskBlockedWithFollowup,
        ],
        HealerScenario::InitScriptEnvironmentFailure => vec![
            HealerSpellId::UpdateProgressWithGuidance,
            HealerSpellId::MarkTaskBlockedWithFollowup,
        ],
        HealerScenario::SubtaskFailed => vec![
            HealerSpellId::RewindUncommittedChanges,
            HealerSpellId::UpdateProgressWithGuidance,
            HealerSpellId::MarkTaskBlockedWithFollowup,
        ],
        HealerScenario::VerificationFailed => vec![
            HealerSpellId::RewindUncommittedChanges,
            HealerSpellId::UpdateProgressWithGuidance,
        ],
        HealerScenario::RuntimeError => vec![
            HealerSpellId::RewindUncommittedChanges,
            HealerSpellId::UpdateProgressWithGuidance,
            HealerSpellId::MarkTaskBlockedWithFollowup,
        ],
        HealerScenario::SubtaskStuck => vec![
            HealerSpellId::UpdateProgressWithGuidance,
            HealerSpellId::MarkTaskBlockedWithFollowup,
        ],
    }
}

/// Options for spell planning.
#[derive(Debug, Clone, Default)]
pub struct PlanSpellsOptions {
    /// Skip spells that require LLM invocation
    pub skip_llm_spells: bool,
    /// Maximum number of spells to return
    pub max_spells: Option<usize>,
}

/// Plan which spells to execute for a given scenario.
pub fn plan_spells(
    scenario: HealerScenario,
    config: &HealerConfig,
    options: &PlanSpellsOptions,
) -> Vec<HealerSpellId> {
    let mut spells = get_scenario_spells(scenario);

    // Filter by allowed/forbidden
    if !config.allowed_spells.is_empty() {
        spells.retain(|s| config.allowed_spells.contains(s));
    }
    spells.retain(|s| !config.forbidden_spells.contains(s));

    // Optionally skip LLM spells
    if options.skip_llm_spells {
        spells.retain(|s| !s.requires_llm());
    }

    // Apply max limit
    if let Some(max) = options.max_spells {
        spells.truncate(max);
    }

    spells
}

// ============================================================================
// Error Pattern Detection
// ============================================================================

/// Detect error patterns in output.
pub fn detect_error_patterns(output: &str) -> Vec<String> {
    let mut patterns = Vec::new();

    // TypeScript errors
    if output.contains("error TS") {
        patterns.push("TypeScript compilation error".to_string());
    }
    if output.to_lowercase().contains("cannot find module")
        || output.to_lowercase().contains("cannot find name")
    {
        patterns.push("Missing module or name".to_string());
    }
    if output.contains("does not exist on type") {
        patterns.push("Property access error".to_string());
    }
    if output.contains("is not assignable to type") {
        patterns.push("Type assignment error".to_string());
    }

    // Test failures
    if output.contains("test") && output.contains("failed") {
        patterns.push("Test failures".to_string());
    }
    if output.contains("expect(") && (output.contains("toBe") || output.contains("toEqual")) {
        patterns.push("Assertion failure".to_string());
    }

    // Import/export issues
    if output.contains("import") && output.to_lowercase().contains("not found") {
        patterns.push("Import resolution error".to_string());
    }

    // Runtime errors
    if output.contains("TypeError:") {
        patterns.push("Runtime type error".to_string());
    }
    if output.contains("ReferenceError:") {
        patterns.push("Reference error".to_string());
    }
    if output.contains("SyntaxError:") {
        patterns.push("Syntax error".to_string());
    }

    patterns
}

/// Build heuristics from error output.
pub fn build_heuristics(
    scenario: HealerScenario,
    error_output: Option<&str>,
    failure_count: u32,
) -> HealerHeuristics {
    let patterns = error_output.map(detect_error_patterns).unwrap_or_default();

    HealerHeuristics {
        scenario,
        failure_count,
        is_flaky: false,
        has_missing_imports: patterns.iter().any(|p| p.contains("Import") || p.contains("module")),
        has_type_errors: patterns.iter().any(|p| p.contains("Type") || p.contains("TypeScript")),
        has_test_assertions: patterns.iter().any(|p| p.contains("Assertion") || p.contains("Test")),
        error_patterns: patterns,
        previous_attempts: 0,
    }
}

// ============================================================================
// Stuck Detection Types
// ============================================================================

/// Configuration for stuck detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StuckDetectionConfig {
    /// Hours before a task is considered stuck (default: 4)
    #[serde(default = "default_task_hours")]
    pub stuck_task_threshold_hours: f64,
    /// Hours before a subtask is considered stuck (default: 2)
    #[serde(default = "default_subtask_hours")]
    pub stuck_subtask_threshold_hours: f64,
    /// Minimum consecutive failures to flag as stuck pattern (default: 3)
    #[serde(default = "default_min_failures")]
    pub min_consecutive_failures: u32,
    /// Whether to scan ATIF trajectories for failure patterns
    #[serde(default = "default_true")]
    pub scan_trajectories: bool,
}

fn default_task_hours() -> f64 { 4.0 }
fn default_subtask_hours() -> f64 { 2.0 }
fn default_min_failures() -> u32 { 3 }

impl Default for StuckDetectionConfig {
    fn default() -> Self {
        Self {
            stuck_task_threshold_hours: 4.0,
            stuck_subtask_threshold_hours: 2.0,
            min_consecutive_failures: 3,
            scan_trajectories: true,
        }
    }
}

/// Reasons why something is considered stuck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StuckReason {
    TimeThresholdExceeded,
    ConsecutiveFailures,
    RepeatedSameError,
    NoProgress,
    ManualFlag,
}

/// A detected failure pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    /// The error message/pattern
    pub pattern: String,
    /// How many times this pattern occurred
    pub occurrences: u32,
    /// Session IDs where this pattern was found
    pub session_ids: Vec<String>,
}

/// Information about a stuck task.
#[derive(Debug, Clone)]
pub struct StuckTaskInfo {
    /// Task ID
    pub task_id: String,
    /// Task title
    pub task_title: String,
    /// Reason why it's stuck
    pub reason: StuckReason,
    /// How long the task has been in_progress (hours)
    pub hours_stuck: f64,
    /// Related failure patterns found
    pub failure_patterns: Vec<FailurePattern>,
}

/// Information about a stuck subtask.
#[derive(Debug, Clone)]
pub struct StuckSubtaskInfo {
    /// Subtask ID
    pub subtask_id: String,
    /// Subtask description
    pub subtask_description: String,
    /// Parent task ID
    pub task_id: String,
    /// Reason why it's stuck
    pub reason: StuckReason,
    /// How long the subtask has been in_progress (hours)
    pub hours_stuck: f64,
    /// Consecutive failure count
    pub failure_count: u32,
}

/// Result from stuck detection scan.
#[derive(Debug, Clone, Default)]
pub struct StuckDetectionResult {
    /// Tasks that appear stuck
    pub stuck_tasks: Vec<StuckTaskInfo>,
    /// Subtasks that appear stuck
    pub stuck_subtasks: Vec<StuckSubtaskInfo>,
    /// Summary statistics
    pub stats: StuckStats,
}

/// Statistics from stuck detection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StuckStats {
    pub tasks_scanned: u32,
    pub subtasks_scanned: u32,
    pub trajectories_scanned: u32,
    pub stuck_task_count: u32,
    pub stuck_subtask_count: u32,
}

// ============================================================================
// Stuck Detection Functions
// ============================================================================

/// Check if a subtask is stuck based on failure count.
pub fn is_subtask_stuck_by_failures(failure_count: u32, config: &StuckDetectionConfig) -> bool {
    failure_count >= config.min_consecutive_failures
}

/// Check if a subtask is stuck based on time.
pub fn is_subtask_stuck_by_time(
    hours_elapsed: f64,
    config: &StuckDetectionConfig,
) -> bool {
    hours_elapsed >= config.stuck_subtask_threshold_hours
}

/// Normalize an error message to create a pattern key.
pub fn normalize_error_pattern(error: &str) -> String {
    let normalized = error
        // Remove timestamps
        .replace(|c: char| c.is_ascii_digit(), "")
        // Remove extra whitespace
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if normalized.len() > 200 {
        normalized[..200].to_string()
    } else {
        normalized
    }
}

/// Summarize stuck detection results.
pub fn summarize_stuck_detection(result: &StuckDetectionResult) -> String {
    let mut lines = Vec::new();

    lines.push(format!(
        "Scanned {} tasks, {} subtasks, {} trajectories",
        result.stats.tasks_scanned,
        result.stats.subtasks_scanned,
        result.stats.trajectories_scanned
    ));

    if result.stuck_tasks.is_empty() && result.stuck_subtasks.is_empty() {
        lines.push("No stuck items detected.".to_string());
        return lines.join("\n");
    }

    if !result.stuck_tasks.is_empty() {
        lines.push(format!("\nStuck tasks ({}):", result.stuck_tasks.len()));
        for stuck in &result.stuck_tasks {
            lines.push(format!(
                "  - {}: {} ({:.1}h, {:?})",
                stuck.task_id, stuck.task_title, stuck.hours_stuck, stuck.reason
            ));
        }
    }

    if !result.stuck_subtasks.is_empty() {
        lines.push(format!("\nStuck subtasks ({}):", result.stuck_subtasks.len()));
        for stuck in &result.stuck_subtasks {
            let desc = if stuck.subtask_description.len() > 50 {
                format!("{}...", &stuck.subtask_description[..50])
            } else {
                stuck.subtask_description.clone()
            };
            lines.push(format!(
                "  - {}: {} ({:?}, {} failures)",
                stuck.subtask_id, desc, stuck.reason, stuck.failure_count
            ));
        }
    }

    lines.join("\n")
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Hash an error string for deduplication keys.
pub fn hash_error(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex_encode(&result[..8])
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Build a healing deduplication key.
pub fn build_healing_key(
    scenario: HealerScenario,
    task_id: &str,
    subtask_id: Option<&str>,
    error_output: &str,
) -> (String, String) {
    let error_hash = hash_error(&format!("{}:{}", scenario, error_output));
    let key = format!(
        "{}:{}:{}:{}",
        task_id,
        subtask_id.unwrap_or("none"),
        scenario,
        error_hash
    );
    (key, error_hash)
}

/// Create a healing attempt record.
pub fn create_healing_attempt(
    key: String,
    scenario: HealerScenario,
    task_id: String,
    subtask_id: Option<String>,
    error_hash: String,
    outcome: HealerOutcomeStatus,
    spells_tried: Vec<HealerSpellId>,
    spells_succeeded: Vec<HealerSpellId>,
    summary: String,
) -> HealingAttempt {
    HealingAttempt {
        key,
        scenario,
        task_id,
        subtask_id,
        error_hash,
        timestamp: Utc::now().to_rfc3339(),
        outcome,
        spells_tried,
        spells_succeeded,
        summary,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_healer_scenario_display() {
        assert_eq!(
            HealerScenario::InitScriptTypecheckFailure.to_string(),
            "InitScriptTypecheckFailure"
        );
        assert_eq!(HealerScenario::SubtaskFailed.to_string(), "SubtaskFailed");
    }

    #[test]
    fn test_healer_spell_requires_llm() {
        assert!(HealerSpellId::FixTypecheckErrors.requires_llm());
        assert!(HealerSpellId::FixTestErrors.requires_llm());
        assert!(!HealerSpellId::RewindUncommittedChanges.requires_llm());
        assert!(!HealerSpellId::UpdateProgressWithGuidance.requires_llm());
    }

    #[test]
    fn test_healer_spell_result() {
        let success = HealerSpellResult::success("Fixed the issue");
        assert!(success.success);
        assert!(success.error.is_none());

        let failure = HealerSpellResult::failure("Could not fix", "TypeScript error");
        assert!(!failure.success);
        assert_eq!(failure.error, Some("TypeScript error".to_string()));
    }

    #[test]
    fn test_healer_outcome() {
        let outcome = HealerOutcome {
            scenario: HealerScenario::SubtaskFailed,
            status: HealerOutcomeStatus::Resolved,
            spells_tried: vec![HealerSpellId::RewindUncommittedChanges],
            spells_succeeded: vec![HealerSpellId::RewindUncommittedChanges],
            summary: "Reverted changes".to_string(),
            verification_passed: Some(true),
            trajectory_session_id: None,
        };

        assert!(outcome.is_resolved());
        assert!(!outcome.is_contained());
    }

    #[test]
    fn test_healer_counters() {
        let mut counters = HealerCounters::new();
        assert_eq!(counters.session_invocations, 0);

        counters.increment_session();
        counters.increment_session();
        assert_eq!(counters.session_invocations, 2);

        counters.increment_subtask("sub-1");
        counters.increment_subtask("sub-1");
        counters.increment_subtask("sub-2");
        assert_eq!(counters.get_subtask_count("sub-1"), 2);
        assert_eq!(counters.get_subtask_count("sub-2"), 1);
        assert_eq!(counters.get_subtask_count("sub-3"), 0);
    }

    #[test]
    fn test_is_scenario_enabled() {
        let config = HealerConfig::default();

        assert!(is_scenario_enabled(HealerScenario::SubtaskFailed, &config));
        assert!(is_scenario_enabled(HealerScenario::RuntimeError, &config));
        assert!(!is_scenario_enabled(HealerScenario::SubtaskStuck, &config));
    }

    #[test]
    fn test_has_exceeded_session_limit() {
        let config = HealerConfig {
            max_invocations_per_session: 2,
            ..Default::default()
        };

        let mut counters = HealerCounters::new();
        assert!(!has_exceeded_session_limit(&counters, &config));

        counters.session_invocations = 2;
        assert!(has_exceeded_session_limit(&counters, &config));
    }

    #[test]
    fn test_should_run_healer_disabled() {
        let config = HealerConfig {
            enabled: false,
            ..Default::default()
        };
        let counters = HealerCounters::new();

        let decision = should_run_healer(HealerScenario::SubtaskFailed, &config, &counters, None);
        assert!(!decision.run);
        assert!(decision.reason.contains("disabled"));
    }

    #[test]
    fn test_should_run_healer_enabled() {
        let config = HealerConfig::default();
        let counters = HealerCounters::new();

        let decision = should_run_healer(HealerScenario::SubtaskFailed, &config, &counters, Some("sub-1"));
        assert!(decision.run);
        assert_eq!(decision.scenario, Some(HealerScenario::SubtaskFailed));
    }

    #[test]
    fn test_should_run_healer_session_limit() {
        let config = HealerConfig {
            max_invocations_per_session: 2,
            ..Default::default()
        };
        let mut counters = HealerCounters::new();
        counters.session_invocations = 2;

        let decision = should_run_healer(HealerScenario::SubtaskFailed, &config, &counters, None);
        assert!(!decision.run);
        assert!(decision.reason.contains("Session limit"));
    }

    #[test]
    fn test_get_scenario_spells() {
        let spells = get_scenario_spells(HealerScenario::SubtaskFailed);
        assert!(spells.contains(&HealerSpellId::RewindUncommittedChanges));
        assert!(spells.contains(&HealerSpellId::UpdateProgressWithGuidance));
    }

    #[test]
    fn test_plan_spells_filters() {
        let config = HealerConfig {
            forbidden_spells: vec![HealerSpellId::MarkTaskBlockedWithFollowup],
            ..Default::default()
        };
        let options = PlanSpellsOptions::default();

        let spells = plan_spells(HealerScenario::SubtaskFailed, &config, &options);
        assert!(!spells.contains(&HealerSpellId::MarkTaskBlockedWithFollowup));
    }

    #[test]
    fn test_plan_spells_skip_llm() {
        let config = HealerConfig::default();
        let options = PlanSpellsOptions {
            skip_llm_spells: true,
            max_spells: None,
        };

        let spells = plan_spells(HealerScenario::InitScriptTypecheckFailure, &config, &options);
        assert!(!spells.contains(&HealerSpellId::FixTypecheckErrors));
    }

    #[test]
    fn test_detect_error_patterns() {
        let output = "error TS2304: Cannot find name 'foo'";
        let patterns = detect_error_patterns(output);
        assert!(patterns.iter().any(|p| p.contains("TypeScript")));
        assert!(patterns.iter().any(|p| p.contains("Missing")));
    }

    #[test]
    fn test_detect_error_patterns_tests() {
        let output = "5 tests failed";
        let patterns = detect_error_patterns(output);
        assert!(patterns.iter().any(|p| p.contains("Test")));
    }

    #[test]
    fn test_build_heuristics() {
        let heuristics = build_heuristics(
            HealerScenario::InitScriptTypecheckFailure,
            Some("error TS2304: Cannot find name"),
            3,
        );

        assert_eq!(heuristics.scenario, HealerScenario::InitScriptTypecheckFailure);
        assert_eq!(heuristics.failure_count, 3);
        assert!(heuristics.has_type_errors);
    }

    #[test]
    fn test_stuck_detection_config_defaults() {
        let config = StuckDetectionConfig::default();
        assert_eq!(config.stuck_task_threshold_hours, 4.0);
        assert_eq!(config.stuck_subtask_threshold_hours, 2.0);
        assert_eq!(config.min_consecutive_failures, 3);
    }

    #[test]
    fn test_is_subtask_stuck_by_failures() {
        let config = StuckDetectionConfig::default();
        assert!(!is_subtask_stuck_by_failures(2, &config));
        assert!(is_subtask_stuck_by_failures(3, &config));
        assert!(is_subtask_stuck_by_failures(5, &config));
    }

    #[test]
    fn test_is_subtask_stuck_by_time() {
        let config = StuckDetectionConfig::default();
        assert!(!is_subtask_stuck_by_time(1.5, &config));
        assert!(is_subtask_stuck_by_time(2.0, &config));
        assert!(is_subtask_stuck_by_time(3.0, &config));
    }

    #[test]
    fn test_summarize_stuck_detection_empty() {
        let result = StuckDetectionResult::default();
        let summary = summarize_stuck_detection(&result);
        assert!(summary.contains("No stuck items"));
    }

    #[test]
    fn test_summarize_stuck_detection_with_items() {
        let result = StuckDetectionResult {
            stuck_tasks: vec![StuckTaskInfo {
                task_id: "task-1".to_string(),
                task_title: "Fix bug".to_string(),
                reason: StuckReason::TimeThresholdExceeded,
                hours_stuck: 5.5,
                failure_patterns: vec![],
            }],
            stuck_subtasks: vec![],
            stats: StuckStats {
                tasks_scanned: 10,
                subtasks_scanned: 5,
                trajectories_scanned: 20,
                stuck_task_count: 1,
                stuck_subtask_count: 0,
            },
        };
        let summary = summarize_stuck_detection(&result);
        assert!(summary.contains("task-1"));
        assert!(summary.contains("5.5h"));
    }

    #[test]
    fn test_hash_error() {
        let hash1 = hash_error("error TS2304");
        let hash2 = hash_error("error TS2304");
        let hash3 = hash_error("different error");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 16); // 8 bytes = 16 hex chars
    }

    #[test]
    fn test_build_healing_key() {
        let (key, error_hash) = build_healing_key(
            HealerScenario::SubtaskFailed,
            "task-1",
            Some("sub-1"),
            "Type error",
        );

        assert!(key.contains("task-1"));
        assert!(key.contains("sub-1"));
        assert!(key.contains("SubtaskFailed"));
        assert!(!error_hash.is_empty());
    }

    #[test]
    fn test_create_healing_attempt() {
        let attempt = create_healing_attempt(
            "key-1".to_string(),
            HealerScenario::SubtaskFailed,
            "task-1".to_string(),
            Some("sub-1".to_string()),
            "hash123".to_string(),
            HealerOutcomeStatus::Resolved,
            vec![HealerSpellId::RewindUncommittedChanges],
            vec![HealerSpellId::RewindUncommittedChanges],
            "Fixed".to_string(),
        );

        assert_eq!(attempt.key, "key-1");
        assert_eq!(attempt.task_id, "task-1");
        assert_eq!(attempt.outcome, HealerOutcomeStatus::Resolved);
    }

    #[test]
    fn test_healer_policy_decision() {
        let run = HealerPolicyDecision::should_run(HealerScenario::SubtaskFailed, "Testing");
        assert!(run.run);
        assert_eq!(run.scenario, Some(HealerScenario::SubtaskFailed));

        let skip = HealerPolicyDecision::skip("Disabled");
        assert!(!skip.run);
        assert!(skip.scenario.is_none());
    }

    #[test]
    fn test_normalize_error_pattern() {
        let normalized = normalize_error_pattern("Error at line 42: type error");
        assert!(!normalized.contains("42"));
        assert!(normalized.contains("Error"));
    }

    #[test]
    fn test_outcome_status_serialization() {
        let json = serde_json::to_string(&HealerOutcomeStatus::Resolved).unwrap();
        assert_eq!(json, "\"resolved\"");

        let json = serde_json::to_string(&HealerOutcomeStatus::Contained).unwrap();
        assert_eq!(json, "\"contained\"");
    }
}
