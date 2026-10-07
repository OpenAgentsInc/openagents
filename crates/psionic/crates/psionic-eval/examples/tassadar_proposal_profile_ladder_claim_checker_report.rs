use psionic_eval::{
    tassadar_proposal_profile_ladder_claim_checker_report_path,
    write_tassadar_proposal_profile_ladder_claim_checker_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_proposal_profile_ladder_claim_checker_report_path();
    let report = write_tassadar_proposal_profile_ladder_claim_checker_report(&output_path)?;
    println!(
        "wrote proposal-profile ladder claim checker report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
