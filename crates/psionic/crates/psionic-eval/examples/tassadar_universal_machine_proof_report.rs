use psionic_eval::{
    tassadar_universal_machine_proof_report_path, write_tassadar_universal_machine_proof_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_universal_machine_proof_report_path();
    let report = write_tassadar_universal_machine_proof_report(&output_path)?;
    println!(
        "wrote universal-machine proof report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
