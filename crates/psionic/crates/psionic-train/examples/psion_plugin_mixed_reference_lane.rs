use psionic_train::{
    psion_plugin_mixed_reference_run_bundle_path, run_psion_plugin_mixed_reference_lane,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = psion_plugin_mixed_reference_run_bundle_path();
    let bundle = run_psion_plugin_mixed_reference_lane()?;
    bundle.write_to_path(&output_path)?;
    println!(
        "wrote {} with digest {}",
        output_path.display(),
        bundle.bundle_digest
    );
    println!("lane id: {}", bundle.lane_id);
    println!(
        "guest training examples: {}",
        bundle.model_artifact.guest_artifact_training_example_count
    );
    Ok(())
}
