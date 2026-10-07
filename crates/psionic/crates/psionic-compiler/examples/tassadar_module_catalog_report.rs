use psionic_compiler::{tassadar_module_catalog_report_path, write_tassadar_module_catalog_report};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_module_catalog_report_path();
    let report = write_tassadar_module_catalog_report(&output_path)?;
    println!(
        "wrote module-catalog report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
