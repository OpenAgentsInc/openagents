use psionic_runtime::{
    tassadar_cross_profile_link_compatibility_runtime_bundle_path,
    write_tassadar_cross_profile_link_compatibility_runtime_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_cross_profile_link_compatibility_runtime_bundle_path();
    let bundle = write_tassadar_cross_profile_link_compatibility_runtime_bundle(&output_path)?;
    println!(
        "wrote cross-profile link compatibility runtime bundle to {} ({})",
        output_path.display(),
        bundle.bundle_digest
    );
    Ok(())
}
