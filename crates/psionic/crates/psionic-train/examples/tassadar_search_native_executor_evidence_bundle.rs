use psionic_train::{
    tassadar_search_native_executor_evidence_bundle_path,
    write_tassadar_search_native_executor_evidence_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_search_native_executor_evidence_bundle_path();
    let bundle = write_tassadar_search_native_executor_evidence_bundle(&output_path)?;
    println!(
        "wrote search-native executor evidence bundle to {} ({})",
        output_path.display(),
        bundle.bundle_digest
    );
    Ok(())
}
