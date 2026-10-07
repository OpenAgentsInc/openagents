use std::path::Path;

use psionic_research::{
    run_tassadar_sparse_rule_compiler_audit, TASSADAR_SPARSE_RULE_COMPILER_AUDIT_OUTPUT_DIR,
    TASSADAR_SPARSE_RULE_COMPILER_AUDIT_REPORT_FILE,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_tassadar_sparse_rule_compiler_audit(Path::new(
        TASSADAR_SPARSE_RULE_COMPILER_AUDIT_OUTPUT_DIR,
    ))?;
    println!(
        "wrote {}/{} ({})",
        TASSADAR_SPARSE_RULE_COMPILER_AUDIT_OUTPUT_DIR,
        TASSADAR_SPARSE_RULE_COMPILER_AUDIT_REPORT_FILE,
        report.report_digest
    );
    Ok(())
}
