use std::path::PathBuf;

use psionic_train::{
    write_builtin_executor_research_branch_packet, PSION_EXECUTOR_RESEARCH_BRANCH_FIXTURE_PATH,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root();
    let packet = write_builtin_executor_research_branch_packet(root.as_path())?;
    println!(
        "wrote {} ({})",
        PSION_EXECUTOR_RESEARCH_BRANCH_FIXTURE_PATH, packet.packet_digest
    );
    Ok(())
}
