use psionic_runtime::{
    psion_plugin_guest_artifact_invocation_path,
    write_psion_plugin_guest_artifact_invocation_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = psion_plugin_guest_artifact_invocation_path();
    let bundle = write_psion_plugin_guest_artifact_invocation_bundle(&output_path)?;
    println!("wrote {} ({})", output_path.display(), bundle.bundle_digest);
    println!(
        "refusal_cases={} tool_name={}",
        bundle.refusal_cases.len(),
        bundle.tool_projection.tool_name
    );
    Ok(())
}
