use std::{env, path::Path, process::ExitCode};

use psionic_train::verify_qwen_legal_pylon_worker_receipt_path;

fn main() -> ExitCode {
    let Some(receipt_path) = env::args().nth(1) else {
        eprintln!("usage: qwen_legal_verify_worker_receipt <receipt.json>");
        return ExitCode::from(2);
    };
    match verify_qwen_legal_pylon_worker_receipt_path(Path::new(receipt_path.as_str())) {
        Ok(verification) => {
            match serde_json::to_string_pretty(&verification) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("failed to serialize receipt verification: {error}");
                    return ExitCode::from(1);
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to verify Pylon worker receipt: {error}");
            ExitCode::from(1)
        }
    }
}
