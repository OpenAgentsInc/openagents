use psionic_eval::{
    tassadar_broad_family_specialization_report_path,
    write_tassadar_broad_family_specialization_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_broad_family_specialization_report_path();
    let report = write_tassadar_broad_family_specialization_report(&output_path)?;
    println!(
        "wrote broad-family specialization report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
