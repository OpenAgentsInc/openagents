use std::{env, path::PathBuf};

use psionic_train::{
    write_first_swarm_open_adapter_receipt_contract,
    FIRST_SWARM_OPEN_ADAPTER_RECEIPT_CONTRACT_FIXTURE_PATH,
};

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
        .unwrap_or_else(|| PathBuf::from(FIRST_SWARM_OPEN_ADAPTER_RECEIPT_CONTRACT_FIXTURE_PATH));
    let contract = write_first_swarm_open_adapter_receipt_contract(&output_path)?;
    println!(
        "wrote {} with contract {}",
        output_path.display(),
        contract.contract_digest
    );
    Ok(())
}
