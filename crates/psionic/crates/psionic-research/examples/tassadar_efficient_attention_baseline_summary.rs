use std::path::Path;

use psionic_research::{
    run_tassadar_efficient_attention_baseline_summary_report,
    TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_OUTPUT_DIR,
    TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_REPORT_FILE,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_tassadar_efficient_attention_baseline_summary_report(Path::new(
        TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_OUTPUT_DIR,
    ))?;
    println!(
        "wrote {}/{} ({})",
        TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_OUTPUT_DIR,
        TASSADAR_EFFICIENT_ATTENTION_BASELINE_SUMMARY_REPORT_FILE,
        report.report_digest
    );
    Ok(())
}
