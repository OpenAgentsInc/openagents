use std::{env, path::PathBuf, process::ExitCode};

use psionic_train::{
    TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR, TASSADAR_EXECUTOR_SUDOKU_9X9_SCALE_PLAN_OUTPUT_DIR,
    write_tassadar_sudoku_9x9_scale_plan,
};

fn main() -> ExitCode {
    let output_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TASSADAR_EXECUTOR_SUDOKU_9X9_SCALE_PLAN_OUTPUT_DIR));
    let baseline_run_dir = PathBuf::from(TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR);

    match write_tassadar_sudoku_9x9_scale_plan(&output_dir, &baseline_run_dir) {
        Ok(plan) => {
            println!(
                "wrote Tassadar 9x9 scale plan `{}` to {}",
                plan.model_id,
                output_dir.display()
            );
            println!("plan_digest={}", plan.plan_digest);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to write Tassadar 9x9 scale plan: {error}");
            ExitCode::FAILURE
        }
    }
}
