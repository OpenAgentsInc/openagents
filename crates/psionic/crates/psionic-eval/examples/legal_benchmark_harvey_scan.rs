use std::env;
use std::process::ExitCode;

use psionic_eval::{harvey_corpus_summary_json, scan_harvey_corpus};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(tasks_root) = args.next() else {
        eprintln!("usage: legal_benchmark_harvey_scan <harvey-tasks-root> <upstream-commit>");
        return ExitCode::from(2);
    };
    let Some(upstream_commit) = args.next() else {
        eprintln!("usage: legal_benchmark_harvey_scan <harvey-tasks-root> <upstream-commit>");
        return ExitCode::from(2);
    };

    match scan_harvey_corpus(tasks_root, upstream_commit) {
        Ok(scan) => match serde_json::to_string_pretty(&harvey_corpus_summary_json(&scan)) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("failed to encode scan summary: {error}");
                ExitCode::from(1)
            }
        },
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
