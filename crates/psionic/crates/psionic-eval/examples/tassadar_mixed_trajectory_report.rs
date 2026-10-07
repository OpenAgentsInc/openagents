use psionic_eval::{tassadar_mixed_trajectory_report_path, write_tassadar_mixed_trajectory_report};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_mixed_trajectory_report_path();
    let report = write_tassadar_mixed_trajectory_report(&path)?;
    println!("wrote {} to {}", report.report_id, path.display());
    Ok(())
}
