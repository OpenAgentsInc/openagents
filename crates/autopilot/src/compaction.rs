//! Compaction prompt templates and LLM integration for autopilot
//!
//! This module provides compaction prompt templates that can be used with
//! the Claude Code CLI's built-in compaction feature via the PreCompact hook.

use serde::{Deserialize, Serialize};

/// Compaction strategy for different use cases
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompactionStrategy {
    /// Preserve technical details and code context
    Detailed,
    /// Focus on key decisions and outcomes
    Summary,
    /// Optimized for continuous autonomous work
    Autonomous,
    /// Preserve plan mode context and phases
    Planning,
}

impl CompactionStrategy {
    /// Get the compaction strategy as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            CompactionStrategy::Detailed => "detailed",
            CompactionStrategy::Summary => "summary",
            CompactionStrategy::Autonomous => "autonomous",
            CompactionStrategy::Planning => "planning",
        }
    }
}

/// Generate compaction instructions based on strategy
pub fn generate_compaction_instructions(strategy: CompactionStrategy) -> String {
    match strategy {
        CompactionStrategy::Detailed => {
            r#"Create a detailed technical summary focusing on:
1. Code changes made (files modified, functions added/changed)
2. Technical decisions and rationale
3. Architecture patterns established
4. Dependencies added or modified
5. Test results and validation
6. Known issues or TODOs

Preserve specific file paths, function names, and technical details that would be needed to continue work."#.to_string()
        }

        CompactionStrategy::Summary => {
            r#"Create a concise summary focusing on:
1. What was accomplished (high-level outcomes)
2. Key decisions made and why
3. Current state of the work
4. What needs to be done next

Keep it brief but capture the essential context needed for handoff."#.to_string()
        }

        CompactionStrategy::Autonomous => {
            r#"Create a handoff-ready summary for autonomous continuation:
1. Tasks completed (mark clearly as DONE)
2. Current task in progress (if any, with specific next steps)
3. Pending tasks from the backlog
4. Key context needed to continue:
   - Active files and their purpose
   - Established patterns to follow
   - Constraints or requirements
5. Any blockers or decisions needed

Format as a clear action plan that another agent can immediately pick up and continue."#.to_string()
        }

        CompactionStrategy::Planning => {
            r#"Create a summary preserving plan mode context:
1. Current phase (explore/design/review/final)
2. Goal and requirements
3. Findings from exploration:
   - Codebase patterns discovered
   - Relevant files and architecture
4. Design decisions:
   - Approaches considered
   - Selected approach and rationale
5. Implementation plan (if created)
6. Outstanding questions or risks

Preserve enough context to resume planning or transition to implementation."#.to_string()
        }
    }
}

/// Generate full compaction prompt with context
pub fn generate_compaction_prompt(
    strategy: CompactionStrategy,
    additional_context: Option<&str>,
) -> String {
    let instructions = generate_compaction_instructions(strategy);

    if let Some(context) = additional_context {
        format!(
            "{}\n\nAdditional context:\n{}",
            instructions, context
        )
    } else {
        instructions
    }
}

/// Detect appropriate compaction strategy based on session context
pub fn detect_strategy(session_context: &str) -> CompactionStrategy {
    // Check for plan mode indicators
    if session_context.contains("plan mode")
        || session_context.contains("Phase 1:")
        || session_context.contains("## Explore")
        || session_context.contains("## Design") {
        return CompactionStrategy::Planning;
    }

    // Check for autonomous mode indicators
    if session_context.contains("FULL AUTO MODE")
        || session_context.contains("issue_ready")
        || session_context.contains("autonomous_session") {
        return CompactionStrategy::Autonomous;
    }

    // Check for detailed technical work
    if session_context.contains("cargo test")
        || session_context.contains("impl ")
        || session_context.contains("fn ")
        || session_context.lines().count() > 100 {
        return CompactionStrategy::Detailed;
    }

    // Default to summary for general work
    CompactionStrategy::Summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_as_str() {
        assert_eq!(CompactionStrategy::Detailed.as_str(), "detailed");
        assert_eq!(CompactionStrategy::Summary.as_str(), "summary");
        assert_eq!(CompactionStrategy::Autonomous.as_str(), "autonomous");
        assert_eq!(CompactionStrategy::Planning.as_str(), "planning");
    }

    #[test]
    fn test_detect_strategy_planning() {
        let context = "We are in plan mode. Phase 1: Explore the codebase";
        assert_eq!(detect_strategy(context), CompactionStrategy::Planning);
    }

    #[test]
    fn test_detect_strategy_autonomous() {
        let context = "FULL AUTO MODE: issue_ready returned issue #12";
        assert_eq!(detect_strategy(context), CompactionStrategy::Autonomous);
    }

    #[test]
    fn test_detect_strategy_detailed() {
        let context = "impl MyStruct {\n    fn new() -> Self {\n        cargo test passed";
        assert_eq!(detect_strategy(context), CompactionStrategy::Detailed);
    }

    #[test]
    fn test_detect_strategy_summary() {
        let context = "Added a new feature";
        assert_eq!(detect_strategy(context), CompactionStrategy::Summary);
    }

    #[test]
    fn test_generate_prompt_with_context() {
        let prompt = generate_compaction_prompt(
            CompactionStrategy::Summary,
            Some("Focus on authentication changes"),
        );
        assert!(prompt.contains("concise summary"));
        assert!(prompt.contains("Focus on authentication changes"));
    }

    #[test]
    fn test_generate_prompt_without_context() {
        let prompt = generate_compaction_prompt(CompactionStrategy::Detailed, None);
        assert!(prompt.contains("detailed technical summary"));
        assert!(!prompt.contains("Additional context"));
    }
}
