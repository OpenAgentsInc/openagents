use psionic_compiler::{
    tassadar_cross_profile_link_compatibility_report_path,
    write_tassadar_cross_profile_link_compatibility_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_cross_profile_link_compatibility_report_path();
    let report = write_tassadar_cross_profile_link_compatibility_report(&output_path)?;
    println!(
        "wrote cross-profile link compatibility report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
