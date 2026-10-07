use psionic_runtime::{
    psion_plugin_guest_artifact_runtime_loading_path,
    write_psion_plugin_guest_artifact_runtime_loading_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = psion_plugin_guest_artifact_runtime_loading_path();
    let bundle = write_psion_plugin_guest_artifact_runtime_loading_bundle(&output_path)?;
    println!("wrote {} ({})", output_path.display(), bundle.bundle_digest);
    println!(
        "refusal_cases={} host_owned_capability_mediation=true",
        bundle.refusal_cases.len()
    );
    Ok(())
}
