use psionic_models::TassadarArticleTransformer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let descriptors = TassadarArticleTransformer::write_committed_reference_artifacts()?;
    for descriptor in descriptors {
        println!(
            "{} {} {}",
            descriptor.model.model_id,
            descriptor.stable_digest(),
            descriptor.artifact_binding.artifact_identity_digest
        );
    }
    Ok(())
}
