use psionic_router::{
    tassadar_module_overlap_resolution_report_path, write_tassadar_module_overlap_resolution_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_module_overlap_resolution_report_path();
    let report = write_tassadar_module_overlap_resolution_report(&output_path)?;
    println!(
        "wrote module-overlap resolution report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
