use std::process::ExitCode;

use psionic_runtime::{
    tassadar_c_to_wasm_compile_receipt_path, write_tassadar_c_to_wasm_compile_receipt,
};

fn main() -> ExitCode {
    let output_path = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(tassadar_c_to_wasm_compile_receipt_path);

    match write_tassadar_c_to_wasm_compile_receipt(&output_path) {
        Ok(receipt) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&receipt)
                    .expect("Tassadar C-to-Wasm compile receipt should serialize"),
            );
            if receipt.succeeded() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!(
                "failed to write Tassadar C-to-Wasm compile receipt `{}`: {error}",
                output_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
