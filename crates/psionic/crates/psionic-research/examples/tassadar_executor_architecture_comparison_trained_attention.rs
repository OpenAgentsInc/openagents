use std::path::PathBuf;

use psionic_research::{
    TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_TRAINED_ATTENTION_OUTPUT_DIR,
    run_tassadar_executor_architecture_comparison_with_trained_attention,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir =
        PathBuf::from(TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_TRAINED_ATTENTION_OUTPUT_DIR);
    let report =
        run_tassadar_executor_architecture_comparison_with_trained_attention(output_dir.as_path())?;
    println!(
        "wrote {} ({})",
        output_dir.join("architecture_comparison_report.json").display(),
        report.report_digest
    );
    Ok(())
}
