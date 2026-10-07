use psionic_runtime::{
    tassadar_state_design_runtime_report_path, write_tassadar_state_design_runtime_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_state_design_runtime_report_path();
    let report = write_tassadar_state_design_runtime_report(&output_path)?;
    println!(
        "wrote state-design runtime report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
