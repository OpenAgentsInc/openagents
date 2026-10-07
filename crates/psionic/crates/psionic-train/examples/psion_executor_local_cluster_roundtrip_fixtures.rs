use psionic_train::{
    write_builtin_executor_local_cluster_roundtrip_packet,
    PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_FIXTURE_PATH,
};

fn main() {
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf();
    let packet = write_builtin_executor_local_cluster_roundtrip_packet(workspace_root.as_path())
        .expect("write local cluster roundtrip packet fixture");
    println!(
        "wrote {} ({})",
        PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_FIXTURE_PATH,
        packet.packet_digest
    );
}
