use psionic_ir::{
    tassadar_universal_substrate_model_path, write_tassadar_universal_substrate_model,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_universal_substrate_model_path();
    let model = write_tassadar_universal_substrate_model(&output_path)?;
    println!(
        "wrote TCM.v1 model to {} ({})",
        output_path.display(),
        model.model_digest
    );
    Ok(())
}
