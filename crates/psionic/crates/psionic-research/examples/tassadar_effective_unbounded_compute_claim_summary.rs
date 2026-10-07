use psionic_research::{
    tassadar_effective_unbounded_compute_claim_summary_path,
    write_tassadar_effective_unbounded_compute_claim_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_effective_unbounded_compute_claim_summary_path();
    let summary = write_tassadar_effective_unbounded_compute_claim_summary(&output_path)?;
    println!(
        "wrote effective-unbounded claim summary to {} ({})",
        output_path.display(),
        summary.report_digest
    );
    Ok(())
}
