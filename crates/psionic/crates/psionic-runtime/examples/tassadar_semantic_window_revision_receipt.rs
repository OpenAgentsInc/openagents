use psionic_runtime::{
    tassadar_semantic_window_revision_receipt_path, write_tassadar_semantic_window_revision_receipt,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let receipt = write_tassadar_semantic_window_revision_receipt(
        tassadar_semantic_window_revision_receipt_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}
