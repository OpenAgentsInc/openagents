use std::path::Path;

use psionic_research::{
    TASSADAR_SUBROUTINE_LIBRARY_ABLATION_OUTPUT_DIR,
    TASSADAR_SUBROUTINE_LIBRARY_ABLATION_REPORT_FILE,
    run_tassadar_subroutine_library_ablation,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_tassadar_subroutine_library_ablation(Path::new(
        TASSADAR_SUBROUTINE_LIBRARY_ABLATION_OUTPUT_DIR,
    ))?;
    println!(
        "wrote {}/{} ({})",
        TASSADAR_SUBROUTINE_LIBRARY_ABLATION_OUTPUT_DIR,
        TASSADAR_SUBROUTINE_LIBRARY_ABLATION_REPORT_FILE,
        report.report_digest
    );
    Ok(())
}
