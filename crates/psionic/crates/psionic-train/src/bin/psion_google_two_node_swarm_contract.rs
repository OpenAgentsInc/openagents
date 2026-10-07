use std::{env, error::Error, path::PathBuf};

use psionic_train::{
    write_psion_google_two_node_swarm_contract, PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_FIXTURE_PATH,
};

fn main() -> Result<(), Box<dyn Error>> {
    let output_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_FIXTURE_PATH));
    let contract = write_psion_google_two_node_swarm_contract(&output_path)?;
    println!("{}", serde_json::to_string_pretty(&contract)?);
    Ok(())
}
