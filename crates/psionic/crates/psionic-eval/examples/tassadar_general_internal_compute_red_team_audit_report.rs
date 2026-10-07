use psionic_eval::{
    tassadar_general_internal_compute_red_team_audit_report_path,
    write_tassadar_general_internal_compute_red_team_audit_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_general_internal_compute_red_team_audit_report_path();
    let report = write_tassadar_general_internal_compute_red_team_audit_report(&output_path)?;
    println!(
        "wrote general internal-compute red-team audit report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
