use psionic_train::{
    tassadar_shared_primitive_transfer_evidence_bundle_path,
    write_tassadar_shared_primitive_transfer_evidence_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_shared_primitive_transfer_evidence_bundle_path();
    let bundle = write_tassadar_shared_primitive_transfer_evidence_bundle(&output_path)?;
    println!(
        "wrote shared primitive transfer evidence bundle to {} ({})",
        output_path.display(),
        bundle.bundle_digest
    );
    Ok(())
}
