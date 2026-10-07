use psionic_research::{
    tassadar_error_regime_summary_report_path, write_tassadar_error_regime_summary_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_error_regime_summary_report_path();
    let report = write_tassadar_error_regime_summary_report(&output_path)?;
    println!(
        "wrote error-regime summary to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
