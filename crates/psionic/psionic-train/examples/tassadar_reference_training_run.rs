use std::{env, path::PathBuf, process::ExitCode};

use psionic_train::{
    TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR, execute_tassadar_reference_training_run,
};

fn main() -> ExitCode {
    let output_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR));

    match execute_tassadar_reference_training_run(&output_dir) {
        Ok(bundle) => {
            println!(
                "wrote Tassadar reference run `{}` to {}",
                bundle.run_id,
                output_dir.display()
            );
            println!("bundle_digest={}", bundle.bundle_digest);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to execute Tassadar reference run: {error}");
            ExitCode::FAILURE
        }
    }
}
