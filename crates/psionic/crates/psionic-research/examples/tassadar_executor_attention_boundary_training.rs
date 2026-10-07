use std::path::PathBuf;

use psionic_research::{
    TASSADAR_EXECUTOR_ATTENTION_BOUNDARY_OUTPUT_DIR,
    run_tassadar_executor_attention_boundary_training,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = PathBuf::from(TASSADAR_EXECUTOR_ATTENTION_BOUNDARY_OUTPUT_DIR);
    let bundle = run_tassadar_executor_attention_boundary_training(output_dir.as_path())?;
    println!(
        "wrote {} ({})",
        output_dir.join("training_report.json").display(),
        bundle.bundle_digest
    );
    Ok(())
}
