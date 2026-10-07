use std::path::PathBuf;

use psionic_research::{
    TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_OUTPUT_DIR,
    run_tassadar_hungarian_compiled_executor_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = PathBuf::from(TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_OUTPUT_DIR);
    let bundle = run_tassadar_hungarian_compiled_executor_bundle(output_dir.as_path())?;
    println!(
        "wrote {} ({})",
        output_dir.join("run_bundle.json").display(),
        bundle.bundle_digest
    );
    Ok(())
}
