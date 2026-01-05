//! CLI commands for RLM.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use fm_bridge::FMClient;

use crate::context::Context;
use crate::error::Result;
use crate::python_executor::PythonExecutor;
use crate::{RlmConfig, RlmEngine};

/// RLM CLI commands.
#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    /// Run RLM with a query
    Run(RunArgs),
}

/// Arguments for the `run` command.
#[derive(Args, Clone, Debug)]
pub struct RunArgs {
    /// The query to send to the RLM
    pub query: String,

    /// FM Bridge URL
    #[arg(long, default_value = "http://localhost:11435")]
    pub fm_url: String,

    /// Maximum number of iterations
    #[arg(long, default_value = "10")]
    pub max_iterations: u32,

    /// Allow shell command execution
    #[arg(long)]
    pub allow_shell: bool,

    /// Python binary to use
    #[arg(long, default_value = "python3")]
    pub python: String,

    /// Load context from a file
    #[arg(long, value_name = "FILE")]
    pub context_file: Option<PathBuf>,

    /// Load context from a directory (recursively)
    #[arg(long, value_name = "DIR")]
    pub context_dir: Option<PathBuf>,
}

/// Execute an RLM CLI command.
pub async fn execute(cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Run(args) => run_rlm(args).await,
    }
}

async fn run_rlm(args: RunArgs) -> Result<()> {
    println!("=== RLM Run ===");
    println!("Query: {}", args.query);
    println!("FM URL: {}", args.fm_url);
    println!("Max iterations: {}", args.max_iterations);
    println!("Allow shell: {}", args.allow_shell);
    println!("Python: {}", args.python);

    // Load context if specified
    let context = if let Some(ref file_path) = args.context_file {
        println!("Context file: {}", file_path.display());
        Some(Context::from_file(file_path)?)
    } else if let Some(ref dir_path) = args.context_dir {
        println!("Context directory: {}", dir_path.display());
        Some(Context::from_directory(dir_path)?)
    } else {
        None
    };

    if let Some(ref ctx) = context {
        println!("\n{}", ctx.summary());
    }

    println!();

    // Check FM Bridge health
    println!("Connecting to FM Bridge...");
    let client = FMClient::with_base_url(&args.fm_url)?;

    match client.health().await {
        Ok(true) => println!("FM Bridge: OK\n"),
        Ok(false) => {
            println!("FM Bridge: NOT HEALTHY");
            println!("\nPlease start FM Bridge first:");
            println!("  pylon-desktop --cli");
            return Ok(());
        }
        Err(e) => {
            println!("FM Bridge: CONNECTION FAILED - {}", e);
            println!("\nPlease start FM Bridge first:");
            println!("  pylon-desktop --cli");
            return Ok(());
        }
    }

    // Check Python availability
    let executor = PythonExecutor::with_binary(&args.python);
    if !executor.is_available() {
        println!("ERROR: Python not available at '{}'", args.python);
        println!("Install Python or specify a different binary with --python");
        return Ok(());
    }

    if let Some(version) = executor.version() {
        println!("Python: {}", version);
    }
    println!();

    // Create engine with config
    let config = RlmConfig {
        max_iterations: args.max_iterations,
        allow_shell: args.allow_shell,
        verbose: true, // Always verbose for CLI
    };

    let mut engine = RlmEngine::with_config(client, executor, config);

    // Set context if loaded
    if let Some(ctx) = context {
        engine.set_context(ctx);
    }

    println!("{}", "=".repeat(60));
    println!();

    // Run RLM
    match engine.run(&args.query).await {
        Ok(result) => {
            println!();
            println!("{}", "=".repeat(60));
            println!("\n=== FINAL RESULT ===\n");
            println!("Output: {}", result.output);
            println!("Iterations: {}", result.iterations);

            if !result.execution_log.is_empty() {
                println!("\n=== Execution Summary ===\n");
                for entry in &result.execution_log {
                    println!("Iteration {}: {} -> {}",
                        entry.iteration,
                        entry.command_type,
                        if entry.result.len() > 50 {
                            format!("{}...", &entry.result[..50])
                        } else {
                            entry.result.clone()
                        }
                    );
                }
            }
        }
        Err(e) => {
            println!();
            println!("{}", "=".repeat(60));
            println!("\n=== ERROR ===\n");
            println!("{}", e);
        }
    }

    Ok(())
}
