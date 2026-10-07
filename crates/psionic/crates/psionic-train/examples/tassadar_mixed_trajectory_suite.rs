use std::path::Path;

use psionic_train::{
    TASSADAR_MIXED_TRAJECTORY_OUTPUT_DIR, execute_tassadar_mixed_trajectory_suite,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new(TASSADAR_MIXED_TRAJECTORY_OUTPUT_DIR);
    let suite = execute_tassadar_mixed_trajectory_suite(output_dir)?;
    println!(
        "wrote {} mixed trajectory cases to {}",
        suite.case_reports.len(),
        output_dir.display()
    );
    Ok(())
}
