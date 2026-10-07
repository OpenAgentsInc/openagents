use std::path::Path;

use psionic_research::{
    TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_OUTPUT_DIR,
    TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_REPORT_FILE,
    run_tassadar_symbolic_program_artifact_suite,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_tassadar_symbolic_program_artifact_suite(Path::new(
        TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_OUTPUT_DIR,
    ))?;
    println!(
        "wrote {}/{} ({})",
        TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_OUTPUT_DIR,
        TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_SUITE_REPORT_FILE,
        report.report_digest
    );
    Ok(())
}
