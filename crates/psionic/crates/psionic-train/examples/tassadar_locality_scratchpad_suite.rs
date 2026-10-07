use psionic_train::{
    execute_tassadar_locality_scratchpad_suite, TASSADAR_LOCALITY_SCRATCHPAD_SUITE_OUTPUT_DIR,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = execute_tassadar_locality_scratchpad_suite(std::path::Path::new(
        TASSADAR_LOCALITY_SCRATCHPAD_SUITE_OUTPUT_DIR,
    ))?;
    println!(
        "wrote locality scratchpad suite to {}/{} ({})",
        TASSADAR_LOCALITY_SCRATCHPAD_SUITE_OUTPUT_DIR,
        "locality_scratchpad_suite.json",
        report.report_digest
    );
    Ok(())
}
