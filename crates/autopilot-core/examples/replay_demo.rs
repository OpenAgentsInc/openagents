//! Demonstration of replay bundle creation and publishing
//!
//! Usage:
//!   cargo run --example replay_demo <session_log.jsonl> [output.json]

use anyhow::Result;
use autopilot_core::replay::{ReplayBundle, redact_replay};
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <session_log.jsonl> [output.json]", args[0]);
        eprintln!();
        eprintln!("Converts a JSONL session log to a publishable replay bundle.");
        eprintln!();
        eprintln!("Example:");
        eprintln!(
            "  {} ~/.openagents/sessions/20251226/153045-abc123.jsonl demo.json",
            args[0]
        );
        std::process::exit(1);
    }

    let input_path = PathBuf::from(&args[1]);
    let output_path = if args.len() >= 3 {
        PathBuf::from(&args[2])
    } else {
        let filename = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("replay");
        PathBuf::from(format!("{}.json", filename))
    };

    println!("Loading session log: {}", input_path.display());

    // Load replay bundle from JSONL
    let mut bundle = ReplayBundle::from_jsonl(&input_path)?;

    println!("Session ID: {}", bundle.id);
    println!("Duration: {}s", bundle.metadata.duration_seconds);
    println!("Timeline events: {}", bundle.timeline.len());
    println!();

    // Apply redaction
    println!("Applying redaction...");
    redact_replay(&mut bundle)?;

    // Save to output
    println!("Saving to: {}", output_path.display());
    bundle.save(&output_path)?;

    println!();
    println!("✓ Replay bundle created successfully!");
    println!();
    println!("Metadata:");
    println!("  Model: {}", bundle.metadata.model);
    println!(
        "  Duration: {}s real / {}s demo @ {}x",
        bundle.metadata.duration_seconds,
        bundle.metadata.demo_duration_seconds,
        bundle.metadata.playback_speed
    );
    println!();
    println!("Receipts:");
    if let Some(tests) = bundle.receipts.tests_run {
        println!(
            "  Tests: {} run, {} passed",
            tests,
            bundle.receipts.tests_passed.unwrap_or(0)
        );
    }
    if let Some(ci) = &bundle.receipts.ci_status {
        println!("  CI Status: {}", ci);
    }
    println!("  Files changed: {}", bundle.receipts.files_changed);

    Ok(())
}
