use psionic_router::{
    tassadar_general_internal_compute_red_team_route_exercises_report_path,
    write_tassadar_general_internal_compute_red_team_route_exercises_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_general_internal_compute_red_team_route_exercises_report_path();
    let report =
        write_tassadar_general_internal_compute_red_team_route_exercises_report(&output_path)?;
    println!(
        "wrote general internal-compute red-team route exercises report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
