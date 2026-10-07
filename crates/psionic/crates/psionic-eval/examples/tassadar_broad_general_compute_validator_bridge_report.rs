use psionic_eval::{
    tassadar_broad_general_compute_validator_bridge_report_path,
    write_tassadar_broad_general_compute_validator_bridge_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_broad_general_compute_validator_bridge_report_path();
    let report = write_tassadar_broad_general_compute_validator_bridge_report(&output_path)?;
    println!(
        "wrote broad general-compute validator bridge report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
