use std::{env, error::Error, path::PathBuf};

use psionic_data::build_psion_reference_corpus;

fn main() -> Result<(), Box<dyn Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("crate should live under the workspace root")?
        .parent()
        .ok_or("workspace root should exist")?
        .to_path_buf();
    let output_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("target/psion_reference_corpus"));
    let bundle = build_psion_reference_corpus(repo_root.as_path())?;
    bundle.write_to_dir(output_dir.as_path())?;
    println!(
        "wrote Psion reference corpus artifacts to {}",
        output_dir.display()
    );
    println!(
        "dataset identity: {}",
        bundle
            .tokenized_corpus_manifest
            .replay_contract
            .stable_dataset_identity
    );
    println!(
        "train sequences: {}",
        bundle
            .shard(psionic_data::DatasetSplitKind::Train)
            .map(|shard| shard.sequences.len())
            .unwrap_or(0)
    );
    println!(
        "validation sequences: {}",
        bundle
            .shard(psionic_data::DatasetSplitKind::Validation)
            .map(|shard| shard.sequences.len())
            .unwrap_or(0)
    );
    println!(
        "held-out sequences: {}",
        bundle
            .shard(psionic_data::DatasetSplitKind::HeldOut)
            .map(|shard| shard.sequences.len())
            .unwrap_or(0)
    );
    Ok(())
}
