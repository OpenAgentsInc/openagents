use psionic_data::TASSADAR_ERROR_REGIME_SWEEP_OUTPUT_DIR;
use psionic_train::execute_tassadar_error_regime_catalog;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = execute_tassadar_error_regime_catalog(std::path::Path::new(
        TASSADAR_ERROR_REGIME_SWEEP_OUTPUT_DIR,
    ))?;
    println!(
        "wrote error-regime sweep to {}/{} ({})",
        TASSADAR_ERROR_REGIME_SWEEP_OUTPUT_DIR,
        "error_regime_sweep_report.json",
        report.report_digest
    );
    Ok(())
}
