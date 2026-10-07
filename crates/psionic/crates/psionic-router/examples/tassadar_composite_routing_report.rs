use psionic_router::{
    tassadar_composite_routing_report_path, write_tassadar_composite_routing_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_composite_routing_report_path();
    let report = write_tassadar_composite_routing_report(&output_path)?;
    println!(
        "wrote composite-routing report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
