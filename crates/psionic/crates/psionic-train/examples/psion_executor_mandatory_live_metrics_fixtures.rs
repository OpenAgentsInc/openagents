use std::path::PathBuf;

use psionic_train::write_builtin_executor_mandatory_live_metrics_packet;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn main() {
    let root = workspace_root();
    let packet = write_builtin_executor_mandatory_live_metrics_packet(root.as_path())
        .expect("write mandatory live metrics fixture");
    println!(
        "wrote {} ({})",
        psionic_train::PSION_EXECUTOR_MANDATORY_LIVE_METRICS_FIXTURE_PATH,
        packet.packet_digest
    );
}
