use std::{env, path::PathBuf, process::ExitCode};

use psionic_train::{
    TASSADAR_EXECUTOR_WINDOWED_FAMILY_COMPARISON_OUTPUT_DIR,
    execute_tassadar_sudoku_9x9_windowed_family_comparison,
};

fn main() -> ExitCode {
    let output_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TASSADAR_EXECUTOR_WINDOWED_FAMILY_COMPARISON_OUTPUT_DIR));

    match execute_tassadar_sudoku_9x9_windowed_family_comparison(&output_dir) {
        Ok(report) => {
            println!(
                "wrote Tassadar 9x9 windowed family comparison to {}",
                output_dir.display()
            );
            println!("report_digest={}", report.report_digest);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to execute Tassadar 9x9 windowed family comparison: {error}");
            ExitCode::FAILURE
        }
    }
}
