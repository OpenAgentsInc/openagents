use psionic_runtime::{
    tassadar_module_trust_isolation_report_path, write_tassadar_module_trust_isolation_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_module_trust_isolation_report_path();
    let report = write_tassadar_module_trust_isolation_report(&output_path)?;
    println!(
        "wrote module trust-isolation report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
