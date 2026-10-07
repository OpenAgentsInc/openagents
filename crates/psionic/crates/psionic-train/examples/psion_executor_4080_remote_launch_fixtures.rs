use std::path::PathBuf;

use psionic_train::write_builtin_executor_4080_remote_launch_packet;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let packet = write_builtin_executor_4080_remote_launch_packet(workspace_root().as_path())?;
    println!(
        "wrote psion executor 4080 remote-launch packet {}",
        packet.packet_digest
    );
    Ok(())
}
