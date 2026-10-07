use psionic_runtime::{
    tassadar_broad_family_specialization_runtime_report_path,
    write_tassadar_broad_family_specialization_runtime_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_broad_family_specialization_runtime_report_path();
    let report = write_tassadar_broad_family_specialization_runtime_report(&output_path)?;
    println!(
        "wrote broad-family specialization runtime report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
