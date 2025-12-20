//! Plan mode infrastructure for autopilot
//!
//! Plan mode is a structured workflow where agents explore and design before executing.
//! When in plan mode:
//! - File writes are restricted (except for the plan file)
//! - Git commits are blocked
//! - Destructive commands are prevented
//! - Reads, analysis, and subagents are allowed

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

/// Plan phases for structured exploration and design
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanPhase {
    /// Explore phase: understand codebase and requirements
    Explore,
    /// Design phase: evaluate approaches and create plan
    Design,
    /// Review phase: validate plan completeness
    Review,
    /// Final phase: prepare for implementation
    Final,
    /// Exit phase: plan mode completed
    Exit,
}

impl PlanPhase {
    /// Get the phase name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanPhase::Explore => "explore",
            PlanPhase::Design => "design",
            PlanPhase::Review => "review",
            PlanPhase::Final => "final",
            PlanPhase::Exit => "exit",
        }
    }

    /// Get the next phase
    pub fn next(&self) -> Option<PlanPhase> {
        match self {
            PlanPhase::Explore => Some(PlanPhase::Design),
            PlanPhase::Design => Some(PlanPhase::Review),
            PlanPhase::Review => Some(PlanPhase::Final),
            PlanPhase::Final => Some(PlanPhase::Exit),
            PlanPhase::Exit => None,
        }
    }
}

/// Global plan mode state
static PLAN_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);

lazy_static::lazy_static! {
    static ref PLAN_FILE_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);
    static ref CURRENT_PHASE: RwLock<PlanPhase> = RwLock::new(PlanPhase::Explore);
}

/// Plan mode configuration
#[derive(Debug, Clone)]
pub struct PlanModeConfig {
    /// Path to the plan file
    pub plan_file: PathBuf,
    /// Goal being planned
    pub goal: String,
}

impl PlanModeConfig {
    /// Create a new plan mode configuration
    pub fn new(slug: &str, goal: impl Into<String>) -> Self {
        let plan_file = Self::generate_plan_path(slug);
        Self {
            plan_file,
            goal: goal.into(),
        }
    }

    /// Generate the plan file path
    /// Format: ~/.claude/plans/{slug}.md
    pub fn generate_plan_path(slug: &str) -> PathBuf {
        let home = dirs::home_dir().expect("Failed to get home directory");
        let plans_dir = home.join(".claude").join("plans");

        // Ensure directory exists
        std::fs::create_dir_all(&plans_dir).ok();

        plans_dir.join(format!("{}.md", slug))
    }
}

/// Enter plan mode
pub fn enter_plan_mode(config: PlanModeConfig) -> Result<String, String> {
    // Set global state
    PLAN_MODE_ACTIVE.store(true, Ordering::SeqCst);

    // Store plan file path
    {
        let mut path = PLAN_FILE_PATH.write().map_err(|e| e.to_string())?;
        *path = Some(config.plan_file.clone());
    }

    // Reset to explore phase
    {
        let mut phase = CURRENT_PHASE.write().map_err(|e| e.to_string())?;
        *phase = PlanPhase::Explore;
    }

    // Create initial plan file template
    let template = format!(
r#"# Plan: {}

## Goal
{}

## Phase 1: Explore
### Current Understanding
[Document your understanding of the codebase and requirements here]

### Subagent Findings
[Results from explore agents investigating the codebase]

## Phase 2: Design
### Approach Options
[List different implementation approaches with pros/cons]

### Recommended Approach
[Describe the recommended approach in detail]

### Subagent Analysis
[Results from plan agents analyzing different perspectives]

## Phase 3: Review
### Implementation Steps
[Break down into specific, actionable steps]

### Risks & Considerations
[List potential issues, edge cases, dependencies]

## Phase 4: Final
### Ready for Implementation
[Confirm all planning is complete and ready to exit plan mode]
"#,
        config.plan_file.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled"),
        config.goal
    );

    std::fs::write(&config.plan_file, template)
        .map_err(|e| format!("Failed to create plan file: {}", e))?;

    Ok(format!(
        "Entered plan mode. Plan file: {}\n\nRestrictions active:\n- File writes blocked (except plan file)\n- Git commits blocked\n- Destructive bash commands blocked\n\nAllowed:\n- Read operations\n- Plan file edits\n- Subagent launches\n- User questions",
        config.plan_file.display()
    ))
}

/// Exit plan mode
pub fn exit_plan_mode() -> Result<String, String> {
    // Verify plan has content
    let plan_file = {
        let path = PLAN_FILE_PATH.read().map_err(|e| e.to_string())?;
        path.clone().ok_or("No plan file set")?
    };

    if !plan_file.exists() {
        return Err("Plan file does not exist".to_string());
    }

    let content = std::fs::read_to_string(&plan_file)
        .map_err(|e| format!("Failed to read plan file: {}", e))?;

    if content.len() < 100 {
        return Err("Plan file is too short. Please add more detail before exiting plan mode.".to_string());
    }

    // Clear global state
    PLAN_MODE_ACTIVE.store(false, Ordering::SeqCst);

    {
        let mut path = PLAN_FILE_PATH.write().map_err(|e| e.to_string())?;
        *path = None;
    }

    Ok(format!(
        "Exited plan mode. Plan saved to: {}\n\nAll restrictions lifted. You can now implement the plan.",
        plan_file.display()
    ))
}

/// Check if plan mode is currently active
pub fn is_plan_mode_active() -> bool {
    PLAN_MODE_ACTIVE.load(Ordering::SeqCst)
}

/// Get the current plan file path
pub fn get_plan_file_path() -> Option<PathBuf> {
    PLAN_FILE_PATH.read().ok()?.clone()
}

/// Check if a file path is the current plan file
pub fn is_plan_file(path: &Path) -> bool {
    if let Some(plan_path) = get_plan_file_path() {
        path == plan_path
    } else {
        false
    }
}

/// Get the current plan phase
pub fn get_current_phase() -> PlanPhase {
    CURRENT_PHASE.read().ok().map(|p| *p).unwrap_or(PlanPhase::Explore)
}

/// Advance to the next plan phase
pub fn advance_phase() -> Result<PlanPhase, String> {
    let mut phase = CURRENT_PHASE.write().map_err(|e| e.to_string())?;
    *phase = phase.next().ok_or("Already at final phase")?;
    Ok(*phase)
}

/// Set the current plan phase
pub fn set_phase(new_phase: PlanPhase) -> Result<(), String> {
    let mut phase = CURRENT_PHASE.write().map_err(|e| e.to_string())?;
    *phase = new_phase;
    Ok(())
}

/// Generate a prompt for an explore subagent
pub fn explore_agent_prompt(topic: &str, focus: &str) -> String {
    format!(
        r#"You are an exploration agent investigating: {}

Focus: {}

Your task:
1. Use Glob, Grep, and Read tools to search for relevant code, patterns, and structures
2. After each tool call, note what you learned
3. Be efficient - prioritize the most relevant files
4. Summarize your findings with file lists and key insights

Remember: You're gathering information to help plan an implementation. Focus on understanding existing patterns, architecture, and constraints."#,
        topic, focus
    )
}

/// Generate a prompt for a plan subagent
pub fn plan_agent_prompt(feature: &str, context: &str, perspective: &str) -> String {
    format!(
        r#"You are a planning agent designing: {}

Context from exploration:
{}

Perspective: {} (focus on this aspect in your analysis)

Your task:
1. Read relevant files for deeper context
2. Analyze trade-offs of different approaches
3. Consider the specified perspective in your analysis
4. Produce a structured implementation plan

Remember: You're designing an approach, not implementing it. Focus on architecture, patterns, and decision rationale."#,
        feature, context, perspective
    )
}

/// Check if a tool should be allowed in plan mode
pub fn is_tool_allowed_in_plan_mode(tool_name: &str, tool_input: &serde_json::Value) -> Result<(), String> {
    if !is_plan_mode_active() {
        return Ok(()); // Not in plan mode, allow everything
    }

    match tool_name {
        // Always allowed
        "Read" | "Grep" | "Glob" | "Task" | "AskUserQuestion" | "TodoWrite" => Ok(()),

        // Conditionally allowed
        "Edit" | "Write" => {
            // Check if it's the plan file
            let file_path = tool_input
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or("No file_path in tool input")?;

            let path = PathBuf::from(file_path);
            if is_plan_file(&path) {
                Ok(())
            } else {
                Err("File writes are not allowed in plan mode (except for the plan file). Use ExitPlanMode to implement the plan.".to_string())
            }
        }

        "Bash" => {
            // Check for destructive commands
            let command = tool_input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Block git commits and pushes
            if command.contains("git commit") || command.contains("git push") {
                return Err("Git commits are not allowed in plan mode. Use ExitPlanMode to commit changes.".to_string());
            }

            // Block file modifications
            if command.contains("rm ") || command.contains("mv ") || command.contains("cp ") {
                return Err("Destructive bash commands are not allowed in plan mode.".to_string());
            }

            // Allow read-only operations
            Ok(())
        }

        // Block execution-related tools
        "NotebookEdit" => {
            Err("Notebook edits are not allowed in plan mode. Use ExitPlanMode to implement changes.".to_string())
        }

        // Allow all other tools (EnterPlanMode, ExitPlanMode, etc.)
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_path_generation() {
        let path = PlanModeConfig::generate_plan_path("test-feature");
        assert!(path.to_str().unwrap().contains(".claude/plans"));
        assert!(path.to_str().unwrap().ends_with("test-feature.md"));
    }

    #[test]
    fn test_tool_restrictions() {
        // Not in plan mode - all tools allowed
        assert!(is_tool_allowed_in_plan_mode("Write", &serde_json::json!({"file_path": "/tmp/test.txt"})).is_ok());

        // Enter plan mode
        let config = PlanModeConfig::new("test", "Test goal");
        enter_plan_mode(config.clone()).unwrap();

        // Read operations allowed
        assert!(is_tool_allowed_in_plan_mode("Read", &serde_json::json!({})).is_ok());
        assert!(is_tool_allowed_in_plan_mode("Grep", &serde_json::json!({})).is_ok());

        // Plan file writes allowed
        assert!(is_tool_allowed_in_plan_mode("Edit", &serde_json::json!({
            "file_path": config.plan_file.to_str().unwrap()
        })).is_ok());

        // Other file writes blocked
        assert!(is_tool_allowed_in_plan_mode("Write", &serde_json::json!({
            "file_path": "/tmp/other.txt"
        })).is_err());

        // Git commits blocked
        assert!(is_tool_allowed_in_plan_mode("Bash", &serde_json::json!({
            "command": "git commit -m 'test'"
        })).is_err());

        // Read-only bash allowed
        assert!(is_tool_allowed_in_plan_mode("Bash", &serde_json::json!({
            "command": "git status"
        })).is_ok());

        // Exit plan mode
        exit_plan_mode().unwrap();

        // All tools allowed again
        assert!(is_tool_allowed_in_plan_mode("Write", &serde_json::json!({"file_path": "/tmp/test.txt"})).is_ok());
    }
}
