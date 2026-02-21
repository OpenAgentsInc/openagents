use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::Parser;
use openagents_runtime_service::shadow::{
    GateDecision, ShadowGatePolicy, generate_parity_report, load_manifest, write_report,
};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "runtime-shadow-harness")]
#[command(about = "Runtime shadow parity diff and cutover gate harness")]
struct Args {
    #[arg(long)]
    legacy_manifest: PathBuf,
    #[arg(long)]
    rust_manifest: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t = 0)]
    max_warnings: u64,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    block_on_critical: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,openagents_runtime_service=debug")),
        )
        .with_current_span(true)
        .init();

    let args = Args::parse();
    let policy = ShadowGatePolicy {
        max_warning_count: args.max_warnings,
        block_on_critical: args.block_on_critical,
    };

    let legacy_manifest = load_manifest(&args.legacy_manifest)?;
    let rust_manifest = load_manifest(&args.rust_manifest)?;
    let report = generate_parity_report(
        &legacy_manifest,
        &rust_manifest,
        &policy,
        &args.legacy_manifest,
        &args.rust_manifest,
    )?;
    write_report(&args.output, &report)?;

    info!(
        gate_decision = ?report.gate.decision,
        critical_diffs = report.totals.critical,
        warning_diffs = report.totals.warning,
        output = %args.output.display(),
        "shadow parity report generated"
    );

    if report.gate.decision == GateDecision::Block {
        return Err(anyhow!(format!(
            "shadow gate blocked cutover: {}",
            report.gate.reason
        )));
    }
    Ok(())
}
