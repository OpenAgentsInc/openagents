use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;

use psionic_eval::{
    default_legal_benchmark_eval_output_dir, resolve_legal_benchmark_eval_suite_ref,
    run_legal_benchmark_eval_suite, LegalBenchmarkEvalSuiteRunConfig,
};

const DEFAULT_BASE_MODEL: &str = "Qwen/Qwen3.6-27B";
const DEFAULT_ADAPTER: &str = "target/legal/qwen36_27b_sft_smoke/adapter.safetensors";

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let suite = required_flag(&args, "--suite")?;
    let model = optional_flag(&args, "--model").unwrap_or_else(|| String::from(DEFAULT_BASE_MODEL));
    let adapter =
        optional_flag(&args, "--adapter").unwrap_or_else(|| String::from(DEFAULT_ADAPTER));
    let out = optional_flag(&args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_legal_benchmark_eval_output_dir(&suite));
    let suite_path = resolve_legal_benchmark_eval_suite_ref(&suite);
    let report = run_legal_benchmark_eval_suite(&LegalBenchmarkEvalSuiteRunConfig {
        suite_path,
        base_model: model,
        adapter,
        output_dir: out,
        replay_command: args,
    })?;
    println!(
        "suite={} base_score_bps={} adapter_score_bps={} delta_bps={} median_adapter_bps={} report_hash={}",
        report.suite_id,
        report.base_model_result.legal_score_bps,
        report.adapter_result.legal_score_bps,
        report.comparison.score_delta_bps,
        report.adapter_result.median_legal_score_bps,
        report.replay_receipt.report_hash
    );
    Ok(())
}

fn required_flag(args: &[String], flag: &str) -> Result<String, io::Error> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: legal_benchmark_eval_suite --suite <suite-or-path> [--model <base>] [--adapter <adapter>] [--out <dir>]",
            )
        })
}

fn optional_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
}
