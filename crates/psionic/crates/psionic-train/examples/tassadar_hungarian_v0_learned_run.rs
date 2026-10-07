use std::{env, path::PathBuf, process::ExitCode};

use psionic_train::{
    TASSADAR_EXECUTOR_HUNGARIAN_V0_LEARNED_OUTPUT_DIR, execute_tassadar_hungarian_v0_learned_run,
};

fn main() -> ExitCode {
    let output_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TASSADAR_EXECUTOR_HUNGARIAN_V0_LEARNED_OUTPUT_DIR));

    match execute_tassadar_hungarian_v0_learned_run(&output_dir) {
        Ok(bundle) => {
            println!("{}", output_dir.display());
            println!("{}", serde_json::to_string_pretty(&bundle).expect("bundle should serialize"));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to execute learned Hungarian-v0 run in `{}`: {error}",
                output_dir.display()
            );
            ExitCode::FAILURE
        }
    }
}
