use psionic_runtime::{
    tassadar_exception_profile_runtime_report_path, write_tassadar_exception_profile_runtime_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_exception_profile_runtime_report_path();
    let report = write_tassadar_exception_profile_runtime_report(&output_path)?;
    println!(
        "wrote exception-profile runtime report to {} ({})",
        output_path.display(),
        report.report_id
    );
    Ok(())
}
