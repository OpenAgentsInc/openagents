use psionic_sandbox::{
    tassadar_import_policy_matrix_report_path, write_tassadar_import_policy_matrix_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_import_policy_matrix_report_path();
    let report = write_tassadar_import_policy_matrix_report(&output_path)?;
    println!(
        "wrote import-policy matrix report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
