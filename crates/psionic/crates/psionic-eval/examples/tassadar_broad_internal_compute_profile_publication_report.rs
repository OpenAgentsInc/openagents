use psionic_eval::{
    tassadar_broad_internal_compute_profile_publication_report_path,
    write_tassadar_broad_internal_compute_profile_publication_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_broad_internal_compute_profile_publication_report_path();
    let report = write_tassadar_broad_internal_compute_profile_publication_report(&output_path)?;
    println!(
        "wrote broad internal-compute profile publication report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
