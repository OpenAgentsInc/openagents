use std::path::Path;

use psionic_train::{
    write_executor_mixture_search_cadence_packet,
    PSION_EXECUTOR_MIXTURE_SEARCH_CADENCE_FIXTURE_PATH,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let packet = write_executor_mixture_search_cadence_packet(&workspace_root)?;
    println!(
        "wrote {} ({})",
        PSION_EXECUTOR_MIXTURE_SEARCH_CADENCE_FIXTURE_PATH,
        packet.packet_digest
    );
    Ok(())
}
