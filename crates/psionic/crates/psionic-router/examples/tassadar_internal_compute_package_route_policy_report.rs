use psionic_router::{
    tassadar_internal_compute_package_route_policy_report_path,
    write_tassadar_internal_compute_package_route_policy_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_internal_compute_package_route_policy_report_path();
    let report = write_tassadar_internal_compute_package_route_policy_report(&output_path)?;
    println!(
        "wrote internal-compute package route-policy report to {} ({})",
        output_path.display(),
        report.report_id
    );
    Ok(())
}
