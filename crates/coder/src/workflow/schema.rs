//! Workflow schema types for the Coder platform.
//!
//! These types define the structure of agent workflows, including triggers,
//! policies, steps, and artifacts.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A complete workflow specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSpec {
    pub version: u32,
    pub workflow_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub project: ProjectRef,
    pub triggers: Vec<Trigger>,
    #[serde(default)]
    pub policies: Policies,
    pub steps: Vec<StepSpec>,
}

/// Reference to a project/repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRef {
    pub provider: RepoProvider,
    pub owner: String,
    pub repo: String,
    #[serde(default)]
    pub r#ref: Option<String>,
}

/// Supported repository providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoProvider {
    Github,
    Gitlab,
}

/// Workflow trigger types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    Manual {},
    Cron { cron: String, timezone: String },
    RepoEvent { event: String },
    Webhook { name: String, secret_ref: String },
}

/// Workflow policies for security and governance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Policies {
    #[serde(default)]
    pub budget: BudgetPolicy,
    #[serde(default)]
    pub repo: RepoPolicy,
    #[serde(default)]
    pub secrets: SecretsPolicy,
    #[serde(default)]
    pub gates: GatesPolicy,
}

/// Budget constraints for workflow execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BudgetPolicy {
    #[serde(default)]
    pub max_credits: Option<u64>,
    #[serde(default)]
    pub max_wall_clock_sec: Option<u64>,
}

/// Repository access constraints.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoPolicy {
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    #[serde(default)]
    pub blocked_paths: Vec<String>,
}

/// Secret access constraints.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretsPolicy {
    #[serde(default)]
    pub allowed: Vec<String>,
}

/// Approval gates policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GatesPolicy {
    /// Named actions that require explicit approval (human or role-based).
    #[serde(default)]
    pub require_human_approval_for: Vec<String>,
}

/// A step in a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepSpec {
    Agent(AgentStep),
    Command(CommandStep),
    Deploy(DeployStep),
    Approve(ApproveStep),
}

/// An agent execution step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub step_id: String,
    pub role: AgentRole,
    pub goal: String,
    #[serde(default)]
    pub inputs: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub on_failure: Option<OnFailure>,
}

/// Agent roles in the MechaCoder system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Architect,
    Implementer,
    Tester,
    Reviewer,
    ReleaseEngineer,
}

/// A command execution step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandStep {
    pub step_id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub on_failure: Option<OnFailure>,
}

/// A deployment step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployStep {
    pub step_id: String,
    pub target: DeployTarget,
    #[serde(default)]
    pub inputs: BTreeMap<String, serde_json::Value>,
}

/// Deployment targets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployTarget {
    Preview,
    Prod,
}

/// An approval step requiring human or policy confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveStep {
    pub step_id: String,
    pub reason: String,
}

/// Failure handling strategies.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OnFailure {
    FailFast { message: String },
    AgentRetry { role: AgentRole, max_attempts: u32 },
}

/// Status of a workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

/// A workflow run instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub run_id: String,
    pub workflow_id: String,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: RunStatus,
    #[serde(default)]
    pub cost_summary: Option<CostSummary>,
    #[serde(default)]
    pub step_runs: Vec<StepRun>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
}

/// Cost summary for a workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    pub credits_used: u64,
    pub wall_clock_sec: u64,
}

/// A step run instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRun {
    pub step_id: String,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: RunStatus,
    #[serde(default)]
    pub cost: Option<u64>,
}

/// Artifacts produced by workflow runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Artifact {
    Patch {
        diff: String,
        base_ref: String,
    },
    PrLink {
        provider: RepoProvider,
        pr_id: u64,
        url: String,
    },
    TestReport {
        passed: u32,
        failed: u32,
        skipped: u32,
        summary: String,
    },
    DeployUrl {
        target: DeployTarget,
        url: String,
    },
    LogBundle {
        key: String,
    },
    ReleaseNotes {
        version: String,
        changelog: String,
    },
}
