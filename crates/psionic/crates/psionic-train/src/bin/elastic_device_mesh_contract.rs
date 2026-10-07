use std::{env, process::ExitCode};

use psionic_train::write_elastic_device_mesh_contract;

fn main() -> ExitCode {
    let mut args = env::args_os();
    let _program = args.next();
    let output_path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin elastic_device_mesh_contract -- <output-path>"
            );
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = write_elastic_device_mesh_contract(&output_path) {
        eprintln!("failed to write elastic device mesh contract: {error}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
