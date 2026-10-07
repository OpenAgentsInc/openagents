use psionic_research::{
    tassadar_shared_primitive_transfer_summary_report_path,
    write_tassadar_shared_primitive_transfer_summary_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_shared_primitive_transfer_summary_report_path();
    let report = write_tassadar_shared_primitive_transfer_summary_report(&output_path)?;
    println!(
        "wrote shared primitive transfer summary to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
