use psionic_router::{
    tassadar_evidence_calibrated_routing_report_path,
    write_tassadar_evidence_calibrated_routing_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_evidence_calibrated_routing_report_path();
    let report = write_tassadar_evidence_calibrated_routing_report(&path)?;
    println!("wrote {} to {}", report.report_id, path.display());
    Ok(())
}
