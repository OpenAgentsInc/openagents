use psionic_research::{
    tassadar_universal_machine_proof_summary_path, write_tassadar_universal_machine_proof_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_universal_machine_proof_summary_path();
    let report = write_tassadar_universal_machine_proof_summary(&output_path)?;
    println!(
        "wrote universal-machine proof summary to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
