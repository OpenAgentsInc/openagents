use psionic_router::{
    tassadar_planner_language_compute_policy_report_path,
    write_tassadar_planner_language_compute_policy_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_planner_language_compute_policy_report_path();
    let report = write_tassadar_planner_language_compute_policy_report(&path)?;
    println!("wrote {} to {}", report.report_id, path.display());
    Ok(())
}
