use psionic_provider::{
    tassadar_composite_accepted_outcome_template_report_path,
    write_tassadar_composite_accepted_outcome_template_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_composite_accepted_outcome_template_report_path();
    let report = write_tassadar_composite_accepted_outcome_template_report(&output_path)?;
    println!(
        "wrote composite accepted-outcome template report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
