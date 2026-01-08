//! Adjutant - The agent that DOES THE WORK.
//!
//! Named after StarCraft's command & control AI.
//!
//! Adjutant is not just a router - it directly uses tools to accomplish tasks.
//! For complex work, it can delegate to Claude Code.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      AUTOPILOT CLI                           │
//! │  (user-facing: `autopilot run`, `autopilot issue claim`)     │
//! └─────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   OANIX (background)                         │
//! │  (discovers environment, reads .openagents/)                 │
//! └─────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                       ADJUTANT                               │
//! │  The actual agent that DOES THE WORK                         │
//! │  - Prioritizes Claude (Pro/Max) via claude-agent-sdk         │
//! │  - Falls back to Cerebras TieredExecutor                     │
//! │  - Uses tools directly (Read, Edit, Bash, Glob, Grep)        │
//! │  - Delegates to Claude Code for very complex work            │
//! │  - Uses RLM for large context analysis                       │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Execution Priority
//!
//! 1. **Claude Pro/Max** - If Claude CLI is installed, use `claude-agent-sdk`
//! 2. **Cerebras TieredExecutor** - If CEREBRAS_API_KEY is set
//! 3. **Analysis-only** - If neither is available

pub mod auth;
pub mod cli;
pub mod claude_executor;
pub mod delegate;
pub mod executor;
pub mod planner;
pub mod rlm_agent;
pub mod tiered;
pub mod tools;

use oanix::{OanixManifest, WorkspaceManifest};
use std::path::PathBuf;
use thiserror::Error;

pub use auth::{get_claude_path, has_claude_cli};
pub use claude_executor::ClaudeExecutor;
pub use executor::TaskResult;
pub use planner::{Complexity, TaskPlan};
pub use rlm_agent::{rlm_agent_definition, rlm_agent_with_write_access};
pub use tiered::TieredExecutor;
pub use tools::{Tool, ToolRegistry};

/// Errors that can occur during Adjutant operations.
#[derive(Error, Debug)]
pub enum AdjutantError {
    #[error("No workspace found - run from a project directory with .openagents/")]
    NoWorkspace,

    #[error("Task planning failed: {0}")]
    PlanningFailed(String),

    #[error("Task execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Tool error: {0}")]
    ToolError(String),

    #[error("Claude Code delegation failed: {0}")]
    DelegationFailed(String),

    #[error("RLM error: {0}")]
    RlmError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// A task for Adjutant to execute.
#[derive(Debug, Clone)]
pub struct Task {
    /// Task ID (e.g., issue number)
    pub id: String,
    /// Task title
    pub title: String,
    /// Task description
    pub description: String,
    /// Files to consider (optional hints)
    pub files: Vec<PathBuf>,
    /// Acceptance criteria
    pub acceptance_criteria: Vec<String>,
}

impl Task {
    /// Create a new task.
    pub fn new(id: impl Into<String>, title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            files: Vec::new(),
            acceptance_criteria: Vec::new(),
        }
    }

    /// Create a task from an issue.
    pub fn from_issue(issue: &issues::Issue) -> Self {
        Self {
            id: format!("#{}", issue.number),
            title: issue.title.clone(),
            description: issue.description.clone().unwrap_or_default(),
            files: Vec::new(),
            acceptance_criteria: Vec::new(),
        }
    }

    /// Convert task to a prompt for Claude.
    pub fn to_prompt(&self) -> String {
        let mut prompt = format!("Task {}: {}\n\n{}", self.id, self.title, self.description);

        if !self.acceptance_criteria.is_empty() {
            prompt.push_str("\n\nAcceptance criteria:\n");
            for criterion in &self.acceptance_criteria {
                prompt.push_str(&format!("- {}\n", criterion));
            }
        }

        prompt
    }
}

/// Adjutant: The agent that DOES THE WORK.
pub struct Adjutant {
    /// Tool registry
    tools: ToolRegistry,
    /// OANIX manifest (compute, network, identity)
    manifest: OanixManifest,
    /// Workspace root
    workspace_root: PathBuf,
}

impl Adjutant {
    /// Create a new Adjutant from an OANIX manifest.
    pub fn new(manifest: OanixManifest) -> Result<Self, AdjutantError> {
        let workspace = manifest
            .workspace
            .as_ref()
            .ok_or(AdjutantError::NoWorkspace)?;

        let workspace_root = workspace.root.clone();
        let tools = ToolRegistry::new(&workspace_root);

        Ok(Self {
            tools,
            manifest,
            workspace_root,
        })
    }

    /// Get the workspace manifest.
    pub fn workspace(&self) -> Option<&WorkspaceManifest> {
        self.manifest.workspace.as_ref()
    }

    /// Execute a task - Adjutant does the work itself.
    ///
    /// This method determines the best execution strategy based on:
    /// - Task complexity (from planner)
    /// - Available backends (Claude CLI, Cerebras, etc.)
    /// - Context size (for RLM routing)
    /// - Task description keywords (analyze, recursive, etc.)
    pub async fn execute(&mut self, task: &Task) -> Result<TaskResult, AdjutantError> {
        tracing::info!("Adjutant analyzing task: {}", task.title);

        // 1. Plan the task
        let plan = self.plan_task(task).await?;
        tracing::info!(
            "Plan: {} files, complexity {:?}",
            plan.files.len(),
            plan.complexity
        );

        // 2. Determine if RLM mode should be used
        let use_rlm = self.should_use_rlm(task, &plan);

        // 3. Check if Claude CLI is available
        if has_claude_cli() {
            // Build context from relevant files
            let context = self.build_context(&plan).await?;

            let executor = ClaudeExecutor::new(&self.workspace_root);

            if use_rlm {
                tracing::info!("Using Claude with RLM support for complex analysis");
                // Enable RLM tools based on environment variable
                let enable_rlm_tools = std::env::var("ADJUTANT_ENABLE_RLM")
                    .map(|v| v == "1" || v.to_lowercase() == "true")
                    .unwrap_or(true);
                return executor.execute_with_rlm(task, &context, enable_rlm_tools).await;
            }

            tracing::info!("Using Claude standard execution");
            return executor.execute(task, &context, &mut self.tools).await;
        }

        // 4. Fallback: Check complexity for delegation or RLM
        if plan.complexity >= Complexity::High || plan.files.len() > 20 {
            tracing::info!("Complexity high - delegating to Claude Code");
            return self.delegate_to_claude_code(task).await;
        }

        if plan.estimated_tokens > 100_000 {
            tracing::info!("Context too large - using RLM");
            return self.execute_with_rlm_delegate(task, &plan).await;
        }

        // 5. Do the work myself using tools
        tracing::info!("Executing with local tools");
        self.execute_with_tools(task, &plan).await
    }

    /// Determine if RLM mode should be used for a task.
    fn should_use_rlm(&self, task: &Task, plan: &TaskPlan) -> bool {
        // High complexity tasks benefit from RLM
        if plan.complexity >= Complexity::High {
            return true;
        }

        // Large context benefits from RLM's orchestrated analysis
        if plan.estimated_tokens > 50_000 {
            return true;
        }

        // Check task description for RLM-friendly keywords
        let description_lower = task.description.to_lowercase();
        let rlm_keywords = [
            "analyze",
            "recursive",
            "investigate",
            "find all",
            "security",
            "audit",
            "review",
            "deep dive",
            "comprehensive",
        ];

        for keyword in &rlm_keywords {
            if description_lower.contains(keyword) {
                return true;
            }
        }

        false
    }

    /// Build context string from planned files.
    async fn build_context(&mut self, plan: &TaskPlan) -> Result<String, AdjutantError> {
        let mut context = String::new();
        for file in &plan.files {
            let result = self.tools.read(file).await.map_err(|e| {
                AdjutantError::ToolError(format!("Failed to read {}: {}", file.display(), e))
            })?;
            if result.success {
                context.push_str(&format!(
                    "\n--- {} ---\n{}\n",
                    file.display(),
                    result.content
                ));
            }
        }
        Ok(context)
    }

    /// Plan a task - analyze what needs to be done.
    async fn plan_task(&self, task: &Task) -> Result<TaskPlan, AdjutantError> {
        planner::plan_task(&self.tools, &self.workspace_root, task).await
    }

    /// Execute task using local tools.
    async fn execute_with_tools(
        &mut self,
        task: &Task,
        plan: &TaskPlan,
    ) -> Result<TaskResult, AdjutantError> {
        executor::execute_with_tools(&mut self.tools, &self.workspace_root, task, plan).await
    }

    /// Delegate complex work to Claude Code.
    async fn delegate_to_claude_code(&self, task: &Task) -> Result<TaskResult, AdjutantError> {
        delegate::delegate_to_claude_code(&self.workspace_root, task).await
    }

    /// Execute task using RLM delegate for large context (fallback when Claude CLI not available).
    async fn execute_with_rlm_delegate(
        &self,
        task: &Task,
        plan: &TaskPlan,
    ) -> Result<TaskResult, AdjutantError> {
        delegate::execute_with_rlm(&self.workspace_root, task, plan).await
    }
}
