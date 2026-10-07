use std::{env, process::ExitCode};

use psionic_train::write_a1_minimal_distributed_lm_support_artifact_catalog;

fn main() -> ExitCode {
    let Some(output_path) = env::args().nth(1) else {
        eprintln!(
            "usage: a1_minimal_distributed_lm_support_artifact_catalog_fixture <output-path>"
        );
        return ExitCode::from(2);
    };
    match write_a1_minimal_distributed_lm_support_artifact_catalog(output_path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
