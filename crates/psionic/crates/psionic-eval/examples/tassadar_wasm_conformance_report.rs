use psionic_eval::{tassadar_wasm_conformance_report_path, write_tassadar_wasm_conformance_report};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_wasm_conformance_report(tassadar_wasm_conformance_report_path())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("Wasm conformance report should serialize")
    );
    Ok(())
}
