use std::{env, path::PathBuf, process::ExitCode};

use psionic_eval::write_tassadar_benchmark_package_set_summary_report;

const DEFAULT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_benchmark_package_set_summary.json";
const DEFAULT_VERSION: &str = "2026.03.17";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn main() -> ExitCode {
    let output_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join(DEFAULT_REPORT_REF));
    let output_path = if output_path.is_dir() {
        output_path.join("tassadar_benchmark_package_set_summary.json")
    } else {
        output_path
    };

    match write_tassadar_benchmark_package_set_summary_report(&output_path, DEFAULT_VERSION) {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("benchmark package-set summary should serialize"),
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to materialize Tassadar benchmark package set summary `{}`: {error}",
                output_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
