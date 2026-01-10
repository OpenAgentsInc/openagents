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
use autopilot::{PlanningInput, PlanningPipeline, PlanningResult};
use dsrs::LM;
use oanix::OanixManifest;
use std::sync::Arc;

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
        output: &O,
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

        // Default assessment (DSPy signature is optional enhancement)
        let (priority_action, urgency, reasoning) = {
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
        };

        // Emit stage to UI
        output.emit_stage(DspyStage::EnvironmentAssessment {
            system_info,
            workspace,
            active_directive,
            open_issues,
            compute_backends,
            priority_action: priority_action.clone(),
            urgency: urgency.clone(),
            reasoning: reasoning.clone(),
        });

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

        let result = pipeline.plan(&input).await?;

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
        let tasks: Vec<TodoTask> = plan
            .implementation_steps
            .iter()
            .enumerate()
            .map(|(i, step)| TodoTask {
                index: i + 1,
                description: step.clone(),
                status: TodoStatus::Pending,
            })
            .collect();

        // Emit todo list to UI
        output.emit_stage(DspyStage::TodoList {
            tasks: tasks.clone(),
        });

        tasks
    }

    /// Get a summary of the repository from workspace info.
    fn get_repo_summary(&self) -> String {
        format!(
            "Repository at {}",
            self.tools.workspace_root().display()
        )
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
            complexity: autopilot::Complexity::Low,
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
}
