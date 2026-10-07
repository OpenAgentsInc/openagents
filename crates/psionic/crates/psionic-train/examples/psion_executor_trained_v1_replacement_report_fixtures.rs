use std::path::PathBuf;

use psionic_train::write_builtin_executor_trained_v1_replacement_report;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn main() {
    let root = workspace_root();
    let report = write_builtin_executor_trained_v1_replacement_report(root.as_path())
        .expect("write executor trained-v1 replacement report fixture");
    println!(
        "wrote {} ({})",
        psionic_train::PSION_EXECUTOR_TRAINED_V1_REPLACEMENT_REPORT_FIXTURE_PATH,
        report.report_digest
    );
}
