use std::path::Path;

use psionic_train::{
    TASSADAR_TRACE_FAMILY_COMPARISON_OUTPUT_DIR, TASSADAR_TRACE_FAMILY_COMPARISON_REPORT_FILE,
    execute_tassadar_trace_family_comparison,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new(TASSADAR_TRACE_FAMILY_COMPARISON_OUTPUT_DIR);
    let report = execute_tassadar_trace_family_comparison(output_dir)?;
    println!(
        "wrote {} with digest {}",
        output_dir
            .join(TASSADAR_TRACE_FAMILY_COMPARISON_REPORT_FILE)
            .display(),
        report.report_digest
    );
    Ok(())
}
