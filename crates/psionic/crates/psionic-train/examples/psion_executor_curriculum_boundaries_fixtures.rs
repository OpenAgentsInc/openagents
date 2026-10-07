use std::path::Path;

use psionic_train::{
    write_executor_curriculum_boundaries_fixture,
    PSION_EXECUTOR_CURRICULUM_BOUNDARIES_FIXTURE_PATH,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let packet = write_executor_curriculum_boundaries_fixture(&workspace_root)?;
    println!(
        "wrote {} ({})",
        PSION_EXECUTOR_CURRICULUM_BOUNDARIES_FIXTURE_PATH,
        packet.packet_digest
    );
    Ok(())
}
