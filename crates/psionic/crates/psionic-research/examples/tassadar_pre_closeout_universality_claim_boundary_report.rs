use psionic_research::{
    tassadar_pre_closeout_universality_claim_boundary_report_path,
    write_tassadar_pre_closeout_universality_claim_boundary_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_pre_closeout_universality_claim_boundary_report_path();
    let report = write_tassadar_pre_closeout_universality_claim_boundary_report(&output_path)?;
    println!(
        "wrote pre-closeout universality claim-boundary report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
