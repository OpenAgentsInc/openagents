use std::path::Path;

use psionic_research::{
    TASSADAR_MODULE_SPECIALIZATION_BENCHMARK_OUTPUT_DIR,
    run_tassadar_module_specialization_benchmark_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_tassadar_module_specialization_benchmark_report(Path::new(
        TASSADAR_MODULE_SPECIALIZATION_BENCHMARK_OUTPUT_DIR,
    ))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
