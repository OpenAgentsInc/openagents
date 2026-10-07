use std::path::Path;

use psionic_research::{
    run_tassadar_trace_family_variant_report, TASSADAR_TRACE_FAMILY_VARIANT_REPORT_FILE,
    TASSADAR_TRACE_FAMILY_VARIANT_REPORT_OUTPUT_DIR,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_tassadar_trace_family_variant_report(Path::new(
        TASSADAR_TRACE_FAMILY_VARIANT_REPORT_OUTPUT_DIR,
    ))?;
    println!(
        "wrote {}/{} ({})",
        TASSADAR_TRACE_FAMILY_VARIANT_REPORT_OUTPUT_DIR,
        TASSADAR_TRACE_FAMILY_VARIANT_REPORT_FILE,
        report.report_digest
    );
    Ok(())
}
