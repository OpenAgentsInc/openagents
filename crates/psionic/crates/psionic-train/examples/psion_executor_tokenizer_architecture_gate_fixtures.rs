use std::path::PathBuf;

use psionic_train::write_builtin_executor_tokenizer_architecture_gate_packet;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn main() {
    let root = workspace_root();
    let packet = write_builtin_executor_tokenizer_architecture_gate_packet(root.as_path())
        .expect("write executor tokenizer/architecture gate fixture");
    println!(
        "wrote {} ({})",
        psionic_train::PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_FIXTURE_PATH,
        packet.packet_digest
    );
}
