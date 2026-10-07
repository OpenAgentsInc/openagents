use std::{env, process::ExitCode};

use psionic_train::write_content_addressed_artifact_exchange_contract;

fn main() -> ExitCode {
    let mut args = env::args_os();
    let _program = args.next();
    let output_path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin content_addressed_artifact_exchange_contract -- <output-path>"
            );
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = write_content_addressed_artifact_exchange_contract(&output_path) {
        eprintln!("failed to write content-addressed artifact exchange contract: {error}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
