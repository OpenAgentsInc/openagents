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
//! │  - Uses tools directly (Read, Edit, Bash, Glob, Grep)        │
//! │  - Sometimes delegates to Claude Code for complex stuff      │
//! │  - Uses RLM for large context analysis                       │
//! └─────────────────────────────────────────────────────────────┘
//! ```

pub mod cli;
pub mod delegate;
pub mod executor;
pub mod planner;
pub mod tiered;
pub mod tools;

use oanix::{OanixManifest, WorkspaceManifest};
use std::path::PathBuf;
use thiserror::Error;

pub use executor::TaskResult;
pub use planner::{Complexity, TaskPlan};
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
    pub async fn execute(&mut self, task: &Task) -> Result<TaskResult, AdjutantError> {
        tracing::info!("Adjutant analyzing task: {}", task.title);

        // 1. Plan the task
        let plan = self.plan_task(task).await?;
        tracing::info!(
            "Plan: {} files, complexity {:?}",
            plan.files.len(),
            plan.complexity
        );

        // 2. Decide: do it myself or delegate?
        if plan.complexity >= Complexity::High || plan.files.len() > 20 {
            tracing::info!("Complexity high - delegating to Claude Code");
            return self.delegate_to_claude_code(task).await;
        }

        if plan.estimated_tokens > 100_000 {
            tracing::info!("Context too large - using RLM");
            return self.execute_with_rlm(task, &plan).await;
        }

        // 3. Do the work myself using tools
        tracing::info!("Executing with local tools");
        self.execute_with_tools(task, &plan).await
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

    /// Execute task using RLM for large context.
    async fn execute_with_rlm(
        &self,
        task: &Task,
        plan: &TaskPlan,
    ) -> Result<TaskResult, AdjutantError> {
        delegate::execute_with_rlm(&self.workspace_root, task, plan).await
    }
}
