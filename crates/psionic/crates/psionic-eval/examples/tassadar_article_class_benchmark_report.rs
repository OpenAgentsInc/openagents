use std::{env, path::PathBuf, process::ExitCode};

use psionic_eval::run_tassadar_article_class_benchmark;

const DEFAULT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json";
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
        output_path.join("tassadar_article_class_benchmark_report.json")
    } else {
        output_path
    };

    match run_tassadar_article_class_benchmark(DEFAULT_VERSION) {
        Ok(report) => {
            if let Some(parent) = output_path.parent() {
                if let Err(error) = std::fs::create_dir_all(parent) {
                    eprintln!(
                        "failed to create Tassadar benchmark report dir `{}`: {error}",
                        parent.display()
                    );
                    return ExitCode::FAILURE;
                }
            }
            match serde_json::to_vec_pretty(&report) {
                Ok(bytes) => {
                    if let Err(error) = std::fs::write(&output_path, bytes) {
                        eprintln!(
                            "failed to write Tassadar benchmark report `{}`: {error}",
                            output_path.display()
                        );
                        return ExitCode::FAILURE;
                    }
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report)
                            .expect("Tassadar benchmark report should serialize"),
                    );
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("failed to serialize Tassadar benchmark report: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        Err(error) => {
            eprintln!("failed to run Tassadar article-class benchmark: {error}");
            ExitCode::FAILURE
        }
    }
}
