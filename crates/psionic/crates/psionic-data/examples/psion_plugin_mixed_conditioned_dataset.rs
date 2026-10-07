use psionic_data::{
    psion_plugin_mixed_conditioned_dataset_bundle_path,
    write_psion_plugin_mixed_conditioned_dataset_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = psion_plugin_mixed_conditioned_dataset_bundle_path();
    let bundle = write_psion_plugin_mixed_conditioned_dataset_bundle(&output_path)?;
    println!(
        "wrote {} with digest {}",
        output_path.display(),
        bundle.bundle_digest
    );
    println!("dataset identity: {}", bundle.stable_dataset_identity);
    println!("split rows: {}", bundle.split_rows.len());
    Ok(())
}
