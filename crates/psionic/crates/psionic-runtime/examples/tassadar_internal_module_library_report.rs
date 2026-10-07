use psionic_runtime::{
    tassadar_internal_module_library_report_path, write_tassadar_internal_module_library_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_internal_module_library_report_path();
    let report = write_tassadar_internal_module_library_report(&output_path)?;
    println!(
        "wrote internal module library report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
