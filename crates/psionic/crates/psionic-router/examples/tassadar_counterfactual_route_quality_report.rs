use psionic_router::{
    tassadar_counterfactual_route_quality_report_path,
    write_tassadar_counterfactual_route_quality_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_counterfactual_route_quality_report_path();
    let report = write_tassadar_counterfactual_route_quality_report(&output_path)?;
    println!(
        "wrote counterfactual route-quality report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
