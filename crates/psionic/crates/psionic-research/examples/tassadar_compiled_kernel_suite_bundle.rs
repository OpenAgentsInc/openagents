use std::path::Path;

use psionic_research::{
    run_tassadar_compiled_kernel_suite_bundle, TASSADAR_COMPILED_KERNEL_SUITE_OUTPUT_DIR,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bundle = run_tassadar_compiled_kernel_suite_bundle(Path::new(
        TASSADAR_COMPILED_KERNEL_SUITE_OUTPUT_DIR,
    ))?;
    println!(
        "wrote {} ({})",
        TASSADAR_COMPILED_KERNEL_SUITE_OUTPUT_DIR,
        bundle.bundle_digest
    );
    Ok(())
}
