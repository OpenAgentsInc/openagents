use std::{env, path::PathBuf};

use psionic_train::write_first_swarm_run_contract;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/first_swarm_run_contract.json"));
    let contract = write_first_swarm_run_contract(&output_path)?;
    println!(
        "wrote {} with contract_digest {}",
        output_path.display(),
        contract.contract_digest
    );
    Ok(())
}
