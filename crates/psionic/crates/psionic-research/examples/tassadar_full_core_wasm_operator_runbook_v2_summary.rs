use psionic_research::{
    tassadar_full_core_wasm_operator_runbook_v2_summary_path,
    write_tassadar_full_core_wasm_operator_runbook_v2_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_full_core_wasm_operator_runbook_v2_summary_path();
    let summary = write_tassadar_full_core_wasm_operator_runbook_v2_summary(&output_path)?;
    println!(
        "wrote full core-Wasm operator runbook v2 summary to {} ({})",
        output_path.display(),
        summary.report_digest
    );
    Ok(())
}
