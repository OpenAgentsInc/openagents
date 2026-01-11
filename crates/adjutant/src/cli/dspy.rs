//! DSPy training and optimization CLI commands.
//!
//! Commands for viewing training data status, running optimization, and exporting data.

use anyhow::Result;
use clap::{Args, Subcommand};
use dsrs::{manifest::CompiledModuleManifest, manifest::Scorecard, MIPROv2, Optimizable, Optimizer};
use std::path::PathBuf;

use crate::dspy::{get_planning_lm, AdjutantModule, AdjutantTrainingDataset};

/// DSPy subcommands
#[derive(Subcommand)]
pub enum DspyCommand {
    /// Show training data statistics
    Status(StatusArgs),
    /// Run optimization on collected training data
    Optimize(OptimizeArgs),
    /// Export training data for sharing
    Export(ExportArgs),
    /// Show session history and statistics
    Sessions(SessionsArgs),
    /// Show performance metrics
    Performance(PerformanceArgs),
    /// Configure auto-optimization
    AutoOptimize(AutoOptimizeArgs),
}

/// Arguments for the status command
#[derive(Args)]
pub struct StatusArgs {}

/// Arguments for the optimize command
#[derive(Args)]
pub struct OptimizeArgs {
    /// Optimizer to use: mipro (default)
    #[arg(short, long, default_value = "mipro")]
    pub optimizer: String,

    /// Which signature to optimize: planning, execution, synthesis, complexity, delegation, rlm
    /// If not specified, optimizes planning (most common)
    #[arg(short, long)]
    pub signature: Option<String>,

    /// Minimum examples required to run optimization
    #[arg(long, default_value = "10")]
    pub min_examples: usize,

    /// Number of candidate prompts to generate
    #[arg(long, default_value = "5")]
    pub num_candidates: usize,

    /// Number of optimization trials
    #[arg(long, default_value = "10")]
    pub num_trials: usize,

    /// Dry run - show what would be optimized without running
    #[arg(long)]
    pub dry_run: bool,
}

/// Arguments for the export command
#[derive(Args)]
pub struct ExportArgs {
    /// Output file path
    #[arg(short, long, default_value = "training_data.json")]
    pub output: String,
}

/// Arguments for the sessions command
#[derive(Args)]
pub struct SessionsArgs {
    /// Number of recent sessions to show
    #[arg(short, long, default_value = "10")]
    pub limit: usize,

    /// Show only failed sessions
    #[arg(long)]
    pub failed_only: bool,

    /// Show detailed decision history
    #[arg(short, long)]
    pub verbose: bool,
}

/// Arguments for the performance command
#[derive(Args)]
pub struct PerformanceArgs {
    /// Show full history
    #[arg(long)]
    pub history: bool,
}

/// Arguments for the auto-optimize command
#[derive(Args)]
pub struct AutoOptimizeArgs {
    /// Enable auto-optimization
    #[arg(long)]
    pub enable: bool,

    /// Disable auto-optimization
    #[arg(long)]
    pub disable: bool,

    /// Set minimum examples threshold
    #[arg(long)]
    pub min_examples: Option<usize>,

    /// Set accuracy threshold (0.0 - 1.0)
    #[arg(long)]
    pub accuracy_threshold: Option<f32>,

    /// Set minimum hours between optimizations
    #[arg(long)]
    pub min_hours: Option<u64>,
}

/// Show training data statistics.
pub async fn status(_args: StatusArgs) -> Result<()> {
    let dataset = AdjutantTrainingDataset::load()?;

    println!("DSPy Training Data Status");
    println!("=========================\n");

    let training_path = crate::dspy::training::training_data_path();
    println!("Location: {}\n", training_path.display());

    println!("Task Execution Signatures:");
    println!(
        "  Planning examples:   {:>4}",
        dataset.planning_examples.len()
    );
    println!(
        "  Execution examples:  {:>4}",
        dataset.execution_examples.len()
    );
    println!(
        "  Synthesis examples:  {:>4}",
        dataset.synthesis_examples.len()
    );

    println!("\nDecision Pipeline Signatures:");
    println!(
        "  Complexity examples: {:>4}",
        dataset.complexity_examples.len()
    );
    println!(
        "  Delegation examples: {:>4}",
        dataset.delegation_examples.len()
    );
    println!(
        "  RLM trigger examples:{:>4}",
        dataset.rlm_trigger_examples.len()
    );

    println!(
        "\nTotal examples: {}\n",
        dataset.len()
    );

    // Show which are ready for optimization (>= 10 examples)
    println!("Ready for Optimization (≥10 examples):");
    let min_examples = 10;

    let ready_signatures = [
        ("planning", dataset.planning_examples.len()),
        ("execution", dataset.execution_examples.len()),
        ("synthesis", dataset.synthesis_examples.len()),
        ("complexity", dataset.complexity_examples.len()),
        ("delegation", dataset.delegation_examples.len()),
        ("rlm", dataset.rlm_trigger_examples.len()),
    ];

    let mut any_ready = false;
    for (name, count) in &ready_signatures {
        if *count >= min_examples {
            println!("  ✓ {} ({} examples)", name, count);
            any_ready = true;
        }
    }

    if !any_ready {
        println!("  (none ready yet - need at least {} examples)", min_examples);
        println!("\n  Tip: Use the Autopilot UI normally to collect training data.");
        println!("  High-confidence decisions (>70%) are automatically recorded.");
    } else {
        println!("\n  Run: autopilot dspy optimize --signature <name>");
    }

    Ok(())
}

/// Run optimization on collected training data.
pub async fn optimize(args: OptimizeArgs) -> Result<()> {
    // 1. Load training data
    let dataset = AdjutantTrainingDataset::load()?;

    // 2. Select signature and get examples
    let signature_name = args.signature.as_deref().unwrap_or("planning");
    let (full_signature_name, examples) = match signature_name {
        "planning" => (
            "SubtaskPlanningSignature",
            dataset.planning_as_examples(),
        ),
        "execution" => (
            "SubtaskExecutionSignature",
            dataset.execution_as_examples(),
        ),
        "synthesis" => (
            "ResultSynthesisSignature",
            dataset.synthesis_as_examples(),
        ),
        "complexity" => (
            "ComplexityClassificationSignature",
            dataset.complexity_as_examples(),
        ),
        "delegation" => (
            "DelegationDecisionSignature",
            dataset.delegation_as_examples(),
        ),
        "rlm" => ("RlmTriggerSignature", dataset.rlm_trigger_as_examples()),
        other => anyhow::bail!(
            "Unknown signature: {}. Use: planning, execution, synthesis, complexity, delegation, rlm",
            other
        ),
    };

    println!("DSPy Optimization");
    println!("=================\n");
    println!("Signature:  {}", full_signature_name);
    println!("Examples:   {}", examples.len());
    println!("Optimizer:  {}", args.optimizer);
    println!("Candidates: {}", args.num_candidates);
    println!("Trials:     {}", args.num_trials);
    println!();

    // 3. Check minimum examples
    if examples.len() < args.min_examples {
        anyhow::bail!(
            "Need at least {} examples, have {}.\n\
             Collect more training data by using the Autopilot UI normally.\n\
             High-confidence decisions are automatically recorded.",
            args.min_examples,
            examples.len()
        );
    }

    if args.dry_run {
        println!("Dry run - would optimize {} with {} examples", full_signature_name, examples.len());
        println!("\nRemove --dry-run to run actual optimization.");
        return Ok(());
    }

    // 4. Check LM availability
    println!("Checking LM availability...");
    let _lm = match get_planning_lm().await {
        Ok(lm) => {
            println!("✓ LM available\n");
            lm
        }
        Err(e) => {
            anyhow::bail!(
                "No LM available for optimization: {}\n\
                 \n\
                 Optimization requires an LM. Options:\n\
                 - Start llama-server on :8080\n\
                 - Install Codex CLI (codex)\n\
                 - Set CEREBRAS_API_KEY\n\
                 - Start Ollama on :11434",
                e
            );
        }
    };

    // 5. Create module and optimizer
    let mut module = AdjutantModule::new();

    let optimizer = match args.optimizer.as_str() {
        "mipro" => MIPROv2::builder()
            .num_candidates(args.num_candidates)
            .num_trials(args.num_trials)
            .minibatch_size(std::cmp::min(25, examples.len()))
            .build(),
        other => anyhow::bail!(
            "Unknown optimizer: {}. Currently supported: mipro",
            other
        ),
    };

    // 6. Run optimization
    println!("Starting optimization (this may take a while)...\n");
    optimizer.compile(&mut module, examples).await?;

    // 7. Extract optimized instruction
    // After optimization, the module's predictors have updated signatures
    let instruction = {
        // Get instruction from planner predictor
        let planner_sig = module.planner.get_signature();
        planner_sig.instruction().to_string()
    };

    // 8. Create manifest
    let manifest = CompiledModuleManifest::new(full_signature_name, &args.optimizer)
        .with_instruction(&instruction)
        .with_scorecard(Scorecard::new(0.0).with_rollouts(args.num_trials))
        .finalize()?;

    // 9. Save manifest
    let manifest_dir = dirs::home_dir()
        .expect("No home directory")
        .join(".openagents/adjutant/manifests");
    std::fs::create_dir_all(&manifest_dir)?;

    let manifest_path = manifest_dir.join(format!(
        "{}.json",
        manifest.compiled_id.as_ref().unwrap()
    ));
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    println!("\n");
    println!("Optimization Complete!");
    println!("======================\n");
    println!("Compiled ID: {}", manifest.compiled_id.as_ref().unwrap());
    println!("Manifest:    {}", manifest_path.display());
    println!("\nOptimized instruction:");
    println!("  {}", instruction);

    Ok(())
}

/// Export training data for sharing.
pub async fn export(args: ExportArgs) -> Result<()> {
    let dataset = AdjutantTrainingDataset::load()?;

    if dataset.is_empty() {
        println!("No training data to export.");
        println!("\nCollect data by using the Autopilot UI normally.");
        return Ok(());
    }

    let output_path = PathBuf::from(&args.output);
    let contents = serde_json::to_string_pretty(&dataset)?;
    std::fs::write(&output_path, contents)?;

    println!("Exported training data to: {}", output_path.display());
    println!("\nExported {} total examples:", dataset.len());
    println!("  Planning:   {}", dataset.planning_examples.len());
    println!("  Execution:  {}", dataset.execution_examples.len());
    println!("  Synthesis:  {}", dataset.synthesis_examples.len());
    println!("  Complexity: {}", dataset.complexity_examples.len());
    println!("  Delegation: {}", dataset.delegation_examples.len());
    println!("  RLM:        {}", dataset.rlm_trigger_examples.len());

    Ok(())
}

/// Show session history and statistics.
pub async fn sessions(args: SessionsArgs) -> Result<()> {
    use crate::dspy::{SessionOutcome, SessionStore};

    let store = SessionStore::open()?;
    let sessions = store.get_recent_sessions(args.limit)?;

    println!("Recent Autopilot Sessions");
    println!("=========================\n");

    if sessions.is_empty() {
        println!("No sessions recorded yet.");
        println!("\nRun: autopilot run \"your task here\"");
        return Ok(());
    }

    // Filter if needed
    let sessions: Vec<_> = if args.failed_only {
        sessions.into_iter().filter(|s| {
            s.outcome.as_ref().map(|o| o.is_failure()).unwrap_or(false)
        }).collect()
    } else {
        sessions
    };

    println!("{:<12} {:<30} {:<12} {:<10} {:<10}",
        "ID", "Task", "Outcome", "Decisions", "Iterations");
    println!("{}", "-".repeat(80));

    for session in &sessions {
        let id_short = session.id.chars().take(8).collect::<String>();
        let title_short: String = session.task_title.chars().take(28).collect();
        let outcome = match &session.outcome {
            Some(SessionOutcome::Success { .. }) => "SUCCESS",
            Some(SessionOutcome::Failed { .. }) => "FAILED",
            Some(SessionOutcome::MaxIterationsReached { .. }) => "MAX_ITER",
            Some(SessionOutcome::UserInterrupted) => "INTERRUPT",
            Some(SessionOutcome::Error(_)) => "ERROR",
            None => "PENDING",
        };

        println!("{:<12} {:<30} {:<12} {:<10} {:<10}",
            id_short,
            title_short,
            outcome,
            session.decisions.len(),
            session.iterations_used
        );

        if args.verbose {
            for decision in &session.decisions {
                let correct = match decision.was_correct {
                    Some(true) => "✓",
                    Some(false) => "✗",
                    None => "?",
                };
                println!("    {} {} (conf: {:.0}%)",
                    correct,
                    decision.decision_type,
                    decision.predicted_confidence * 100.0
                );
            }
            println!();
        }
    }

    let index = store.index();
    println!("\nSummary:");
    println!("  Total sessions: {}", index.total_sessions);
    println!("  Success rate:   {:.1}%", index.success_rate() * 100.0);

    Ok(())
}

/// Show performance metrics.
pub async fn performance(args: PerformanceArgs) -> Result<()> {
    use crate::dspy::{LabeledExamplesStore, PerformanceTracker};

    let tracker = PerformanceTracker::open()?;
    let summary = tracker.summary();

    println!("Performance Metrics");
    println!("===================\n");

    println!("Decision Accuracy (Rolling Window):");
    println!("  Complexity:   {:>5.1}%", summary.complexity_accuracy * 100.0);
    println!("  Delegation:   {:>5.1}%", summary.delegation_accuracy * 100.0);
    println!("  RLM Trigger:  {:>5.1}%", summary.rlm_accuracy * 100.0);
    println!();
    println!("  Overall:      {:>5.1}%", summary.overall_accuracy * 100.0);
    println!("  Total decisions: {}", summary.total_decisions);

    // Show labeled examples count
    if let Ok(labeled) = LabeledExamplesStore::load() {
        println!("\nLabeled Training Examples:");
        println!("  Complexity:   {:>4}", labeled.count_by_type("complexity"));
        println!("  Delegation:   {:>4}", labeled.count_by_type("delegation"));
        println!("  RLM Trigger:  {:>4}", labeled.count_by_type("rlm_trigger"));
        println!("  Total:        {:>4}", labeled.total_count());
    }

    println!("\nOptimization History:");
    println!("  Runs:         {}", summary.optimization_count);
    if let Some(last) = summary.last_optimization {
        println!("  Last run:     {}", last.format("%Y-%m-%d %H:%M"));
    } else {
        println!("  Last run:     never");
    }

    // Show auto-optimizer config
    let config = crate::dspy::AutoOptimizerConfig::load();
    println!("\nAuto-Optimization: {}", if config.enabled { "ENABLED" } else { "DISABLED" });
    println!("  Triggers:");
    println!("    - {} new labeled examples", config.min_labeled_examples);
    println!("    - Accuracy below {:.0}%", config.accuracy_threshold * 100.0);
    println!("    - Min {} hours between runs", config.min_hours_between_optimizations);

    if args.history {
        let metrics = tracker.metrics();
        if !metrics.history.is_empty() {
            println!("\nAccuracy History:");
            for snapshot in metrics.history.iter().rev().take(10) {
                println!("  {} - C:{:.0}% D:{:.0}% R:{:.0}% Task:{:.0}%",
                    snapshot.timestamp.format("%Y-%m-%d"),
                    snapshot.complexity_accuracy * 100.0,
                    snapshot.delegation_accuracy * 100.0,
                    snapshot.rlm_accuracy * 100.0,
                    snapshot.overall_task_success_rate * 100.0,
                );
            }
        }
    }

    Ok(())
}

/// Configure auto-optimization.
pub async fn auto_optimize(args: AutoOptimizeArgs) -> Result<()> {
    use crate::dspy::AutoOptimizerConfig;

    let mut config = AutoOptimizerConfig::load();
    let mut changed = false;

    if args.enable {
        config.enabled = true;
        changed = true;
        println!("Auto-optimization ENABLED");
    } else if args.disable {
        config.enabled = false;
        changed = true;
        println!("Auto-optimization DISABLED");
    }

    if let Some(examples) = args.min_examples {
        config.min_labeled_examples = examples;
        changed = true;
        println!("Min examples set to: {}", examples);
    }

    if let Some(threshold) = args.accuracy_threshold {
        if !(0.0..=1.0).contains(&threshold) {
            anyhow::bail!("Accuracy threshold must be between 0.0 and 1.0");
        }
        config.accuracy_threshold = threshold;
        changed = true;
        println!("Accuracy threshold set to: {:.0}%", threshold * 100.0);
    }

    if let Some(hours) = args.min_hours {
        config.min_hours_between_optimizations = hours;
        changed = true;
        println!("Min hours between runs set to: {}", hours);
    }

    if changed {
        config.save()?;
        println!("\nConfiguration saved.");
    } else {
        // Just show current config
        println!("Auto-Optimization Configuration");
        println!("===============================\n");
        println!("Enabled:         {}", if config.enabled { "yes" } else { "no" });
        println!("Min examples:    {}", config.min_labeled_examples);
        println!("Accuracy threshold: {:.0}%", config.accuracy_threshold * 100.0);
        println!("Min hours:       {}", config.min_hours_between_optimizations);
        println!("Background:      {}", if config.background_optimization { "yes" } else { "no" });
        println!("\nUse --enable, --disable, or set thresholds to modify.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_status_empty_dataset() {
        // This test just ensures status doesn't panic with empty data
        // It will print "no data" message
        let result = status(StatusArgs {}).await;
        // May fail if file doesn't exist, that's ok for this test
        let _ = result;
    }
}
