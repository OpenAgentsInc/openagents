#![allow(
    clippy::print_stdout,
    reason = "CLI bench intentionally prints the serialized throughput report."
)]

use anyhow::Result;
use autopilot_desktop::logging;
use autopilot_desktop::throughput_bench::run_default_throughput_bench;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "autopilot-throughput-bench")]
#[command(about = "Measure targeted buyer/provider throughput over NIP-28 + NIP-90 lanes")]
struct Cli {
    #[arg(long)]
    provider_compute_ms: Option<u64>,
}

fn main() -> Result<()> {
    logging::init();
    let cli = Cli::parse();
    let report = run_default_throughput_bench(cli.provider_compute_ms)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
