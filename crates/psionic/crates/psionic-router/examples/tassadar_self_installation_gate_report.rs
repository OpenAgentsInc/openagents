use psionic_router::{
    tassadar_self_installation_gate_report_path, write_tassadar_self_installation_gate_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_self_installation_gate_report_path();
    let report = write_tassadar_self_installation_gate_report(&output_path)?;
    println!(
        "wrote self-installation gate report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
