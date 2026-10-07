use psionic_eval::{
    tassadar_effective_unbounded_compute_claim_report_path,
    write_tassadar_effective_unbounded_compute_claim_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_effective_unbounded_compute_claim_report_path();
    let report = write_tassadar_effective_unbounded_compute_claim_report(&output_path)?;
    println!(
        "wrote effective-unbounded claim report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
