use std::{env, process::ExitCode};

use psionic_collectives::write_collective_failure_semantics_contract;

fn main() -> ExitCode {
    let mut args = env::args_os();
    let _program = args.next();
    let output_path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!(
                "usage: cargo run -p psionic-collectives --bin collective_failure_semantics_contract -- <output-path>"
            );
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = write_collective_failure_semantics_contract(&output_path) {
        eprintln!("failed to write collective failure semantics contract: {error}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
