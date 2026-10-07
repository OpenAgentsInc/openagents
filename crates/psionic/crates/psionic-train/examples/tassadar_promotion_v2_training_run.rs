use std::{env, path::PathBuf, process::ExitCode};

use psionic_train::{
    TASSADAR_EXECUTOR_PROMOTION_V2_RUN_OUTPUT_DIR,
    execute_tassadar_promotion_v2_training_run_with_artifacts,
};

fn main() -> ExitCode {
    let output_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TASSADAR_EXECUTOR_PROMOTION_V2_RUN_OUTPUT_DIR));
    eprintln!(
        "starting Tassadar promotion v2 run at {} (set OPENAGENTS_TASSADAR_PROGRESS=0 to mute progress output)",
        output_dir.display()
    );

    match execute_tassadar_promotion_v2_training_run_with_artifacts(&output_dir) {
        Ok(bundle) => {
            println!(
                "wrote Tassadar promotion v2 run `{}` to {}",
                bundle.run_id,
                output_dir.display()
            );
            println!("bundle_digest={}", bundle.bundle_digest);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to execute Tassadar promotion v2 run: {error}");
            ExitCode::FAILURE
        }
    }
}
