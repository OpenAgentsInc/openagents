//! Run command - start the autopilot loop

use crate::autopilot_loop::{
    generate_session_id, AcpChannelOutput, AutopilotConfig, AutopilotLoop, AutopilotResult,
};
use crate::cli::blocker::{analyze_blockers, print_blocker_summary};
use crate::cli::boot::{boot_fast, boot_full, print_quick_checks};
use crate::cli::directive::build_directive_task;
use crate::cli::stream::CliAcpRenderer;
use crate::{Adjutant, ExecutionBackend, Task};
use agent_client_protocol_schema as acp;
use clap::{Args, ValueEnum};
use oanix::{OanixManifest, WorkspaceManifest};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Run command arguments
#[derive(Args)]
pub struct RunArgs {
    /// Specific issue number to work on
    #[arg(short, long)]
    pub issue: Option<u32>,

    /// Run continuously, claiming and completing issues
    #[arg(short, long)]
    pub loop_mode: bool,

    /// Ad-hoc task description (instead of issue)
    pub task: Option<String>,

    /// Run full environment discovery (slower)
    #[arg(long)]
    pub full_boot: bool,

    /// Execution backend (auto, claude, codex, local-llm, local-tools)
    #[arg(long, value_enum, default_value_t = BackendChoice::Auto)]
    pub backend: BackendChoice,

    /// Maximum iterations for the autopilot loop
    #[arg(long, default_value_t = 10)]
    pub max_iterations: usize,

    /// Skip verification after completion
    #[arg(long)]
    pub no_verify: bool,
}

/// Supported execution backends for the CLI.
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum BackendChoice {
    Auto,
    Claude,
    Codex,
    #[value(name = "local-llm", alias = "llama", alias = "gptoss")]
    LocalLlm,
    #[value(name = "local-tools", alias = "tools")]
    LocalTools,
}

impl BackendChoice {
    fn from_env() -> Option<Self> {
        let value = std::env::var("AUTOPILOT_BACKEND").ok()?;
        let value = value.trim().to_lowercase();
        match value.as_str() {
            "auto" => Some(BackendChoice::Auto),
            "claude" => Some(BackendChoice::Claude),
            "codex" => Some(BackendChoice::Codex),
            "local-llm" | "llama" | "gptoss" => Some(BackendChoice::LocalLlm),
            "local-tools" | "tools" => Some(BackendChoice::LocalTools),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            BackendChoice::Auto => "auto",
            BackendChoice::Claude => "claude",
            BackendChoice::Codex => "codex",
            BackendChoice::LocalLlm => "local-llm",
            BackendChoice::LocalTools => "local-tools",
        }
    }
}

impl From<BackendChoice> for ExecutionBackend {
    fn from(value: BackendChoice) -> Self {
        match value {
            BackendChoice::Auto => ExecutionBackend::Auto,
            BackendChoice::Claude => ExecutionBackend::Claude,
            BackendChoice::Codex => ExecutionBackend::Codex,
            BackendChoice::LocalLlm => ExecutionBackend::LocalLlm,
            BackendChoice::LocalTools => ExecutionBackend::LocalTools,
        }
    }
}

/// Run the autopilot loop
pub async fn run(args: RunArgs) -> anyhow::Result<()> {
    println!("OANIX v0.1.0 - OpenAgents NIX");
    println!("{}", "=".repeat(55));
    println!();
    println!("Booting...");
    if !args.full_boot {
        println!("Fast boot: skipping network/compute discovery.");
    }
    print_quick_checks();

    // Boot OANIX
    let manifest = if args.full_boot {
        boot_full().await?
    } else {
        boot_fast().await?
    };

    let backend_choice = resolve_backend_choice(&args);
    println!("Backend: {}", backend_choice.label());
    let backend: ExecutionBackend = backend_choice.into();

    // Set up interrupt handling
    let interrupt_flag = Arc::new(AtomicBool::new(false));
    let int_flag = interrupt_flag.clone();
    ctrlc::set_handler(move || {
        int_flag.store(true, Ordering::Relaxed);
    })?;

    // Determine what to work on
    if let Some(task_desc) = args.task.as_deref() {
        // Ad-hoc task with full autopilot loop
        let task = Task::new("adhoc", "Ad-hoc Task", task_desc);
        println!();
        println!("Working on ad-hoc task: {}", task.title);
        println!("Press Ctrl+C to interrupt");
        println!();

        let adjutant = build_adjutant(&manifest, backend)?;
        let config = build_autopilot_config(&manifest, &args);
        let result = run_autopilot_task(adjutant, task, config, interrupt_flag.clone()).await;
        print_autopilot_result(&result);
        return Ok(());
    }

    // Working on issues
    let workspace = manifest
        .workspace
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No .openagents/ folder found"))?;

    if args.loop_mode {
        // Continuous loop mode
        println!();
        println!("Starting autopilot loop...");
        println!("Press Ctrl+C to stop");
        println!();

        loop {
            if interrupt_flag.load(Ordering::Relaxed) {
                println!("Interrupted by user.");
                break;
            }

            // Find next available issue
            let next_issue = workspace
                .issues
                .iter()
                .filter(|i| i.status == "open" && !i.is_blocked)
                .min_by_key(|i| match i.priority.as_str() {
                    "urgent" => 0,
                    "high" => 1,
                    "medium" => 2,
                    "low" => 3,
                    _ => 4,
                });

            match next_issue {
                Some(issue) => {
                    println!("Claiming issue #{}: {}", issue.number, issue.title);

                    // Create task from issue summary
                    let task = Task::new(
                        format!("#{}", issue.number),
                        &issue.title,
                        format!("Issue #{}: {}", issue.number, issue.title),
                    );

                    let adjutant = build_adjutant(&manifest, backend)?;
                    let config = build_autopilot_config(&manifest, &args);
                    let result =
                        run_autopilot_task(adjutant, task, config, interrupt_flag.clone()).await;
                    print_autopilot_result(&result);

                    if !matches!(result, AutopilotResult::Success(_)) {
                        println!("Task failed, waiting before retry...");
                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    }
                }
                None => {
                    println!("No actionable issues. Waiting...");
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;

                    // Re-boot to refresh issue list
                    // In production, we'd have a more efficient way to check
                    break; // For now, exit rather than infinite loop
                }
            }
        }
    } else {
        // Single issue mode
        let issue = if let Some(number) = args.issue {
            // Specific issue requested
            Some(
                workspace
                    .issues
                    .iter()
                    .find(|i| i.number == number)
                    .ok_or_else(|| anyhow::anyhow!("Issue #{} not found", number))?,
            )
        } else {
            // Find next available issue
            workspace
                .issues
                .iter()
                .filter(|i| i.status == "open" && !i.is_blocked)
                .min_by_key(|i| match i.priority.as_str() {
                    "urgent" => 0,
                    "high" => 1,
                    "medium" => 2,
                    "low" => 3,
                    _ => 4,
                })
        };

        match issue {
            Some(issue) => {
                println!();
                println!("Working on issue #{}: {}", issue.number, issue.title);
                println!();

                // Create task from issue
                let task = Task::new(
                    format!("#{}", issue.number),
                    &issue.title,
                    format!("Issue #{}: {}", issue.number, issue.title),
                );

                let adjutant = build_adjutant(&manifest, backend)?;
                let config = build_autopilot_config(&manifest, &args);
                let result =
                    run_autopilot_task(adjutant, task, config, interrupt_flag.clone()).await;
                print_autopilot_result(&result);
            }
            None => {
                // No actionable issues - use smart fallback
                let config = build_autopilot_config(&manifest, &args);
                find_work_when_blocked(
                    workspace,
                    &manifest,
                    backend,
                    config,
                    interrupt_flag.clone(),
                )
                .await?;
            }
        }
    }

    Ok(())
}

fn resolve_backend_choice(args: &RunArgs) -> BackendChoice {
    if args.backend == BackendChoice::Auto {
        BackendChoice::from_env().unwrap_or(args.backend)
    } else {
        args.backend
    }
}

fn build_autopilot_config(manifest: &OanixManifest, args: &RunArgs) -> AutopilotConfig {
    let workspace_root = manifest
        .workspace
        .as_ref()
        .map(|w| w.root.clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    AutopilotConfig {
        max_iterations: args.max_iterations,
        workspace_root,
        verify_completion: !args.no_verify,
    }
}

fn build_adjutant(manifest: &OanixManifest, backend: ExecutionBackend) -> anyhow::Result<Adjutant> {
    let mut adjutant = Adjutant::new(manifest.clone())?;
    adjutant.set_execution_backend(backend);
    Ok(adjutant)
}

async fn run_autopilot_task(
    adjutant: Adjutant,
    task: Task,
    config: AutopilotConfig,
    interrupt_flag: Arc<AtomicBool>,
) -> AutopilotResult {
    let (acp_tx, mut acp_rx) = mpsc::unbounded_channel::<acp::SessionNotification>();
    let session_id = generate_session_id();
    let output = AcpChannelOutput::new(session_id, acp_tx);

    let loop_runner = AutopilotLoop::new(adjutant, task, config, output, interrupt_flag);
    let mut renderer = CliAcpRenderer::new(std::io::stdout());

    let mut loop_fut = Box::pin(loop_runner.run());
    let result = loop {
        tokio::select! {
            res = &mut loop_fut => {
                break res;
            }
            maybe = acp_rx.recv() => {
                if let Some(notification) = maybe {
                    renderer.handle_notification(notification);
                }
            }
        }
    };

    while let Ok(notification) = acp_rx.try_recv() {
        renderer.handle_notification(notification);
    }
    renderer.finish();

    result
}

fn print_autopilot_result(result: &AutopilotResult) {
    println!();
    println!("{}", "=".repeat(55));

    match result {
        AutopilotResult::Success(task_result) => {
            println!("Task completed successfully");
            println!();
            println!("Summary: {}", task_result.summary);

            if !task_result.modified_files.is_empty() {
                println!();
                println!("Modified files:");
                for file in &task_result.modified_files {
                    println!("  - {}", file);
                }
            }

            if let Some(hash) = &task_result.commit_hash {
                println!();
                println!("Commit: {}", hash);
            }
        }
        AutopilotResult::Failed(task_result) => {
            println!("Task failed definitively");
            println!();
            println!("Summary: {}", task_result.summary);
            if let Some(error) = &task_result.error {
                println!("Error: {}", error);
            }
        }
        AutopilotResult::MaxIterationsReached { iterations, last_result } => {
            println!("Max iterations ({}) reached without success", iterations);
            if let Some(result) = last_result {
                println!();
                println!("Last result: {}", result.summary);
            }
        }
        AutopilotResult::UserInterrupted { iterations } => {
            println!("Interrupted by user after {} iterations", iterations);
        }
        AutopilotResult::Error(msg) => {
            println!("Error during execution: {}", msg);
        }
    }
}

/// Find work when all issues are blocked.
///
/// This function analyzes WHY issues are blocked and either:
/// 1. Suggests unblocking work (e.g., implementing a missing crate)
/// 2. Falls back to working on the active directive
async fn find_work_when_blocked(
    workspace: &WorkspaceManifest,
    manifest: &OanixManifest,
    backend: ExecutionBackend,
    config: AutopilotConfig,
    interrupt_flag: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    println!();
    println!("No actionable issues. Analyzing blockers...");
    println!();

    // Collect blocked issues
    let blocked: Vec<_> = workspace
        .issues
        .iter()
        .filter(|i| i.is_blocked)
        .collect();

    if blocked.is_empty() {
        println!("No blocked issues found. All issues may be completed or in progress.");
        return Ok(());
    }

    // Print blocked issue summary
    print_blocker_summary(&blocked);
    println!();

    // Analyze blockers
    let analysis = analyze_blockers(&blocked);

    println!("Blocker analysis:");
    if analysis.needs_code > 0 {
        println!("  {} need code implemented first", analysis.needs_code);
    }
    if analysis.needs_infra > 0 {
        println!("  {} need infrastructure/setup", analysis.needs_infra);
    }
    if analysis.token_budget > 0 {
        println!("  {} are token budget issues", analysis.token_budget);
    }
    if analysis.needs_env > 0 {
        println!("  {} need special environment (GUI, etc.)", analysis.needs_env);
    }
    if analysis.architectural > 0 {
        println!("  {} have architectural concerns", analysis.architectural);
    }
    if analysis.dependencies > 0 {
        println!("  {} are waiting on dependencies", analysis.dependencies);
    }

    // Show suggested work if we have one
    if let Some(suggestion) = &analysis.suggested_work {
        println!();
        println!("Suggested unblocking work: {}", suggestion);

        // Create an ad-hoc task for the unblocking work
        let task = Task::new(
            "unblock",
            "Unblocking Work",
            format!(
                "Autopilot has determined that the following work would unblock multiple issues:\n\n\
                 {}\n\n\
                 Please analyze what's needed and implement it.",
                suggestion
            ),
        );

        println!();
        println!("Starting unblocking work...");
        println!();

        let adjutant = build_adjutant(manifest, backend)?;
        let result = run_autopilot_task(adjutant, task, config.clone(), interrupt_flag.clone()).await;
        print_autopilot_result(&result);
        return Ok(());
    }

    // Fall back to directive-based work
    println!();
    println!("No clear unblocking path. Checking active directive...");

    if let Some(directive_id) = &workspace.active_directive {
        let task = build_directive_task(workspace, directive_id)?;
        let adjutant = build_adjutant(manifest, backend)?;
        let result = run_autopilot_task(adjutant, task, config, interrupt_flag.clone()).await;
        print_autopilot_result(&result);
    } else {
        println!();
        println!("No active directive set.");
        println!();
        println!("To proceed, either:");
        println!("  1. Unblock an issue manually");
        println!("  2. Set an active directive in .openagents/");
        println!("  3. Run with an ad-hoc task: autopilot run \"your task here\"");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_backend_choices() {
        assert_eq!(ExecutionBackend::from(BackendChoice::Claude), ExecutionBackend::Claude);
        assert_eq!(ExecutionBackend::from(BackendChoice::Codex), ExecutionBackend::Codex);
        assert_eq!(ExecutionBackend::from(BackendChoice::LocalLlm), ExecutionBackend::LocalLlm);
        assert_eq!(ExecutionBackend::from(BackendChoice::LocalTools), ExecutionBackend::LocalTools);
    }
}
