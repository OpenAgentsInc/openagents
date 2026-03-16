use std::{env, path::PathBuf, process::ExitCode};

use psionic_train::{
    TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR, augment_tassadar_reference_run_with_telemetry,
};

fn main() -> ExitCode {
    let output_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR));

    match augment_tassadar_reference_run_with_telemetry(&output_dir) {
        Ok(bundle) => {
            println!(
                "updated Tassadar telemetry for `{}` at {}",
                bundle.run_id,
                output_dir.display()
            );
            println!("bundle_digest={}", bundle.bundle_digest);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to augment Tassadar telemetry: {error}");
            ExitCode::FAILURE
        }
    }
}
