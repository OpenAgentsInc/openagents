use psionic_runtime::{
    tassadar_relaxed_simd_runtime_report_path, write_tassadar_relaxed_simd_runtime_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report =
        write_tassadar_relaxed_simd_runtime_report(tassadar_relaxed_simd_runtime_report_path())?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
