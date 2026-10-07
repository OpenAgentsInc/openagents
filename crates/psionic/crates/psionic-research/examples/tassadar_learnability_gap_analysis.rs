use psionic_research::{
    tassadar_learnability_gap_analysis_report_path, write_tassadar_learnability_gap_analysis_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_learnability_gap_analysis_report_path();
    let report = write_tassadar_learnability_gap_analysis_report(&output_path)?;
    println!("wrote {} ({})", output_path.display(), report.report_digest);
    Ok(())
}
