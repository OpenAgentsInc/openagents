use std::{env, path::PathBuf, process::ExitCode};

use psionic_research::{
    TASSADAR_ACCEPTANCE_REPORT_FILE, tassadar_acceptance_report_path,
    write_tassadar_acceptance_report,
};

fn main() -> ExitCode {
    let output_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(tassadar_acceptance_report_path);
    let output_path = if output_path.is_dir() {
        output_path.join(TASSADAR_ACCEPTANCE_REPORT_FILE)
    } else {
        output_path
    };

    match write_tassadar_acceptance_report(&output_path) {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("tassadar acceptance report should serialize"),
            );
            if report.current_truth_holds() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!(
                "failed to write Tassadar acceptance report `{}`: {error}",
                output_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
