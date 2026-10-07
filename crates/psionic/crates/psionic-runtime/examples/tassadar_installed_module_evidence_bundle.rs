use psionic_runtime::{
    tassadar_installed_module_evidence_bundle_path, write_tassadar_installed_module_evidence_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_installed_module_evidence_bundle_path();
    let bundle = write_tassadar_installed_module_evidence_bundle(&output_path)?;
    println!(
        "wrote installed-module evidence bundle to {} ({})",
        output_path.display(),
        bundle.bundle_digest
    );
    Ok(())
}
