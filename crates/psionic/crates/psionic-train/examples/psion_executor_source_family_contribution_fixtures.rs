use std::path::Path;

use psionic_train::{
    write_executor_source_family_contribution_report,
    PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH,
};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report = write_executor_source_family_contribution_report(root)
        .expect("write source-family contribution report");
    println!(
        "wrote {} ({})",
        PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH,
        report.report_digest
    );
}
