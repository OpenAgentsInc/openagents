//! AutopilotAgent - Browser-compatible agent that uses /compute mount.
//!
//! This agent implements the runtime Agent trait and submits compute jobs
//! through the /compute filesystem mount instead of spawning local processes.
//!
//! On native platforms, the agent uses DSPy-powered planning for optimizable
//! structured outputs. On WASM, it falls back to legacy prompt-based planning.

use openagents_runtime::{
    Agent, AgentConfig, AgentContext, AgentEnv, AgentState, Trigger, TickResult,
    error::Result, types::Timestamp,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// DSPy planning integration (native-only)
#[cfg(not(target_arch = "wasm32"))]
use crate::dspy_planning::{PlanningInput, PlanningPipeline, PlanningResult};

// DSPy execution integration (native-only)
#[cfg(not(target_arch = "wasm32"))]
use crate::dspy_execution::{ExecutionPipeline, ExecutionInput, ExecutionDecision, ExecutionAction};

/// Helper to create a TickResult that hibernates.
fn tick_hibernate() -> TickResult {
    TickResult {
        should_hibernate: true,
        ..Default::default()
    }
}

/// Helper to create a TickResult that reschedules after given milliseconds.
fn tick_reschedule_millis(millis: u64) -> TickResult {
    let alarm = Timestamp::from_millis(Timestamp::now().as_millis() + millis);
    TickResult {
        next_alarm: Some(alarm),
        ..Default::default()
    }
}

/// Autopilot execution phase.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum AutopilotPhase {
    /// Initial planning phase - analyze issue and create plan.
    #[default]
    Planning,
    /// Execution phase - implement the plan.
    Executing,
    /// Review phase - verify changes and run tests.
    Reviewing,
    /// Completed - all phases finished.
    Complete,
    /// Failed with error.
    Failed,
}

/// Persistent state for AutopilotAgent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutopilotState {
    /// Current execution phase.
    pub phase: AutopilotPhase,
    /// Generated plan from planning phase.
    pub plan: Option<String>,
    /// Pending job ID (if waiting for a compute job).
    pub pending_job_id: Option<String>,
    /// Changes made during execution.
    pub changes: Vec<String>,
    /// Error message if failed.
    pub error: Option<String>,
    /// Tick count for debugging.
    pub tick_count: u64,
    /// Current step index being executed (for DSPy execution).
    pub current_step_index: usize,
    /// Parsed plan steps (from JSON plan).
    pub plan_steps: Vec<String>,
    /// Execution history as JSON array of previous actions and results.
    pub execution_history: String,
}

impl AgentState for AutopilotState {
    fn version() -> u32 {
        1
    }
}

/// Configuration for AutopilotAgent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutopilotConfig {
    /// Model to use for compute requests.
    pub model: String,
    /// Maximum cost in micro-USD per tick.
    pub max_cost_per_tick_usd: u64,
    /// Maximum cost in micro-USD per day.
    pub max_cost_per_day_usd: u64,
}

impl AgentConfig for AutopilotConfig {
    fn validate(&self) -> Result<()> {
        if self.model.is_empty() {
            return Err(openagents_runtime::error::AgentError::Tick(
                "invalid config: model cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// AutopilotAgent - Autonomous coding agent that runs in browser via /compute mount.
///
/// This agent:
/// 1. Analyzes issues and creates implementation plans
/// 2. Executes plans by submitting NIP-90 jobs via /compute
/// 3. Reviews changes and runs verification
/// 4. Reports results via /hud mount
pub struct AutopilotAgent {
    /// Environment for accessing mounted filesystems.
    env: Arc<AgentEnv>,
    /// Repository URL to work on.
    repo_url: String,
    /// Issue description to solve.
    issue_description: String,
    /// Configuration.
    config: AutopilotConfig,
    /// DSPy planning pipeline (native-only).
    #[cfg(not(target_arch = "wasm32"))]
    planning_pipeline: PlanningPipeline,
    /// DSPy execution pipeline (native-only).
    #[cfg(not(target_arch = "wasm32"))]
    execution_pipeline: ExecutionPipeline,
}

impl AutopilotAgent {
    /// Create a new AutopilotAgent.
    pub fn new(
        env: Arc<AgentEnv>,
        repo_url: String,
        issue_description: String,
    ) -> Self {
        Self {
            env,
            repo_url,
            issue_description,
            config: AutopilotConfig {
                model: "claude-sonnet-4-20250514".to_string(),
                max_cost_per_tick_usd: 5_000_000,  // $5
                max_cost_per_day_usd: 100_000_000, // $100
            },
            #[cfg(not(target_arch = "wasm32"))]
            planning_pipeline: PlanningPipeline::new(),
            #[cfg(not(target_arch = "wasm32"))]
            execution_pipeline: ExecutionPipeline::new(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(
        env: Arc<AgentEnv>,
        repo_url: String,
        issue_description: String,
        config: AutopilotConfig,
    ) -> Self {
        Self {
            env,
            repo_url,
            issue_description,
            config,
            #[cfg(not(target_arch = "wasm32"))]
            planning_pipeline: PlanningPipeline::new(),
            #[cfg(not(target_arch = "wasm32"))]
            execution_pipeline: ExecutionPipeline::new(),
        }
    }

    /// Run the planning phase.
    ///
    /// On native platforms, this uses the DSPy-powered PlanningPipeline for
    /// optimizable structured outputs. Falls back to legacy prompt-based
    /// planning if DSPy fails or on WASM.
    fn run_planning(&self, ctx: &mut AgentContext<AutopilotState>) -> Result<TickResult> {
        // Check if we're waiting for a job result (legacy mode)
        if let Some(job_id) = &ctx.state.pending_job_id {
            return self.poll_job_result(ctx, job_id.clone());
        }

        // On native, try DSPy planning first
        #[cfg(not(target_arch = "wasm32"))]
        {
            match self.try_dspy_planning(ctx) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    // Log DSPy failure and fall back to legacy
                    tracing::warn!("DSPy planning failed, falling back to legacy: {}", e);
                    self.write_hud_status("Planning", "Using legacy planning (DSPy unavailable)");
                }
            }
        }

        // Legacy planning via /compute mount
        self.run_planning_legacy(ctx)
    }

    /// Legacy planning via /compute mount (prompt-based).
    fn run_planning_legacy(&self, ctx: &mut AgentContext<AutopilotState>) -> Result<TickResult> {
        let prompt = self.build_planning_prompt();
        match self.submit_chat_job(&prompt, PLANNING_SYSTEM_PROMPT) {
            Ok(job_id) => {
                ctx.state.pending_job_id = Some(job_id.clone());
                self.write_hud_status("Planning", &format!("Submitted planning job: {}", job_id));
                Ok(tick_reschedule_millis(1000))
            }
            Err(e) => {
                ctx.state.phase = AutopilotPhase::Failed;
                ctx.state.error = Some(format!("Failed to submit planning job: {}", e));
                self.write_hud_status("Failed", &format!("Planning failed: {}", e));
                Ok(tick_hibernate())
            }
        }
    }

    /// DSPy-powered planning (native-only).
    ///
    /// Uses the PlanningPipeline to generate structured, optimizable plans.
    #[cfg(not(target_arch = "wasm32"))]
    fn try_dspy_planning(&self, ctx: &mut AgentContext<AutopilotState>) -> std::result::Result<TickResult, anyhow::Error> {
        use anyhow::Context;

        // Build structured input
        let input = PlanningInput {
            repository_summary: self.get_repo_summary(),
            issue_description: self.issue_description.clone(),
            relevant_files: self.get_relevant_files(),
            code_patterns: None,
        };

        self.write_hud_status("Planning", "Running DSPy planning pipeline...");

        // Run DSPy pipeline synchronously (block on async)
        // Note: In a real async runtime, this would be properly awaited
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.planning_pipeline.plan(&input))
        }).context("DSPy planning pipeline failed")?;

        // Store structured result as JSON
        let plan_json = serde_json::to_string_pretty(&result)
            .context("Failed to serialize planning result")?;

        ctx.state.plan = Some(plan_json.clone());
        ctx.state.phase = AutopilotPhase::Executing;

        self.write_hud_status(
            "Executing",
            &format!(
                "DSPy plan created (complexity: {:?}, confidence: {:.2})",
                result.complexity,
                result.confidence
            ),
        );

        Ok(tick_reschedule_millis(100))
    }

    /// Get repository summary for DSPy planning.
    #[cfg(not(target_arch = "wasm32"))]
    fn get_repo_summary(&self) -> String {
        // Try to read from /repo mount or return placeholder
        if let Ok(readme_bytes) = self.env.read("/repo/README.md") {
            let readme = String::from_utf8_lossy(&readme_bytes);
            format!("Repository: {}\n\nREADME excerpt:\n{}", self.repo_url, &readme[..readme.len().min(1000)])
        } else {
            format!("Repository: {}", self.repo_url)
        }
    }

    /// Get list of relevant files for DSPy planning.
    #[cfg(not(target_arch = "wasm32"))]
    fn get_relevant_files(&self) -> String {
        // Try to list source files from /repo mount
        if let Ok(_files_bytes) = self.env.read("/repo/.git/index") {
            // Git index exists, repo is available
            // For now, return placeholder - could list actual files
            "Source files available in repository".to_string()
        } else {
            "Repository files not yet indexed".to_string()
        }
    }

    /// DSPy-powered execution (native-only).
    ///
    /// Uses the ExecutionPipeline to decide next actions step-by-step.
    #[cfg(not(target_arch = "wasm32"))]
    fn try_dspy_execution(&self, ctx: &mut AgentContext<AutopilotState>) -> std::result::Result<TickResult, anyhow::Error> {
        use anyhow::Context;

        // Get current step
        let step = ctx.state.plan_steps.get(ctx.state.current_step_index)
            .context("No more steps to execute")?
            .clone();

        // Build input
        let input = ExecutionInput {
            plan_step: step.clone(),
            current_file_state: self.get_current_file_state(),
            execution_history: ctx.state.execution_history.clone(),
        };

        self.write_hud_status(
            "Executing",
            &format!("DSPy execution: step {}/{}", ctx.state.current_step_index + 1, ctx.state.plan_steps.len())
        );

        // Get decision from DSPy
        let decision = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.execution_pipeline.decide(&input))
        }).context("DSPy execution decision failed")?;

        // Update execution history
        let history_entry = serde_json::json!({
            "step": step,
            "action": format!("{:?}", decision.next_action),
            "reasoning": decision.reasoning,
            "progress": decision.progress_estimate
        });
        let mut history: Vec<serde_json::Value> = serde_json::from_str(&ctx.state.execution_history)
            .unwrap_or_default();
        history.push(history_entry);
        ctx.state.execution_history = serde_json::to_string(&history).unwrap_or_default();

        // Execute based on action type
        match decision.next_action {
            ExecutionAction::Complete => {
                ctx.state.current_step_index += 1;
                if ctx.state.current_step_index >= ctx.state.plan_steps.len() {
                    ctx.state.phase = AutopilotPhase::Reviewing;
                    self.write_hud_status("Reviewing", "All steps executed via DSPy");
                } else {
                    self.write_hud_status(
                        "Executing",
                        &format!("Step {} complete, moving to next", ctx.state.current_step_index)
                    );
                }
                Ok(tick_reschedule_millis(100))
            }
            ExecutionAction::EditFile | ExecutionAction::RunCommand | ExecutionAction::ReadFile => {
                // Submit action as compute job
                match self.submit_execution_action(ctx, &decision) {
                    Ok(job_id) => {
                        ctx.state.pending_job_id = Some(job_id.clone());
                        self.write_hud_status(
                            "Executing",
                            &format!("Submitted {:?} job: {}", decision.next_action, job_id)
                        );
                        Ok(tick_reschedule_millis(1000))
                    }
                    Err(e) => {
                        anyhow::bail!("Failed to submit action: {}", e);
                    }
                }
            }
            ExecutionAction::Unknown => {
                anyhow::bail!("Unknown action from DSPy: {:?}", decision.next_action);
            }
        }
    }

    /// Get current file state for execution context.
    #[cfg(not(target_arch = "wasm32"))]
    fn get_current_file_state(&self) -> Option<String> {
        // Return None for now - would be populated during file editing
        None
    }

    /// Submit an execution action as a compute job.
    #[cfg(not(target_arch = "wasm32"))]
    fn submit_execution_action(
        &self,
        ctx: &mut AgentContext<AutopilotState>,
        decision: &ExecutionDecision,
    ) -> std::result::Result<String, String> {
        let kind = match decision.next_action {
            ExecutionAction::EditFile => "file_edit",
            ExecutionAction::RunCommand => "command",
            ExecutionAction::ReadFile => "file_read",
            _ => "chat",
        };

        let request = serde_json::json!({
            "model": self.config.model,
            "kind": kind,
            "input": decision.action_params,
            "context": {
                "step_index": ctx.state.current_step_index,
                "reasoning": decision.reasoning,
            },
            "max_cost_usd": self.config.max_cost_per_tick_usd
        });

        let request_bytes = serde_json::to_vec(&request)
            .map_err(|e| format!("serialize error: {}", e))?;

        let response = self.env.call("/compute/new", &request_bytes)
            .map_err(|e| format!("submit error: {}", e))?;

        let job: serde_json::Value = serde_json::from_slice(&response)
            .map_err(|e| format!("parse error: {}", e))?;

        job.get("job_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "missing job_id".to_string())
    }

    /// Parse plan JSON into executable steps.
    fn parse_plan_steps(&self, ctx: &mut AgentContext<AutopilotState>) {
        if let Some(plan_json) = &ctx.state.plan {
            // Try to parse as structured PlanningResult
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(plan_json) {
                // Look for steps array in the parsed JSON
                if let Some(steps) = parsed.get("steps").and_then(|s| s.as_array()) {
                    ctx.state.plan_steps = steps.iter()
                        .filter_map(|s| {
                            // Steps might be strings or objects with description
                            if let Some(str_val) = s.as_str() {
                                Some(str_val.to_string())
                            } else if let Some(desc) = s.get("description").and_then(|d| d.as_str()) {
                                Some(desc.to_string())
                            } else {
                                None
                            }
                        })
                        .collect();
                }
            }

            // If no steps parsed, treat the whole plan as a single step
            if ctx.state.plan_steps.is_empty() {
                ctx.state.plan_steps = vec![plan_json.clone()];
            }
        }

        // Initialize execution history if empty
        if ctx.state.execution_history.is_empty() {
            ctx.state.execution_history = "[]".to_string();
        }
    }

    /// Run the execution phase.
    ///
    /// On native platforms, this uses DSPy-powered step-by-step execution.
    /// Falls back to legacy monolithic patch generation if DSPy fails or on WASM.
    fn run_execution(&self, ctx: &mut AgentContext<AutopilotState>) -> Result<TickResult> {
        // Check if we're waiting for a job result
        if let Some(job_id) = &ctx.state.pending_job_id {
            return self.poll_job_result(ctx, job_id.clone());
        }

        // Parse plan steps on first execution tick
        if ctx.state.plan_steps.is_empty() {
            self.parse_plan_steps(ctx);
        }

        // On native, try DSPy execution first
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Only try DSPy if we have parsed steps
            if !ctx.state.plan_steps.is_empty() {
                match self.try_dspy_execution(ctx) {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        tracing::warn!("DSPy execution failed, falling back to legacy: {}", e);
                        self.write_hud_status("Executing", "Using legacy execution (DSPy unavailable)");
                    }
                }
            }
        }

        // Legacy: submit monolithic patch_gen job
        self.run_execution_legacy(ctx)
    }

    /// Legacy execution via /compute mount (monolithic patch generation).
    fn run_execution_legacy(&self, ctx: &mut AgentContext<AutopilotState>) -> Result<TickResult> {
        let plan = ctx.state.plan.clone().unwrap_or_default();
        match self.submit_patch_gen_job(&plan) {
            Ok(job_id) => {
                ctx.state.pending_job_id = Some(job_id.clone());
                self.write_hud_status("Executing", &format!("Submitted patch job: {}", job_id));
                Ok(tick_reschedule_millis(2000))
            }
            Err(e) => {
                ctx.state.phase = AutopilotPhase::Failed;
                ctx.state.error = Some(format!("Failed to submit patch job: {}", e));
                self.write_hud_status("Failed", &format!("Execution failed: {}", e));
                Ok(tick_hibernate())
            }
        }
    }

    /// Run the review phase.
    fn run_review(&self, ctx: &mut AgentContext<AutopilotState>) -> Result<TickResult> {
        // Check if we're waiting for a job result
        if let Some(job_id) = &ctx.state.pending_job_id {
            return self.poll_job_result(ctx, job_id.clone());
        }

        // Submit a CodeReview job
        let changes = ctx.state.changes.join("\n");
        match self.submit_code_review_job(&changes) {
            Ok(job_id) => {
                ctx.state.pending_job_id = Some(job_id.clone());
                self.write_hud_status("Reviewing", &format!("Submitted review job: {}", job_id));
                Ok(tick_reschedule_millis(2000))
            }
            Err(e) => {
                // Review failure is not fatal, mark complete
                ctx.state.phase = AutopilotPhase::Complete;
                self.write_hud_status("Complete", &format!("Review skipped: {}", e));
                Ok(tick_hibernate())
            }
        }
    }

    /// Poll for job result.
    fn poll_job_result(
        &self,
        ctx: &mut AgentContext<AutopilotState>,
        job_id: String,
    ) -> Result<TickResult> {
        let status_path = format!("/compute/jobs/{}/status", job_id);

        match self.env.read(&status_path) {
            Ok(status_bytes) => {
                let status: serde_json::Value = serde_json::from_slice(&status_bytes)
                    .unwrap_or_else(|_| serde_json::json!({"status": "unknown"}));

                let status_str = status.get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                match status_str {
                    "complete" => {
                        // Read result
                        let result_path = format!("/compute/jobs/{}/result", job_id);
                        let result = self.env.read(&result_path)
                            .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
                            .unwrap_or_default();

                        // Clear pending job
                        ctx.state.pending_job_id = None;

                        // Transition to next phase
                        self.handle_job_complete(ctx, &result);

                        Ok(tick_reschedule_millis(100))
                    }
                    "failed" => {
                        ctx.state.pending_job_id = None;
                        ctx.state.phase = AutopilotPhase::Failed;
                        ctx.state.error = Some(format!("Job {} failed", job_id));
                        self.write_hud_status("Failed", &format!("Job failed: {}", job_id));
                        Ok(tick_hibernate())
                    }
                    _ => {
                        // Still processing, reschedule
                        self.write_hud_status(&format!("{:?}", ctx.state.phase), &format!("Job {} status: {}", job_id, status_str));
                        Ok(tick_reschedule_millis(1000))
                    }
                }
            }
            Err(e) => {
                // Error reading status, retry
                tracing::warn!("Error reading job status: {}", e);
                Ok(tick_reschedule_millis(2000))
            }
        }
    }

    /// Handle job completion and transition phases.
    fn handle_job_complete(&self, ctx: &mut AgentContext<AutopilotState>, result: &str) {
        match ctx.state.phase {
            AutopilotPhase::Planning => {
                ctx.state.plan = Some(result.to_string());
                ctx.state.phase = AutopilotPhase::Executing;
                self.write_hud_status("Executing", "Plan created, starting execution");
            }
            AutopilotPhase::Executing => {
                ctx.state.changes.push(result.to_string());
                ctx.state.phase = AutopilotPhase::Reviewing;
                self.write_hud_status("Reviewing", "Execution complete, starting review");
            }
            AutopilotPhase::Reviewing => {
                ctx.state.phase = AutopilotPhase::Complete;
                self.write_hud_status("Complete", "All phases completed successfully");
            }
            _ => {}
        }
    }

    /// Submit a chat job to /compute mount.
    fn submit_chat_job(&self, prompt: &str, system: &str) -> std::result::Result<String, String> {
        let request = serde_json::json!({
            "model": self.config.model,
            "kind": "chat",
            "input": {
                "messages": [
                    {"role": "system", "content": system},
                    {"role": "user", "content": prompt}
                ]
            },
            "stream": false,
            "max_cost_usd": self.config.max_cost_per_tick_usd
        });

        let request_bytes = serde_json::to_vec(&request)
            .map_err(|e| format!("serialize error: {}", e))?;

        let response = self.env.call("/compute/new", &request_bytes)
            .map_err(|e| format!("submit error: {}", e))?;

        let job: serde_json::Value = serde_json::from_slice(&response)
            .map_err(|e| format!("parse error: {}", e))?;

        job.get("job_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "missing job_id".to_string())
    }

    /// Submit a PatchGen job.
    fn submit_patch_gen_job(&self, plan: &str) -> std::result::Result<String, String> {
        let request = serde_json::json!({
            "model": "claude-code",
            "kind": "patch_gen",
            "input": {
                "repo": self.repo_url,
                "ref": "main",
                "issue": format!("{}\n\nPlan:\n{}", self.issue_description, plan)
            },
            "max_cost_usd": self.config.max_cost_per_tick_usd * 2
        });

        let request_bytes = serde_json::to_vec(&request)
            .map_err(|e| format!("serialize error: {}", e))?;

        let response = self.env.call("/compute/new", &request_bytes)
            .map_err(|e| format!("submit error: {}", e))?;

        let job: serde_json::Value = serde_json::from_slice(&response)
            .map_err(|e| format!("parse error: {}", e))?;

        job.get("job_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "missing job_id".to_string())
    }

    /// Submit a CodeReview job.
    fn submit_code_review_job(&self, diff: &str) -> std::result::Result<String, String> {
        let request = serde_json::json!({
            "model": "claude-code",
            "kind": "code_review",
            "input": {
                "diff": diff,
                "focus": ["correctness", "security", "performance"]
            },
            "max_cost_usd": self.config.max_cost_per_tick_usd
        });

        let request_bytes = serde_json::to_vec(&request)
            .map_err(|e| format!("serialize error: {}", e))?;

        let response = self.env.call("/compute/new", &request_bytes)
            .map_err(|e| format!("submit error: {}", e))?;

        let job: serde_json::Value = serde_json::from_slice(&response)
            .map_err(|e| format!("parse error: {}", e))?;

        job.get("job_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "missing job_id".to_string())
    }

    /// Build planning prompt.
    fn build_planning_prompt(&self) -> String {
        format!(
            "Repository: {}\n\nIssue to solve:\n{}\n\nPlease analyze this issue and create a detailed implementation plan.",
            self.repo_url,
            self.issue_description
        )
    }

    /// Write status update to HUD.
    fn write_hud_status(&self, phase: &str, message: &str) {
        let status = serde_json::json!({
            "phase": phase,
            "message": message,
            "timestamp_ms": Timestamp::now().as_millis()
        });

        if let Ok(bytes) = serde_json::to_vec(&status) {
            let _ = self.env.write("/hud/autopilot_status", &bytes);
        }
    }
}

impl Agent for AutopilotAgent {
    type State = AutopilotState;
    type Config = AutopilotConfig;

    fn on_create(&self, ctx: &mut AgentContext<Self::State>) -> Result<()> {
        self.write_hud_status("Initializing", "Autopilot agent created");
        ctx.state.tick_count = 0;
        Ok(())
    }

    fn on_wake(&self, ctx: &mut AgentContext<Self::State>) -> Result<()> {
        self.write_hud_status(&format!("{:?}", ctx.state.phase), "Agent waking up");
        Ok(())
    }

    fn on_trigger(
        &self,
        ctx: &mut AgentContext<Self::State>,
        _trigger: Trigger,
    ) -> Result<TickResult> {
        ctx.state.tick_count += 1;

        match ctx.state.phase {
            AutopilotPhase::Planning => self.run_planning(ctx),
            AutopilotPhase::Executing => self.run_execution(ctx),
            AutopilotPhase::Reviewing => self.run_review(ctx),
            AutopilotPhase::Complete | AutopilotPhase::Failed => {
                // Terminal states - sleep forever
                Ok(tick_hibernate())
            }
        }
    }

    fn on_sleep(&self, ctx: &mut AgentContext<Self::State>) -> Result<()> {
        self.write_hud_status(&format!("{:?}", ctx.state.phase), "Agent going to sleep");
        Ok(())
    }
}

/// System prompt for planning phase.
const PLANNING_SYSTEM_PROMPT: &str = r#"You are an expert software architect analyzing issues and creating implementation plans.

Given a repository URL and issue description, create a detailed step-by-step implementation plan.
Include:
1. Analysis of the issue
2. Files that need to be modified
3. Specific changes to make
4. Test strategy
5. Potential risks and mitigations

Be concise but thorough."#;
