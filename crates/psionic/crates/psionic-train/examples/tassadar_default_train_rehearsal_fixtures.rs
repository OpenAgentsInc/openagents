use std::{path::PathBuf, process::ExitCode};

use psionic_train::write_tassadar_default_train_rehearsal_fixtures;

fn main() -> ExitCode {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    match write_tassadar_default_train_rehearsal_fixtures(workspace_root.as_path()) {
        Ok(bundle) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&bundle)
                    .expect("default-train rehearsal bundle should serialize"),
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to write Tassadar default-train rehearsal fixtures: {error}");
            ExitCode::FAILURE
        }
    }
}
