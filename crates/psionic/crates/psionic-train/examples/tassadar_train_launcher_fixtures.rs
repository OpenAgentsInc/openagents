use std::{path::PathBuf, process::ExitCode};

use psionic_train::write_tassadar_train_launcher_fixtures;

fn main() -> ExitCode {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    match write_tassadar_train_launcher_fixtures(workspace_root.as_path()) {
        Ok(output) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&output.retained_summary)
                    .expect("tassadar launcher retained summary should serialize"),
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to write Tassadar launcher fixtures: {error}");
            ExitCode::FAILURE
        }
    }
}
