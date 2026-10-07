use std::{env, process::ExitCode};

use psionic_train::write_shadow_window_ramp_contract;

fn main() -> ExitCode {
    let mut args = env::args_os();
    let _program = args.next();
    let output_path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin shadow_window_ramp_contract -- <output-path>"
            );
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = write_shadow_window_ramp_contract(&output_path) {
        eprintln!("failed to write shadow window ramp contract: {error}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
