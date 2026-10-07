use std::path::Path;

use psionic_research::{
    TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_OUTPUT_DIR,
    TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_REPORT_FILE,
    run_tassadar_program_to_weights_benchmark_suite,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_tassadar_program_to_weights_benchmark_suite(Path::new(
        TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_OUTPUT_DIR,
    ))?;
    println!(
        "wrote {}/{} ({})",
        TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_OUTPUT_DIR,
        TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_REPORT_FILE,
        report.report_digest
    );
    Ok(())
}
