//! Compaction prompt templates and LLM integration for autopilot
//!
//! This module provides compaction prompt templates that can be used with
//! the Claude Code CLI's built-in compaction feature via the PreCompact hook.
//!
//! Implements shape-preserving compaction that keeps first/last conversation windows
//! intact while only compacting the middle section.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

## Core Requirements
1. Tasks completed (mark clearly as DONE)
2. Current task in progress (if any, with specific next steps)
3. Pending tasks from the backlog
4. Any blockers or decisions needed

## Critical Context (MUST PRESERVE)
The following context is frequently lost and severely impacts task completion:

- **File paths**: ALWAYS include specific file paths with line numbers (e.g., `src/main.rs:142`)
- **Error messages**: ALWAYS include full error messages if any errors occurred
- **Issue/Directive IDs**: ALWAYS mention active issue numbers (#1234) and directive IDs (d-004)
- **Test results**: ALWAYS state which tests passed/failed and why
- **Function/type names**: ALWAYS preserve exact names of functions, structs, types being worked on
- **Architectural decisions**: ALWAYS explain why certain approaches were chosen over alternatives

## Key Context Details
- Active files and their purpose (with full paths)
- Established patterns to follow
- Constraints or requirements
- Recent error messages (if any)

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
        format!("{}\n\nAdditional context:\n{}", instructions, context)
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
        || session_context.contains("## Design")
    {
        return CompactionStrategy::Planning;
    }

    // Check for autonomous mode indicators
    if session_context.contains("FULL AUTO MODE")
        || session_context.contains("issue_ready")
        || session_context.contains("autonomous_session")
    {
        return CompactionStrategy::Autonomous;
    }

    // Check for detailed technical work
    if session_context.contains("cargo test")
        || session_context.contains("impl ")
        || session_context.contains("fn ")
        || session_context.lines().count() > 100
    {
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

/// Configuration for shape-preserving compaction
#[derive(Debug, Clone, Copy)]
pub struct ShapePreservingConfig {
    /// Number of turns to preserve at the beginning (default: 3)
    pub first_window_turns: usize,
    /// Number of turns to preserve at the end (default: 10)
    pub last_window_turns: usize,
    /// Token threshold to trigger compaction (default: 80000)
    pub compaction_threshold_tokens: u64,
}

impl Default for ShapePreservingConfig {
    fn default() -> Self {
        Self {
            first_window_turns: 3,
            last_window_turns: 10,
            compaction_threshold_tokens: 80_000,
        }
    }
}

/// Message abstraction for compaction analysis
#[derive(Debug, Clone)]
pub struct Message {
    /// Role (user, assistant, system)
    pub role: String,
    /// Message content
    pub content: String,
    /// Estimated token count (rough estimate: chars / 4)
    pub estimated_tokens: u64,
}

impl Message {
    /// Create a new message with estimated token count
    pub fn new(role: String, content: String) -> Self {
        let estimated_tokens = (content.len() as u64) / 4;
        Self {
            role,
            content,
            estimated_tokens,
        }
    }

    /// Parse message from SDK message JSON
    pub fn from_sdk_message(msg: &Value) -> Option<Self> {
        let msg_type = msg.get("type")?.as_str()?;

        match msg_type {
            "user" => {
                let content = msg
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("");
                Some(Message::new("user".to_string(), content.to_string()))
            }
            "assistant" => {
                let content = msg
                    .get("message")
                    .and_then(|m| serde_json::to_string(m).ok())
                    .unwrap_or_default();
                Some(Message::new("assistant".to_string(), content))
            }
            "system" => {
                let content = serde_json::to_string(msg).unwrap_or_default();
                Some(Message::new("system".to_string(), content))
            }
            _ => None,
        }
    }
}

/// Window of messages to preserve or compact
#[derive(Debug)]
pub struct MessageWindow {
    /// Messages in this window
    pub messages: Vec<Message>,
    /// Total estimated tokens
    pub total_tokens: u64,
}

impl MessageWindow {
    /// Create an empty window
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            total_tokens: 0,
        }
    }

    /// Add a message to the window
    pub fn push(&mut self, message: Message) {
        self.total_tokens += message.estimated_tokens;
        self.messages.push(message);
    }

    /// Get the content for summarization
    pub fn to_summarizable_text(&self) -> String {
        self.messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Result of shape-preserving compaction analysis
#[derive(Debug)]
pub struct CompactionPlan {
    /// Messages in the first window (preserve as-is)
    pub first_window: MessageWindow,
    /// Messages in the middle window (to be compacted)
    pub middle_window: MessageWindow,
    /// Messages in the last window (preserve as-is)
    pub last_window: MessageWindow,
    /// Whether compaction is needed
    pub should_compact: bool,
    /// Total tokens across all windows
    pub total_tokens: u64,
}

impl CompactionPlan {
    /// Analyze messages and create a compaction plan
    pub fn analyze(messages: Vec<Message>, config: ShapePreservingConfig) -> Self {
        let total_tokens: u64 = messages.iter().map(|m| m.estimated_tokens).sum();
        let should_compact = total_tokens > config.compaction_threshold_tokens;

        // If we don't need to compact, return everything in the last window
        if !should_compact {
            let mut last_window = MessageWindow::new();
            for msg in messages {
                last_window.push(msg);
            }
            return Self {
                first_window: MessageWindow::new(),
                middle_window: MessageWindow::new(),
                last_window,
                should_compact: false,
                total_tokens,
            };
        }

        // Calculate message ranges
        let total_messages = messages.len();
        let first_end = config.first_window_turns.min(total_messages);
        let last_start = if total_messages > config.last_window_turns {
            total_messages - config.last_window_turns
        } else {
            total_messages
        };

        // Ensure no overlap
        let (first_end, middle_start, middle_end, last_start) = if first_end >= last_start {
            // Windows would overlap - adjust
            let mid_point = total_messages / 2;
            (mid_point, mid_point, mid_point, mid_point)
        } else {
            (first_end, first_end, last_start, last_start)
        };

        // Build windows
        let mut first_window = MessageWindow::new();
        let mut middle_window = MessageWindow::new();
        let mut last_window = MessageWindow::new();

        for (i, msg) in messages.into_iter().enumerate() {
            if i < first_end {
                first_window.push(msg);
            } else if i >= middle_start && i < middle_end {
                middle_window.push(msg);
            } else if i >= last_start {
                last_window.push(msg);
            }
        }

        Self {
            first_window,
            middle_window,
            last_window,
            should_compact: true,
            total_tokens,
        }
    }

    /// Get summary of the plan for logging
    pub fn summary(&self) -> String {
        format!(
            "Compaction plan: {} total tokens, first: {} msgs ({} tokens), middle: {} msgs ({} tokens), last: {} msgs ({} tokens), compact: {}",
            self.total_tokens,
            self.first_window.messages.len(),
            self.first_window.total_tokens,
            self.middle_window.messages.len(),
            self.middle_window.total_tokens,
            self.last_window.messages.len(),
            self.last_window.total_tokens,
            self.should_compact
        )
    }
}

#[cfg(test)]
mod shape_preserving_tests {
    use super::*;

    fn create_test_message(role: &str, content: &str) -> Message {
        Message::new(role.to_string(), content.to_string())
    }

    #[test]
    fn test_message_token_estimation() {
        let msg = create_test_message("user", "Hello world");
        assert_eq!(msg.estimated_tokens, 11 / 4); // "Hello world".len() = 11, /4 = 2
    }

    #[test]
    fn test_compaction_not_needed() {
        let messages = vec![
            create_test_message("user", "Hello"),
            create_test_message("assistant", "Hi there"),
        ];

        let config = ShapePreservingConfig {
            compaction_threshold_tokens: 1000,
            ..Default::default()
        };

        let plan = CompactionPlan::analyze(messages, config);
        assert!(!plan.should_compact);
        assert_eq!(plan.first_window.messages.len(), 0);
        assert_eq!(plan.middle_window.messages.len(), 0);
        assert_eq!(plan.last_window.messages.len(), 2);
    }

    #[test]
    fn test_compaction_needed_with_all_windows() {
        // Create enough messages to trigger compaction
        let mut messages = Vec::new();
        for i in 0..20 {
            messages.push(create_test_message(
                if i % 2 == 0 { "user" } else { "assistant" },
                &"x".repeat(1000), // 1000 chars = ~250 tokens
            ));
        }

        let config = ShapePreservingConfig {
            first_window_turns: 3,
            last_window_turns: 5,
            compaction_threshold_tokens: 1000, // 20 messages * 250 tokens = 5000 > 1000
        };

        let plan = CompactionPlan::analyze(messages, config);
        assert!(plan.should_compact);
        assert_eq!(plan.first_window.messages.len(), 3);
        assert_eq!(plan.middle_window.messages.len(), 12); // 20 - 3 - 5
        assert_eq!(plan.last_window.messages.len(), 5);
    }

    #[test]
    fn test_compaction_with_window_overlap() {
        // Test case where first + last windows would overlap
        let messages = vec![
            create_test_message("user", &"x".repeat(1000)),
            create_test_message("assistant", &"x".repeat(1000)),
            create_test_message("user", &"x".repeat(1000)),
        ];

        let config = ShapePreservingConfig {
            first_window_turns: 2,
            last_window_turns: 2,
            compaction_threshold_tokens: 500, // Will trigger compaction
        };

        let plan = CompactionPlan::analyze(messages, config);
        // When windows overlap, should adjust to prevent overlap
        assert!(plan.should_compact);
    }

    #[test]
    fn test_message_window_to_text() {
        let mut window = MessageWindow::new();
        window.push(create_test_message("user", "Hello"));
        window.push(create_test_message("assistant", "Hi there"));

        let text = window.to_summarizable_text();
        assert!(text.contains("user: Hello"));
        assert!(text.contains("assistant: Hi there"));
    }

    #[test]
    fn test_default_config() {
        let config = ShapePreservingConfig::default();
        assert_eq!(config.first_window_turns, 3);
        assert_eq!(config.last_window_turns, 10);
        assert_eq!(config.compaction_threshold_tokens, 80_000);
    }
}
