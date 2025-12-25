//! Plan mode infrastructure for autopilot
//!
//! Plan mode is a structured workflow where agents explore and design before executing.
//! When in plan mode:
//! - File writes are restricted (except for the plan file)
//! - Git commits are blocked
//! - Destructive commands are prevented
//! - SQLite write commands are blocked (always, not just in plan mode)
//! - Reads, analysis, and subagents are allowed

use std::path::{Component, Path, PathBuf};
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Sanitize a slug to prevent path traversal attacks
///
/// This function validates that the slug:
/// 1. Does not contain path traversal sequences (../, ..\)
/// 2. Does not contain path separators (/, \)
/// 3. Only contains safe filename characters
///
/// Returns the sanitized slug or an error if validation fails.
fn sanitize_slug(slug: &str) -> Result<String, String> {
    // Reject empty slugs
    if slug.is_empty() {
        return Err("Slug cannot be empty".to_string());
    }

    // Create a path from the slug to validate components
    let path = PathBuf::from(slug);

    // Validate each component
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                // Only allow the slug itself as a normal component
                // Reject if it contains path separators
                let name_str = name.to_string_lossy();
                if name_str != slug {
                    return Err("Slug cannot contain path separators".to_string());
                }
            }
            Component::ParentDir => {
                return Err("Slug cannot contain '..' path components".to_string());
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("Slug cannot be an absolute path".to_string());
            }
            Component::CurDir => {
                return Err("Slug cannot contain '.' path components".to_string());
            }
        }
    }

    // Additional validation: reject common dangerous characters
    let dangerous_chars = ['/', '\\', '\0'];
    for ch in dangerous_chars {
        if slug.contains(ch) {
            return Err(format!("Slug cannot contain '{}' character", ch));
        }
    }

    Ok(slug.to_string())
}

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
    ///
    /// Returns an error if the slug is invalid or contains path traversal attempts.
    pub fn new(slug: &str, goal: impl Into<String>) -> Result<Self, String> {
        let plan_file = Self::generate_plan_path(slug)?;
        Ok(Self {
            plan_file,
            goal: goal.into(),
        })
    }

    /// Generate the plan file path
    /// Format: ~/.claude/plans/{slug}.md
    ///
    /// Returns an error if the slug contains unsafe characters or path traversal attempts.
    pub fn generate_plan_path(slug: &str) -> Result<PathBuf, String> {
        // Sanitize slug to prevent path traversal
        let sanitized_slug = sanitize_slug(slug)?;

        let home = dirs::home_dir().ok_or("Failed to get home directory")?;
        let plans_dir = home.join(".claude").join("plans");

        // Ensure directory exists
        std::fs::create_dir_all(&plans_dir)
            .map_err(|e| format!("Failed to create plans directory: {}", e))?;

        Ok(plans_dir.join(format!("{}.md", sanitized_slug)))
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
        config
            .plan_file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled"),
        config.goal
    );

    std::fs::write(&config.plan_file, template)
        .map_err(|e| format!("Failed to create plan file: {}", e))?;

    let phase_prompt = get_phase_prompt(PlanPhase::Explore);

    Ok(format!(
        "Entered plan mode. Plan file: {}\n\nRestrictions active:\n- File writes blocked (except plan file)\n- Git commits blocked\n- Destructive bash commands blocked\n\nAllowed:\n- Read operations\n- Plan file edits\n- Subagent launches\n- User questions\n\n{}\n",
        config.plan_file.display(),
        phase_prompt
    ))
}

/// Exit plan mode configuration
#[derive(Default)]
pub struct ExitPlanModeConfig {
    /// Whether to launch a swarm to implement the plan
    pub launch_swarm: bool,
    /// Number of teammates in the swarm (defaults to 3 if launch_swarm is true)
    pub teammate_count: Option<usize>,
}

/// Exit plan mode
pub fn exit_plan_mode() -> Result<String, String> {
    exit_plan_mode_with_config(Default::default())
}

/// Exit plan mode with optional swarm configuration
pub fn exit_plan_mode_with_config(config: ExitPlanModeConfig) -> Result<String, String> {
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
        return Err(
            "Plan file is too short. Please add more detail before exiting plan mode.".to_string(),
        );
    }

    // Clear global state
    PLAN_MODE_ACTIVE.store(false, Ordering::SeqCst);

    {
        let mut path = PLAN_FILE_PATH.write().map_err(|e| e.to_string())?;
        *path = None;
    }

    let mut message = format!(
        "Exited plan mode. Plan saved to: {}\n\nAll restrictions lifted.",
        plan_file.display()
    );

    if config.launch_swarm {
        let teammate_count = config.teammate_count.unwrap_or(3);
        message.push_str(&format!(
            "\n\nSwarm execution is configured but not yet implemented. Planned configuration:\n- Teammates: {}\n- Plan: {}\n\nFor now, you can implement the plan manually.",
            teammate_count,
            plan_file.display()
        ));
    } else {
        message.push_str(" You can now implement the plan.");
    }

    Ok(message)
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
    CURRENT_PHASE
        .read()
        .ok()
        .map(|p| *p)
        .unwrap_or(PlanPhase::Explore)
}

/// Advance to the next plan phase and return the new phase with its prompt
pub fn advance_phase() -> Result<String, String> {
    let new_phase = {
        let mut phase = CURRENT_PHASE.write().map_err(|e| e.to_string())?;
        *phase = phase.next().ok_or("Already at final phase")?;
        *phase
    };

    let phase_prompt = get_phase_prompt(new_phase);

    Ok(format!(
        "Advanced to {} phase.\n\n{}",
        new_phase.as_str().to_uppercase(),
        phase_prompt
    ))
}

/// Set the current plan phase
pub fn set_phase(new_phase: PlanPhase) -> Result<(), String> {
    let mut phase = CURRENT_PHASE.write().map_err(|e| e.to_string())?;
    *phase = new_phase;
    Ok(())
}

/// Get phase-specific guidance prompt
pub fn get_phase_prompt(phase: PlanPhase) -> &'static str {
    match phase {
        PlanPhase::Explore => {
            r#"
EXPLORE PHASE - Focus on understanding

Your goal: Gather information to understand the codebase and requirements.

Recommended tools:
- Glob: Find files by pattern (e.g., "**/*.rs", "src/*/mod.rs")
- Grep: Search code for keywords, patterns, and existing implementations
- Read: Examine key files to understand structure and patterns
- Task (Explore agent): Launch exploration agents for thorough investigation

PARALLEL EXPLORATION STRATEGY:
For comprehensive exploration, launch up to 3 Explore agents in parallel using the Task tool.
Each agent should investigate a different aspect:
  1. Existing patterns and similar implementations
  2. Dependencies, constraints, and technical requirements
  3. Files and modules that need modification

You can use the suggest_explore_agents() helper (see planmode module) or craft custom prompts
for parallel exploration. Launch agents in parallel for efficiency.

What to do:
1. Use Glob/Grep to locate relevant files and code patterns
2. Consider launching parallel Explore agents for thorough investigation
3. Read key files to understand architecture and existing patterns
4. Aggregate findings from multiple sources
5. Document findings in your plan file
6. Note constraints, dependencies, and existing patterns
7. Identify files that will need modification

What NOT to do:
- Don't write code or modify files (except the plan)
- Don't make implementation decisions yet
- Don't commit anything

When ready: Move to Design phase to evaluate approaches.
"#
        }
        PlanPhase::Design => {
            r#"
DESIGN PHASE - Evaluate approaches

Your goal: Design and compare implementation approaches.

Recommended tools:
- Read: Deep-dive into relevant files for detailed context
- Task (Plan agent): Launch planning agents to analyze different perspectives
- AskUserQuestion: Clarify ambiguities or get user preferences

PARALLEL DESIGN ANALYSIS:
For comprehensive design analysis, launch multiple Plan agents in parallel using the Task tool.
Each agent should analyze from a different perspective:
  1. Simplicity and maintainability
  2. Performance and scalability
  3. Type safety and error handling

You can use the suggest_plan_agents() helper (see planmode module) or craft custom prompts
for multi-perspective analysis. Launch agents in parallel for efficiency.

What to do:
1. List multiple implementation approaches with pros/cons
2. Consider launching parallel Plan agents for multi-perspective analysis
3. Consider trade-offs: complexity, maintainability, performance
4. Evaluate how each approach fits existing patterns
5. Aggregate insights from different perspectives
6. Identify risks and edge cases for each approach
7. Document your recommended approach with rationale
8. Update the Design section of your plan file

What NOT to do:
- Don't implement yet
- Don't commit to a single approach without comparing alternatives
- Don't skip documenting trade-offs

When ready: Move to Review phase to validate completeness.
"#
        }
        PlanPhase::Review => {
            r#"
REVIEW PHASE - Validate and refine

Your goal: Ensure the plan is complete and aligned with requirements.

Recommended tools:
- Read: Verify all necessary files are accounted for
- AskUserQuestion: Validate assumptions and clarify requirements
- Task: Get additional perspectives if needed

What to do:
1. Break down implementation into specific, actionable steps
2. List files to create/modify/delete
3. Identify potential risks and edge cases
4. Verify alignment with user intent
5. Ask clarifying questions if anything is ambiguous
6. Document implementation steps in the Review section
7. Ensure nothing critical is missing

What NOT to do:
- Don't skip validation steps
- Don't assume - ask if uncertain
- Don't move forward with gaps in understanding

When ready: Move to Final phase to prepare for implementation.
"#
        }
        PlanPhase::Final => {
            r#"
FINAL PHASE - Prepare for implementation

Your goal: Finalize the plan and prepare to exit plan mode.

What to do:
1. Review the entire plan for completeness
2. Ensure all sections are filled with sufficient detail
3. Verify the implementation steps are clear and actionable
4. Confirm risks and considerations are documented
5. Add any final notes or reminders
6. Mark the plan as ready for implementation

What NOT to do:
- Don't exit without a complete plan
- Don't leave ambiguities or TODOs in the plan
- Don't skip documenting risks

When ready: Use ExitPlanMode to lift restrictions and begin implementation.
"#
        }
        PlanPhase::Exit => {
            "Plan mode completed. All restrictions lifted. You can now implement the plan."
        }
    }
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

/// Generate exploration topics for parallel investigation during Explore phase
///
/// Returns up to 3 investigation areas based on the goal
pub fn generate_exploration_topics(goal: &str) -> Vec<(String, String)> {
    // Returns (topic, focus) pairs for parallel exploration
    vec![
        (
            format!("Existing patterns for {}", goal),
            "Find similar implementations, patterns, and architectural choices in the codebase".to_string()
        ),
        (
            format!("Dependencies and constraints for {}", goal),
            "Identify external dependencies, libraries, and technical constraints that will impact implementation".to_string()
        ),
        (
            format!("Files and modules for {}", goal),
            "Locate all files, modules, and components that will need to be created or modified".to_string()
        ),
    ]
}

/// Generate design perspectives for parallel analysis during Design phase
///
/// Returns different architectural perspectives to consider
pub fn generate_design_perspectives() -> Vec<String> {
    vec![
        "Simplicity and maintainability".to_string(),
        "Performance and scalability".to_string(),
        "Type safety and error handling".to_string(),
    ]
}

/// Format exploration results for aggregation into the plan
pub fn format_exploration_results(results: &[(String, String)]) -> String {
    let mut formatted = String::from("## Exploration Results\n\n");

    for (i, (topic, findings)) in results.iter().enumerate() {
        formatted.push_str(&format!("### Investigation {}: {}\n\n", i + 1, topic));
        formatted.push_str(findings);
        formatted.push_str("\n\n");
    }

    formatted
}

/// Format design analysis results for aggregation into the plan
pub fn format_design_results(results: &[(String, String)]) -> String {
    let mut formatted = String::from("## Design Analysis\n\n");

    for (perspective, analysis) in results {
        formatted.push_str(&format!("### Perspective: {}\n\n", perspective));
        formatted.push_str(analysis);
        formatted.push_str("\n\n");
    }

    formatted
}

/// Suggested prompts for launching parallel explore agents
///
/// Returns a list of Task tool prompts ready to be executed in parallel
pub fn suggest_explore_agents(goal: &str) -> Vec<String> {
    generate_exploration_topics(goal)
        .into_iter()
        .map(|(topic, focus)| explore_agent_prompt(&topic, &focus))
        .collect()
}

/// Suggested prompts for launching parallel plan agents
///
/// Returns a list of Task tool prompts ready to be executed in parallel
pub fn suggest_plan_agents(feature: &str, context: &str) -> Vec<String> {
    generate_design_perspectives()
        .into_iter()
        .map(|perspective| plan_agent_prompt(feature, context, &perspective))
        .collect()
}

/// Check if a tool should be allowed in plan mode
pub fn is_tool_allowed_in_plan_mode(
    tool_name: &str,
    tool_input: &serde_json::Value,
) -> Result<(), String> {
    // Check sqlite3 write commands first (always blocked, regardless of plan mode)
    if tool_name == "Bash" {
        let command = tool_input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Block sqlite3 write commands
        // These must use the provided MCP APIs to maintain data consistency
        if command.contains("sqlite3") {
            let cmd_upper = command.to_uppercase();
            if cmd_upper.contains("INSERT")
                || cmd_upper.contains("UPDATE")
                || cmd_upper.contains("DELETE")
                || cmd_upper.contains("DROP")
                || cmd_upper.contains("ALTER")
                || cmd_upper.contains("CREATE TABLE")
                || cmd_upper.contains("TRUNCATE")
            {
                return Err("SQLite write commands are not allowed via sqlite3. Use the provided MCP APIs (issue_create, issue_update, issue_claim, issue_complete, issue_block) to maintain data consistency.".to_string());
            }
        }
    }

    if !is_plan_mode_active() {
        return Ok(()); // Not in plan mode, allow everything else
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
            // Check for destructive commands in plan mode
            let command = tool_input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Block git commits and pushes
            if command.contains("git commit") || command.contains("git push") {
                return Err(
                    "Git commits are not allowed in plan mode. Use ExitPlanMode to commit changes."
                        .to_string(),
                );
            }

            // Block file modifications
            if command.contains("rm ") || command.contains("mv ") || command.contains("cp ") {
                return Err("Destructive bash commands are not allowed in plan mode.".to_string());
            }

            // Allow read-only operations
            Ok(())
        }

        // Block execution-related tools
        "NotebookEdit" => Err(
            "Notebook edits are not allowed in plan mode. Use ExitPlanMode to implement changes."
                .to_string(),
        ),

        // Allow all other tools (EnterPlanMode, ExitPlanMode, etc.)
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to serialize tests that modify global plan mode state
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    #[ignore] // Requires filesystem access to ~/.claude/plans
    fn test_plan_path_generation() {
        let path = PlanModeConfig::generate_plan_path("test-feature").unwrap();
        assert!(path.to_str().unwrap().contains(".claude/plans"));
        assert!(path.to_str().unwrap().ends_with("test-feature.md"));
    }

    #[test]
    #[ignore] // Requires filesystem access to ~/.claude/plans
    fn test_tool_restrictions() {
        // Acquire mutex to serialize with other tests that modify global state
        let _guard = TEST_MUTEX.lock().unwrap();

        // Ensure clean state - directly reset global state
        PLAN_MODE_ACTIVE.store(false, Ordering::SeqCst);
        {
            let mut path = PLAN_FILE_PATH.write().expect("RwLock poisoned in test");
            *path = None;
        }

        // Not in plan mode - all tools allowed
        assert!(
            is_tool_allowed_in_plan_mode(
                "Write",
                &serde_json::json!({"file_path": "/tmp/test.txt"})
            )
            .is_ok()
        );

        // Enter plan mode
        let config = PlanModeConfig::new("test", "Test goal").unwrap();
        enter_plan_mode(config.clone()).unwrap();

        // Read operations allowed
        assert!(is_tool_allowed_in_plan_mode("Read", &serde_json::json!({})).is_ok());
        assert!(is_tool_allowed_in_plan_mode("Grep", &serde_json::json!({})).is_ok());

        // Plan file writes allowed
        assert!(
            is_tool_allowed_in_plan_mode(
                "Edit",
                &serde_json::json!({
                    "file_path": config.plan_file.to_str().unwrap()
                })
            )
            .is_ok()
        );

        // Other file writes blocked
        assert!(
            is_tool_allowed_in_plan_mode(
                "Write",
                &serde_json::json!({
                    "file_path": "/tmp/other.txt"
                })
            )
            .is_err()
        );

        // Git commits blocked
        assert!(
            is_tool_allowed_in_plan_mode(
                "Bash",
                &serde_json::json!({
                    "command": "git commit -m 'test'"
                })
            )
            .is_err()
        );

        // Read-only bash allowed
        assert!(
            is_tool_allowed_in_plan_mode(
                "Bash",
                &serde_json::json!({
                    "command": "git status"
                })
            )
            .is_ok()
        );

        // Exit plan mode
        exit_plan_mode().unwrap();

        // All tools allowed again
        assert!(
            is_tool_allowed_in_plan_mode(
                "Write",
                &serde_json::json!({"file_path": "/tmp/test.txt"})
            )
            .is_ok()
        );
    }

    #[test]
    fn test_sqlite3_write_blocking() {
        // SQLite write commands blocked regardless of plan mode

        // INSERT blocked
        assert!(
            is_tool_allowed_in_plan_mode(
                "Bash",
                &serde_json::json!({
                    "command": "sqlite3 autopilot.db \"INSERT INTO issues VALUES (1, 'test')\""
                })
            )
            .is_err()
        );

        // UPDATE blocked
        assert!(is_tool_allowed_in_plan_mode("Bash", &serde_json::json!({
            "command": "sqlite3 autopilot.db \"UPDATE issues SET status='done' WHERE number=1\""
        })).is_err());

        // DELETE blocked
        assert!(
            is_tool_allowed_in_plan_mode(
                "Bash",
                &serde_json::json!({
                    "command": "sqlite3 autopilot.db \"DELETE FROM issues WHERE number=1\""
                })
            )
            .is_err()
        );

        // DROP blocked
        assert!(
            is_tool_allowed_in_plan_mode(
                "Bash",
                &serde_json::json!({
                    "command": "sqlite3 autopilot.db \"DROP TABLE issues\""
                })
            )
            .is_err()
        );

        // ALTER blocked
        assert!(
            is_tool_allowed_in_plan_mode(
                "Bash",
                &serde_json::json!({
                    "command": "sqlite3 autopilot.db \"ALTER TABLE issues ADD COLUMN foo TEXT\""
                })
            )
            .is_err()
        );

        // CREATE TABLE blocked
        assert!(
            is_tool_allowed_in_plan_mode(
                "Bash",
                &serde_json::json!({
                    "command": "sqlite3 autopilot.db \"CREATE TABLE foo (id INTEGER)\""
                })
            )
            .is_err()
        );

        // TRUNCATE blocked
        assert!(
            is_tool_allowed_in_plan_mode(
                "Bash",
                &serde_json::json!({
                    "command": "sqlite3 autopilot.db \"TRUNCATE TABLE issues\""
                })
            )
            .is_err()
        );

        // Read-only SELECT allowed
        assert!(
            is_tool_allowed_in_plan_mode(
                "Bash",
                &serde_json::json!({
                    "command": "sqlite3 autopilot.db \"SELECT * FROM issues\""
                })
            )
            .is_ok()
        );

        // Case insensitive - lowercase insert blocked
        assert!(
            is_tool_allowed_in_plan_mode(
                "Bash",
                &serde_json::json!({
                    "command": "sqlite3 autopilot.db \"insert into issues values (1, 'test')\""
                })
            )
            .is_err()
        );

        // Mixed case UPDATE blocked
        assert!(
            is_tool_allowed_in_plan_mode(
                "Bash",
                &serde_json::json!({
                    "command": "sqlite3 autopilot.db \"Update issues SET status='done'\""
                })
            )
            .is_err()
        );
    }

    #[test]
    fn test_phase_prompts() {
        // Test that each phase has a non-empty prompt
        assert!(!get_phase_prompt(PlanPhase::Explore).is_empty());
        assert!(!get_phase_prompt(PlanPhase::Design).is_empty());
        assert!(!get_phase_prompt(PlanPhase::Review).is_empty());
        assert!(!get_phase_prompt(PlanPhase::Final).is_empty());
        assert!(!get_phase_prompt(PlanPhase::Exit).is_empty());

        // Test that prompts contain expected keywords
        assert!(get_phase_prompt(PlanPhase::Explore).contains("EXPLORE PHASE"));
        assert!(get_phase_prompt(PlanPhase::Design).contains("DESIGN PHASE"));
        assert!(get_phase_prompt(PlanPhase::Review).contains("REVIEW PHASE"));
        assert!(get_phase_prompt(PlanPhase::Final).contains("FINAL PHASE"));
    }

    #[test]
    #[ignore] // Requires filesystem access to ~/.claude/plans
    fn test_phase_advancement() {
        // Acquire mutex to serialize with other tests that modify global state
        let _guard = TEST_MUTEX.lock().unwrap();

        // Start in Explore phase
        let config = PlanModeConfig::new("test-phases", "Test phase advancement").unwrap();
        enter_plan_mode(config).unwrap();

        // Should start in Explore
        assert_eq!(get_current_phase(), PlanPhase::Explore);

        // Advance to Design
        let result = advance_phase();
        assert!(result.is_ok());
        assert_eq!(get_current_phase(), PlanPhase::Design);

        // Advance to Review
        let result = advance_phase();
        assert!(result.is_ok());
        assert_eq!(get_current_phase(), PlanPhase::Review);

        // Advance to Final
        let result = advance_phase();
        assert!(result.is_ok());
        assert_eq!(get_current_phase(), PlanPhase::Final);

        // Advance to Exit
        let result = advance_phase();
        assert!(result.is_ok());
        assert_eq!(get_current_phase(), PlanPhase::Exit);

        // Cannot advance beyond Exit
        let result = advance_phase();
        assert!(result.is_err());

        // Exit plan mode
        exit_plan_mode().unwrap();
    }

    #[test]
    #[ignore] // Requires filesystem access to ~/.claude/plans
    fn test_enter_plan_mode_includes_prompt() {
        // Acquire mutex to serialize with other tests that modify global state
        let _guard = TEST_MUTEX.lock().unwrap();

        // Enter plan mode should include the initial Explore phase prompt
        let config = PlanModeConfig::new("test-prompt", "Test initial prompt").unwrap();
        let result = enter_plan_mode(config).unwrap();

        // Should contain the Explore phase guidance
        assert!(result.contains("EXPLORE PHASE"));
        assert!(result.contains("Entered plan mode"));

        // Exit plan mode
        exit_plan_mode().unwrap();
    }

    #[test]
    #[ignore] // Requires filesystem access to ~/.claude/plans
    fn test_slug_sanitization() {
        // Test path traversal rejection
        assert!(PlanModeConfig::new("../../../etc/passwd", "goal").is_err());
        assert!(PlanModeConfig::new("../../secret", "goal").is_err());
        assert!(PlanModeConfig::new("/etc/passwd", "goal").is_err());
        assert!(PlanModeConfig::new("test/path", "goal").is_err());
        assert!(PlanModeConfig::new("test\\path", "goal").is_err());

        // Test valid slugs
        assert!(PlanModeConfig::new("my-feature", "goal").is_ok());
        assert!(PlanModeConfig::new("test123", "goal").is_ok());
        assert!(PlanModeConfig::new("feature_xyz", "goal").is_ok());
    }

    #[test]
    fn test_generate_exploration_topics() {
        let goal = "add user authentication";
        let topics = generate_exploration_topics(goal);

        // Should generate 3 topics
        assert_eq!(topics.len(), 3);

        // Check that topics are relevant
        assert!(topics[0].0.contains(goal));
        assert!(topics[1].0.contains(goal));
        assert!(topics[2].0.contains(goal));

        // Check that each has a focus
        assert!(!topics[0].1.is_empty());
        assert!(!topics[1].1.is_empty());
        assert!(!topics[2].1.is_empty());
    }

    #[test]
    fn test_generate_design_perspectives() {
        let perspectives = generate_design_perspectives();

        // Should generate 3 perspectives
        assert_eq!(perspectives.len(), 3);

        // Check for expected perspectives
        assert!(perspectives.iter().any(|p| p.contains("maintainability")));
        assert!(perspectives.iter().any(|p| p.contains("Performance")));
        assert!(perspectives.iter().any(|p| p.contains("Type safety")));
    }

    #[test]
    fn test_explore_agent_prompt() {
        let topic = "existing authentication patterns";
        let focus = "Find all auth-related modules";
        let prompt = explore_agent_prompt(topic, focus);

        // Should contain the topic and focus
        assert!(prompt.contains(topic));
        assert!(prompt.contains(focus));

        // Should contain expected instructions
        assert!(prompt.contains("Glob"));
        assert!(prompt.contains("Grep"));
        assert!(prompt.contains("Read"));
    }

    #[test]
    fn test_plan_agent_prompt() {
        let feature = "JWT authentication";
        let context = "Found 3 existing auth modules";
        let perspective = "Security and type safety";
        let prompt = plan_agent_prompt(feature, context, perspective);

        // Should contain all parameters
        assert!(prompt.contains(feature));
        assert!(prompt.contains(context));
        assert!(prompt.contains(perspective));

        // Should be about planning not implementing
        assert!(prompt.contains("designing"));
        assert!(prompt.contains("architecture"));
    }

    #[test]
    fn test_suggest_explore_agents() {
        let goal = "implement caching layer";
        let prompts = suggest_explore_agents(goal);

        // Should generate 3 prompts (one per exploration topic)
        assert_eq!(prompts.len(), 3);

        // Each should be a complete prompt
        for prompt in &prompts {
            assert!(prompt.contains("You are an exploration agent"));
            assert!(prompt.contains(goal));
        }
    }

    #[test]
    fn test_suggest_plan_agents() {
        let feature = "add caching";
        let context = "Found Redis and in-memory options";
        let prompts = suggest_plan_agents(feature, context);

        // Should generate 3 prompts (one per perspective)
        assert_eq!(prompts.len(), 3);

        // Each should be a complete prompt
        for prompt in &prompts {
            assert!(prompt.contains("You are a planning agent"));
            assert!(prompt.contains(feature));
            assert!(prompt.contains(context));
        }
    }

    #[test]
    fn test_format_exploration_results() {
        let results = vec![
            ("Finding A".to_string(), "Details about A".to_string()),
            ("Finding B".to_string(), "Details about B".to_string()),
        ];
        let formatted = format_exploration_results(&results);

        // Should contain markdown headers
        assert!(formatted.contains("## Exploration Results"));
        assert!(formatted.contains("### Investigation 1"));
        assert!(formatted.contains("### Investigation 2"));

        // Should contain the topics and findings
        assert!(formatted.contains("Finding A"));
        assert!(formatted.contains("Details about A"));
        assert!(formatted.contains("Finding B"));
        assert!(formatted.contains("Details about B"));
    }

    #[test]
    fn test_format_design_results() {
        let results = vec![
            ("Perspective A".to_string(), "Analysis A".to_string()),
            ("Perspective B".to_string(), "Analysis B".to_string()),
        ];
        let formatted = format_design_results(&results);

        // Should contain markdown headers
        assert!(formatted.contains("## Design Analysis"));
        assert!(formatted.contains("### Perspective: Perspective A"));
        assert!(formatted.contains("### Perspective: Perspective B"));

        // Should contain the analyses
        assert!(formatted.contains("Analysis A"));
        assert!(formatted.contains("Analysis B"));
    }
}
