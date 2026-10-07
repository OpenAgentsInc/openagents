use psionic_runtime::{
    tassadar_compiled_distillation_target_bundle_path,
    write_tassadar_compiled_distillation_target_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_compiled_distillation_target_bundle_path();
    let bundle = write_tassadar_compiled_distillation_target_bundle(&output_path)?;
    println!(
        "wrote compiled distillation target bundle to {} ({})",
        output_path.display(),
        bundle.bundle_digest
    );
    Ok(())
}
