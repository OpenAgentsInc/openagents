use psionic_runtime::{
    psion_plugin_guest_artifact_identity_path, psion_plugin_guest_artifact_manifest_path,
    write_reference_psion_plugin_guest_artifact_identity,
    write_reference_psion_plugin_guest_artifact_manifest,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_path = psion_plugin_guest_artifact_manifest_path();
    let identity_path = psion_plugin_guest_artifact_identity_path();
    let manifest = write_reference_psion_plugin_guest_artifact_manifest(&manifest_path)?;
    let identity = write_reference_psion_plugin_guest_artifact_identity(&identity_path)?;
    println!(
        "wrote {} ({})",
        manifest_path.display(),
        manifest.manifest_digest
    );
    println!(
        "wrote {} ({})",
        identity_path.display(),
        identity.identity_digest
    );
    Ok(())
}
