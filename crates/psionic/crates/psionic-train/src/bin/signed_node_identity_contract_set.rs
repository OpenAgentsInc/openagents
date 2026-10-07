use std::{env, process::ExitCode};

use psionic_train::write_signed_node_identity_contract_set;

fn main() -> ExitCode {
    let mut args = env::args_os();
    let _program = args.next();
    let output_path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!(
                "usage: cargo run -p psionic-train --bin signed_node_identity_contract_set -- <output-path>"
            );
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = write_signed_node_identity_contract_set(&output_path) {
        eprintln!("failed to write signed node identity contract set: {error}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
