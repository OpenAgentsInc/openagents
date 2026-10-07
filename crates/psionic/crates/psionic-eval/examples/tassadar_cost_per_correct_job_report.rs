use psionic_eval::{
    tassadar_cost_per_correct_job_report_path, write_tassadar_cost_per_correct_job_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_cost_per_correct_job_report_path();
    let report = write_tassadar_cost_per_correct_job_report(&output_path)?;
    println!(
        "wrote cost-per-correct report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
