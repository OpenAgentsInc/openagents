use std::{env, process::ExitCode};

use psionic_train::write_incentivized_decentralized_run_contract;

fn main() -> ExitCode {
    let mut args = env::args_os();
    let _program = args.next();
    let output_path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin incentivized_decentralized_run_contract -- <output-path>"
            );
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = write_incentivized_decentralized_run_contract(&output_path) {
        eprintln!("failed to write incentivized decentralized run contract: {error}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
