use psionic_router::{
    tassadar_hybrid_process_controller_route_report_path,
    write_tassadar_hybrid_process_controller_route_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_hybrid_process_controller_route_report_path();
    let report = write_tassadar_hybrid_process_controller_route_report(&output_path)?;
    println!(
        "wrote hybrid process controller route report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
