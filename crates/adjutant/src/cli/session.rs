use std::path::PathBuf;

use anyhow::{Context, Result};
use autopilot_core::paths::OpenAgentsPaths;
use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::Value;

#[derive(Subcommand)]
pub enum SessionCommand {
    /// Show session artifacts
    Show(ShowArgs),
}

#[derive(Args)]
pub struct ShowArgs {
    /// Session id
    pub session_id: String,

    /// Emit JSON output
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize)]
struct SessionShowPaths {
    session_dir: String,
    pr_summary_path: String,
    receipt_path: String,
    replay_path: String,
}

#[derive(Serialize)]
struct SessionShowResponse {
    session_id: String,
    paths: SessionShowPaths,
    pr_summary: Option<String>,
    receipt: Option<Value>,
}

pub async fn show(args: ShowArgs) -> Result<()> {
    let paths = OpenAgentsPaths::default();
    let session_dir = paths.session_dir(&args.session_id);
    if !session_dir.exists() {
        anyhow::bail!("Session directory not found: {}", session_dir.display());
    }

    let pr_summary_path = session_dir.join("PR_SUMMARY.md");
    let receipt_path = session_dir.join("RECEIPT.json");
    let replay_path = session_dir.join("REPLAY.jsonl");

    let pr_summary = read_optional_string(&pr_summary_path).context("reading PR_SUMMARY.md")?;
    let receipt = read_optional_json(&receipt_path).context("reading RECEIPT.json")?;

    let response = SessionShowResponse {
        session_id: args.session_id.clone(),
        paths: SessionShowPaths {
            session_dir: session_dir.to_string_lossy().to_string(),
            pr_summary_path: pr_summary_path.to_string_lossy().to_string(),
            receipt_path: receipt_path.to_string_lossy().to_string(),
            replay_path: replay_path.to_string_lossy().to_string(),
        },
        pr_summary,
        receipt,
    };

    if args.json {
        let output = serde_json::to_string_pretty(&response)?;
        println!("{}", output);
        return Ok(());
    }

    print_human(&response);
    Ok(())
}

fn read_optional_string(path: &PathBuf) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(path)?;
    Ok(Some(contents))
}

fn read_optional_json(path: &PathBuf) -> Result<Option<Value>> {
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(path)?;
    let value = serde_json::from_str(&contents)?;
    Ok(Some(value))
}

fn print_human(response: &SessionShowResponse) {
    println!("Session: {}", response.session_id);
    println!("Directory: {}", response.paths.session_dir);
    println!("PR_SUMMARY: {}", response.paths.pr_summary_path);
    println!("RECEIPT: {}", response.paths.receipt_path);
    println!("REPLAY: {}", response.paths.replay_path);
    if response.pr_summary.is_none() {
        println!("PR_SUMMARY.md: missing");
    }
    if response.receipt.is_none() {
        println!("RECEIPT.json: missing or unreadable");
    }
}
