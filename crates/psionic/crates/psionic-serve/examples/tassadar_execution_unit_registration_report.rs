use psionic_serve::{
    tassadar_execution_unit_registration_report_path,
    write_tassadar_execution_unit_registration_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_execution_unit_registration_report_path();
    let report = write_tassadar_execution_unit_registration_report(&output_path)?;
    println!(
        "wrote execution-unit registration report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
