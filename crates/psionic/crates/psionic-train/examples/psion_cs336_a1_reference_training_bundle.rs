use std::{error::Error, fs, path::PathBuf};

use psionic_train::{
    CS336_A1_REFERENCE_TINY_CHECKPOINT_STEP2_FIXTURE_PATH,
    CS336_A1_REFERENCE_TINY_CHECKPOINT_STEP4_FIXTURE_PATH,
    CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH,
    CS336_A1_REFERENCE_TINY_TRAINING_BUNDLE_FIXTURE_PATH,
    write_cs336_a1_reference_tiny_training_bundle,
};

const TINY_CORPUS: &str = "the cat sat on the mat.\nthe cat saw the mat.\n";

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let corpus_path = root.join(CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH);
    if let Some(parent) = corpus_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&corpus_path, TINY_CORPUS)?;

    let bundle = write_cs336_a1_reference_tiny_training_bundle(&root)?;
    println!(
        "wrote {} {} {} resume_matches_uninterrupted={}",
        root.join(CS336_A1_REFERENCE_TINY_TRAINING_BUNDLE_FIXTURE_PATH)
            .display(),
        root.join(CS336_A1_REFERENCE_TINY_CHECKPOINT_STEP2_FIXTURE_PATH)
            .display(),
        root.join(CS336_A1_REFERENCE_TINY_CHECKPOINT_STEP4_FIXTURE_PATH)
            .display(),
        bundle.resume_matches_uninterrupted,
    );
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}
