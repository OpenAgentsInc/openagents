use psionic_eval::{tassadar_memory_abi_v2_report_path, write_tassadar_memory_abi_v2_report};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_memory_abi_v2_report(tassadar_memory_abi_v2_report_path())?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
