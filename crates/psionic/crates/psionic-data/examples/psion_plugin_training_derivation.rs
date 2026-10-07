use psionic_data::{
    psion_plugin_training_derivation_bundle_path, write_psion_plugin_training_derivation_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = psion_plugin_training_derivation_bundle_path();
    let bundle = write_psion_plugin_training_derivation_bundle(&output_path)?;
    println!(
        "wrote {} with digest {}",
        output_path.display(),
        bundle.bundle_digest
    );
    println!("derived records: {}", bundle.records.len());
    println!(
        "controller surfaces: {}",
        bundle.controller_surface_counts.len()
    );
    Ok(())
}
