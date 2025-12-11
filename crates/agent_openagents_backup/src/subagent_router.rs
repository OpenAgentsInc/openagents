//! Subagent Router
//!
//! Routes subtasks to the best available subagent:
//! 1. Claude Code (when enabled and appropriate for complex tasks)
//! 2. FM - Apple Foundation Models (when enabled and available on macOS)
//! 3. Minimal subagent (OpenRouter/fallback)

use crate::claude_code_detector::{detect_claude_code, ClaudeCodeAvailability, DetectClaudeCodeOptions};
use crate::error::{AgentError, AgentResult};
use crate::types::{ClaudeCodeSettings, SubagentResult, Subtask};
use serde::{Deserialize, Serialize};

/// Keywords that indicate a complex task suitable for Claude Code
const COMPLEX_KEYWORDS: &[&str] = &[
    "refactor",
    "multi-file",
    "multi file",
    "search",
    "fetch",
    "investigate",
];

/// FM (Foundation Models) settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FMSettings {
    /// Whether FM is enabled
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Port for FM bridge (default: 11435)
    #[serde(default)]
    pub port: Option<u16>,
    /// Enable Voyager-style skill injection
    #[serde(default)]
    pub use_skills: Option<bool>,
    /// Enable Generative Agents-style memory injection
    #[serde(default)]
    pub use_memory: Option<bool>,
    /// Enable Reflexion pattern
    #[serde(default)]
    pub use_reflection: Option<bool>,
    /// Max reflection-based retries
    #[serde(default)]
    pub max_reflection_retries: Option<u32>,
    /// Project root for loading skills/memories
    #[serde(default)]
    pub project_root: Option<String>,
    /// Max skills to inject
    #[serde(default)]
    pub max_skills: Option<usize>,
    /// Max memories to inject
    #[serde(default)]
    pub max_memories: Option<usize>,
    /// Minimum similarity for skill/memory matching
    #[serde(default)]
    pub min_similarity: Option<f32>,
}

impl FMSettings {
    /// Get the port with default
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(11435)
    }

    /// Check if skills are enabled
    pub fn skills_enabled(&self) -> bool {
        self.use_skills.unwrap_or(true)
    }

    /// Check if memory is enabled
    pub fn memory_enabled(&self) -> bool {
        self.use_memory.unwrap_or(false)
    }

    /// Get max skills to inject
    pub fn max_skills(&self) -> usize {
        self.max_skills.unwrap_or(3)
    }

    /// Get max memories to inject
    pub fn max_memories(&self) -> usize {
        self.max_memories.unwrap_or(3)
    }

    /// Get minimum similarity threshold
    pub fn min_similarity(&self) -> f32 {
        self.min_similarity.unwrap_or(0.3)
    }
}

/// FM availability result
#[derive(Debug, Clone)]
pub struct FMAvailability {
    /// Whether FM is available
    pub available: bool,
    /// Error message if not available
    pub error: Option<String>,
}

/// Check if FM is enabled in settings
pub fn is_fm_enabled(settings: Option<&FMSettings>) -> bool {
    settings.map(|s| s.enabled.unwrap_or(true)).unwrap_or(false)
}

/// Detect if FM (Apple Foundation Models) is available
pub fn detect_fm_availability(_port: u16) -> FMAvailability {
    // Platform check - FM requires macOS
    #[cfg(not(target_os = "macos"))]
    {
        return FMAvailability {
            available: false,
            error: Some("FM requires macOS".to_string()),
        };
    }

    #[cfg(target_os = "macos")]
    {
        // TODO: Implement actual FM bridge health check
        // For now, assume available on macOS
        FMAvailability {
            available: true,
            error: None,
        }
    }
}

/// Check if Claude Code should be enabled
pub fn should_enable_claude_code(settings: Option<&ClaudeCodeSettings>) -> bool {
    settings.map(|s| s.enabled.unwrap_or(true)).unwrap_or(true)
}

/// Check if a subtask should use Claude Code based on complexity
pub fn should_use_claude_code(subtask: &Subtask, settings: Option<&ClaudeCodeSettings>) -> bool {
    if !should_enable_claude_code(settings) {
        return false;
    }

    let prefer_complex = settings
        .and_then(|s| s.prefer_for_complex_tasks)
        .unwrap_or(true);

    if !prefer_complex {
        return true;
    }

    let description = subtask.description.to_lowercase();

    // Long descriptions indicate complexity
    if description.len() > 300 {
        return true;
    }

    // Check for complex keywords
    COMPLEX_KEYWORDS
        .iter()
        .any(|keyword| description.contains(keyword))
}

/// Check if fallback to minimal subagent is enabled
pub fn is_fallback_enabled(settings: Option<&ClaudeCodeSettings>) -> bool {
    settings
        .and_then(|s| s.fallback_to_minimal)
        .unwrap_or(true)
}

/// Options for running the best available subagent
#[derive(Debug, Clone)]
pub struct RunSubagentOptions {
    /// Working directory
    pub cwd: String,
    /// OpenAgents directory
    pub openagents_dir: String,
    /// Model to use for minimal subagent
    pub model: Option<String>,
    /// Claude Code settings
    pub claude_code: Option<ClaudeCodeSettings>,
    /// FM settings
    pub fm: Option<FMSettings>,
    /// Maximum turns
    pub max_turns: Option<u32>,
    /// Verification commands
    pub verification_commands: Option<Vec<String>>,
    /// Additional context for prompts
    pub additional_context: Option<String>,
    /// Reflections from previous failures
    pub reflections: Option<String>,
}

impl Default for RunSubagentOptions {
    fn default() -> Self {
        Self {
            cwd: ".".to_string(),
            openagents_dir: ".openagents".to_string(),
            model: None,
            claude_code: None,
            fm: None,
            max_turns: Some(300),
            verification_commands: None,
            additional_context: None,
            reflections: None,
        }
    }
}

/// Agent type used for routing subtasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouterAgentType {
    /// Claude Code SDK
    ClaudeCode,
    /// Apple Foundation Models
    Fm,
    /// Minimal OpenRouter subagent
    Minimal,
}

impl std::fmt::Display for RouterAgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouterAgentType::ClaudeCode => write!(f, "claude-code"),
            RouterAgentType::Fm => write!(f, "fm"),
            RouterAgentType::Minimal => write!(f, "minimal"),
        }
    }
}

/// Routing decision for a subtask
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    /// Which agent to use
    pub agent: RouterAgentType,
    /// Reason for the decision
    pub reason: String,
}

/// Determine which agent should handle a subtask
pub fn route_subtask(
    subtask: &Subtask,
    options: &RunSubagentOptions,
) -> AgentResult<RoutingDecision> {
    // Check Claude Code first
    let try_claude = should_use_claude_code(subtask, options.claude_code.as_ref());

    if try_claude {
        let availability = detect_claude_code(&DetectClaudeCodeOptions::default());

        if availability.available {
            return Ok(RoutingDecision {
                agent: RouterAgentType::ClaudeCode,
                reason: "Claude Code available and task is complex".to_string(),
            });
        }
    }

    // Check FM next
    let try_fm = is_fm_enabled(options.fm.as_ref());

    if try_fm {
        let port = options.fm.as_ref().map(|f| f.port()).unwrap_or(11435);
        let fm_availability = detect_fm_availability(port);

        if fm_availability.available {
            return Ok(RoutingDecision {
                agent: RouterAgentType::Fm,
                reason: "FM available on macOS".to_string(),
            });
        }
    }

    // Fall back to minimal
    Ok(RoutingDecision {
        agent: RouterAgentType::Minimal,
        reason: "Using minimal subagent (default fallback)".to_string(),
    })
}

/// Verification result from subagent routing
#[derive(Debug, Clone)]
pub struct RouterVerificationResult {
    /// Whether verification passed
    pub passed: bool,
    /// Output from verification commands
    pub outputs: Vec<String>,
}

/// Format verification error message
pub fn format_verification_error(outputs: &[String]) -> String {
    let first_output = outputs.iter().find(|o| !o.trim().is_empty());

    match first_output {
        Some(output) => {
            let summary: String = output
                .trim()
                .lines()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");

            if summary.is_empty() {
                "Verification failed (typecheck/tests)".to_string()
            } else {
                format!("Verification failed (typecheck/tests): {}", summary)
            }
        }
        None => "Verification failed (typecheck/tests)".to_string(),
    }
}

/// Merge files modified from multiple subagent runs
pub fn merge_files_modified(file_sets: &[&[String]]) -> Vec<String> {
    let mut merged = std::collections::HashSet::new();
    for files in file_sets {
        for file in *files {
            merged.insert(file.clone());
        }
    }
    merged.into_iter().collect()
}

/// Learning metrics from FM subagent routing
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterLearningMetrics {
    /// IDs of skills that were injected
    #[serde(default)]
    pub skills_injected: Vec<String>,
    /// IDs of memories that were injected
    #[serde(default)]
    pub memories_injected: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_enable_claude_code_default() {
        assert!(should_enable_claude_code(None));
    }

    #[test]
    fn test_should_enable_claude_code_disabled() {
        let settings = ClaudeCodeSettings {
            enabled: Some(false),
            ..Default::default()
        };
        assert!(!should_enable_claude_code(Some(&settings)));
    }

    #[test]
    fn test_should_use_claude_code_complex_keywords() {
        let subtask = Subtask {
            id: "sub-1".to_string(),
            description: "Refactor the authentication module".to_string(),
            status: crate::types::SubtaskStatus::Pending,
            ..Default::default()
        };

        assert!(should_use_claude_code(&subtask, None));
    }

    #[test]
    fn test_should_use_claude_code_long_description() {
        let long_desc = "a".repeat(350);
        let subtask = Subtask {
            id: "sub-1".to_string(),
            description: long_desc,
            status: crate::types::SubtaskStatus::Pending,
            ..Default::default()
        };

        assert!(should_use_claude_code(&subtask, None));
    }

    #[test]
    fn test_should_use_claude_code_simple_task() {
        let subtask = Subtask {
            id: "sub-1".to_string(),
            description: "Fix typo".to_string(),
            status: crate::types::SubtaskStatus::Pending,
            ..Default::default()
        };

        assert!(!should_use_claude_code(&subtask, None));
    }

    #[test]
    fn test_is_fallback_enabled_default() {
        assert!(is_fallback_enabled(None));
    }

    #[test]
    fn test_is_fallback_enabled_disabled() {
        let settings = ClaudeCodeSettings {
            fallback_to_minimal: Some(false),
            ..Default::default()
        };
        assert!(!is_fallback_enabled(Some(&settings)));
    }

    #[test]
    fn test_format_verification_error_empty() {
        let outputs: Vec<String> = vec![];
        let error = format_verification_error(&outputs);
        assert_eq!(error, "Verification failed (typecheck/tests)");
    }

    #[test]
    fn test_format_verification_error_with_output() {
        let outputs = vec!["error: type mismatch\nat line 42\nin file.rs".to_string()];
        let error = format_verification_error(&outputs);
        assert!(error.contains("type mismatch"));
    }

    #[test]
    fn test_merge_files_modified() {
        let set1 = vec!["a.rs".to_string(), "b.rs".to_string()];
        let set2 = vec!["b.rs".to_string(), "c.rs".to_string()];
        let merged = merge_files_modified(&[&set1, &set2]);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_fm_settings_defaults() {
        let settings = FMSettings::default();
        assert_eq!(settings.port(), 11435);
        assert!(settings.skills_enabled());
        assert!(!settings.memory_enabled());
        assert_eq!(settings.max_skills(), 3);
    }

    #[test]
    fn test_agent_type_display() {
        assert_eq!(format!("{}", RouterAgentType::ClaudeCode), "claude-code");
        assert_eq!(format!("{}", RouterAgentType::Fm), "fm");
        assert_eq!(format!("{}", RouterAgentType::Minimal), "minimal");
    }

    #[test]
    fn test_route_subtask_simple() {
        let subtask = Subtask {
            id: "sub-1".to_string(),
            description: "Simple task".to_string(),
            status: crate::types::SubtaskStatus::Pending,
            ..Default::default()
        };
        let options = RunSubagentOptions::default();

        let decision = route_subtask(&subtask, &options).unwrap();
        // Should fall back to minimal since Claude Code likely not available in tests
        assert_eq!(decision.agent, RouterAgentType::Minimal);
    }
}
