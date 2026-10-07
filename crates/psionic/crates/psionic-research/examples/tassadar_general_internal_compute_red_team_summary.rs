use psionic_research::{
    tassadar_general_internal_compute_red_team_summary_path,
    write_tassadar_general_internal_compute_red_team_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_general_internal_compute_red_team_summary_path();
    let report = write_tassadar_general_internal_compute_red_team_summary(&output_path)?;
    println!(
        "wrote general internal-compute red-team summary to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
