use psionic_eval::{tassadar_trap_exception_report_path, write_tassadar_trap_exception_report};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_trap_exception_report_path();
    let report = write_tassadar_trap_exception_report(&output_path)?;
    println!(
        "wrote trap/exception report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
