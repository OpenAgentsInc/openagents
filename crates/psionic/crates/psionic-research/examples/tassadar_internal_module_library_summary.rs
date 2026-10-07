use psionic_research::{
    tassadar_internal_module_library_summary_report_path,
    write_tassadar_internal_module_library_summary_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_internal_module_library_summary_report_path();
    let report = write_tassadar_internal_module_library_summary_report(&output_path)?;
    println!(
        "wrote internal module library summary to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
