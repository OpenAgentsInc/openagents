use std::{env, path::PathBuf, process::ExitCode};

use psionic_train::{
    TASSADAR_EXECUTOR_SUDOKU_9X9_REFERENCE_RUN_OUTPUT_DIR,
    augment_tassadar_sudoku_9x9_run_with_promotion_artifacts,
};

fn main() -> ExitCode {
    let output_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TASSADAR_EXECUTOR_SUDOKU_9X9_REFERENCE_RUN_OUTPUT_DIR));

    match augment_tassadar_sudoku_9x9_run_with_promotion_artifacts(&output_dir) {
        Ok(bundle) => {
            println!(
                "{}",
                output_dir.join("promotion_bundle.json").display()
            );
            println!("{}", serde_json::to_string_pretty(&bundle).expect("bundle should serialize"));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to augment Tassadar Sudoku 9x9 promotion artifacts in `{}`: {error}",
                output_dir.display()
            );
            ExitCode::FAILURE
        }
    }
}
