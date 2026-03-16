#![allow(
    clippy::print_stdout,
    reason = "operator harness intentionally prints the final report"
)]

use std::path::PathBuf;

use anyhow::Result;
use autopilot_desktop::apple_architecture_explainer_reference_run::{
    ArchitectureExplainerFirstRunConfig, run_architecture_explainer_reference_cycle,
};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "apple-architecture-explainer-reference-run")]
#[command(
    about = "Execute the first real Psionic architecture explainer Apple adapter operator run"
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
    #[arg(long)]
    export_path: Option<PathBuf>,
    #[arg(long)]
    json_report_path: Option<PathBuf>,
    #[arg(long)]
    markdown_report_path: Option<PathBuf>,
    #[arg(long)]
    package_name: Option<String>,
    #[arg(long)]
    author: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    license: Option<String>,
    #[arg(long, default_value = "http://127.0.0.1:11435")]
    apple_fm_base_url: String,
    #[arg(long)]
    control_base_url: Option<String>,
    #[arg(long)]
    control_bearer_token: Option<String>,
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
    if let Some(path) = cli.export_path {
        config.export_path = path;
    }
    if let Some(path) = cli.json_report_path {
        config.json_report_path = path;
    }
    if let Some(path) = cli.markdown_report_path {
        config.markdown_report_path = path;
    }
    if let Some(value) = cli.package_name {
        config.package_name = value;
    }
    if let Some(value) = cli.author {
        config.author = value;
    }
    if let Some(value) = cli.description {
        config.description = value;
    }
    if let Some(value) = cli.license {
        config.license = value;
    }
    config.apple_fm_base_url = cli.apple_fm_base_url;
    config.control_base_url = cli.control_base_url;
    config.control_bearer_token = cli.control_bearer_token;

    let report = run_architecture_explainer_reference_cycle(&config)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
