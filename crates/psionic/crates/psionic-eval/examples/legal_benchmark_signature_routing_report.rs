use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;

use psionic_eval::{
    build_legal_benchmark_signature_routing_report, load_legal_benchmark_signature_routing_suite,
    write_legal_benchmark_signature_routing_report,
};

const DEFAULT_SUITE: &str = "fixtures/legal_benchmark/signature_routing/harvey_public_synthetic_signature_routing_suite.json";
const DEFAULT_OUT: &str = "fixtures/legal_benchmark/signature_routing/harvey_public_synthetic_signature_routing_report.json";

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let suite_path = optional_flag(&args, "--suite")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SUITE));
    let output_path = optional_flag(&args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));
    if has_flag(&args, "--help") {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: legal_benchmark_signature_routing_report [--suite <path>] [--out <path>]",
        )));
    }

    let suite = load_legal_benchmark_signature_routing_suite(&suite_path)?;
    let report = build_legal_benchmark_signature_routing_report(&suite)?;
    write_legal_benchmark_signature_routing_report(&output_path, &report)?;
    println!(
        "suite={} fixtures={} selection_bps={} raw_mean_bps={} probe_mean_bps={} delta_bps={} report_hash={}",
        report.suite_id,
        report.summary.fixture_count,
        report.summary.selection_pass_rate_bps,
        report.summary.raw_codex_mean_score_bps,
        report.summary.probe_codex_mean_score_bps,
        report.summary.mean_score_delta_bps,
        report.report_hash
    );
    Ok(())
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn optional_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
}
