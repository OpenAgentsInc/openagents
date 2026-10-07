use psionic_research::{
    tassadar_broad_family_specialization_summary_path,
    write_tassadar_broad_family_specialization_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_broad_family_specialization_summary_path();
    let summary = write_tassadar_broad_family_specialization_summary(&output_path)?;
    println!(
        "wrote broad-family specialization summary to {} ({})",
        output_path.display(),
        summary.report_digest
    );
    Ok(())
}
