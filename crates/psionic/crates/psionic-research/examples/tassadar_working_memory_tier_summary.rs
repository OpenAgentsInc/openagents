use psionic_research::{
    tassadar_working_memory_tier_summary_report_path,
    write_tassadar_working_memory_tier_summary_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_working_memory_tier_summary_report_path();
    let report = write_tassadar_working_memory_tier_summary_report(&output_path)?;
    println!(
        "wrote working-memory tier summary to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
