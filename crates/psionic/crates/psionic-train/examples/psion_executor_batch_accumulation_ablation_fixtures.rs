use std::path::PathBuf;

use psionic_train::write_builtin_executor_batch_accumulation_ablation_packet;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn main() {
    let root = workspace_root();
    let packet = write_builtin_executor_batch_accumulation_ablation_packet(root.as_path())
        .expect("write executor batch/accumulation ablation fixture");
    println!(
        "wrote {} ({})",
        psionic_train::PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_FIXTURE_PATH,
        packet.packet_digest
    );
}
