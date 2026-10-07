use std::{env, error::Error, path::PathBuf};

use psionic_train::{
    write_cross_provider_training_program_manifest,
    CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_FIXTURE_PATH,
};

fn main() -> Result<(), Box<dyn Error>> {
    let output_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_FIXTURE_PATH));
    let manifest = write_cross_provider_training_program_manifest(&output_path)?;
    println!("{}", serde_json::to_string_pretty(&manifest)?);
    Ok(())
}
