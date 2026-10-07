use std::{env, path::PathBuf, process::ExitCode};

use psionic_research::{
    TASSADAR_TRAINABLE_SURFACE_ABLATION_OUTPUT_DIR, run_tassadar_trainable_surface_ablation,
};

fn main() -> ExitCode {
    let output_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TASSADAR_TRAINABLE_SURFACE_ABLATION_OUTPUT_DIR));

    match run_tassadar_trainable_surface_ablation(&output_dir) {
        Ok(report) => {
            println!(
                "wrote Tassadar trainable-surface ablation to {}",
                output_dir.display()
            );
            println!("report_digest={}", report.report_digest);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to execute Tassadar trainable-surface ablation: {error}");
            ExitCode::FAILURE
        }
    }
}
