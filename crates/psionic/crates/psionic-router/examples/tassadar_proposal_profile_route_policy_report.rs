use psionic_router::{
    tassadar_proposal_profile_route_policy_report_path,
    write_tassadar_proposal_profile_route_policy_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_proposal_profile_route_policy_report_path();
    let report = write_tassadar_proposal_profile_route_policy_report(&output_path)?;
    println!(
        "wrote proposal-profile route policy report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
