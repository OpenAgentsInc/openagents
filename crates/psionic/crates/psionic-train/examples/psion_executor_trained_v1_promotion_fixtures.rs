use std::path::PathBuf;

use psionic_train::write_builtin_executor_trained_v1_promotion_artifacts;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn main() {
    let root = workspace_root();
    let packet = write_builtin_executor_trained_v1_promotion_artifacts(root.as_path())
        .expect("write executor trained-v1 promotion fixtures");
    println!(
        "wrote {} ({})",
        psionic_train::PSION_EXECUTOR_TRAINED_V1_PROMOTION_FIXTURE_PATH,
        packet.packet_digest
    );
}
