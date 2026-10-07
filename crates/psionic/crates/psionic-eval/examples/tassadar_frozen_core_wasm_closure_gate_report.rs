use psionic_eval::{
    tassadar_frozen_core_wasm_closure_gate_report_path,
    write_tassadar_frozen_core_wasm_closure_gate_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_frozen_core_wasm_closure_gate_report(
        tassadar_frozen_core_wasm_closure_gate_report_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
