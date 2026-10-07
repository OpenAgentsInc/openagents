use psionic_provider::{
    tassadar_exact_compute_market_report_path, write_tassadar_exact_compute_market_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_exact_compute_market_report_path();
    let report = write_tassadar_exact_compute_market_report(&output_path)?;
    println!(
        "wrote exact-compute market report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
