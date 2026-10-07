use psionic_router::{
    tassadar_linked_program_bundle_route_report_path,
    write_tassadar_linked_program_bundle_route_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_linked_program_bundle_route_report_path();
    let report = write_tassadar_linked_program_bundle_route_report(&output_path)?;
    println!(
        "wrote linked-program bundle route report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
