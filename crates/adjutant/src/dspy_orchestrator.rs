//! DSPy Orchestrator for Autopilot Planning Stages.
//!
//! Orchestrates the DSPy pipeline stages that run before tool execution:
//!
//! 1. **Environment Assessment** - Uses SituationAssessmentSignature
//! 2. **Planning** - Uses PlanningPipeline
//! 3. **Todo List Creation** - Converts plan steps to actionable tasks
//!
//! Each stage emits UI events through the AutopilotOutput trait so the
//! user can see progress in real-time.

use crate::autopilot_loop::{AutopilotOutput, DspyStage, TodoStatus, TodoTask};
use crate::{Task, TaskPlan, ToolRegistry};
use anyhow::Result;
use autopilot_core::{PlanningInput, PlanningPipeline, PlanningResult};
use dsrs::LM;
use dsrs::callbacks::DspyCallback;
use oanix::{OanixManifest, SituationInput, SituationPipeline};
use serde_json::json;
use std::sync::Arc;

fn has_action_verb(step: &str) -> bool {
    const VERBS: [&str; 18] = [
        "read",
        "write",
        "update",
        "add",
        "remove",
        "delete",
        "create",
        "edit",
        "modify",
        "verify",
        "check",
        "run",
        "summarize",
        "summarise",
        "provide",
        "respond",
        "list",
        "search",
    ];

    VERBS.iter().any(|verb| step.contains(verb))
}

fn normalize_step(step: &str) -> String {
    let trimmed = step.trim();
    let stripped = trimmed.trim_start_matches(|c: char| {
        c.is_ascii_digit() || c.is_whitespace() || matches!(c, '.' | ')' | '-' | '*' | ':' | '•')
    });
    stripped.to_lowercase()
}

fn is_summary_step(step: &str) -> bool {
    step.contains("summary")
        || step.contains("summarize")
        || step.contains("summarise")
        || step.contains("synthesize")
        || step.contains("one sentence")
        || step.contains("one-sentence")
}

fn is_non_actionable_step(step: &str) -> bool {
    let normalized = normalize_step(step);
    if normalized.is_empty() {
        return true;
    }

    if normalized.starts_with("summary:") {
        return true;
    }

    let always_skip_prefixes = [
        "no implementation",
        "no file modifications",
        "no code changes",
        "no changes required",
        "none required",
        "none",
        "not applicable",
    ];

    if always_skip_prefixes
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
    {
        return true;
    }

    let conditional_prefixes = ["no tests", "no testing"];
    if conditional_prefixes
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
    {
        return !has_action_verb(&normalized);
    }

    false
}

/// Format bytes as human-readable string.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

/// Result from environment assessment stage.
#[derive(Debug, Clone)]
pub struct AssessmentResult {
    /// What action the agent should prioritize
    pub priority_action: String,
    /// Urgency level
    pub urgency: String,
    /// Reasoning for the assessment
    pub reasoning: String,
}

/// DSPy orchestrator for running planning stages.
pub struct DspyOrchestrator {
    /// Optional LM instance for DSPy calls
    lm: Option<Arc<LM>>,
    /// Tool registry for finding relevant files
    tools: ToolRegistry,
}

impl DspyOrchestrator {
    /// Create a new orchestrator with optional LM.
    pub fn new(lm: Option<Arc<LM>>, tools: ToolRegistry) -> Self {
        Self { lm, tools }
    }

    /// Stage 1: Environment Assessment.
    ///
    /// Analyzes the current system state and determines what the agent
    /// should prioritize. Emits the assessment to the UI.
    pub async fn assess_environment<O: AutopilotOutput>(
        &self,
        manifest: &OanixManifest,
        _output: &O,
    ) -> Result<AssessmentResult> {
        // Extract info from manifest for display
        let system_info = format!(
            "{} cores, {} RAM",
            manifest.hardware.cpu_cores,
            format_bytes(manifest.hardware.ram_bytes)
        );

        let workspace = manifest
            .workspace
            .as_ref()
            .map(|w| w.root.display().to_string())
            .unwrap_or_else(|| ".".to_string());

        let active_directive = manifest
            .workspace
            .as_ref()
            .and_then(|w| w.active_directive.clone())
            .and_then(|id| {
                manifest
                    .workspace
                    .as_ref()
                    .and_then(|w| w.directives.iter().find(|d| d.id == id))
                    .map(|d| format!("{}: {}", d.id, d.title))
            });

        let open_issues = manifest
            .workspace
            .as_ref()
            .map(|w| w.issues.iter().filter(|i| i.status == "open").count())
            .unwrap_or(0);

        let compute_backends: Vec<String> = manifest
            .compute
            .backends
            .iter()
            .filter(|b| b.ready)
            .map(|b| b.name.clone())
            .collect();

        let system_state = json!({
            "hardware": {
                "cpu_cores": manifest.hardware.cpu_cores,
                "cpu_model": manifest.hardware.cpu_model.clone(),
                "ram_bytes": manifest.hardware.ram_bytes,
                "ram_available": manifest.hardware.ram_available,
                "gpus": manifest.hardware.gpus.iter().map(|gpu| json!({
                    "name": gpu.name.clone(),
                    "backend": gpu.backend.clone(),
                    "available": gpu.available,
                })).collect::<Vec<_>>(),
            },
            "compute": {
                "total_models": manifest.compute.total_models,
                "backends": manifest.compute.backends.iter().map(|backend| json!({
                    "id": backend.id.clone(),
                    "name": backend.name.clone(),
                    "endpoint": backend.endpoint.clone(),
                    "models": backend.models.clone(),
                    "ready": backend.ready,
                })).collect::<Vec<_>>(),
            },
            "network": {
                "has_internet": manifest.network.has_internet,
                "total_providers": manifest.network.total_providers,
                "pylon_count": manifest.network.pylon_count,
                "pylons_online": manifest.network.pylons_online,
                "pylon_pubkeys": manifest.network.pylon_pubkeys.clone(),
                "relays": manifest.network.relays.iter().map(|relay| json!({
                    "url": relay.url.clone(),
                    "connected": relay.connected,
                    "latency_ms": relay.latency_ms,
                })).collect::<Vec<_>>(),
            },
            "identity": {
                "initialized": manifest.identity.initialized,
                "npub": manifest.identity.npub.clone(),
                "wallet_balance_sats": manifest.identity.wallet_balance_sats,
                "network": manifest.identity.network.clone(),
            },
            "workspace": manifest.workspace.as_ref().map(|w| json!({
                "root": w.root.display().to_string(),
                "project_name": w.project_name.clone(),
                "has_openagents": w.has_openagents,
                "open_issues": w.open_issues,
                "pending_issues": w.pending_issues,
                "active_directive": w.active_directive.clone(),
                "directives": w.directives.clone(),
                "issues": w.issues.clone(),
            })),
        });

        let pending_events = {
            let mut events = Vec::new();
            if open_issues > 0 {
                events.push(json!({
                    "type": "open_issues",
                    "count": open_issues,
                }));
            }
            if let Some(active) = active_directive.as_ref() {
                events.push(json!({
                    "type": "active_directive",
                    "value": active,
                }));
            }
            serde_json::to_string(&events)?
        };

        let pipeline = match &self.lm {
            Some(lm) => SituationPipeline::with_lm(lm.clone()),
            None => SituationPipeline::new(),
        };

        let dspy_result = pipeline
            .assess(&SituationInput {
                system_state: serde_json::to_string(&system_state)?,
                pending_events,
                recent_history: "[]".to_string(),
            })
            .await;

        // DSPy assessment with heuristic fallback when no LM is configured.
        let (priority_action, urgency, reasoning) = match dspy_result {
            Ok(result) => (
                result.priority_action.to_string(),
                result.urgency.to_string(),
                result.reasoning,
            ),
            Err(_) => {
                let action = if open_issues > 0 {
                    "WORK_ISSUE"
                } else {
                    "AWAIT_USER"
                };
                let has_compute = !compute_backends.is_empty();
                let reasoning = if has_compute {
                    format!(
                        "Ready to work with {} backend(s). {} open issue(s).",
                        compute_backends.len(),
                        open_issues
                    )
                } else {
                    "No compute backends available. Awaiting user direction.".to_string()
                };
                (action.to_string(), "NORMAL".to_string(), reasoning)
            }
        };

        // Skip emitting EnvironmentAssessment card for now - it obscures other content
        let _ = (
            system_info,
            workspace,
            active_directive,
            open_issues,
            compute_backends,
        );

        Ok(AssessmentResult {
            priority_action,
            urgency,
            reasoning,
        })
    }

    /// Stage 2: Planning.
    ///
    /// Creates a structured implementation plan using the PlanningPipeline.
    /// Emits the plan to the UI.
    pub async fn create_plan<O: AutopilotOutput>(
        &self,
        task: &Task,
        plan: &TaskPlan,
        output: &O,
    ) -> Result<PlanningResult> {
        self.create_plan_with_callback(task, plan, output, None, None)
            .await
    }

    /// Stage 2: Planning with streaming callback.
    ///
    /// Creates a structured implementation plan using the PlanningPipeline.
    /// Emits the plan to the UI and streams tokens via callback.
    pub async fn create_plan_with_callback<O: AutopilotOutput>(
        &self,
        task: &Task,
        plan: &TaskPlan,
        output: &O,
        callback: Option<&dyn DspyCallback>,
        test_strategy_override: Option<String>,
    ) -> Result<PlanningResult> {
        // Build repository summary
        let repo_summary = self.get_repo_summary();

        // Get relevant files as string
        let relevant_files = plan
            .files
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        let input = PlanningInput {
            repository_summary: repo_summary,
            issue_description: task.description.clone(),
            relevant_files,
            code_patterns: None,
        };

        // Run planning pipeline
        let pipeline = if let Some(ref lm) = self.lm {
            PlanningPipeline::with_lm(lm.clone())
        } else {
            PlanningPipeline::new()
        };

        let mut result = pipeline.plan_with_callback(&input, callback).await?;
        if let Some(test_strategy) = test_strategy_override {
            result.test_strategy = test_strategy;
        }

        // Emit planning stage to UI
        output.emit_stage(DspyStage::Planning {
            analysis: result.analysis.clone(),
            files_to_modify: result.files_to_modify.clone(),
            implementation_steps: result.implementation_steps.clone(),
            test_strategy: result.test_strategy.clone(),
            complexity: format!("{:?}", result.complexity),
            confidence: result.confidence,
        });

        Ok(result)
    }

    /// Stage 3: Create Todo List.
    ///
    /// Converts the planning result's implementation steps into
    /// actionable todo items. Emits the todo list to the UI.
    pub fn create_todo_list<O: AutopilotOutput>(
        &self,
        plan: &PlanningResult,
        output: &O,
    ) -> Vec<TodoTask> {
        let mut filtered_steps: Vec<&String> = plan
            .implementation_steps
            .iter()
            .filter(|step| !is_non_actionable_step(step))
            .collect();

        let mut summary_seen = false;
        filtered_steps.retain(|step| {
            let normalized = normalize_step(step);
            if is_summary_step(&normalized) {
                if summary_seen {
                    return false;
                }
                summary_seen = true;
            }
            true
        });

        let tasks: Vec<TodoTask> = filtered_steps
            .into_iter()
            .enumerate()
            .map(|(i, step)| TodoTask {
                index: i + 1,
                description: step.clone(),
                status: TodoStatus::Pending,
            })
            .collect();

        // Emit todo list to UI
        if !tasks.is_empty() {
            output.emit_stage(DspyStage::TodoList {
                tasks: tasks.clone(),
            });
        }

        tasks
    }

    /// Get a summary of the repository from workspace info.
    fn get_repo_summary(&self) -> String {
        format!("Repository at {}", self.tools.workspace_root().display())
    }

    /// Find relevant files for a task using the tool registry.
    pub async fn find_relevant_files(&self, task: &Task) -> Result<Vec<String>> {
        // Extract keywords from task description
        let keywords: Vec<&str> = task
            .description
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .take(5)
            .collect();

        let mut files = Vec::new();

        // Search for each keyword
        for keyword in keywords {
            if let Ok(result) = self.tools.grep(keyword, None).await {
                if result.success {
                    // Extract file paths from grep output
                    for line in result.content.lines().take(10) {
                        if let Some(path) = line.split(':').next() {
                            if !files.contains(&path.to_string()) {
                                files.push(path.to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_task_creation() {
        let steps = vec![
            "Read the file".to_string(),
            "Make changes".to_string(),
            "Run tests".to_string(),
        ];

        let planning_result = PlanningResult {
            analysis: "Test analysis".to_string(),
            files_to_modify: vec!["src/main.rs".to_string()],
            implementation_steps: steps,
            test_strategy: "Run cargo test".to_string(),
            risk_factors: vec![],
            complexity: autopilot_core::Complexity::Low,
            confidence: 0.9,
        };

        // Create a mock output that does nothing
        struct MockOutput;
        impl AutopilotOutput for MockOutput {
            fn iteration_start(&self, _: usize, _: usize) {}
            fn token(&self, _: &str) {}
            fn verification_start(&self) {}
            fn verification_result(&self, _: bool, _: &str) {}
            fn error(&self, _: &str) {}
            fn interrupted(&self) {}
            fn max_iterations(&self, _: usize) {}
            fn emit_stage(&self, _: DspyStage) {}
        }

        let tools = ToolRegistry::new(std::path::PathBuf::from("."));
        let orchestrator = DspyOrchestrator::new(None, tools);
        let tasks = orchestrator.create_todo_list(&planning_result, &MockOutput);

        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].index, 1);
        assert_eq!(tasks[0].description, "Read the file");
        assert_eq!(tasks[0].status, TodoStatus::Pending);
    }

    #[test]
    fn filters_non_actionable_steps() {
        let steps = vec![
            "Read README.md".to_string(),
            "Summary: OpenAgents does X".to_string(),
            "No tests required".to_string(),
            "3. No file modifications needed".to_string(),
            "Verify output matches expectation".to_string(),
        ];

        let planning_result = PlanningResult {
            analysis: "Test analysis".to_string(),
            files_to_modify: vec![],
            implementation_steps: steps,
            test_strategy: "Run cargo test".to_string(),
            risk_factors: vec![],
            complexity: autopilot_core::Complexity::Low,
            confidence: 0.9,
        };

        struct MockOutput;
        impl AutopilotOutput for MockOutput {
            fn iteration_start(&self, _: usize, _: usize) {}
            fn token(&self, _: &str) {}
            fn verification_start(&self) {}
            fn verification_result(&self, _: bool, _: &str) {}
            fn error(&self, _: &str) {}
            fn interrupted(&self) {}
            fn max_iterations(&self, _: usize) {}
            fn emit_stage(&self, _: DspyStage) {}
        }

        let tools = ToolRegistry::new(std::path::PathBuf::from("."));
        let orchestrator = DspyOrchestrator::new(None, tools);
        let tasks = orchestrator.create_todo_list(&planning_result, &MockOutput);

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].description, "Read README.md");
        assert_eq!(tasks[1].description, "Verify output matches expectation");
    }

    #[test]
    fn dedupes_summary_steps() {
        let steps = vec![
            "Read README.md".to_string(),
            "Synthesize the key information into one sentence".to_string(),
            "Report the summary".to_string(),
        ];

        let planning_result = PlanningResult {
            analysis: "Test analysis".to_string(),
            files_to_modify: vec![],
            implementation_steps: steps,
            test_strategy: "Run cargo test".to_string(),
            risk_factors: vec![],
            complexity: autopilot_core::Complexity::Low,
            confidence: 0.9,
        };

        struct MockOutput;
        impl AutopilotOutput for MockOutput {
            fn iteration_start(&self, _: usize, _: usize) {}
            fn token(&self, _: &str) {}
            fn verification_start(&self) {}
            fn verification_result(&self, _: bool, _: &str) {}
            fn error(&self, _: &str) {}
            fn interrupted(&self) {}
            fn max_iterations(&self, _: usize) {}
            fn emit_stage(&self, _: DspyStage) {}
        }

        let tools = ToolRegistry::new(std::path::PathBuf::from("."));
        let orchestrator = DspyOrchestrator::new(None, tools);
        let tasks = orchestrator.create_todo_list(&planning_result, &MockOutput);

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].description, "Read README.md");
        assert_eq!(
            tasks[1].description,
            "Synthesize the key information into one sentence"
        );
    }

    #[test]
    fn helper_functions_normalize_and_filter_steps() {
        assert_eq!(normalize_step("1. Update README"), "update readme");
        assert!(has_action_verb("read file"));
        assert!(!has_action_verb("nothing to do"));
        assert!(is_summary_step("summary of changes"));
        assert!(is_summary_step("please summarize results"));
        assert!(is_non_actionable_step("No tests required"));
        assert!(is_non_actionable_step("None"));
        assert!(!is_non_actionable_step("Run tests"));
    }

    #[test]
    fn formats_bytes_for_display() {
        assert_eq!(format_bytes(0), "0 bytes");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    }
}
