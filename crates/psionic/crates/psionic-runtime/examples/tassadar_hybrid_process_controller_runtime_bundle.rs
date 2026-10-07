use psionic_runtime::{
    tassadar_hybrid_process_controller_runtime_bundle_path,
    write_tassadar_hybrid_process_controller_runtime_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_hybrid_process_controller_runtime_bundle_path();
    let bundle = write_tassadar_hybrid_process_controller_runtime_bundle(&output_path)?;
    println!(
        "wrote hybrid process controller runtime bundle to {} ({})",
        output_path.display(),
        bundle.bundle_digest
    );
    Ok(())
}
