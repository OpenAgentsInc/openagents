//! TestGen CLI
//!
//! Command-line interface for test generation and evolution.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use testgen::{
    format_as_pytest, GeneratedTest, ReflectionEntry, TestCategory, TestGenConfig,
    TestGenConfigInput, TestGenContext, TestGenEmitter, TestGenStore, TestGenerator,
    EnvironmentInfo,
};

/// Default TB2 directory
const DEFAULT_TB2_DIR: &str = "/Users/christopherdavid/code/terminal-bench-2";

#[derive(Parser)]
#[command(name = "testgen")]
#[command(about = "Test generation and evolution system for Terminal-Bench")]
#[command(version)]
struct Cli {
    /// Database path (default: testgen.db)
    #[arg(short, long, default_value = "testgen.db")]
    database: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate tests for a task
    Generate {
        /// Task ID
        #[arg(short, long)]
        task: String,

        /// Task description
        #[arg(short, long)]
        description: String,

        /// Generation context (benchmark, commander, mechacoder)
        #[arg(short, long, default_value = "benchmark")]
        context: String,

        /// Output file path for pytest format (optional)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Terminal-Bench 2 integration
    Tb2 {
        #[command(subcommand)]
        action: Tb2Action,
    },

    /// Show current configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Show statistics
    Stats,

    /// List recent runs
    Runs {
        /// Maximum number of runs to show
        #[arg(short, long, default_value = "10")]
        limit: u32,

        /// Filter by task ID
        #[arg(short, long)]
        task: Option<String>,
    },
}

#[derive(Subcommand)]
enum Tb2Action {
    /// List all TB2 tasks
    List {
        /// Filter by name pattern
        #[arg(short, long)]
        filter: Option<String>,

        /// TB2 directory path
        #[arg(long, default_value = DEFAULT_TB2_DIR)]
        tb2_dir: PathBuf,
    },

    /// Run testgen on a TB2 task
    Run {
        /// Task name (directory name in TB2)
        task: String,

        /// Output pytest file (default: <task>/test_generated.py)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// TB2 directory path
        #[arg(long, default_value = DEFAULT_TB2_DIR)]
        tb2_dir: PathBuf,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current config
    Show,

    /// List all configs
    List,

    /// Create a new config
    Create {
        /// Temperature
        #[arg(long, default_value = "0.3")]
        temperature: f64,

        /// Min tests per category
        #[arg(long, default_value = "2")]
        min_tests: u32,

        /// Max tests per category
        #[arg(long, default_value = "5")]
        max_tests: u32,
    },

    /// Set current config
    SetCurrent {
        /// Config ID
        id: i64,
    },
}

/// Simple CLI emitter that prints to stdout
struct CliEmitter;

impl TestGenEmitter for CliEmitter {
    fn on_progress(&self, phase: &str, category: Option<TestCategory>, round: u32, status: &str) {
        if let Some(cat) = category {
            println!("[{} {} round {}] {}", phase, cat, round, status);
        } else {
            println!("[{}] {}", phase, status);
        }
    }

    fn on_test(&self, test: &GeneratedTest) {
        println!(
            "  + {} ({}): {} -> {}",
            test.id,
            test.category,
            truncate(&test.input, 40),
            test.expected_output
                .as_ref()
                .map(|s| truncate(s, 30))
                .unwrap_or_else(|| "null".to_string())
        );
    }

    fn on_reflection(&self, entry: &ReflectionEntry) {
        println!(
            "  * Reflection: {}",
            truncate(&entry.reflection_text, 60)
        );
    }

    fn on_complete(&self, total_tests: u32, total_rounds: u32, duration_ms: u64) {
        println!();
        println!("=== Generation Complete ===");
        println!("Total tests: {}", total_tests);
        println!("Total rounds: {}", total_rounds);
        println!("Duration: {}ms", duration_ms);
    }

    fn on_error(&self, error: &str) {
        eprintln!("ERROR: {}", error);
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize telemetry
    telemetry::init_default("testgen");

    let cli = Cli::parse();
    let store = TestGenStore::open(&cli.database)?;

    match cli.command {
        Commands::Generate {
            task,
            description,
            context,
            output,
        } => {
            let context = match context.as_str() {
                "benchmark" => TestGenContext::Benchmark,
                "commander" => TestGenContext::Commander,
                "mechacoder" => TestGenContext::MechaCoder,
                _ => TestGenContext::Benchmark,
            };

            println!("Generating tests for task: {}", task);
            println!("Context: {:?}", context);
            println!();

            let client = fm_bridge::FMClient::new();
            let generator = TestGenerator::new(client);
            let emitter = CliEmitter;
            let environment = EnvironmentInfo::minimal();

            let result = generator
                .generate_iteratively(&description, &task, &environment, context, &emitter)
                .await?;

            println!();
            println!("Generated {} tests", result.tests.len());
            println!("Total tokens used: {}", result.total_tokens_used);

            // Write to output file if specified
            if let Some(output_path) = output {
                let pytest_content = format_as_pytest(&result.tests, &task, Some(&description));
                std::fs::write(&output_path, pytest_content)?;
                println!("Wrote pytest file to: {}", output_path.display());
            }

            // Save results if we have a config
            if let Some(config) = store.get_current_config(None)? {
                use testgen::{
                    analyze_testgen_run, compute_overall_score, generate_run_id,
                    generate_session_id, TestGenRunInput, TestGenTrajectory,
                };

                let trajectory = TestGenTrajectory {
                    session_id: generate_session_id(),
                    task_id: task.clone(),
                    task_description: description.clone(),
                    total_tests: result.tests.len() as u32,
                    total_rounds: result.category_rounds.values().sum(),
                    category_rounds: result.category_rounds.clone(),
                    comprehensiveness_score: result.comprehensiveness_score,
                    total_tokens_used: result.total_tokens_used,
                    duration_ms: 0,
                    tests: result.tests.clone(),
                    reflections: result.reflections.clone(),
                    environment: environment.clone(),
                    uncertainties: vec![],
                };

                let analysis = analyze_testgen_run(&trajectory);
                let analysis = compute_overall_score(analysis, result.comprehensiveness_score);

                let run_input = TestGenRunInput {
                    run_id: generate_run_id(),
                    session_id: trajectory.session_id.clone(),
                    config_id: config.id,
                    task_id: task,
                    total_tests: result.tests.len() as u32,
                    comprehensiveness_score: result.comprehensiveness_score,
                    duration_ms: 0,
                    total_tokens: result.total_tokens_used,
                    category_balance: Some(analysis.category_balance),
                    anti_cheat_coverage: Some(analysis.anti_cheat_coverage),
                    parameter_discovery: Some(analysis.parameter_discovery),
                    reflection_effectiveness: Some(analysis.reflection_effectiveness),
                    token_efficiency: Some(analysis.token_efficiency),
                    meta_model: None,
                    proposed_change: None,
                    change_accepted: false,
                    score: analysis.overall_score,
                };

                let run = store.save_run(&run_input)?;
                println!("Saved run: {} (score: {})", run.run_id, run.score);
            }
        }

        Commands::Tb2 { action } => match action {
            Tb2Action::List { filter, tb2_dir } => {
                if !tb2_dir.exists() {
                    eprintln!("TB2 directory not found: {}", tb2_dir.display());
                    eprintln!("Set --tb2-dir to your terminal-bench-2 location");
                    return Ok(());
                }

                let mut tasks: Vec<String> = std::fs::read_dir(&tb2_dir)?
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .filter(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        !name.starts_with('.') && e.path().join("instruction.md").exists()
                    })
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();

                tasks.sort();

                // Apply filter if provided
                if let Some(pattern) = &filter {
                    tasks.retain(|t| t.contains(pattern));
                }

                println!("TB2 Tasks ({}):", tasks.len());
                println!();
                for (i, task) in tasks.iter().enumerate() {
                    println!("  {:3}. {}", i + 1, task);
                }
            }

            Tb2Action::Run { task, output, tb2_dir } => {
                let task_dir = tb2_dir.join(&task);
                let instruction_path = task_dir.join("instruction.md");

                if !instruction_path.exists() {
                    eprintln!("Task not found: {}", task);
                    eprintln!("Looking for: {}", instruction_path.display());
                    return Ok(());
                }

                let description = std::fs::read_to_string(&instruction_path)?;

                println!("=== TB2 Task: {} ===", task);
                println!("Description:");
                println!("{}", truncate(&description, 200));
                println!();

                let client = fm_bridge::FMClient::new();
                let generator = TestGenerator::new(client);
                let emitter = CliEmitter;
                let environment = EnvironmentInfo::minimal();

                let result = generator
                    .generate_iteratively(
                        &description,
                        &task,
                        &environment,
                        TestGenContext::Benchmark,
                        &emitter,
                    )
                    .await?;

                println!();
                println!("Generated {} tests", result.tests.len());
                println!("Total tokens used: {}", result.total_tokens_used);

                // Write output
                let output_path = output.unwrap_or_else(|| task_dir.join("test_generated.py"));
                let pytest_content = format_as_pytest(&result.tests, &task, Some(&description));
                std::fs::write(&output_path, &pytest_content)?;
                println!("Wrote pytest file to: {}", output_path.display());

                // Save to store if config exists
                if let Some(config) = store.get_current_config(None)? {
                    use testgen::{
                        analyze_testgen_run, compute_overall_score, generate_run_id,
                        generate_session_id, TestGenRunInput, TestGenTrajectory,
                    };

                    let trajectory = TestGenTrajectory {
                        session_id: generate_session_id(),
                        task_id: task.clone(),
                        task_description: description.clone(),
                        total_tests: result.tests.len() as u32,
                        total_rounds: result.category_rounds.values().sum(),
                        category_rounds: result.category_rounds.clone(),
                        comprehensiveness_score: result.comprehensiveness_score,
                        total_tokens_used: result.total_tokens_used,
                        duration_ms: 0,
                        tests: result.tests.clone(),
                        reflections: result.reflections.clone(),
                        environment: environment.clone(),
                        uncertainties: vec![],
                    };

                    let analysis = analyze_testgen_run(&trajectory);
                    let analysis = compute_overall_score(analysis, result.comprehensiveness_score);

                    let run_input = TestGenRunInput {
                        run_id: generate_run_id(),
                        session_id: trajectory.session_id.clone(),
                        config_id: config.id,
                        task_id: task,
                        total_tests: result.tests.len() as u32,
                        comprehensiveness_score: result.comprehensiveness_score,
                        duration_ms: 0,
                        total_tokens: result.total_tokens_used,
                        category_balance: Some(analysis.category_balance),
                        anti_cheat_coverage: Some(analysis.anti_cheat_coverage),
                        parameter_discovery: Some(analysis.parameter_discovery),
                        reflection_effectiveness: Some(analysis.reflection_effectiveness),
                        token_efficiency: Some(analysis.token_efficiency),
                        meta_model: None,
                        proposed_change: None,
                        change_accepted: false,
                        score: analysis.overall_score,
                    };

                    let run = store.save_run(&run_input)?;
                    println!("Saved run: {} (score: {})", run.run_id, run.score);
                }
            }
        },

        Commands::Config { action } => match action {
            ConfigAction::Show => {
                if let Some(config) = store.get_current_config(None)? {
                    print_config(&config);
                } else {
                    println!("No current config set.");
                    println!("Create one with: testgen config create");
                }
            }

            ConfigAction::List => {
                let configs = store.get_all_configs()?;
                if configs.is_empty() {
                    println!("No configs found.");
                } else {
                    println!("Configs ({}):", configs.len());
                    for config in configs {
                        println!(
                            "  #{} v{} - temp={:.2} tests={}-{} {}",
                            config.id,
                            config.version,
                            config.temperature,
                            config.min_tests_per_category,
                            config.max_tests_per_category,
                            if config.is_current { "[CURRENT]" } else { "" }
                        );
                    }
                }
            }

            ConfigAction::Create {
                temperature,
                min_tests,
                max_tests,
            } => {
                let input = TestGenConfigInput {
                    version: Some("1.0.0".to_string()),
                    temperature: Some(temperature),
                    min_tests_per_category: Some(min_tests),
                    max_tests_per_category: Some(max_tests),
                    ..Default::default()
                };

                let config = store.save_config(&input)?;
                println!("Created config #{}", config.id);

                // Set as current if no current exists
                if store.get_current_config(None)?.is_none() {
                    store.set_current_config(config.id)?;
                    println!("Set as current config");
                }
            }

            ConfigAction::SetCurrent { id } => {
                store.set_current_config(id)?;
                println!("Set config #{} as current", id);
            }
        },

        Commands::Stats => {
            let stats = store.get_stats()?;
            println!("=== TestGen Statistics ===");
            println!("Total runs: {}", stats.total_runs);
            println!("Total configs: {}", stats.total_configs);
            println!("Average score: {:.0}", stats.average_score);
            println!("Best score: {}", stats.best_score);
            println!(
                "Average comprehensiveness: {:.1}",
                stats.average_comprehensiveness
            );
            println!(
                "Average token efficiency: {:.3}",
                stats.average_token_efficiency
            );
            println!("Config evolutions: {}", stats.config_evolution_count);
        }

        Commands::Runs { limit, task } => {
            let runs = if let Some(task_id) = task {
                store.get_run_history(&task_id, limit)?
            } else {
                store.get_recent_runs(limit)?
            };

            if runs.is_empty() {
                println!("No runs found.");
            } else {
                println!("Recent runs ({}):", runs.len());
                for run in runs {
                    println!(
                        "  {} - {} - score={} tests={} tokens={}",
                        run.run_id, run.task_id, run.score, run.total_tests, run.total_tokens
                    );
                }
            }
        }
    }

    Ok(())
}

fn print_config(config: &TestGenConfig) {
    println!("=== Current Config ===");
    println!("ID: {}", config.id);
    println!("Version: {}", config.version);
    println!("Temperature: {}", config.temperature);
    println!("Max tokens: {}", config.max_tokens);
    println!(
        "Tests per category: {}-{}",
        config.min_tests_per_category, config.max_tests_per_category
    );
    println!("Max rounds per category: {}", config.max_rounds_per_category);
    println!("Environment weight: {}", config.environment_weight);
    println!("Anti-cheat weight: {}", config.anti_cheat_weight);
    println!("Precision weight: {}", config.precision_weight);
    println!("Primary model: {:?}", config.primary_model);
    println!("Reflection model: {:?}", config.reflection_model);
    println!(
        "Comprehensiveness target: {}-{}",
        config.min_comprehensiveness_score, config.target_comprehensiveness_score
    );
    println!("Hash: {}", config.config_hash);
    println!("Created: {}", config.created_at);
}
