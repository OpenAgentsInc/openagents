use psionic_eval::{
    tassadar_state_design_study_report_path, write_tassadar_state_design_study_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_state_design_study_report_path();
    let report = write_tassadar_state_design_study_report(&output_path)?;
    println!(
        "wrote state-design study report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
