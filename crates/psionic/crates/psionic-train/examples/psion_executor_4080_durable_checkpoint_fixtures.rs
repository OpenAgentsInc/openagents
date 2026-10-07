use std::path::Path;

use psionic_train::write_builtin_executor_4080_durable_checkpoint_packet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let packet = write_builtin_executor_4080_durable_checkpoint_packet(workspace_root.as_path())?;
    println!(
        "wrote psion executor 4080 durable-checkpoint packet {}",
        packet.packet_digest
    );
    Ok(())
}
