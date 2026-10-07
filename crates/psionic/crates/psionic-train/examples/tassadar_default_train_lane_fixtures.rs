use std::{path::PathBuf, process::ExitCode};

use psionic_train::write_builtin_tassadar_default_train_lane_contract;

fn main() -> ExitCode {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    match write_builtin_tassadar_default_train_lane_contract(workspace_root.as_path()) {
        Ok(contract) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&contract)
                    .expect("tassadar default-train lane contract should serialize"),
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to write Tassadar default-train lane contract: {error}");
            ExitCode::FAILURE
        }
    }
}
