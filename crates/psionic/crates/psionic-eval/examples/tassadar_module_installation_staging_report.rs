use psionic_eval::{
    tassadar_module_installation_staging_report_path,
    write_tassadar_module_installation_staging_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_module_installation_staging_report_path();
    let report = write_tassadar_module_installation_staging_report(&output_path)?;
    println!(
        "wrote module installation staging report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
