use psionic_eval::{
    tassadar_shared_primitive_transfer_report_path, write_tassadar_shared_primitive_transfer_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_shared_primitive_transfer_report_path();
    let report = write_tassadar_shared_primitive_transfer_report(&output_path)?;
    println!(
        "wrote shared primitive transfer report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
