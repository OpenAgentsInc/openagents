use std::path::PathBuf;

use psionic_train::write_builtin_executor_unified_throughput_reporting_packet;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn main() {
    let root = workspace_root();
    let packet = write_builtin_executor_unified_throughput_reporting_packet(root.as_path())
        .expect("write unified throughput packet");
    println!(
        "wrote {} ({})",
        psionic_train::PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_FIXTURE_PATH,
        packet.report_digest
    );
}
