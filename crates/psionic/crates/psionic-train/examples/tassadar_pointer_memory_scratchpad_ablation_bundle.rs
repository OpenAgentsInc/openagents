use psionic_train::{
    tassadar_pointer_memory_scratchpad_ablation_bundle_path,
    write_tassadar_pointer_memory_scratchpad_ablation_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_pointer_memory_scratchpad_ablation_bundle_path();
    let bundle = write_tassadar_pointer_memory_scratchpad_ablation_bundle(&output_path)?;
    println!(
        "wrote pointer/memory/scratchpad ablation bundle to {} ({})",
        output_path.display(),
        bundle.bundle_digest
    );
    Ok(())
}
