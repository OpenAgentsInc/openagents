use std::path::Path;

use psionic_train::{
    run_tassadar_scratchpad_framework_comparison_report, TASSADAR_SCRATCHPAD_FRAMEWORK_OUTPUT_DIR,
    TASSADAR_SCRATCHPAD_FRAMEWORK_REPORT_FILE,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_tassadar_scratchpad_framework_comparison_report(Path::new(
        TASSADAR_SCRATCHPAD_FRAMEWORK_OUTPUT_DIR,
    ))?;
    println!(
        "wrote {}/{} ({})",
        TASSADAR_SCRATCHPAD_FRAMEWORK_OUTPUT_DIR,
        TASSADAR_SCRATCHPAD_FRAMEWORK_REPORT_FILE,
        report.report_digest
    );
    Ok(())
}
