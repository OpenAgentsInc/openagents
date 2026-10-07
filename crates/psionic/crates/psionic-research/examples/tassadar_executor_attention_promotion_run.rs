use std::path::PathBuf;

use psionic_research::{
    TASSADAR_EXECUTOR_ATTENTION_PROMOTION_OUTPUT_DIR,
    run_tassadar_executor_attention_promotion,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = PathBuf::from(TASSADAR_EXECUTOR_ATTENTION_PROMOTION_OUTPUT_DIR);
    let bundle = run_tassadar_executor_attention_promotion(output_dir.as_path())?;
    println!(
        "wrote {} ({})",
        output_dir.join("promotion_bundle.json").display(),
        bundle.bundle_digest
    );
    Ok(())
}
