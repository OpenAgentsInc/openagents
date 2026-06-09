#![allow(
    clippy::print_stdout,
    reason = "acceptance harness intentionally prints the final receipt"
)]

use std::path::PathBuf;

use anyhow::Result;
use autopilot_desktop::apple_architecture_explainer_reference_run::{
    ArchitectureExplainerFirstRunConfig, run_architecture_explainer_acceptance_harness,
};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "apple-architecture-explainer-acceptance-harness")]
#[command(
    about = "Run the full Psionic Apple acceptance harness for the architecture-explainer lane"
)]
struct Cli {
    #[arg(long)]
    train_dataset_path: Option<PathBuf>,
    #[arg(long)]
    held_out_dataset_path: Option<PathBuf>,
    #[arg(long)]
    benchmark_dataset_path: Option<PathBuf>,
    #[arg(long)]
    corpus_manifest_path: Option<PathBuf>,
    #[arg(long)]
    experiment_manifest_path: Option<PathBuf>,
    #[arg(long, default_value = "http://127.0.0.1:11435")]
    apple_fm_base_url: String,
    #[arg(long)]
    control_base_url: Option<String>,
    #[arg(long)]
    control_bearer_token: Option<String>,
    #[arg(long)]
    training_policy_override_path: Option<PathBuf>,
    #[arg(long)]
    acceptance_report_path: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut config = ArchitectureExplainerFirstRunConfig::reference();
    if let Some(path) = cli.train_dataset_path {
        config.train_dataset_path = path;
    }
    if let Some(path) = cli.held_out_dataset_path {
        config.held_out_dataset_path = path;
    }
    if let Some(path) = cli.benchmark_dataset_path {
        config.benchmark_dataset_path = path;
    }
    if let Some(path) = cli.corpus_manifest_path {
        config.corpus_manifest_path = path;
    }
    if let Some(path) = cli.experiment_manifest_path {
        config.experiment_manifest_path = path;
    }
    config.apple_fm_base_url = cli.apple_fm_base_url;
    config.control_base_url = cli.control_base_url;
    config.control_bearer_token = cli.control_bearer_token;
    config.training_policy_override_path = cli.training_policy_override_path;

    let acceptance_report_path = cli.acceptance_report_path.unwrap_or_else(|| {
        std::env::temp_dir().join("openagents_psionic_apple_acceptance_report.json")
    });
    let report =
        run_architecture_explainer_acceptance_harness(&config, acceptance_report_path.as_path())?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !report.acceptance_passed {
        std::process::exit(1);
    }
    Ok(())
}
