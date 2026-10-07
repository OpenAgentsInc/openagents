use std::path::PathBuf;

use psionic_train::write_builtin_executor_continue_restart_policy_packet;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn main() {
    let root = workspace_root();
    let packet = write_builtin_executor_continue_restart_policy_packet(root.as_path())
        .expect("write continue restart policy fixture");
    println!(
        "wrote {} ({})",
        psionic_train::PSION_EXECUTOR_CONTINUE_RESTART_POLICY_FIXTURE_PATH,
        packet.packet_digest
    );
}
