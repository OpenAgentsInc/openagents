use psionic_eval::{
    tassadar_wasm_module_ingress_report_path, write_tassadar_wasm_module_ingress_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report =
        write_tassadar_wasm_module_ingress_report(tassadar_wasm_module_ingress_report_path())?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
