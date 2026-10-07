use psionic_provider::{
    tassadar_accepted_outcome_binding_report_path, write_tassadar_accepted_outcome_binding_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_accepted_outcome_binding_report_path();
    let report = write_tassadar_accepted_outcome_binding_report(&output_path)?;
    println!(
        "wrote accepted-outcome binding report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
