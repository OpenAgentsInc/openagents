use psionic_eval::{
    tassadar_full_core_wasm_public_acceptance_gate_report_path,
    write_tassadar_full_core_wasm_public_acceptance_gate_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_full_core_wasm_public_acceptance_gate_report_path();
    let report = write_tassadar_full_core_wasm_public_acceptance_gate_report(&output_path)?;
    println!(
        "wrote full core-Wasm public acceptance gate report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
