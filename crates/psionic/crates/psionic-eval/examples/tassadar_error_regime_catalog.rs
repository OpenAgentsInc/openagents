use psionic_eval::{
    tassadar_error_regime_catalog_report_path, write_tassadar_error_regime_catalog_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_error_regime_catalog_report_path();
    let report = write_tassadar_error_regime_catalog_report(&output_path)?;
    println!(
        "wrote error-regime catalog to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
