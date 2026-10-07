use std::path::PathBuf;

use psionic_train::write_builtin_executor_local_cluster_dashboard_packet;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn main() {
    let root = workspace_root();
    let packet = write_builtin_executor_local_cluster_dashboard_packet(root.as_path())
        .expect("dashboard fixture should write");
    println!(
        "wrote {} ({})",
        "fixtures/psion/executor/psion_executor_local_cluster_dashboard_v1.json",
        packet.dashboard_digest
    );
}
