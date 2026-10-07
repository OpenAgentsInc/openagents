use std::path::Path;

use psionic_train::{
    TASSADAR_NEGATIVE_INVOCATION_OUTPUT_DIR, execute_tassadar_negative_invocation_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new(TASSADAR_NEGATIVE_INVOCATION_OUTPUT_DIR);
    let bundle = execute_tassadar_negative_invocation_bundle(output_dir)?;
    println!(
        "wrote {} cases to {}",
        bundle.cases.len(),
        output_dir.display()
    );
    Ok(())
}
