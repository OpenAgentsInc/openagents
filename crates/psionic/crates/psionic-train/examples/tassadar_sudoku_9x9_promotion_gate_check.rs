use std::{env, path::PathBuf, process::ExitCode};

use psionic_train::{
    TASSADAR_EXECUTOR_PROMOTION_GATE_REPORT_FILE, TASSADAR_EXECUTOR_SUDOKU_9X9_REFERENCE_RUN_OUTPUT_DIR,
    check_tassadar_sudoku_9x9_promotion_gate_report,
};

fn main() -> ExitCode {
    let input_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TASSADAR_EXECUTOR_SUDOKU_9X9_REFERENCE_RUN_OUTPUT_DIR));
    let report_path = if input_path.is_dir() {
        input_path.join(TASSADAR_EXECUTOR_PROMOTION_GATE_REPORT_FILE)
    } else {
        input_path
    };

    match check_tassadar_sudoku_9x9_promotion_gate_report(&report_path) {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("promotion gate check report should serialize"),
            );
            if report.consistent() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!(
                "failed to check Tassadar Sudoku 9x9 promotion gate report `{}`: {error}",
                report_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
