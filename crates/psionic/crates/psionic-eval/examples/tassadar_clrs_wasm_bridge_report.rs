use psionic_eval::{tassadar_clrs_wasm_bridge_report_path, write_tassadar_clrs_wasm_bridge_report};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_clrs_wasm_bridge_report(tassadar_clrs_wasm_bridge_report_path())?;
    println!(
        "wrote CLRS-to-Wasm bridge report to {} ({})",
        tassadar_clrs_wasm_bridge_report_path().display(),
        report.report_digest
    );
    Ok(())
}
