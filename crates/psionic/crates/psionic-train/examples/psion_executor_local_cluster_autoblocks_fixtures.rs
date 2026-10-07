use std::path::PathBuf;

use psionic_train::write_builtin_executor_local_cluster_autoblocks_report;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn main() {
    let root = workspace_root();
    let report = write_builtin_executor_local_cluster_autoblocks_report(root.as_path())
        .expect("autoblock fixture should write");
    println!(
        "wrote {} ({})",
        "fixtures/psion/executor/psion_executor_local_cluster_autoblocks_v1.json",
        report.report_digest
    );
}
