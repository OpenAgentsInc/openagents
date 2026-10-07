use psionic_eval::{
    tassadar_subset_profile_promotion_gate_report_path,
    write_tassadar_subset_profile_promotion_gate_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_subset_profile_promotion_gate_report_path();
    let report = write_tassadar_subset_profile_promotion_gate_report(&output_path)?;
    println!(
        "wrote subset profile promotion gate report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
