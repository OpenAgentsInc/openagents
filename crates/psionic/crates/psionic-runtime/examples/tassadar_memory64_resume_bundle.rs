use psionic_runtime::{
    tassadar_memory64_resume_bundle_path, write_tassadar_memory64_resume_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_memory64_resume_bundle_path();
    let bundle = write_tassadar_memory64_resume_bundle(&output_path)?;
    println!(
        "wrote memory64 resume bundle to {} ({})",
        output_path.display(),
        bundle.bundle_id
    );
    Ok(())
}
