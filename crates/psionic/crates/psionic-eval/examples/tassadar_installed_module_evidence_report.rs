use psionic_eval::{
    tassadar_installed_module_evidence_report_path, write_tassadar_installed_module_evidence_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_installed_module_evidence_report_path();
    let report = write_tassadar_installed_module_evidence_report(&output_path)?;
    println!(
        "wrote installed-module evidence report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
