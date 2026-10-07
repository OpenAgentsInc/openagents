use std::path::PathBuf;

use psionic_research::{
    ATTNRES_RESIDUAL_COMPARISON_OUTPUT_DIR, ATTNRES_RESIDUAL_COMPARISON_REPORT_FILE,
    run_attnres_residual_comparison,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = PathBuf::from(ATTNRES_RESIDUAL_COMPARISON_OUTPUT_DIR);
    let report = run_attnres_residual_comparison(output_dir.as_path())?;
    println!(
        "wrote {} ({})",
        output_dir
            .join(ATTNRES_RESIDUAL_COMPARISON_REPORT_FILE)
            .display(),
        report.stable_digest()
    );
    Ok(())
}
