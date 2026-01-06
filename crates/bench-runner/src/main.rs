//! RLM Paper Replication Benchmark Runner
//!
//! CLI tool for running experiments from the RLM paper (arXiv 2512.24601v1).
//!
//! # Usage
//!
//! ```bash
//! # Run a single experiment
//! cargo run --bin bench_runner -- \
//!   --dataset s-niah \
//!   --method rlm \
//!   --model gpt-4
//!
//! # Run with specific output directory
//! cargo run --bin bench_runner -- \
//!   --dataset browsecomp \
//!   --method base \
//!   --output ./results
//!
//! # Use real data from data/ directory
//! cargo run --bin bench_runner -- \
//!   --dataset s-niah \
//!   --method rlm \
//!   --data-dir ./data
//!
//! # Generate results table from previous runs
//! cargo run --bin bench_runner -- \
//!   --table markdown \
//!   --results-dir ./results
//! ```

mod ablation;
mod analysis;
mod matrix;
mod table;

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, ValueEnum};
use lm_router::backends::{FmBridgeBackend, MockBackend, OpenAiBackend, OpenRouterBackend};
use lm_router::LmRouter;

use bench_datasets::sniah::generate_synthetic_tasks;
use bench_harness::{
    ExactMatchMetric, F1Metric, Metric, MultipleChoiceAccuracy, NumericDecayMetric, TaskInstance,
    TrajectoryWriter,
};
use rlm_methods::{
    BaseMethod, CodeActBm25Method, Method, RlmFullMethod, RlmNoSubcallsMethod, SummaryAgentMethod,
};

/// Available methods for benchmarking.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum MethodType {
    /// Direct LLM call with full context
    Base,
    /// Iterative context summarization
    Summary,
    /// ReAct-style with BM25 retrieval
    CodeactBm25,
    /// Full RLM with llm_query
    Rlm,
    /// RLM without recursive sub-calls (ablation)
    RlmNoSubcalls,
}

/// Available datasets for benchmarking.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum DatasetType {
    /// Single Needle-in-a-Haystack (50 tasks)
    SNiah,
    /// BrowseComp-Plus (150 tasks from 100K docs)
    Browsecomp,
    /// OOLONG TREC Coarse (numeric answers)
    OolongTrec,
    /// OOLONG Pairs (pairwise aggregation)
    OolongPairs,
    /// CodeQA from LongBench v2
    Codeqa,
}

/// Table output formats.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum TableFormat {
    /// Markdown table
    Markdown,
    /// LaTeX table
    Latex,
    /// CSV format
    Csv,
    /// JSON format
    Json,
}

/// Predefined experiment configurations.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExperimentPreset {
    /// Table 1: All 5 methods × all 5 datasets (25 experiments)
    Table1,
    /// Quick test: Base and RLM on S-NIAH only
    QuickTest,
    /// Ablation: RLM vs RLM-NoSubcalls on all datasets
    Ablation,
}

/// RLM Paper Replication Benchmark Runner
#[derive(Parser, Debug)]
#[command(name = "bench_runner")]
#[command(about = "Run experiments from the RLM paper (arXiv 2512.24601v1)")]
struct Args {
    /// Dataset to evaluate on (required unless --table is used)
    #[arg(long, value_enum)]
    dataset: Option<DatasetType>,

    /// Method to use for solving tasks (required unless --table is used)
    #[arg(long, value_enum)]
    method: Option<MethodType>,

    /// Model name for LLM backend (root model for RLM)
    #[arg(long, default_value = "openai/gpt-5")]
    model: String,

    /// Output directory for results
    #[arg(long, default_value = "./results")]
    output: PathBuf,

    /// Maximum number of tasks to run (for testing)
    #[arg(long)]
    max_tasks: Option<usize>,

    /// Maximum context length in characters
    #[arg(long, default_value = "100000")]
    max_context_len: usize,

    /// Data directory for real benchmark data (if not provided, uses synthetic)
    #[arg(long)]
    data_dir: Option<PathBuf>,

    /// Generate results table from previous runs (skips running experiments)
    #[arg(long, value_enum)]
    table: Option<TableFormat>,

    /// Directory containing results for table generation
    #[arg(long)]
    results_dir: Option<PathBuf>,

    /// Run predefined experiment matrix (skips single experiment mode)
    #[arg(long, value_enum)]
    experiment: Option<ExperimentPreset>,

    /// Resume from checkpoint (for experiment matrix)
    #[arg(long)]
    resume: bool,

    /// Generate ablation analysis report (RLM vs RLM-NoSubcalls)
    #[arg(long)]
    ablation: bool,

    /// Analyze failures and generate error report
    #[arg(long)]
    analyze_failures: bool,

    /// Export failures to JSONL file
    #[arg(long)]
    export_failures: Option<PathBuf>,

    /// Use mock backend for testing (returns fixed responses)
    #[arg(long)]
    mock: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

/// Get the appropriate metric for a dataset.
fn get_metric_for_dataset(dataset: DatasetType) -> Box<dyn Metric> {
    match dataset {
        // S-NIAH and BrowseComp use exact match for needle/answer retrieval
        DatasetType::SNiah | DatasetType::Browsecomp => Box::new(ExactMatchMetric),
        // OOLONG TrecCoarse uses numeric decay (counting tasks)
        DatasetType::OolongTrec => Box::new(NumericDecayMetric),
        // OOLONG Pairs uses F1 for set comparison
        DatasetType::OolongPairs => Box::new(F1Metric),
        // CodeQA uses multiple choice accuracy
        DatasetType::Codeqa => Box::new(MultipleChoiceAccuracy),
    }
}

fn create_method(method_type: MethodType, router: Arc<LmRouter>, model: &str) -> Box<dyn Method> {
    match method_type {
        MethodType::Base => Box::new(BaseMethod::new(router, model)),
        MethodType::Summary => Box::new(SummaryAgentMethod::new(router, model)),
        MethodType::CodeactBm25 => Box::new(CodeActBm25Method::new(router, model)),
        MethodType::Rlm => Box::new(RlmFullMethod::new(router, model)),
        MethodType::RlmNoSubcalls => Box::new(RlmNoSubcallsMethod::new(router, model)),
    }
}

/// Load tasks from data directory based on dataset type.
async fn load_tasks_from_dir(
    data_dir: &PathBuf,
    dataset: DatasetType,
    max_tasks: Option<usize>,
) -> Result<Vec<Box<dyn TaskInstance>>, Box<dyn std::error::Error>> {
    use bench_harness::{GroundTruth, SimpleTask};
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file_path = match dataset {
        DatasetType::SNiah => data_dir.join("sniah").join("sniah.jsonl"),
        DatasetType::Browsecomp => data_dir.join("browsecomp").join("tasks.jsonl"),
        DatasetType::OolongTrec => data_dir.join("oolong").join("trec_coarse.jsonl"),
        DatasetType::OolongPairs => data_dir.join("oolong").join("pairs.jsonl"),
        DatasetType::Codeqa => data_dir.join("codeqa").join("codeqa.jsonl"),
    };

    tracing::info!("Loading tasks from: {}", file_path.display());

    if !file_path.exists() {
        return Err(format!("Data file not found: {}", file_path.display()).into());
    }

    let file = File::open(&file_path)?;
    let reader = BufReader::new(file);
    let mut tasks: Vec<Box<dyn TaskInstance>> = Vec::new();

    for (i, line) in reader.lines().enumerate() {
        if let Some(max) = max_tasks {
            if i >= max {
                break;
            }
        }

        let line = line?;
        let json: serde_json::Value = serde_json::from_str(&line)?;

        let task = match dataset {
            DatasetType::SNiah => {
                let id = json["id"].as_str().unwrap_or(&format!("sniah-{i:03}")).to_string();
                let query = json["query"].as_str().unwrap_or("").to_string();
                let context = json["context"].as_str().unwrap_or("").to_string();
                let needle = json["needle"].as_str().unwrap_or("").to_string();

                SimpleTask::new(id, query, GroundTruth::exact(needle)).with_context(context)
            }
            DatasetType::Browsecomp => {
                let id = json["id"].as_str().unwrap_or(&format!("bc-{i:03}")).to_string();
                let query = json["query"].as_str().unwrap_or("").to_string();
                let context = json["context"].as_str().unwrap_or("").to_string();
                let answer = json["answer"].as_str().unwrap_or("").to_string();

                SimpleTask::new(id, query, GroundTruth::exact(answer)).with_context(context)
            }
            DatasetType::OolongTrec => {
                let id = json["id"].as_str().unwrap_or(&format!("oolong-trec-{i:03}")).to_string();
                let query = json["query"].as_str().unwrap_or("").to_string();
                let context = json["context"].as_str().unwrap_or("").to_string();
                let answer = json["answer"].as_f64().unwrap_or(0.0);
                let tolerance = json["tolerance"].as_f64().unwrap_or(0.0);

                SimpleTask::new(id, query, GroundTruth::numeric_with_tolerance(answer, tolerance))
                    .with_context(context)
            }
            DatasetType::OolongPairs => {
                let id = json["id"].as_str().unwrap_or(&format!("oolong-pairs-{i:03}")).to_string();
                let query = json["query"].as_str().unwrap_or("").to_string();
                let context = json["context"].as_str().unwrap_or("").to_string();
                let answer: Vec<String> = json["answer"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                SimpleTask::new(id, query, GroundTruth::string_set(answer)).with_context(context)
            }
            DatasetType::Codeqa => {
                let id = json["id"].as_str().unwrap_or(&format!("codeqa-{i:03}")).to_string();
                let question = json["question"].as_str().unwrap_or("").to_string();
                let code = json["code"].as_str().unwrap_or("").to_string();
                let answer = json["answer"].as_str().unwrap_or("A").chars().next().unwrap_or('A');
                let choices: Vec<String> = json["choices"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let context = format!("{}\n\nCode:\n```\n{}\n```", question, code);
                SimpleTask::new(id, question, GroundTruth::multiple_choice(answer, choices))
                    .with_context(context)
            }
        };

        tasks.push(Box::new(task));
    }

    Ok(tasks)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing
    if args.verbose {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();
    }

    // Handle table generation mode
    if let Some(format) = args.table {
        let results_dir = args
            .results_dir
            .as_ref()
            .unwrap_or(&args.output);

        tracing::info!("Generating results table from: {}", results_dir.display());

        let table = table::load_results_from_dir(results_dir)?;

        let output = match format {
            TableFormat::Markdown => table.to_markdown(),
            TableFormat::Latex => table.to_latex(),
            TableFormat::Csv => table.to_csv(),
            TableFormat::Json => table.to_json(),
        };

        println!("{}", output);
        return Ok(());
    }

    // Handle ablation analysis mode
    if args.ablation {
        let results_dir = args.results_dir.as_ref().unwrap_or(&args.output);

        tracing::info!(
            "Generating ablation report from: {}",
            results_dir.display()
        );

        let report = ablation::load_ablation_from_results(results_dir)?;
        println!("{}", report.to_markdown());
        return Ok(());
    }

    // Handle failure analysis mode
    if args.analyze_failures {
        let results_dir = args.results_dir.as_ref().unwrap_or(&args.output);

        tracing::info!("Analyzing failures from: {}", results_dir.display());

        let analysis = analysis::load_failures_from_trajectories(results_dir)?;
        println!("{}", analysis.to_markdown());

        // Export if requested
        if let Some(ref export_path) = args.export_failures {
            analysis.export_jsonl(export_path)?;
            tracing::info!("Failures exported to: {}", export_path.display());
        }

        return Ok(());
    }

    // Handle experiment matrix mode
    if let Some(preset) = args.experiment {
        let config = match preset {
            ExperimentPreset::Table1 => {
                matrix::presets::table1(args.output.clone(), args.model.clone())
            }
            ExperimentPreset::QuickTest => {
                matrix::presets::quick_test(args.output.clone(), args.model.clone())
            }
            ExperimentPreset::Ablation => {
                matrix::presets::ablation(args.output.clone(), args.model.clone())
            }
        };

        let experiments = config.experiment_keys();
        let checkpoint_path = args.output.join("matrix_checkpoint.json");

        // Load or create checkpoint
        let mut checkpoint = if args.resume {
            matrix::MatrixCheckpoint::load_or_create(&checkpoint_path, experiments)?
        } else {
            matrix::MatrixCheckpoint::new(experiments)
        };

        matrix::print_matrix_status(&checkpoint);

        // Create output directory
        std::fs::create_dir_all(&args.output)?;

        // Run experiments
        while let Some(key) = checkpoint.next_experiment().cloned() {
            tracing::info!("\n=== Running {} × {} ===", key.method, key.dataset);

            checkpoint.start_experiment(&key);
            checkpoint.save(&checkpoint_path)?;

            // Create experiment output directory
            let exp_output = config.experiment_output_dir(&key);
            std::fs::create_dir_all(&exp_output)?;

            // Run the experiment (simplified - would call into main logic)
            // For now, just mark as complete
            // TODO: Actually run the experiment with proper method/dataset parsing
            tracing::info!("  Output: {}", exp_output.display());
            tracing::info!("  (Experiment execution not yet integrated)");

            checkpoint.complete_experiment(&key);
            checkpoint.save(&checkpoint_path)?;

            let (completed, total) = checkpoint.progress();
            tracing::info!("Progress: {}/{}", completed, total);
        }

        // Generate final table
        tracing::info!("\n=== Experiment Matrix Complete ===");
        let table = table::load_results_from_dir(&args.output)?;
        println!("\n{}", table.to_markdown());

        return Ok(());
    }

    // Require dataset and method for single experiment mode
    let dataset = args.dataset.ok_or("--dataset is required when not using --table")?;
    let method_type = args.method.ok_or("--method is required when not using --table")?;

    tracing::info!("Starting benchmark runner");
    tracing::info!("Dataset: {:?}", dataset);
    tracing::info!("Method: {:?}", method_type);
    tracing::info!("Model: {}", args.model);

    // Create LM Router with appropriate backend
    let router = if args.mock {
        tracing::info!("Using mock backend for testing");
        let mock_backend = MockBackend::new()
            .with_model("mock-model")
            .with_response("ALPHA-7749-BRAVO"); // Default needle for S-NIAH
        Arc::new(
            LmRouter::builder()
                .add_backend(mock_backend)
                .default_backend("mock")
                .build(),
        )
    } else if std::env::var("OPENROUTER_API_KEY").is_ok() {
        tracing::info!("Using OpenRouter backend");
        let openrouter_backend = OpenRouterBackend::new()?;
        Arc::new(
            LmRouter::builder()
                .add_backend(openrouter_backend)
                .default_backend("openrouter")
                .build(),
        )
    } else if std::env::var("OPENAI_API_KEY").is_ok() {
        tracing::info!("Using OpenAI backend");
        let openai_backend = OpenAiBackend::new()?;
        Arc::new(
            LmRouter::builder()
                .add_backend(openai_backend)
                .default_backend("openai")
                .build(),
        )
    } else {
        tracing::info!("Using FM Bridge backend");
        let fm_backend = FmBridgeBackend::new()?;
        Arc::new(
            LmRouter::builder()
                .add_backend(fm_backend)
                .default_backend("fm-bridge")
                .build(),
        )
    };

    // Create method
    let method = create_method(method_type, router.clone(), &args.model);

    // Load dataset
    let dataset_name = format!("{:?}", dataset).to_lowercase();
    tracing::info!("Loading dataset: {}", dataset_name);

    // Load tasks from data directory or generate synthetic
    let tasks: Vec<Box<dyn TaskInstance>> = if let Some(ref data_dir) = args.data_dir {
        load_tasks_from_dir(data_dir, dataset, args.max_tasks).await?
    } else {
        // Fall back to synthetic generation
        let num_tasks = args.max_tasks.unwrap_or(match dataset {
            DatasetType::SNiah => 50,
            DatasetType::Browsecomp => 10,
            DatasetType::OolongTrec => 10,
            DatasetType::OolongPairs => 10,
            DatasetType::Codeqa => 10,
        });

        let synthetic = generate_synthetic_tasks(num_tasks, args.max_context_len);

        if !matches!(dataset, DatasetType::SNiah) {
            tracing::warn!(
                "Dataset {:?} not fully implemented, using synthetic S-NIAH tasks",
                dataset
            );
        }

        synthetic.into_iter().map(|t| Box::new(t) as Box<dyn TaskInstance>).collect()
    };

    tracing::info!("Loaded {} tasks", tasks.len());

    // Create output directory
    std::fs::create_dir_all(&args.output)?;

    // Create trajectory writer
    let trajectory_path = args.output.join(format!(
        "{}-{}-trajectories.jsonl",
        dataset_name,
        method.name()
    ));
    let mut trajectory_writer = TrajectoryWriter::new(&trajectory_path)?;

    // Run experiment
    tracing::info!("Starting experiment...");
    let start = std::time::Instant::now();

    let mut correct = 0;
    let mut total = 0;
    let metric = get_metric_for_dataset(dataset);
    tracing::info!("Using metric: {}", metric.name());

    for (i, task) in tasks.iter().enumerate() {
        tracing::info!("Task {}/{}: {}", i + 1, tasks.len(), task.id());

        match method.solve(task.as_ref()).await {
            Ok(result) => {
                let metric_value = metric.compute(&result.answer, task.ground_truth());
                if metric_value.score > 0.5 {
                    correct += 1;
                }
                total += 1;

                tracing::info!(
                    "  Answer: {} | Expected: {:?} | Score: {:.2}",
                    result.answer,
                    task.ground_truth(),
                    metric_value.score
                );

                // Write trajectory
                if let Err(e) = trajectory_writer.write(&result.trajectory) {
                    tracing::warn!("Failed to write trajectory: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("  Failed: {}", e);
                total += 1;
            }
        }
    }

    let duration = start.elapsed();
    let accuracy = if total > 0 {
        correct as f64 / total as f64
    } else {
        0.0
    };

    // Flush trajectory writer
    trajectory_writer.flush()?;

    // Print summary
    println!("\n=== Experiment Results ===");
    println!("Dataset: {:?}", dataset);
    println!("Method: {:?}", method_type);
    println!("Model: {}", args.model);
    println!("Tasks: {}/{}", correct, total);
    println!("Accuracy: {:.2}%", accuracy * 100.0);
    println!("Duration: {:.2}s", duration.as_secs_f64());
    println!("Output: {}", args.output.display());

    // Save summary
    let summary_path = args.output.join("summary.json");
    let summary = serde_json::json!({
        "dataset": format!("{:?}", dataset),
        "method": format!("{:?}", method_type),
        "model": args.model,
        "correct": correct,
        "total": total,
        "accuracy": accuracy,
        "duration_secs": duration.as_secs_f64(),
    });
    std::fs::write(&summary_path, serde_json::to_string_pretty(&summary)?)?;
    tracing::info!("Summary saved to {}", summary_path.display());

    Ok(())
}
